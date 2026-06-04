use super::*;

fn oauth_store_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("priority-agent")
        .join("mcp_oauth_tokens.json")
}

fn load_persisted_oauth_token(server_name: &str) -> Option<McpOAuthToken> {
    let path = oauth_store_path();
    let text = std::fs::read_to_string(path).ok()?;
    let map: HashMap<String, McpOAuthToken> = serde_json::from_str(&text).ok()?;
    map.get(server_name).cloned()
}

fn save_persisted_oauth_token(server_name: &str, token: &McpOAuthToken) -> anyhow::Result<()> {
    let path = oauth_store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut map: HashMap<String, McpOAuthToken> = if path.exists() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        HashMap::new()
    };
    map.insert(server_name.to_string(), token.clone());
    std::fs::write(&path, serde_json::to_string_pretty(&map)?)?;
    // Restrict file permissions to owner-only on Unix (chmod 600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) fn parse_oauth_token_response(v: &Value) -> anyhow::Result<McpOAuthToken> {
    let access_token = v["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("OAuth response missing access_token"))?
        .to_string();
    let refresh_token = v["refresh_token"].as_str().map(str::to_string);
    let token_type = v["token_type"].as_str().unwrap_or("Bearer").to_string();
    let expires_at = v["expires_in"]
        .as_u64()
        .map(|sec| now_unix_secs().saturating_add(sec));
    let scope = v["scope"].as_str().map(str::to_string);
    Ok(McpOAuthToken {
        access_token,
        refresh_token,
        token_type,
        expires_at,
        scope,
    })
}

/// MCP 传输连接内部实现
pub(super) enum McpTransportConnection {
    /// stdio 长连接
    Stdio {
        stdin: Arc<Mutex<tokio::process::ChildStdin>>,
        child: tokio::process::Child,
        stderr_handle: tokio::task::JoinHandle<()>,
        stdout_handle: tokio::task::JoinHandle<()>,
    },
    /// WebSocket 长连接
    WebSocket {
        write_tx: tokio::sync::mpsc::UnboundedSender<String>,
        read_handle: tokio::task::JoinHandle<()>,
        disconnected: Arc<std::sync::atomic::AtomicBool>,
    },
    /// HTTP 连接（无状态）
    Http { client: reqwest::Client },
}

/// MCP 客户端连接状态
pub(super) struct McpConnection {
    /// 待处理请求的响应通道
    pub(super) pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<McpResponse>>>>,
    /// 传输层实现
    pub(super) transport: McpTransportConnection,
}

/// MCP 客户端 - 连接单个 MCP 服务器
pub struct McpClient {
    /// 服务器配置
    config: McpServerConfig,
    /// 已发现的工具
    tools: Arc<RwLock<Vec<McpToolDef>>>,
    /// 请求 ID 计数器
    request_id: Arc<std::sync::atomic::AtomicU64>,
    /// 已建立的连接（长连接）
    pub(super) connection: Arc<Mutex<Option<McpConnection>>>,
    /// OAuth token（可持久化）
    oauth_token: Arc<Mutex<Option<McpOAuthToken>>>,
    /// 熔断器状态
    circuit_breaker: Arc<std::sync::Mutex<CircuitBreaker>>,
}

/// 熔断器状态
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 熔断阈值
    pub failure_threshold: u32,
    /// 熔断是否触发
    pub is_open: bool,
    /// 熔断触发时间
    pub opened_at_ms: Option<u64>,
    /// 熔断恢复超时（毫秒）
    pub recovery_timeout_ms: u64,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout_ms: u64) -> Self {
        Self {
            consecutive_failures: 0,
            failure_threshold,
            is_open: false,
            opened_at_ms: None,
            recovery_timeout_ms,
        }
    }

    /// 记录一次失败
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.failure_threshold && !self.is_open {
            self.is_open = true;
            self.opened_at_ms = Some(now_epoch_ms());
            info!(
                "Circuit breaker opened after {} failures",
                self.consecutive_failures
            );
        }
    }

    /// 记录一次成功
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        if self.is_open {
            self.is_open = false;
            self.opened_at_ms = None;
            info!("Circuit breaker closed after successful call");
        }
    }

    /// 检查是否可以执行调用
    pub fn can_execute(&self) -> bool {
        if !self.is_open {
            return true;
        }
        // 检查恢复超时
        if let Some(opened_at) = self.opened_at_ms {
            let now = now_epoch_ms();
            if now.saturating_sub(opened_at) >= self.recovery_timeout_ms {
                return true; // 超过恢复时间，允许一次调用测试
            }
        }
        false
    }

    /// 检查是否完全打开（不允许任何调用）
    pub fn is_fully_open(&self) -> bool {
        if !self.is_open {
            return false;
        }
        // 如果超过恢复时间，半开状态允许测试调用
        if let Some(opened_at) = self.opened_at_ms {
            let now = now_epoch_ms();
            now.saturating_sub(opened_at) < self.recovery_timeout_ms
        } else {
            false
        }
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 30000) // 5次失败触发，30秒后允许重试
    }
}

impl McpClient {
    /// 创建新的 MCP 客户端
    pub fn new(config: McpServerConfig) -> Self {
        let server_name = config.name.clone();
        Self {
            config,
            tools: Arc::new(RwLock::new(Vec::new())),
            request_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            connection: Arc::new(Mutex::new(None)),
            oauth_token: Arc::new(Mutex::new(load_persisted_oauth_token(&server_name))),
            circuit_breaker: Arc::new(std::sync::Mutex::new(CircuitBreaker::default())),
        }
    }

    /// 获取下一个请求 ID
    fn next_id(&self) -> u64 {
        self.request_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// 检查熔断器状态
    pub fn is_circuit_open(&self) -> bool {
        let cb = self.circuit_breaker.lock().unwrap();
        cb.is_open && cb.is_fully_open()
    }

    /// 获取熔断器状态描述
    pub fn circuit_breaker_status(&self) -> String {
        let cb = self.circuit_breaker.lock().unwrap();
        if cb.is_open {
            if cb.is_fully_open() {
                format!(
                    "OPEN (failures={}, recovery={}ms remaining)",
                    cb.consecutive_failures, cb.recovery_timeout_ms
                )
            } else {
                "HALF-OPEN (testing recovery)".to_string()
            }
        } else {
            format!("CLOSED (failures={})", cb.consecutive_failures)
        }
    }

    pub fn oauth_configured(&self) -> bool {
        self.config.oauth.is_some()
    }

    pub fn oauth_token_present(&self) -> bool {
        self.oauth_token
            .try_lock()
            .map(|token| token.is_some())
            .unwrap_or(false)
    }

    pub fn reset_circuit_breaker(&self) {
        if let Ok(mut cb) = self.circuit_breaker.lock() {
            cb.consecutive_failures = 0;
            cb.is_open = false;
            cb.opened_at_ms = None;
        }
    }

    /// 启动心跳检测（后台任务）
    pub fn start_heartbeat(self: &Arc<Self>, interval_secs: u64) -> tokio::task::JoinHandle<()> {
        let client = self.clone();
        tokio::spawn(async move {
            let interval = tokio::time::Duration::from_secs(interval_secs);
            loop {
                tokio::time::sleep(interval).await;

                // 执行健康检查
                if let Err(e) = client.health_check().await {
                    debug!("MCP heartbeat failed for {}: {}", client.config.name, e);
                    // 注意：health_check 失败不触发熔断，因为 heartbeat 是轻量检测
                }
            }
        })
    }

    /// 确保已连接到 MCP 服务器
    async fn ensure_connected(&self) -> anyhow::Result<()> {
        // 检查熔断器
        {
            let cb = self.circuit_breaker.lock().unwrap();
            if cb.is_open && cb.is_fully_open() {
                anyhow::bail!("MCP server '{}' circuit breaker is open", self.config.name);
            }
        }

        if self.config.oauth.is_some() {
            self.ensure_oauth_token_valid().await?;
        }
        match self.config.transport {
            McpTransport::Stdio => self.ensure_connected_stdio().await,
            McpTransport::WebSocket => self.ensure_connected_websocket().await,
            McpTransport::Http => self.ensure_connected_http().await,
        }
    }

    /// 确保已连接到 MCP 服务器（stdio 长连接）
    async fn ensure_connected_stdio(&self) -> anyhow::Result<()> {
        let mut conn_guard = self.connection.lock().await;
        if conn_guard.is_some() {
            return Ok(());
        }

        info!("Connecting to MCP server {} via stdio", self.config.name);

        let cmd = self.config.command.trim();
        if cmd.is_empty() {
            return Err(anyhow::anyhow!(
                "MCP server '{}' command is empty",
                self.config.name
            ));
        }
        // 如果提供的是绝对路径，验证文件存在且可执行
        let cmd_path = std::path::Path::new(cmd);
        if cmd_path.is_absolute() && !cmd_path.exists() {
            return Err(anyhow::anyhow!(
                "MCP server '{}' absolute command '{}' does not exist",
                self.config.name,
                cmd
            ));
        }

        let mut child = tokio::process::Command::new(cmd)
            .args(&self.config.args)
            .envs(&self.config.env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open child stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open child stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open child stderr"))?;

        let pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<McpResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_stdout = pending.clone();
        let server_name = self.config.name.clone();

        // stderr 读取任务
        let stderr_server_name = server_name.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                debug!("MCP server {} stderr: {}", stderr_server_name, line);
            }
        });

        // stdout 读取任务 — 按 JSON-RPC Content-Length 协议解析
        let stdout_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut content_length: Option<usize> = None;
                loop {
                    let mut header = String::new();
                    match reader.read_line(&mut header).await {
                        Ok(0) => {
                            debug!("MCP stdout EOF for {}", server_name);
                            return;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            warn!("MCP stdout read error from {}: {}", server_name, e);
                            return;
                        }
                    }
                    let header = header.trim();
                    if header.is_empty() {
                        break;
                    }
                    if let Some(val) = header.strip_prefix("Content-Length: ") {
                        content_length = val.parse().ok();
                    }
                }

                let len = match content_length {
                    Some(n) => n,
                    None => {
                        warn!("MCP message without Content-Length from {}", server_name);
                        continue;
                    }
                };

                // Cap Content-Length to prevent OOM from malicious servers
                const MAX_MCP_MESSAGE_LEN: usize = 10 * 1024 * 1024; // 10 MiB
                if len > MAX_MCP_MESSAGE_LEN {
                    warn!(
                        "MCP message too large from {}: {} bytes (max {})",
                        server_name, len, MAX_MCP_MESSAGE_LEN
                    );
                    continue;
                }

                let mut body = vec![0u8; len];
                if let Err(e) = reader.read_exact(&mut body).await {
                    warn!("MCP stdout body read error from {}: {}", server_name, e);
                    return;
                }

                let body_str = match String::from_utf8(body) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("MCP invalid UTF-8 from {}: {}", server_name, e);
                        continue;
                    }
                };

                debug!(
                    "MCP message from {}: {}",
                    server_name,
                    &body_str[..body_str.len().min(500)]
                );

                match serde_json::from_str::<McpResponse>(&body_str) {
                    Ok(response) => {
                        let id = response.id;
                        if let Some(tx) = pending_stdout.lock().await.remove(&id) {
                            let _ = tx.send(response);
                        } else {
                            warn!("Received MCP response with unknown request id: {}", id);
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to parse MCP response from {}: {} | body: {}",
                            server_name,
                            e,
                            &body_str[..body_str.len().min(200)]
                        );
                    }
                }
            }
        });

        *conn_guard = Some(McpConnection {
            pending,
            transport: McpTransportConnection::Stdio {
                stdin: Arc::new(Mutex::new(stdin)),
                child,
                stderr_handle,
                stdout_handle,
            },
        });

        Ok(())
    }

    /// 确保已连接到 MCP 服务器（WebSocket 长连接）
    async fn ensure_connected_websocket(&self) -> anyhow::Result<()> {
        let mut conn_guard = self.connection.lock().await;
        if conn_guard.is_some() {
            return Ok(());
        }

        let url = self.config.websocket_url.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "MCP server '{}' uses websocket transport but websocket_url is missing",
                self.config.name
            )
        })?;

        info!(
            "Connecting to MCP server {} via websocket: {}",
            self.config.name, url
        );

        if url.starts_with("ws://") {
            warn!(
                "MCP server '{}' uses unencrypted websocket (ws://). Consider using wss:// for sensitive data.",
                self.config.name
            );
        }

        use tokio_tungstenite::connect_async;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::http::HeaderValue;

        let mut request = url.into_client_request()?;
        let mut headers = self.config.headers.clone();
        if let Some(token) = self.oauth_token.lock().await.as_ref().cloned() {
            if !token.access_token.is_empty() {
                headers.insert(
                    "Authorization".to_string(),
                    format!("Bearer {}", token.access_token),
                );
            }
        }

        for (k, v) in &headers {
            // 阻止 HTTP header injection：拒绝包含换行或空字符的 header
            if k.contains('\r')
                || k.contains('\n')
                || k.contains('\0')
                || v.contains('\r')
                || v.contains('\n')
                || v.contains('\0')
            {
                warn!("Skipping MCP server '{}' header '{}' due to forbidden characters (\r / \n / \0)", self.config.name, k);
                continue;
            }
            if let Ok(name) = k.parse::<tokio_tungstenite::tungstenite::http::HeaderName>() {
                if let Ok(value) = HeaderValue::from_str(v) {
                    request.headers_mut().insert(name, value);
                }
            }
        }

        let (mut ws_stream, _) = connect_async(request).await?;
        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<McpResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_ws = pending.clone();
        let server_name = self.config.name.clone();
        let disconnected = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let disconnected_ws = disconnected.clone();

        let read_handle = tokio::spawn(async move {
            use futures::SinkExt;
            use futures::StreamExt;
            loop {
                tokio::select! {
                    maybe_text = write_rx.recv() => {
                        match maybe_text {
                            Some(text) => {
                                if ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(text)).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    maybe_msg = ws_stream.next() => {
                        match maybe_msg {
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                debug!(
                                    "MCP websocket message from {}: {}",
                                    server_name,
                                    &text[..text.len().min(500)]
                                );
                                match serde_json::from_str::<McpResponse>(&text) {
                                    Ok(response) => {
                                        let id = response.id;
                                        if let Some(tx) = pending_ws.lock().await.remove(&id) {
                                            let _ = tx.send(response);
                                        } else {
                                            warn!(
                                                "Received MCP websocket response with unknown request id: {}",
                                                id
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to parse MCP websocket response from {}: {} | body: {}",
                                            server_name, e,
                                            &text[..text.len().min(200)]
                                        );
                                    }
                                }
                            }
                            Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                                debug!("MCP websocket closed for {}", server_name);
                                break;
                            }
                            Some(Ok(_)) => {}
                            Some(Err(e)) => {
                                warn!("MCP websocket error from {}: {}", server_name, e);
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
            disconnected_ws.store(true, std::sync::atomic::Ordering::SeqCst);
            debug!("MCP websocket task exited for {}", server_name);
        });

        *conn_guard = Some(McpConnection {
            pending,
            transport: McpTransportConnection::WebSocket {
                write_tx,
                read_handle,
                disconnected,
            },
        });

        Ok(())
    }

    /// 确保已连接到 MCP 服务器（HTTP 无状态）
    async fn ensure_connected_http(&self) -> anyhow::Result<()> {
        let mut conn_guard = self.connection.lock().await;
        if conn_guard.is_some() {
            return Ok(());
        }

        let url = self.config.http_url.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "MCP server '{}' uses http transport but http_url is missing",
                self.config.name
            )
        })?;

        info!(
            "Connecting to MCP server {} via http: {}",
            self.config.name, url
        );

        *conn_guard = Some(McpConnection {
            pending: Arc::new(Mutex::new(HashMap::new())),
            transport: McpTransportConnection::Http {
                client: reqwest::Client::builder().no_proxy().build()?,
            },
        });
        Ok(())
    }

    fn token_expired(token: &McpOAuthToken) -> bool {
        if let Some(exp) = token.expires_at {
            let now = now_unix_secs();
            // 刷新提前量：60s
            now + 60 >= exp
        } else {
            false
        }
    }

    async fn ensure_oauth_token_valid(&self) -> anyhow::Result<()> {
        let has_token = self.oauth_token.lock().await.as_ref().cloned();
        if let Some(token) = has_token {
            if !Self::token_expired(&token) {
                return Ok(());
            }
            if token.refresh_token.is_some() {
                self.refresh_oauth_token().await?;
                return Ok(());
            }
        }
        self.authenticate_oauth().await?;
        Ok(())
    }

    async fn refresh_oauth_token(&self) -> anyhow::Result<()> {
        let oauth = self
            .config
            .oauth
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("OAuth config missing"))?;
        let refresh_token = self
            .oauth_token
            .lock()
            .await
            .as_ref()
            .and_then(|t| t.refresh_token.clone())
            .ok_or_else(|| anyhow::anyhow!("No refresh token available"))?;

        let token_url = oauth
            .token_url
            .clone()
            .or(self.config.oauth_token_url.clone())
            .ok_or_else(|| anyhow::anyhow!("OAuth token URL missing"))?;

        let mut form: Vec<(&str, String)> = vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token),
            ("client_id", oauth.client_id.clone()),
        ];
        if let Some(secret) = oauth.client_secret.clone() {
            form.push(("client_secret", secret));
        }

        let resp = reqwest::Client::new()
            .post(token_url)
            .form(&form)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("OAuth refresh failed with status {}", resp.status());
        }
        let v: Value = resp.json().await?;
        let token = parse_oauth_token_response(&v)?;
        self.set_oauth_token(token).await?;
        Ok(())
    }

    pub async fn authenticate_oauth(&self) -> anyhow::Result<()> {
        let oauth =
            self.config.oauth.as_ref().ok_or_else(|| {
                anyhow::anyhow!("OAuth is not configured for '{}'", self.config.name)
            })?;

        let token_url = oauth
            .token_url
            .clone()
            .or(self.config.oauth_token_url.clone())
            .ok_or_else(|| anyhow::anyhow!("OAuth token URL missing"))?;

        let mut form: Vec<(&str, String)> = vec![
            ("grant_type", "client_credentials".to_string()),
            ("client_id", oauth.client_id.clone()),
        ];
        if let Some(secret) = oauth.client_secret.clone() {
            form.push(("client_secret", secret));
        }
        if !oauth.scopes.is_empty() {
            form.push(("scope", oauth.scopes.join(" ")));
        }

        let resp = reqwest::Client::new()
            .post(token_url)
            .form(&form)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("OAuth authentication failed with status {}", resp.status());
        }

        let v: Value = resp.json().await?;
        let token = parse_oauth_token_response(&v)?;
        self.set_oauth_token(token).await?;
        Ok(())
    }

    async fn set_oauth_token(&self, token: McpOAuthToken) -> anyhow::Result<()> {
        {
            let mut guard = self.oauth_token.lock().await;
            *guard = Some(token.clone());
        }
        save_persisted_oauth_token(&self.config.name, &token)?;
        Ok(())
    }

    /// 关闭 MCP 连接并清理资源
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let mut conn_guard = self.connection.lock().await;
        if let Some(conn) = conn_guard.take() {
            match conn.transport {
                McpTransportConnection::Stdio {
                    mut child,
                    stderr_handle,
                    stdout_handle,
                    ..
                } => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    stderr_handle.abort();
                    stdout_handle.abort();
                }
                McpTransportConnection::WebSocket { read_handle, .. } => {
                    read_handle.abort();
                }
                McpTransportConnection::Http { .. } => {}
            }
        }
        Ok(())
    }

    /// 发送请求并获取响应（长连接，含 WebSocket 自动重连）
    async fn send_request(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.send_request_with_retry(method, params, 0).await
    }

    /// 记录成功到熔断器
    fn circuit_record_success(&self) {
        if let Ok(mut cb) = self.circuit_breaker.lock() {
            cb.record_success();
        }
    }

    /// 记录失败到熔断器
    fn circuit_record_failure(&self) {
        if let Ok(mut cb) = self.circuit_breaker.lock() {
            cb.record_failure();
        }
    }

    /// 发送请求并获取响应（内部，支持重试）
    async fn send_request_with_retry(
        &self,
        method: &str,
        params: Value,
        retry_count: u32,
    ) -> anyhow::Result<Value> {
        const MAX_RETRIES: u32 = 1;

        self.ensure_connected().await?;

        let id = self.next_id();
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params: params.clone(),
        };

        let request_json = serde_json::to_string(&request)?;

        debug!(
            "MCP request to {}: {}",
            self.config.name,
            &request_json[..request_json.len().min(200)]
        );

        // HTTP transport: direct JSON-RPC over HTTP POST
        let http_client = {
            let conn_guard = self.connection.lock().await;
            if let Some(McpConnection {
                transport: McpTransportConnection::Http { client },
                ..
            }) = conn_guard.as_ref()
            {
                Some(client.clone())
            } else {
                None
            }
        };
        if let Some(client) = http_client {
            let url = self.config.http_url.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "MCP server '{}' uses http transport but http_url is missing",
                    self.config.name
                )
            })?;
            let mut req = client.post(url).json(&request);
            for (k, v) in &self.config.headers {
                req = req.header(k, v);
            }
            if let Some(token) = self.oauth_token.lock().await.as_ref().cloned() {
                if !token.access_token.is_empty() {
                    req = req.bearer_auth(token.access_token);
                }
            }
            let resp = req.send().await?;
            if !resp.status().is_success() {
                anyhow::bail!("MCP HTTP request failed with status {}", resp.status());
            }
            let response: McpResponse = resp.json().await?;
            if let Some(err) = response.error {
                anyhow::bail!("MCP error ({}): {}", err.code, err.message);
            }
            return response
                .result
                .ok_or_else(|| anyhow::anyhow!("MCP response has no result"));
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut conn_guard = self.connection.lock().await;
            let conn = conn_guard
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("MCP connection not established"))?;

            // WebSocket 断线检测：若已断开，清理旧连接并重试一次
            if let McpTransportConnection::WebSocket { disconnected, .. } = &conn.transport {
                if disconnected.load(std::sync::atomic::Ordering::SeqCst)
                    && retry_count < MAX_RETRIES
                {
                    warn!(
                        "MCP websocket {} disconnected, reconnecting...",
                        self.config.name
                    );
                    *conn_guard = None;
                    drop(conn_guard);
                    return Box::pin(self.send_request_with_retry(
                        method,
                        params.clone(),
                        retry_count + 1,
                    ))
                    .await;
                }
            }

            conn.pending.lock().await.insert(id, tx);

            match &conn.transport {
                McpTransportConnection::Stdio { stdin, .. } => {
                    let mut stdin = stdin.lock().await;
                    let msg = format!(
                        "Content-Length: {}\r\n\r\n{}",
                        request_json.len(),
                        request_json
                    );
                    stdin.write_all(msg.as_bytes()).await?;
                }
                McpTransportConnection::WebSocket { write_tx, .. } => {
                    if write_tx.send(request_json).is_err() && retry_count < MAX_RETRIES {
                        // 发送失败（接收端已关闭），标记断开并重试
                        drop(conn_guard);
                        let mut conn_guard = self.connection.lock().await;
                        *conn_guard = None;
                        drop(conn_guard);
                        warn!(
                            "MCP websocket {} send failed, reconnecting...",
                            self.config.name
                        );
                        return Box::pin(self.send_request_with_retry(
                            method,
                            params,
                            retry_count + 1,
                        ))
                        .await;
                    }
                }
                McpTransportConnection::Http { .. } => {
                    // handled by HTTP fast-path above
                    unreachable!("HTTP transport should return before pending channel flow");
                }
            }
        }

        let response = match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(_)) => {
                // 从 pending 中清理
                if let Some(conn) = self.connection.lock().await.as_ref() {
                    conn.pending.lock().await.remove(&id);
                }
                self.circuit_record_failure();
                anyhow::bail!("MCP response channel closed")
            }
            Err(_) => {
                if let Some(conn) = self.connection.lock().await.as_ref() {
                    conn.pending.lock().await.remove(&id);
                }
                self.circuit_record_failure();
                anyhow::bail!("MCP request timed out after 30 seconds")
            }
        };

        if let Some(err) = response.error {
            self.circuit_record_failure();
            anyhow::bail!("MCP error ({}): {}", err.code, err.message);
        }

        let result = response
            .result
            .ok_or_else(|| anyhow::anyhow!("MCP response has no result"))?;

        // 成功，记录到熔断器
        self.circuit_record_success();
        Ok(result)
    }

    /// 发现服务器提供的工具
    pub async fn discover_tools(&self) -> anyhow::Result<Vec<McpToolDef>> {
        info!("Discovering tools from MCP server: {}", self.config.name);

        let result = self.send_request("tools/list", json!({})).await?;

        let tools_array = result["tools"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("tools/list did not return tools array"))?;

        let mut tools = Vec::new();
        for tool_value in tools_array {
            let tool = McpToolDef {
                name: tool_value["name"].as_str().unwrap_or("unnamed").to_string(),
                description: tool_value["description"].as_str().unwrap_or("").to_string(),
                input_schema: tool_value["inputSchema"].clone(),
                server_name: self.config.name.clone(),
            };
            tools.push(tool);
        }

        // 缓存工具列表
        *self.tools.write().await = tools.clone();

        info!("Discovered {} tools from {}", tools.len(), self.config.name);
        Ok(tools)
    }

    /// 调用远程工具
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> anyhow::Result<String> {
        debug!("Calling MCP tool {} on {}", tool_name, self.config.name);

        let result = self
            .send_request(
                "tools/call",
                json!({
                    "name": tool_name,
                    "arguments": arguments
                }),
            )
            .await?;

        // MCP 返回 content 数组
        if let Some(content) = result["content"].as_array() {
            let mut output = String::new();
            for item in content {
                match item["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = item["text"].as_str() {
                            output.push_str(text);
                            output.push('\n');
                        }
                    }
                    Some("image") => {
                        output.push_str("[image data]\n");
                    }
                    _ => {
                        output.push_str(&format!("{}\n", item));
                    }
                }
            }
            Ok(output.trim().to_string())
        } else {
            // 兼容：直接返回 result
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }

    /// 发现服务器提供的资源
    pub async fn discover_resources(&self) -> anyhow::Result<Vec<McpResourceDef>> {
        info!(
            "Discovering resources from MCP server: {}",
            self.config.name
        );

        let result = self.send_request("resources/list", json!({})).await?;

        let resources_array = result["resources"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("resources/list did not return resources array"))?;

        let mut resources = Vec::new();
        for res_value in resources_array {
            let resource = McpResourceDef {
                uri: res_value["uri"].as_str().unwrap_or("").to_string(),
                name: res_value["name"].as_str().unwrap_or("unnamed").to_string(),
                mime_type: res_value["mimeType"].as_str().map(String::from),
                description: res_value["description"].as_str().map(String::from),
                server_name: self.config.name.clone(),
            };
            resources.push(resource);
        }

        info!(
            "Discovered {} resources from {}",
            resources.len(),
            self.config.name
        );
        Ok(resources)
    }

    /// 发现服务器提供的 prompts
    pub async fn discover_prompts(&self) -> anyhow::Result<Vec<McpPromptDef>> {
        info!("Discovering prompts from MCP server: {}", self.config.name);

        let result = self.send_request("prompts/list", json!({})).await?;
        let prompts_array = result["prompts"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("prompts/list did not return prompts array"))?;

        let mut prompts = Vec::new();
        for prompt_value in prompts_array {
            prompts.push(McpPromptDef {
                name: prompt_value["name"]
                    .as_str()
                    .unwrap_or("unnamed")
                    .to_string(),
                description: prompt_value["description"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                arguments: prompt_value["arguments"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
                server_name: self.config.name.clone(),
            });
        }

        Ok(prompts)
    }

    /// 读取资源内容
    pub async fn read_resource(&self, uri: &str) -> anyhow::Result<Value> {
        debug!("Reading MCP resource {} on {}", uri, self.config.name);

        let result = self
            .send_request(
                "resources/read",
                json!({
                    "uri": uri
                }),
            )
            .await?;

        Ok(result)
    }

    /// 健康检查：确保连接存活
    pub async fn health_check(&self) -> anyhow::Result<()> {
        self.ensure_connected().await?;

        let mut conn_guard = self.connection.lock().await;
        if let Some(conn) = &mut *conn_guard {
            match &mut conn.transport {
                // 对于 stdio，额外检查子进程是否仍在运行
                McpTransportConnection::Stdio { child, .. } => {
                    match child.try_wait() {
                        Ok(Some(_status)) => {
                            anyhow::bail!("MCP server '{}' process has exited", self.config.name)
                        }
                        Ok(None) => {} // still running
                        Err(e) => anyhow::bail!(
                            "Failed to check MCP server '{}' status: {}",
                            self.config.name,
                            e
                        ),
                    }
                }
                // 对于 WebSocket，检查断开标志
                McpTransportConnection::WebSocket { disconnected, .. } => {
                    if disconnected.load(std::sync::atomic::Ordering::SeqCst) {
                        // 清理旧连接，下次调用 ensure_connected 会自动重连
                        *conn_guard = None;
                        anyhow::bail!("MCP server '{}' websocket disconnected", self.config.name);
                    }
                }
                McpTransportConnection::Http { .. } => {}
            }
        }

        Ok(())
    }

    /// 获取已缓存的工具列表
    pub async fn get_tools(&self) -> Vec<McpToolDef> {
        self.tools.read().await.clone()
    }

    /// 获取服务器名称
    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub fn transport(&self) -> &McpTransport {
        &self.config.transport
    }

    pub fn endpoint_summary(&self) -> String {
        match self.config.transport {
            McpTransport::Stdio => {
                if self.config.command.trim().is_empty() {
                    "stdio:<missing_command>".to_string()
                } else {
                    format!("stdio:{}", self.config.command)
                }
            }
            McpTransport::WebSocket => format!(
                "websocket:{}",
                self.config
                    .websocket_url
                    .clone()
                    .unwrap_or_else(|| "<missing_url>".to_string())
            ),
            McpTransport::Http => format!(
                "http:{}",
                self.config
                    .http_url
                    .clone()
                    .unwrap_or_else(|| "<missing_url>".to_string())
            ),
        }
    }
}

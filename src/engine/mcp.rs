//! MCP (Model Context Protocol) 客户端
//!
//! 支持连接外部 MCP 服务器，发现和调用远程工具。
//! 类似 Claude Code 和 Hermes Agent 的 MCP 集成。
//!
//! 协议：<https://spec.modelcontextprotocol.io/>
//!
//! 当前支持：
//! - stdio transport（通过子进程通信）
//! - websocket transport（长连接）
//! - http transport（JSON-RPC over HTTP POST）
//! - 工具发现 (tools/list)
//! - 工具调用 (tools/call)
//! - OAuth token 获取/刷新/本地持久化

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// MCP 传输类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    #[default]
    Stdio,
    WebSocket,
    Http,
}

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// 服务器名称
    pub name: String,
    /// 传输类型
    #[serde(default)]
    pub transport: McpTransport,
    /// 启动命令（用于 stdio transport）
    #[serde(default)]
    pub command: String,
    /// 命令参数
    #[serde(default)]
    pub args: Vec<String>,
    /// 环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// WebSocket URL（用于 websocket transport）
    #[serde(default)]
    pub websocket_url: Option<String>,
    /// HTTP URL（用于 HTTP transport）
    #[serde(default)]
    pub http_url: Option<String>,
    /// WebSocket 请求头（用于 websocket transport）
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// OAuth 配置（用于需要 OAuth 认证的服务器）
    #[serde(default)]
    pub oauth: Option<McpOAuthConfig>,
    /// OAuth token URL（用于获取/刷新 token）
    #[serde(default)]
    pub oauth_token_url: Option<String>,
}

/// MCP OAuth 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpOAuthConfig {
    /// OAuth 客户端 ID
    pub client_id: String,
    /// OAuth 客户端密钥
    #[serde(default)]
    pub client_secret: Option<String>,
    /// OAuth 授权 URL
    pub auth_url: Option<String>,
    /// OAuth token URL
    pub token_url: Option<String>,
    /// OAuth scopes
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// MCP OAuth token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpOAuthToken {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_type: String,
    /// Unix timestamp seconds
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// MCP 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    /// 工具名称
    pub name: String,
    /// 工具描述
    #[serde(default)]
    pub description: String,
    /// 输入参数 JSON Schema
    pub input_schema: Value,
    /// 所属服务器
    pub server_name: String,
}

/// MCP 资源定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceDef {
    /// 资源 URI
    pub uri: String,
    /// 资源名称
    pub name: String,
    /// MIME 类型
    #[serde(default)]
    pub mime_type: Option<String>,
    /// 资源描述
    #[serde(default)]
    pub description: Option<String>,
    /// 所属服务器
    pub server_name: String,
}

/// MCP prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptDef {
    /// Prompt name
    pub name: String,
    /// Prompt description
    #[serde(default)]
    pub description: String,
    /// Prompt argument schema/metadata from the MCP server.
    #[serde(default)]
    pub arguments: Vec<Value>,
    /// 所属服务器
    pub server_name: String,
}

/// MCP 请求
#[derive(Debug, Serialize)]
struct McpRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Value,
}

/// MCP 响应
#[derive(Debug, Deserialize)]
struct McpResponse {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<McpError>,
}

#[derive(Debug, Deserialize)]
struct McpError {
    code: i64,
    message: String,
}

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

fn parse_oauth_token_response(v: &Value) -> anyhow::Result<McpOAuthToken> {
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
enum McpTransportConnection {
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
struct McpConnection {
    /// 待处理请求的响应通道
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<McpResponse>>>>,
    /// 传输层实现
    transport: McpTransportConnection,
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
    connection: Arc<Mutex<Option<McpConnection>>>,
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

/// MCP 管理器 - 管理多个 MCP 服务器
pub struct McpManager {
    /// 已连接的服务器
    clients: HashMap<String, Arc<McpClient>>,
    /// 已批准的服务器名称
    approved_servers: std::sync::Mutex<HashSet<String>>,
    /// 是否需要服务器批准（默认 true）
    require_server_approval: std::sync::atomic::AtomicBool,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            approved_servers: std::sync::Mutex::new(HashSet::new()),
            require_server_approval: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// 添加 MCP 服务器
    pub fn add_server(&mut self, config: McpServerConfig) {
        let name = config.name.clone();
        let client = Arc::new(McpClient::new(config));
        self.clients.insert(name, client);
    }

    /// 从配置文件加载服务器
    pub fn load_from_config(configs: Vec<McpServerConfig>) -> Self {
        let mut manager = Self::new();
        for config in configs {
            manager.add_server(config);
        }
        manager
    }

    /// 批准指定 MCP 服务器
    pub fn approve_server(&self, name: &str) {
        let mut set = self
            .approved_servers
            .lock()
            .expect("approved_servers mutex poisoned while approving server");
        set.insert(name.to_string());
        info!("MCP server '{}' approved", name);
    }

    /// 撤销指定 MCP 服务器的批准
    pub fn revoke_server(&self, name: &str) {
        let mut set = self
            .approved_servers
            .lock()
            .expect("approved_servers mutex poisoned while revoking server");
        set.remove(name);
        info!("MCP server '{}' approval revoked", name);
    }

    /// 检查服务器是否已批准
    pub fn is_server_approved(&self, name: &str) -> bool {
        !self
            .require_server_approval
            .load(std::sync::atomic::Ordering::SeqCst)
            || self
                .approved_servers
                .lock()
                .expect("approved_servers mutex poisoned while checking approval")
                .contains(name)
    }

    /// 设置是否需要服务器批准
    pub fn set_require_server_approval(&self, require: bool) {
        self.require_server_approval
            .store(require, std::sync::atomic::Ordering::SeqCst);
    }

    /// 获取已批准的服务器列表
    pub fn approved_server_names(&self) -> Vec<String> {
        self.approved_servers
            .lock()
            .expect("approved_servers mutex poisoned while listing approvals")
            .iter()
            .cloned()
            .collect()
    }

    /// 发现所有服务器的工具
    pub async fn discover_all_tools(&self) -> Vec<McpToolDef> {
        let mut all_tools = Vec::new();

        for (name, client) in &self.clients {
            if !self.is_server_approved(name) {
                continue;
            }
            match client.discover_tools().await {
                Ok(tools) => all_tools.extend(tools),
                Err(e) => warn!("Failed to discover tools from {}: {}", name, e),
            }
        }

        all_tools
    }

    /// 调用工具（自动找到正确的服务器）
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> anyhow::Result<String> {
        // 先检查缓存
        for (name, client) in &self.clients {
            if !self.is_server_approved(name) {
                continue;
            }
            let tools = client.get_tools().await;
            if tools.iter().any(|t| t.name == tool_name) {
                client.health_check().await?;
                return client.call_tool(tool_name, arguments).await;
            }
        }

        // 如果没找到，尝试发现
        let all_tools = self.discover_all_tools().await;
        if let Some(tool_def) = all_tools.iter().find(|t| t.name == tool_name) {
            if let Some(client) = self.clients.get(&tool_def.server_name) {
                client.health_check().await?;
                return client.call_tool(tool_name, arguments).await;
            }
        }

        anyhow::bail!("MCP tool '{}' not found on any approved server", tool_name)
    }

    /// 在指定服务器上调用工具
    pub async fn call_tool_on_server(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> anyhow::Result<String> {
        if !self.is_server_approved(server_name) {
            anyhow::bail!(
                "MCP server '{}' is not approved. Use '/mcp approve {}' to approve it.",
                server_name,
                server_name
            );
        }
        if let Some(client) = self.clients.get(server_name) {
            client.health_check().await?;
            return client.call_tool(tool_name, arguments).await;
        }
        anyhow::bail!("MCP server '{}' not found", server_name)
    }

    /// 列出所有可用的 MCP 工具
    pub async fn list_tools(&self) -> Vec<McpToolDef> {
        self.discover_all_tools().await
    }

    /// 列出健康且已批准服务器上的 MCP 工具。
    pub async fn list_available_tools(&self) -> Vec<McpToolDef> {
        let available = self.available_servers();
        let mut all_tools = Vec::new();
        for name in available {
            let Some(client) = self.clients.get(&name) else {
                continue;
            };
            match client.discover_tools().await {
                Ok(tools) => all_tools.extend(tools),
                Err(e) => warn!(
                    "Failed to discover tools from available server {}: {}",
                    name, e
                ),
            }
        }
        all_tools
    }

    /// 发现所有服务器的资源
    pub async fn discover_all_resources(&self) -> Vec<McpResourceDef> {
        let mut all_resources = Vec::new();

        for (name, client) in &self.clients {
            if !self.is_server_approved(name) {
                continue;
            }
            match client.discover_resources().await {
                Ok(resources) => all_resources.extend(resources),
                Err(e) => warn!("Failed to discover resources from {}: {}", name, e),
            }
        }

        all_resources
    }

    /// 发现所有已批准服务器的 prompts。
    pub async fn discover_all_prompts(&self) -> Vec<McpPromptDef> {
        let mut all_prompts = Vec::new();

        for (name, client) in &self.clients {
            if !self.is_server_approved(name) {
                continue;
            }
            match client.discover_prompts().await {
                Ok(prompts) => all_prompts.extend(prompts),
                Err(e) => warn!("Failed to discover prompts from {}: {}", name, e),
            }
        }

        all_prompts
    }

    /// 读取指定服务器上的资源
    pub async fn read_resource(&self, server_name: &str, uri: &str) -> anyhow::Result<Value> {
        if !self.is_server_approved(server_name) {
            anyhow::bail!(
                "MCP server '{}' is not approved. Use '/mcp approve {}' to approve it.",
                server_name,
                server_name
            );
        }
        if let Some(client) = self.clients.get(server_name) {
            client.health_check().await?;
            client.read_resource(uri).await
        } else {
            anyhow::bail!("MCP server '{}' not found", server_name)
        }
    }

    /// 检查指定服务器的健康状态
    pub async fn health_check(&self, server_name: &str) -> anyhow::Result<()> {
        if let Some(client) = self.clients.get(server_name) {
            client.health_check().await
        } else {
            anyhow::bail!("MCP server '{}' not found", server_name)
        }
    }

    /// 获取管理器中的服务器列表
    pub fn server_names(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    pub fn server_summaries(&self) -> Vec<String> {
        let mut rows: Vec<String> = self
            .clients
            .iter()
            .map(|(name, client)| {
                let transport = match client.transport() {
                    McpTransport::Stdio => "stdio",
                    McpTransport::WebSocket => "websocket",
                    McpTransport::Http => "http",
                };
                let approved = if self.is_server_approved(name) {
                    "approved"
                } else {
                    "pending"
                };
                format!(
                    "- {} [{}] {} ({})",
                    name,
                    transport,
                    client.endpoint_summary(),
                    approved
                )
            })
            .collect();
        rows.sort();
        rows
    }

    /// 获取指定客户端
    pub fn get_client(&self, name: &str) -> Option<Arc<McpClient>> {
        self.clients.get(name).cloned()
    }

    /// 对指定服务器执行 OAuth 认证
    pub async fn authenticate_server(&self, server_name: &str) -> anyhow::Result<()> {
        if let Some(client) = self.clients.get(server_name) {
            client.authenticate_oauth().await
        } else {
            anyhow::bail!("MCP server '{}' not found", server_name)
        }
    }

    /// Reset a server's circuit breaker so the next call can retry immediately.
    pub fn repair_server(&self, server_name: &str) -> anyhow::Result<String> {
        let Some(client) = self.clients.get(server_name) else {
            anyhow::bail!("MCP server '{}' not found", server_name);
        };
        client.reset_circuit_breaker();
        Ok(format!(
            "MCP server '{}' repair applied: circuit breaker reset. Run /mcp status or retry the MCP action.",
            server_name
        ))
    }

    /// 关闭所有 MCP 客户端并清理子进程
    pub async fn shutdown(&self) {
        for (name, client) in &self.clients {
            if let Err(e) = client.shutdown().await {
                warn!("Failed to shutdown MCP client {}: {}", name, e);
            }
        }
    }

    /// 获取健康诊断报告
    pub fn health_diagnostics(&self) -> Vec<McpServerHealth> {
        self.clients
            .iter()
            .map(|(name, client)| {
                let circuit_status = client.circuit_breaker_status();
                let is_approved = self.is_server_approved(name);
                let oauth_configured = client.oauth_configured();
                let oauth_token_present = client.oauth_token_present();
                let transport = match client.transport() {
                    McpTransport::Stdio => "stdio",
                    McpTransport::WebSocket => "websocket",
                    McpTransport::Http => "http",
                };

                // 确定健康状态
                let health = if !is_approved {
                    McpHealthStatus::Pending
                } else if circuit_status.contains("OPEN") {
                    McpHealthStatus::Unhealthy
                } else if circuit_status.contains("HALF-OPEN") {
                    McpHealthStatus::Degraded
                } else {
                    McpHealthStatus::Healthy
                };
                let repair_hint = if !is_approved {
                    format!("/mcp approve {}", name)
                } else if oauth_configured && !oauth_token_present {
                    format!("/mcp auth {}", name)
                } else if matches!(
                    health,
                    McpHealthStatus::Unhealthy | McpHealthStatus::Degraded
                ) {
                    format!("/mcp repair {}", name)
                } else {
                    "none".to_string()
                };

                McpServerHealth {
                    name: name.clone(),
                    transport: transport.to_string(),
                    health,
                    circuit_breaker: circuit_status,
                    approved: is_approved,
                    oauth_configured,
                    oauth_token_present,
                    repair_hint,
                }
            })
            .collect()
    }

    /// 获取健康报告字符串（用于 /doctor）
    pub fn health_report(&self) -> String {
        let diagnostics = self.health_diagnostics();
        if diagnostics.is_empty() {
            return "mcp_health: no servers configured".to_string();
        }

        let summary: Vec<String> = diagnostics
            .iter()
            .map(|d| {
                let status = match d.health {
                    McpHealthStatus::Healthy => "HEALTHY",
                    McpHealthStatus::Degraded => "DEGRADED",
                    McpHealthStatus::Unhealthy => "UNHEALTHY",
                    McpHealthStatus::Pending => "PENDING",
                };
                format!("{}:{}({})", d.name, status, d.circuit_breaker)
            })
            .collect();

        format!("mcp_health: {}", summary.join(", "))
    }

    /// 获取可用的（健康的）服务器列表
    pub fn available_servers(&self) -> Vec<String> {
        self.health_diagnostics()
            .into_iter()
            .filter(|d| d.health == McpHealthStatus::Healthy && d.approved)
            .map(|d| d.name)
            .collect()
    }

    /// 获取降级模式下的服务器列表（半开或熔断中但可部分工作）
    pub fn degraded_servers(&self) -> Vec<String> {
        self.health_diagnostics()
            .into_iter()
            .filter(|d| {
                matches!(
                    d.health,
                    McpHealthStatus::Degraded | McpHealthStatus::Unhealthy
                ) && d.approved
            })
            .map(|d| d.name)
            .collect()
    }
}

/// MCP 服务器健康状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpHealthStatus {
    /// 健康
    Healthy,
    /// 降级（熔断器半开，尝试恢复中）
    Degraded,
    /// 不健康（熔断器打开）
    Unhealthy,
    /// 待批准
    Pending,
}

/// MCP 服务器健康信息
#[derive(Debug, Clone)]
pub struct McpServerHealth {
    /// 服务器名称
    pub name: String,
    /// 传输类型
    pub transport: String,
    /// 健康状态
    pub health: McpHealthStatus,
    /// 熔断器状态描述
    pub circuit_breaker: String,
    /// 是否已批准
    pub approved: bool,
    /// Whether OAuth is configured for this server.
    pub oauth_configured: bool,
    /// Whether an OAuth token is currently stored.
    pub oauth_token_present: bool,
    /// Human-readable repair command or next action.
    pub repair_hint: String,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

/// MCP 工具适配器 - 将 MCP 工具包装为本地 Tool
pub struct McpToolAdapter {
    /// 工具定义
    tool_def: McpToolDef,
    /// MCP 客户端
    client: Arc<McpClient>,
}

impl McpToolAdapter {
    pub fn new(tool_def: McpToolDef, client: Arc<McpClient>) -> Self {
        Self { tool_def, client }
    }
}

#[async_trait::async_trait]
impl crate::tools::Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.tool_def.name
    }

    fn description(&self) -> &str {
        &self.tool_def.description
    }

    fn parameters(&self) -> Value {
        self.tool_def.input_schema.clone()
    }

    async fn execute(
        &self,
        params: Value,
        _context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        match self.client.call_tool(&self.tool_def.name, params).await {
            Ok(result) => crate::tools::ToolResult::success(result),
            Err(e) => crate::tools::ToolResult::error(format!(
                "MCP tool '{}' failed: {}",
                self.tool_def.name, e
            )),
        }
    }
}

/// MCP 管理工具 - 让 agent 管理 MCP 服务器连接
pub struct McpManageTool;

#[async_trait::async_trait]
impl crate::tools::Tool for McpManageTool {
    fn name(&self) -> &str {
        "mcp"
    }

    fn description(&self) -> &str {
        "Manage MCP (Model Context Protocol) server connections. List servers, \
         discover tools, and call remote tools."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list_servers", "list_tools", "list_prompts", "call_tool", "auth_server"],
                    "description": "list_servers: show connected servers. \
                                   list_tools: show all available MCP tools. \
                                   list_prompts: show MCP prompts that can become commands. \
                                   call_tool: invoke an MCP tool. \
                                   auth_server: authenticate a server with OAuth."
                },
                "server_name": {
                    "type": "string",
                    "description": "MCP server name (for 'auth_server')"
                },
                "tool_name": {
                    "type": "string",
                    "description": "Tool name (for 'call_tool')"
                },
                "arguments": {
                    "type": "object",
                    "description": "Tool arguments (for 'call_tool')"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        let action = params["action"].as_str().unwrap_or("list_servers");

        // 检查 MCP 管理器是否可用
        let mcp_manager = match &context.mcp_manager {
            Some(manager) => manager.clone(),
            None => {
                return crate::tools::ToolResult::error(
                    "MCP manager not available. Configure MCP servers in settings.".to_string(),
                );
            }
        };

        match action {
            "list_servers" => {
                let servers = mcp_manager.server_summaries();
                if servers.is_empty() {
                    crate::tools::ToolResult::success("No MCP servers configured.".to_string())
                } else {
                    crate::tools::ToolResult::success(format!(
                        "Connected MCP servers:\n{}",
                        servers.join("\n")
                    ))
                }
            }
            "list_tools" => {
                let tools = mcp_manager.list_tools().await;
                if tools.is_empty() {
                    crate::tools::ToolResult::success(
                        "No tools available from MCP servers.".to_string(),
                    )
                } else {
                    let tool_list: Vec<String> = tools
                        .iter()
                        .map(|t| format!("- {} ({})", t.name, t.server_name))
                        .collect();
                    crate::tools::ToolResult::success(format!(
                        "Available MCP tools ({}):\n{}",
                        tools.len(),
                        tool_list.join("\n")
                    ))
                }
            }
            "list_prompts" => {
                let prompts = mcp_manager.discover_all_prompts().await;
                if prompts.is_empty() {
                    crate::tools::ToolResult::success(
                        "No prompts available from approved MCP servers.".to_string(),
                    )
                } else {
                    let prompt_list = prompts
                        .iter()
                        .map(|p| {
                            let desc = if p.description.is_empty() {
                                "(no description)"
                            } else {
                                &p.description
                            };
                            format!("- /mcp__{}__{}: {}", p.server_name, p.name, desc)
                        })
                        .collect::<Vec<_>>();
                    crate::tools::ToolResult::success_with_data(
                        format!(
                            "Available MCP prompts ({}):\n{}",
                            prompts.len(),
                            prompt_list.join("\n")
                        ),
                        json!({ "prompts": prompts }),
                    )
                }
            }
            "call_tool" => {
                let tool_name = params["tool_name"].as_str().unwrap_or("");
                if tool_name.is_empty() {
                    return crate::tools::ToolResult::error(
                        "tool_name is required for 'call_tool'".to_string(),
                    );
                }

                let arguments = params["arguments"].clone();

                match mcp_manager.call_tool(tool_name, arguments).await {
                    Ok(result) => crate::tools::ToolResult::success(result),
                    Err(e) => crate::tools::ToolResult::error(format!(
                        "MCP tool '{}' failed: {}",
                        tool_name, e
                    )),
                }
            }
            "auth_server" => {
                let server_name = params["server_name"].as_str().unwrap_or("");
                if server_name.is_empty() {
                    return crate::tools::ToolResult::error(
                        "server_name is required for 'auth_server'".to_string(),
                    );
                }
                match mcp_manager.authenticate_server(server_name).await {
                    Ok(_) => crate::tools::ToolResult::success(format!(
                        "MCP OAuth authentication succeeded for '{}'",
                        server_name
                    )),
                    Err(e) => crate::tools::ToolResult::error(format!(
                        "MCP OAuth authentication failed for '{}': {}",
                        server_name, e
                    )),
                }
            }
            _ => crate::tools::ToolResult::error(format!("Unknown action: {}", action)),
        }
    }

    fn is_available(&self, context: &crate::tools::ToolContext) -> bool {
        context.mcp_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &crate::tools::ToolContext) -> Option<String> {
        Some("MCP manager not configured".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_config() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            transport: McpTransport::Stdio,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "test-mcp".to_string()],
            env: HashMap::new(),
            websocket_url: None,
            http_url: None,
            headers: HashMap::new(),
            oauth: None,
            oauth_token_url: None,
        };
        assert_eq!(config.name, "test-server");
        assert_eq!(config.command, "npx");
    }

    #[test]
    fn test_mcp_manager_creation() {
        let manager = McpManager::new();
        assert!(manager.server_names().is_empty());
    }

    #[test]
    fn test_mcp_tool_def() {
        let tool = McpToolDef {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                }
            }),
            server_name: "filesystem".to_string(),
        };
        assert_eq!(tool.name, "read_file");
        assert_eq!(tool.server_name, "filesystem");
    }

    #[test]
    fn test_mcp_request_serialization() {
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "tools/list".to_string(),
            params: json!({}),
        };
        let json_str = serde_json::to_string(&request).unwrap();
        assert!(json_str.contains("jsonrpc"));
        assert!(json_str.contains("tools/list"));
    }

    #[test]
    fn test_mcp_manager_approval() {
        let manager = McpManager::new();
        assert!(!manager.is_server_approved("test-server"));

        manager.approve_server("test-server");
        assert!(manager.is_server_approved("test-server"));

        manager.revoke_server("test-server");
        assert!(!manager.is_server_approved("test-server"));
    }

    #[test]
    fn test_mcp_manager_approval_disabled() {
        let manager = McpManager::new();
        manager.set_require_server_approval(false);
        assert!(manager.is_server_approved("any-server"));
    }

    #[test]
    fn test_mcp_manager_approved_names() {
        let manager = McpManager::new();
        manager.approve_server("server-a");
        manager.approve_server("server-b");
        let names = manager.approved_server_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"server-a".to_string()));
        assert!(names.contains(&"server-b".to_string()));
    }

    #[test]
    fn test_parse_oauth_token_response() {
        let v = json!({
            "access_token": "acc-1",
            "refresh_token": "ref-1",
            "token_type": "Bearer",
            "expires_in": 3600,
            "scope": "read write"
        });
        let token = parse_oauth_token_response(&v).expect("parse token");
        assert_eq!(token.access_token, "acc-1");
        assert_eq!(token.refresh_token.as_deref(), Some("ref-1"));
        assert_eq!(token.token_type, "Bearer");
        assert!(token.expires_at.is_some());
    }

    #[test]
    fn test_http_endpoint_summary() {
        let config = McpServerConfig {
            name: "http-test".to_string(),
            transport: McpTransport::Http,
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            websocket_url: None,
            http_url: Some("https://mcp.example.com/rpc".to_string()),
            headers: HashMap::new(),
            oauth: None,
            oauth_token_url: None,
        };
        let client = McpClient::new(config);
        assert!(client
            .endpoint_summary()
            .contains("http:https://mcp.example.com/rpc"));
    }

    #[tokio::test]
    async fn test_mcp_websocket_disconnect_detected() {
        let config = McpServerConfig {
            name: "ws-test".to_string(),
            transport: McpTransport::WebSocket,
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            websocket_url: Some("ws://localhost:9999".to_string()),
            http_url: None,
            headers: HashMap::new(),
            oauth: None,
            oauth_token_url: None,
        };
        let client = McpClient::new(config);

        // 手动构造一个已断开的 WebSocket 连接
        let (write_tx, _write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let handle = tokio::spawn(async {});
        handle.abort();
        let disconnected = Arc::new(std::sync::atomic::AtomicBool::new(true));

        let fake_conn = McpConnection {
            pending: Arc::new(Mutex::new(HashMap::new())),
            transport: McpTransportConnection::WebSocket {
                write_tx,
                read_handle: handle,
                disconnected,
            },
        };

        {
            let mut conn = client.connection.lock().await;
            *conn = Some(fake_conn);
        }

        // health_check 应检测到断开并返回错误
        let result = client.health_check().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disconnected"));

        // 连接应被清空
        let conn = client.connection.lock().await;
        assert!(conn.is_none());
    }

    #[tokio::test]
    async fn test_mcp_http_transport_list_tools() {
        use axum::{extract::Json, routing::post, Router};
        use std::net::SocketAddr;

        async fn rpc_handler(Json(req): Json<Value>) -> Json<Value> {
            let id = req["id"].as_u64().unwrap_or(0);
            let method = req["method"].as_str().unwrap_or("");
            if method == "tools/list" {
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [{
                            "name": "echo_http",
                            "description": "Echo tool",
                            "inputSchema": {"type":"object","properties":{"x":{"type":"string"}}}
                        }]
                    }
                }))
            } else {
                Json(json!({
                    "jsonrpc":"2.0",
                    "id": id,
                    "error": {"code": -32601, "message": "method not found"}
                }))
            }
        }

        let app = Router::new().route("/rpc", post(rpc_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr: SocketAddr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let config = McpServerConfig {
            name: "http-local".to_string(),
            transport: McpTransport::Http,
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            websocket_url: None,
            http_url: Some(format!("http://{}/rpc", addr)),
            headers: HashMap::new(),
            oauth: None,
            oauth_token_url: None,
        };
        let client = McpClient::new(config);
        let tools = client.discover_tools().await.expect("discover tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo_http");
    }
}

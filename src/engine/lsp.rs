//! LSP (Language Server Protocol) 客户端
//!
//! 支持连接语言服务器，获取诊断、悬停、定义、引用、符号等信息。
//! 复用 MCP 的长连接 stdio 模式。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// 诊断缓存最大条目数
const MAX_DIAGNOSTIC_CACHE_SIZE: usize = 1000;

/// LSP 服务器配置
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// 服务器名称
    pub name: String,
    /// 启动命令
    pub command: String,
    /// 命令参数
    pub args: Vec<String>,
    /// 工作区根目录
    pub root_uri: String,
}

/// LSP 诊断信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspDiagnostic {
    pub range: LspRange,
    pub severity: Option<u8>,
    pub code: Option<Value>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspLocation {
    pub uri: String,
    pub range: LspRange,
}

/// JSON-RPC 请求
#[derive(Debug, Serialize)]
struct LspRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Value,
}

/// JSON-RPC 通知
#[derive(Debug, Serialize)]
struct LspNotification {
    jsonrpc: String,
    method: String,
    params: Value,
}

/// JSON-RPC 响应
#[derive(Debug, Deserialize)]
struct LspResponse {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<LspError>,
}

#[derive(Debug, Deserialize)]
struct LspError {
    code: i64,
    message: String,
}

/// LSP 连接状态
struct LspConnection {
    /// 待处理请求的响应通道
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<LspResponse>>>>,
    /// 标准输入写入端
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    /// 子进程句柄（用于关闭）
    child: tokio::process::Child,
    /// stderr 读取任务
    stderr_handle: tokio::task::JoinHandle<()>,
    /// stdout 读取任务
    stdout_handle: tokio::task::JoinHandle<()>,
}

/// LSP 客户端 - 连接单个语言服务器
pub struct LspClient {
    /// 服务器配置
    config: LspServerConfig,
    /// 请求 ID 计数器
    request_id: Arc<std::sync::atomic::AtomicU64>,
    /// 已建立的连接（长连接）
    connection: Arc<Mutex<Option<LspConnection>>>,
    /// 已初始化标志
    initialized: Arc<RwLock<bool>>,
    /// 诊断缓存: uri -> diagnostics
    diagnostics: Arc<RwLock<HashMap<String, Vec<LspDiagnostic>>>>,
}

impl LspClient {
    /// 创建新的 LSP 客户端
    pub fn new(config: LspServerConfig) -> Self {
        Self {
            config,
            request_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            connection: Arc::new(Mutex::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            diagnostics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取下一个请求 ID
    fn next_id(&self) -> u64 {
        self.request_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// 确保已连接到 LSP 服务器（stdio 长连接）
    async fn ensure_connected(&self) -> anyhow::Result<()> {
        let mut conn_guard = self.connection.lock().await;
        if conn_guard.is_some() {
            return Ok(());
        }

        info!("Connecting to LSP server {} via stdio", self.config.name);

        let mut child = tokio::process::Command::new(&self.config.command)
            .args(&self.config.args)
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

        let pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<LspResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_stdout = pending.clone();
        let server_name = self.config.name.clone();
        let diagnostics = self.diagnostics.clone();

        // stderr 读取任务
        let stderr_server_name = server_name.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                debug!("LSP server {} stderr: {}", stderr_server_name, line);
            }
        });

        // stdout 读取任务 — 按 JSON-RPC Content-Length 协议解析
        let stdout_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                // 读取 headers
                let mut content_length: Option<usize> = None;
                loop {
                    let mut header = String::new();
                    match reader.read_line(&mut header).await {
                        Ok(0) => {
                            debug!("LSP stdout EOF for {}", server_name);
                            return;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            warn!("LSP stdout read error from {}: {}", server_name, e);
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
                        warn!("LSP message without Content-Length from {}", server_name);
                        continue;
                    }
                };

                let mut body = vec![0u8; len];
                if let Err(e) = reader.read_exact(&mut body).await {
                    warn!("LSP stdout body read error from {}: {}", server_name, e);
                    return;
                }

                let body_str = match String::from_utf8(body) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("LSP invalid UTF-8 from {}: {}", server_name, e);
                        continue;
                    }
                };

                debug!(
                    "LSP message from {}: {}",
                    server_name,
                    &body_str[..body_str.len().min(500)]
                );

                // 先尝试解析为响应
                if let Ok(response) = serde_json::from_str::<LspResponse>(&body_str) {
                    let id = response.id;
                    if let Some(tx) = pending_stdout.lock().await.remove(&id) {
                        let _ = tx.send(response);
                    } else {
                        warn!("Received LSP response with unknown request id: {}", id);
                    }
                    continue;
                }

                // 尝试解析为通知
                if let Ok(notification) = serde_json::from_str::<Value>(&body_str) {
                    if notification.get("id").is_none() {
                        if let Some(method) = notification["method"].as_str() {
                            if method == "textDocument/publishDiagnostics" {
                                if let Some(params) = notification.get("params") {
                                    if let Some(uri) = params["uri"].as_str() {
                                        let diags: Vec<LspDiagnostic> = params["diagnostics"]
                                            .as_array()
                                            .map(|arr| {
                                                arr.iter()
                                                    .filter_map(|v| {
                                                        serde_json::from_value(v.clone()).ok()
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        let mut diag_map = diagnostics.write().await;
                                        if diag_map.len() >= MAX_DIAGNOSTIC_CACHE_SIZE {
                                            let to_remove: Vec<String> = diag_map
                                                .keys()
                                                .take(diag_map.len() / 2)
                                                .cloned()
                                                .collect();
                                            for k in to_remove {
                                                diag_map.remove(&k);
                                            }
                                        }
                                        diag_map.insert(uri.to_string(), diags);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        *conn_guard = Some(LspConnection {
            pending,
            stdin: Arc::new(Mutex::new(stdin)),
            child,
            stderr_handle,
            stdout_handle,
        });

        Ok(())
    }

    /// 关闭 LSP 连接并清理子进程
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        if *self.initialized.read().await {
            let _ = self.request("shutdown", json!({})).await;
            let _ = self.notify("exit", json!({})).await;
        }

        let mut conn_guard = self.connection.lock().await;
        if let Some(mut conn) = conn_guard.take() {
            let _ = conn.child.kill().await;
            let _ = conn.child.wait().await;
            conn.stderr_handle.abort();
            conn.stdout_handle.abort();
        }

        *self.initialized.write().await = false;
        Ok(())
    }

    /// 初始化 LSP 连接
    pub async fn initialize(&self) -> anyhow::Result<()> {
        if *self.initialized.read().await {
            return Ok(());
        }

        self.ensure_connected().await?;

        let params = json!({
            "processId": std::process::id(),
            "rootUri": self.config.root_uri,
            "capabilities": {
                "textDocument": {
                    "hover": { "dynamicRegistration": false },
                    "definition": { "dynamicRegistration": false },
                    "references": { "dynamicRegistration": false },
                    "documentSymbol": { "dynamicRegistration": false },
                    "publishDiagnostics": { "relatedInformation": false }
                },
                "workspace": {
                    "symbol": { "dynamicRegistration": false }
                }
            }
        });

        let _ = self.request("initialize", params).await?;

        // 发送 initialized 通知
        self.notify("initialized", json!({})).await?;

        *self.initialized.write().await = true;
        info!("LSP server {} initialized", self.config.name);

        Ok(())
    }

    /// 发送请求并获取响应
    async fn request(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.ensure_connected().await?;

        let id = self.next_id();
        let request = LspRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let request_json = serde_json::to_string(&request)?;

        debug!(
            "LSP request to {}: {}",
            self.config.name,
            &request_json[..request_json.len().min(200)]
        );

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let conn_guard = self.connection.lock().await;
            let conn = conn_guard
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("LSP connection not established"))?;
            conn.pending.lock().await.insert(id, tx);

            let mut stdin = conn.stdin.lock().await;
            let msg = format!(
                "Content-Length: {}\r\n\r\n{}",
                request_json.len(),
                request_json
            );
            stdin.write_all(msg.as_bytes()).await?;
        }

        let response = match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(_)) => {
                if let Some(conn) = self.connection.lock().await.as_ref() {
                    conn.pending.lock().await.remove(&id);
                }
                anyhow::bail!("LSP response channel closed")
            }
            Err(_) => {
                if let Some(conn) = self.connection.lock().await.as_ref() {
                    conn.pending.lock().await.remove(&id);
                }
                anyhow::bail!("LSP request timed out after 30 seconds")
            }
        };

        if let Some(err) = response.error {
            anyhow::bail!("LSP error ({}): {}", err.code, err.message);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("LSP response has no result"))
    }

    /// 发送通知（不需要响应）
    async fn notify(&self, method: &str, params: Value) -> anyhow::Result<()> {
        self.ensure_connected().await?;

        let notification = LspNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let notification_json = serde_json::to_string(&notification)?;

        let conn_guard = self.connection.lock().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LSP connection not established"))?;
        let mut stdin = conn.stdin.lock().await;
        let msg = format!(
            "Content-Length: {}\r\n\r\n{}",
            notification_json.len(),
            notification_json
        );
        stdin.write_all(msg.as_bytes()).await?;

        Ok(())
    }

    /// 打开文档通知（用于让服务器分析文件）
    pub async fn text_document_did_open(
        &self,
        uri: &str,
        language_id: &str,
        text: &str,
    ) -> anyhow::Result<()> {
        self.initialize().await?;
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        )
        .await
    }

    /// 获取文档诊断
    pub async fn get_diagnostics(&self, uri: &str) -> Vec<LspDiagnostic> {
        self.diagnostics
            .read()
            .await
            .get(uri)
            .cloned()
            .unwrap_or_default()
    }

    /// Hover
    pub async fn text_document_hover(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
        .await
    }

    /// Definition
    pub async fn text_document_definition(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
        .await
    }

    /// References
    pub async fn text_document_references(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "context": { "includeDeclaration": include_declaration }
            }),
        )
        .await
    }

    /// Document Symbols
    pub async fn text_document_document_symbol(&self, uri: &str) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "textDocument/documentSymbol",
            json!({
                "textDocument": { "uri": uri }
            }),
        )
        .await
    }

    /// Workspace Symbols
    pub async fn workspace_symbol(&self, query: &str) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "workspace/symbol",
            json!({
                "query": query
            }),
        )
        .await
    }

    /// Go to Implementation
    pub async fn text_document_implementation(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "textDocument/implementation",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
        .await
    }

    /// Prepare Call Hierarchy
    pub async fn text_document_prepare_call_hierarchy(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "textDocument/prepareCallHierarchy",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
        .await
    }

    /// Call Hierarchy Incoming Calls
    pub async fn call_hierarchy_incoming_calls(&self, item: &Value) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "callHierarchy/incomingCalls",
            json!({
                "item": item
            }),
        )
        .await
    }

    /// Call Hierarchy Outgoing Calls
    pub async fn call_hierarchy_outgoing_calls(&self, item: &Value) -> anyhow::Result<Value> {
        self.initialize().await?;
        self.request(
            "callHierarchy/outgoingCalls",
            json!({
                "item": item
            }),
        )
        .await
    }

    /// 获取服务器名称
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

/// LSP 管理器 - 管理多个语言服务器
pub struct LspManager {
    /// 已连接的服务器
    clients: HashMap<String, Arc<LspClient>>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// 添加 LSP 服务器
    pub fn add_server(&mut self, config: LspServerConfig) {
        let name = config.name.clone();
        let client = Arc::new(LspClient::new(config));
        self.clients.insert(name, client);
    }

    /// 自动检测并添加语言服务器
    pub fn detect_servers(&mut self, working_dir: &Path) {
        let root_uri = format!(
            "file://{}",
            working_dir
                .canonicalize()
                .unwrap_or_else(|_| working_dir.to_path_buf())
                .display()
        );

        // Rust
        if working_dir.join("Cargo.toml").exists() {
            self.add_server(LspServerConfig {
                name: "rust-analyzer".to_string(),
                command: "rust-analyzer".to_string(),
                args: vec![],
                root_uri: root_uri.clone(),
            });
            info!("Auto-detected rust-analyzer for Rust project");
        }

        // TypeScript / JavaScript
        if working_dir.join("package.json").exists() {
            self.add_server(LspServerConfig {
                name: "typescript-language-server".to_string(),
                command: "typescript-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                root_uri: root_uri.clone(),
            });
            info!("Auto-detected typescript-language-server for TS/JS project");
        }

        // Go
        if working_dir.join("go.mod").exists() {
            self.add_server(LspServerConfig {
                name: "gopls".to_string(),
                command: "gopls".to_string(),
                args: vec![],
                root_uri: root_uri.clone(),
            });
            info!("Auto-detected gopls for Go project");
        }

        // Python
        if working_dir.join("requirements.txt").exists()
            || working_dir.join("pyproject.toml").exists()
            || glob::Pattern::new("*.py")
                .ok()
                .and_then(|p| {
                    std::fs::read_dir(working_dir).ok().map(|rd| {
                        rd.flatten()
                            .any(|e| p.matches(e.file_name().to_string_lossy().as_ref()))
                    })
                })
                .unwrap_or(false)
        {
            self.add_server(LspServerConfig {
                name: "pylsp".to_string(),
                command: "pylsp".to_string(),
                args: vec![],
                root_uri: root_uri.clone(),
            });
            info!("Auto-detected pylsp for Python project");
        }
    }

    /// 获取指定客户端
    pub fn get_client(&self, name: &str) -> Option<Arc<LspClient>> {
        self.clients.get(name).cloned()
    }

    /// 获取第一个可用的客户端（用于自动选择）
    pub fn first_client(&self) -> Option<Arc<LspClient>> {
        self.clients.values().next().cloned()
    }

    /// 获取管理器中的服务器列表
    pub fn server_names(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    /// 获取客户端数量
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// 关闭所有 LSP 客户端并清理子进程
    pub async fn shutdown(&self) {
        for (name, client) in &self.clients {
            if let Err(e) = client.shutdown().await {
                warn!("Failed to shutdown LSP client {}: {}", name, e);
            }
        }
    }

    /// 动态注册 LSP 服务器
    pub async fn register_server(&mut self, config: LspServerConfig) -> anyhow::Result<()> {
        let name = config.name.clone();
        if self.clients.contains_key(&name) {
            anyhow::bail!("Server '{}' is already registered", name);
        }
        let client = Arc::new(LspClient::new(config));
        // 尝试初始化连接
        client.initialize().await?;
        self.clients.insert(name.clone(), client);
        info!("Dynamically registered LSP server: {}", name);
        Ok(())
    }

    /// 动态注销 LSP 服务器
    pub async fn unregister_server(&mut self, name: &str) -> anyhow::Result<()> {
        if let Some(client) = self.clients.remove(name) {
            client.shutdown().await?;
            info!("Unregistered LSP server: {}", name);
            Ok(())
        } else {
            anyhow::bail!("Server '{}' is not registered", name)
        }
    }

    /// 检查服务器是否已注册
    pub fn is_registered(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }

    /// 获取所有已注册服务器的状态
    pub fn server_status(&self) -> Vec<ServerStatus> {
        self.clients
            .keys()
            .map(|name| ServerStatus {
                name: name.clone(),
                connected: true, // 如果在 clients 中，说明已连接
            })
            .collect()
    }
}

/// 服务器状态
#[derive(Debug, Clone)]
pub struct ServerStatus {
    pub name: String,
    pub connected: bool,
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 辅助函数：将文件路径转换为 LSP URI
pub fn path_to_uri(path: &Path) -> String {
    format!(
        "file://{}",
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
    )
}

/// 辅助函数：从 LSP URI 获取文件路径
pub fn uri_to_path(uri: &str) -> PathBuf {
    if let Some(path_str) = uri.strip_prefix("file://") {
        PathBuf::from(path_str)
    } else {
        PathBuf::from(uri)
    }
}

/// 辅助函数：根据文件扩展名推断 languageId
pub fn language_id_from_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("jsx") => "javascriptreact",
        Some("tsx") => "typescriptreact",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") => "c",
        Some("cpp") | Some("cc") | Some("cxx") => "cpp",
        Some("h") => "c",
        Some("hpp") => "cpp",
        Some("md") => "markdown",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("yaml") | Some("yml") => "yaml",
        Some("sh") => "shellscript",
        _ => "plaintext",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_lsp_server_config() {
        let config = LspServerConfig {
            name: "rust-analyzer".to_string(),
            command: "rust-analyzer".to_string(),
            args: vec![],
            root_uri: "file:///project".to_string(),
        };
        assert_eq!(config.name, "rust-analyzer");
    }

    #[test]
    fn test_lsp_manager_creation() {
        let manager = LspManager::new();
        assert!(manager.server_names().is_empty());
    }

    #[test]
    fn test_path_to_uri() {
        let path = Path::new("/home/user/project/src/main.rs");
        let uri = path_to_uri(path);
        assert!(uri.starts_with("file://"));
        assert!(uri.contains("main.rs"));
    }

    #[test]
    fn test_uri_to_path() {
        let path = uri_to_path("file:///home/user/project/src/main.rs");
        assert!(path.to_string_lossy().contains("main.rs"));
    }

    #[test]
    fn test_language_id_from_path() {
        assert_eq!(language_id_from_path(Path::new("main.rs")), "rust");
        assert_eq!(language_id_from_path(Path::new("main.ts")), "typescript");
        assert_eq!(language_id_from_path(Path::new("main.py")), "python");
        assert_eq!(language_id_from_path(Path::new("README")), "plaintext");
    }
}

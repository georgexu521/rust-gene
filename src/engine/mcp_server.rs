//! MCP 服务器实现
//!
//! 将本 agent 作为 MCP 服务器运行，供其他 MCP 客户端连接调用。
//!
//! 协议：<https://spec.modelcontextprotocol.io/>

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::tools::{ToolContext, ToolRegistry};

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// 服务器名称
    pub name: String,
    /// 传输类型
    #[serde(default)]
    pub transport: McpServerTransport,
    /// 端口（用于 HTTP transport）
    #[serde(default)]
    pub port: Option<u16>,
    /// 启动命令（用于 stdio transport 时启动子进程）
    #[serde(default)]
    pub command: String,
    /// 命令参数
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpServerTransport {
    #[default]
    Stdio,
    Http,
}

/// MCP 服务器请求
#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Value,
    method: String,
    params: Option<Value>,
}

/// MCP 响应
#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Debug, Serialize)]
struct McpError {
    code: i64,
    message: String,
}

impl McpError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new(-32600, msg)
    }

    fn method_not_found(msg: impl Into<String>) -> Self {
        Self::new(-32601, msg)
    }

    #[allow(dead_code)]
    fn internal_error(msg: impl Into<String>) -> Self {
        Self::new(-32603, msg)
    }
}

/// MCP 服务器事件
#[derive(Debug)]
pub enum McpServerEvent {
    /// 服务器已启动
    Started,
    /// 收到工具调用请求
    ToolCall { name: String, params: Value },
    /// 服务器已关闭
    Stopped,
}

/// MCP 服务器
pub struct McpServer {
    /// 服务器配置
    config: McpServerConfig,
    /// 工具注册表
    tool_registry: Arc<ToolRegistry>,
    /// 工具上下文
    tool_context: Option<ToolContext>,
    /// 事件发送通道
    event_tx: Option<mpsc::Sender<McpServerEvent>>,
}

impl McpServer {
    /// 创建新的 MCP 服务器
    pub fn new(config: McpServerConfig, tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            config,
            tool_registry,
            tool_context: None,
            event_tx: None,
        }
    }

    /// 设置工具上下文
    pub fn with_context(mut self, context: ToolContext) -> Self {
        self.tool_context = Some(context);
        self
    }

    /// 设置事件通道
    pub fn with_event_channel(mut self, tx: mpsc::Sender<McpServerEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// 获取服务器名称
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// 获取工具列表（用于 tools/list）
    fn handle_list_tools(&self) -> Value {
        let tools: Vec<Value> = self
            .tool_registry
            .iter_tools()
            .map(|tool| {
                json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "inputSchema": tool.parameters(),
                })
            })
            .collect();

        json!({
            "tools": tools
        })
    }

    /// 处理工具调用
    async fn handle_call_tool(&self, name: &str, arguments: Value) -> Result<Value, McpError> {
        let tool = self
            .tool_registry
            .get(name)
            .ok_or_else(|| McpError::method_not_found(format!("Tool '{}' not found", name)))?;

        let context = self
            .tool_context
            .clone()
            .unwrap_or_else(|| ToolContext::new(".", "mcp-server"));

        // 克隆 arguments 以便后续事件使用
        let arguments_clone = arguments.clone();
        let result = tool.execute(arguments, context).await;

        // 发送事件
        if let Some(ref tx) = self.event_tx {
            let _ = tx
                .send(McpServerEvent::ToolCall {
                    name: name.to_string(),
                    params: arguments_clone,
                })
                .await;
        }

        // 转换结果
        if result.success {
            Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": result.content
                    }
                ],
                "isError": false
            }))
        } else {
            Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": result.error.unwrap_or_else(|| "Unknown error".to_string())
                    }
                ],
                "isError": true
            }))
        }
    }

    /// 处理请求
    async fn handle_request(&self, req: McpRequest) -> McpResponse {
        debug!(
            "MCP server {} received: {} {:?}",
            self.config.name, req.method, req.id
        );

        let result: Result<Value, McpError> = match req.method.as_str() {
            "tools/list" => Ok(self.handle_list_tools()),
            "tools/call" => {
                let params = req.params.unwrap_or(json!({}));
                let name = match params["name"].as_str() {
                    Some(n) => n,
                    None => {
                        return McpResponse {
                            jsonrpc: "2.0".to_string(),
                            id: req.id,
                            result: None,
                            error: Some(McpError::invalid_request("Missing 'name' parameter")),
                        };
                    }
                };
                let arguments = params["arguments"].clone();
                match self.handle_call_tool(name, arguments).await {
                    Ok(result) => Ok(result),
                    Err(e) => Err(e),
                }
            }
            "initialize" => Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": self.config.name, "version": "1.0.0" }
            })),
            "ping" => Ok(json!({})),
            _ => Err(McpError::method_not_found(format!(
                "Unknown method: {}",
                req.method
            ))),
        };

        match result {
            Ok(result) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(result),
                error: None,
            },
            Err(e) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(e),
            },
        }
    }

    /// 通过 stdio 运行服务器（读取 JSON-RPC 请求）
    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        info!("Starting MCP server {} via stdio", self.config.name);

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();

        let mut reader = BufReader::new(stdin).lines();

        // 发送初始化
        let init_response = json!({
            "jsonrpc": "2.0",
            "id": Value::Null,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": self.config.name, "version": "1.0.0" }
            }
        });
        let init_line = serde_json::to_string(&init_response)?;
        stdout.write_all(init_line.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;

        // 主循环
        while let Ok(Some(line)) = reader.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            // 解析请求
            let req: McpRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to parse MCP request: {}", e);
                    let error_resp = json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": { "code": -32700, "message": "Parse error" }
                    });
                    stdout
                        .write_all(
                            serde_json::to_string(&error_resp)
                                .expect("MCP error response must serialize")
                                .as_bytes(),
                        )
                        .await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            // 处理请求
            let response = self.handle_request(req).await;

            // 发送响应
            let resp_line = serde_json::to_string(&response)?;
            stdout.write_all(resp_line.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    /// 运行 HTTP 服务器
    pub async fn run_http(&self, port: u16) -> anyhow::Result<()> {
        info!("Starting MCP server {} on port {}", self.config.name, port);

        use axum::{routing::post, Json, Router};

        async fn handle_rpc(
            server: axum::extract::State<Arc<McpServer>>,
            Json(req): Json<McpRequest>,
        ) -> Json<McpResponse> {
            Json(server.handle_request(req).await)
        }

        let app = Router::new()
            .route("/mcp", post(handle_rpc))
            .with_state(Arc::new(self.clone()));

        let addr = format!("0.0.0.0:{}", port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!("MCP HTTP server listening on {}", addr);

        axum::serve(listener, app).await?;

        Ok(())
    }

    /// 启动服务器（根据配置选择传输方式）
    pub async fn start(&self) -> anyhow::Result<()> {
        match self.config.transport {
            McpServerTransport::Stdio => self.run_stdio().await,
            McpServerTransport::Http => {
                let port = self.config.port.unwrap_or(8787);
                self.run_http(port).await
            }
        }
    }
}

impl Clone for McpServer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            tool_registry: self.tool_registry.clone(),
            tool_context: self.tool_context.clone(),
            event_tx: self.event_tx.clone(),
        }
    }
}

/// MCP 服务器管理器
pub struct McpServerManager {
    /// 已启动的服务器
    servers: HashMap<String, Arc<McpServer>>,
    /// 服务器任务句柄
    handles: HashMap<String, tokio::task::JoinHandle<()>>,
}

impl McpServerManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            handles: HashMap::new(),
        }
    }

    /// 启动一个 MCP 服务器
    pub async fn start_server(
        &mut self,
        config: McpServerConfig,
        tool_registry: Arc<ToolRegistry>,
        context: ToolContext,
    ) -> anyhow::Result<()> {
        let name = config.name.clone();
        let server = McpServer::new(config, tool_registry).with_context(context);

        let server = Arc::new(server);
        self.servers.insert(name.clone(), server.clone());

        // 启动服务器任务
        let server_name = name.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("MCP server {} stopped with error: {}", server_name, e);
            }
        });

        self.handles.insert(name, handle);

        Ok(())
    }

    /// 停止服务器
    pub async fn stop_server(&self, name: &str) -> anyhow::Result<()> {
        if let Some(handle) = self.handles.get(name) {
            handle.abort();
        }
        Ok(())
    }

    /// 停止所有服务器
    pub async fn stop_all(&self) {
        for handle in self.handles.values() {
            handle.abort();
        }
    }

    /// 获取服务器列表
    pub fn server_names(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }
}

impl Default for McpServerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_config() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            transport: McpServerTransport::Stdio,
            port: None,
            command: String::new(),
            args: vec![],
        };
        assert_eq!(config.name, "test-server");
        assert_eq!(config.transport, McpServerTransport::Stdio);
    }

    #[test]
    fn test_mcp_error_codes() {
        assert_eq!(McpError::invalid_request("test").code, -32600);
        assert_eq!(McpError::method_not_found("test").code, -32601);
        assert_eq!(McpError::internal_error("test").code, -32603);
    }
}

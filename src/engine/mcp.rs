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

mod client;
#[cfg(test)]
use client::{parse_oauth_token_response, McpConnection, McpTransportConnection};
pub use client::{CircuitBreaker, McpClient};

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

    /// Build runtime-visible MCP facts for CLI status, routing, and diagnostics.
    pub async fn runtime_facts(&self) -> Vec<McpServerRuntimeFacts> {
        let mut diagnostics = self.health_diagnostics();
        diagnostics.sort_by(|a, b| a.name.cmp(&b.name));

        let mut facts = Vec::with_capacity(diagnostics.len());
        for diagnostic in diagnostics {
            let mut tools = Vec::new();
            let mut resources = Vec::new();
            let mut prompts = Vec::new();
            let mut discovery_errors = Vec::new();

            if diagnostic.approved && diagnostic.health == McpHealthStatus::Healthy {
                if let Some(client) = self.clients.get(&diagnostic.name) {
                    match client.discover_tools().await {
                        Ok(items) => {
                            tools = items.into_iter().map(|tool| tool.name).collect();
                            tools.sort();
                        }
                        Err(error) => discovery_errors.push(format!("tools: {}", error)),
                    }
                    match client.discover_resources().await {
                        Ok(items) => {
                            resources = items.into_iter().map(|resource| resource.uri).collect();
                            resources.sort();
                        }
                        Err(error) => discovery_errors.push(format!("resources: {}", error)),
                    }
                    match client.discover_prompts().await {
                        Ok(items) => {
                            prompts = items.into_iter().map(|prompt| prompt.name).collect();
                            prompts.sort();
                        }
                        Err(error) => discovery_errors.push(format!("prompts: {}", error)),
                    }
                }
            }

            let commands = prompts
                .iter()
                .map(|prompt| format!("/mcp__{}__{}", diagnostic.name, prompt))
                .collect::<Vec<_>>();
            let diagnostic_text = if !diagnostic.approved {
                format!(
                    "server pending approval; next action: {}",
                    diagnostic.repair_hint
                )
            } else if diagnostic.oauth_configured && !diagnostic.oauth_token_present {
                format!(
                    "oauth token missing; next action: {}",
                    diagnostic.repair_hint
                )
            } else if !discovery_errors.is_empty() {
                format!(
                    "discovery degraded; {}; next action: {}",
                    discovery_errors.join("; "),
                    diagnostic.repair_hint
                )
            } else if matches!(
                diagnostic.health,
                McpHealthStatus::Degraded | McpHealthStatus::Unhealthy
            ) {
                format!("server degraded; next action: {}", diagnostic.repair_hint)
            } else {
                "server ready".to_string()
            };

            facts.push(McpServerRuntimeFacts {
                name: diagnostic.name,
                transport: diagnostic.transport,
                health: diagnostic.health,
                approved: diagnostic.approved,
                oauth_configured: diagnostic.oauth_configured,
                oauth_token_present: diagnostic.oauth_token_present,
                circuit_breaker: diagnostic.circuit_breaker,
                repair_hint: diagnostic.repair_hint,
                tool_count: tools.len(),
                resource_count: resources.len(),
                prompt_count: prompts.len(),
                commands,
                tools,
                resources,
                prompts,
                diagnostic: diagnostic_text,
            });
        }

        facts
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Runtime-visible MCP integration facts for diagnostics and prompt/routing state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerRuntimeFacts {
    pub name: String,
    pub transport: String,
    pub health: McpHealthStatus,
    pub approved: bool,
    pub oauth_configured: bool,
    pub oauth_token_present: bool,
    pub circuit_breaker: String,
    pub repair_hint: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub prompt_count: usize,
    pub commands: Vec<String>,
    pub tools: Vec<String>,
    pub resources: Vec<String>,
    pub prompts: Vec<String>,
    pub diagnostic: String,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

mod tool;
pub use tool::{McpManageTool, McpToolAdapter};

#[cfg(test)]
mod tests;

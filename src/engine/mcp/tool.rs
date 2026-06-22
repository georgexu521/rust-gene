//! MCP runtime adapter support.
//!
//! Bridges configured MCP servers into the tool surface while preserving server-scoped boundaries.

use super::*;

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

fn scoped_mcp_servers(context: &crate::tools::ToolContext) -> Option<Vec<String>> {
    let value = context.metadata.get("allowed_mcp_servers")?;
    let servers = value
        .split(',')
        .map(str::trim)
        .filter(|server| !server.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if servers.is_empty() {
        None
    } else {
        Some(servers)
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn ensure_mcp_server_allowed(
    context: &crate::tools::ToolContext,
    server_name: &str,
) -> std::result::Result<(), crate::tools::ToolResult> {
    let Some(servers) = scoped_mcp_servers(context) else {
        return Ok(());
    };
    if servers.iter().any(|server| server == server_name) {
        Ok(())
    } else {
        Err(crate::tools::ToolResult::error(format!(
            "MCP server '{}' is outside this agent's allowed MCP scope: {}",
            server_name,
            servers.join(", ")
        )))
    }
}

#[async_trait::async_trait]
impl crate::tools::Tool for McpManageTool {
    fn name(&self) -> &str {
        "mcp"
    }

    fn description(&self) -> &str {
        "Manage MCP (Model Context Protocol) server connections. List servers, \
         discover tools/prompts/resources, read resources, authenticate servers, \
         and call remote tools."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "list_servers",
                        "status",
                        "list_tools",
                        "list_prompts",
                        "list_resources",
                        "read_resource",
                        "call_tool",
                        "auth_server",
                        "repair_server"
                    ],
                    "description": "list_servers: show connected servers. \
                                   status: show health, approval, auth, and discovered MCP runtime facts. \
                                   list_tools: show all available MCP tools. \
                                   list_prompts: show MCP prompts that can become commands. \
                                   list_resources: show available MCP resources. \
                                   read_resource: read one MCP resource by URI. \
                                   call_tool: invoke an MCP tool. \
                                   auth_server: authenticate a server with OAuth. \
                                   repair_server: reset a server circuit breaker."
                },
                "server_name": {
                    "type": "string",
                    "description": "MCP server name (for server-scoped actions)"
                },
                "tool_name": {
                    "type": "string",
                    "description": "Tool name (for 'call_tool')"
                },
                "uri": {
                    "type": "string",
                    "description": "Resource URI (for 'read_resource')"
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
            "status" => {
                let facts = mcp_manager.runtime_facts().await;
                if facts.is_empty() {
                    crate::tools::ToolResult::success("No MCP servers configured.".to_string())
                } else {
                    let rows = facts
                        .iter()
                        .map(|fact| {
                            format!(
                                "- {} [{}] health={:?} approved={} oauth={} tools={} resources={} prompts={} next={}",
                                fact.name,
                                fact.transport,
                                fact.health,
                                fact.approved,
                                if fact.oauth_configured {
                                    if fact.oauth_token_present {
                                        "configured/token"
                                    } else {
                                        "configured/missing_token"
                                    }
                                } else {
                                    "none"
                                },
                                fact.tool_count,
                                fact.resource_count,
                                fact.prompt_count,
                                fact.repair_hint
                            )
                        })
                        .collect::<Vec<_>>();
                    crate::tools::ToolResult::success_with_data(
                        format!("MCP runtime status:\n{}", rows.join("\n")),
                        json!({ "servers": facts }),
                    )
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
            "list_resources" => {
                let server_name = params["server_name"]
                    .as_str()
                    .filter(|name| !name.is_empty());
                if let Some(name) = server_name {
                    if let Err(result) = ensure_mcp_server_allowed(&context, name) {
                        return result;
                    }
                } else if let Some(servers) = scoped_mcp_servers(&context) {
                    return crate::tools::ToolResult::error(format!(
                        "This agent is scoped to MCP servers: {}. Pass server_name explicitly.",
                        servers.join(", ")
                    ));
                }

                let resources = if let Some(name) = server_name {
                    if !mcp_manager.is_server_approved(name) {
                        return crate::tools::ToolResult::error(format!(
                            "MCP server '{}' is not approved. Use '/mcp approve {}' to approve it.",
                            name, name
                        ));
                    }
                    if let Err(error) = mcp_manager.health_check(name).await {
                        return crate::tools::ToolResult::error(format!(
                            "MCP server '{}' is not healthy enough to list resources: {}",
                            name, error
                        ));
                    }
                    match mcp_manager.get_client(name) {
                        Some(client) => match client.discover_resources().await {
                            Ok(resources) => resources,
                            Err(error) => {
                                return crate::tools::ToolResult::error(format!(
                                    "Failed to discover resources from {}: {}",
                                    name, error
                                ));
                            }
                        },
                        None => {
                            return crate::tools::ToolResult::error(format!(
                                "MCP server '{}' not found",
                                name
                            ));
                        }
                    }
                } else {
                    mcp_manager.discover_all_resources().await
                };

                if resources.is_empty() {
                    crate::tools::ToolResult::success("No MCP resources available.".to_string())
                } else {
                    let resource_list = resources
                        .iter()
                        .map(|resource| {
                            format!(
                                "- {} ({}): {} [{}]",
                                resource.name,
                                resource.server_name,
                                resource.uri,
                                resource.mime_type.as_deref().unwrap_or("unknown")
                            )
                        })
                        .collect::<Vec<_>>();
                    crate::tools::ToolResult::success_with_data(
                        format!(
                            "Available MCP resources ({}):\n{}",
                            resources.len(),
                            resource_list.join("\n")
                        ),
                        json!({ "resources": resources }),
                    )
                }
            }
            "read_resource" => {
                let server_name = params["server_name"].as_str().unwrap_or("");
                let uri = params["uri"].as_str().unwrap_or("");
                if server_name.is_empty() || uri.is_empty() {
                    return crate::tools::ToolResult::error(
                        "server_name and uri are required for 'read_resource'".to_string(),
                    );
                }
                if let Err(result) = ensure_mcp_server_allowed(&context, server_name) {
                    return result;
                }

                match mcp_manager.read_resource(server_name, uri).await {
                    Ok(resource) => {
                        let content = serde_json::to_string_pretty(&resource)
                            .unwrap_or_else(|_| resource.to_string());
                        crate::tools::ToolResult::success_with_data(
                            content,
                            json!({
                                "server_name": server_name,
                                "uri": uri,
                                "resource": resource,
                            }),
                        )
                    }
                    Err(error) => crate::tools::ToolResult::error(format!(
                        "Failed to read MCP resource '{}': {}",
                        uri, error
                    )),
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
            "repair_server" => {
                let server_name = params["server_name"].as_str().unwrap_or("");
                if server_name.is_empty() {
                    return crate::tools::ToolResult::error(
                        "server_name is required for 'repair_server'".to_string(),
                    );
                }
                if let Err(result) = ensure_mcp_server_allowed(&context, server_name) {
                    return result;
                }
                match mcp_manager.repair_server(server_name) {
                    Ok(message) => crate::tools::ToolResult::success(message),
                    Err(error) => crate::tools::ToolResult::error(format!(
                        "MCP repair failed for '{}': {}",
                        server_name, error
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

//! MCP 深度集成工具
//!
//! 提供直接调用 MCP 工具、资源发现和读取、以及认证功能。

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// 直接调用 MCP 工具
pub struct MCPTool;

#[async_trait]
impl Tool for MCPTool {
    fn name(&self) -> &str {
        "mcp_tool"
    }

    fn description(&self) -> &str {
        "Call a tool directly on a specific MCP server. \
         Use this when you know the server name and tool name."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Name of the MCP server"
                },
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool to call"
                },
                "arguments": {
                    "type": "object",
                    "description": "Arguments to pass to the tool"
                }
            },
            "required": ["server_name", "tool_name"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let server_name = params["server_name"].as_str().unwrap_or("");
        let tool_name = params["tool_name"].as_str().unwrap_or("");
        let arguments = params["arguments"].clone();

        if server_name.is_empty() || tool_name.is_empty() {
            return ToolResult::error("server_name and tool_name are required");
        }

        let manager = match &context.mcp_manager {
            Some(m) => m,
            None => return ToolResult::error("MCP manager not available"),
        };

        match manager
            .call_tool_on_server(server_name, tool_name, arguments)
            .await
        {
            Ok(result) => ToolResult::success(result),
            Err(e) => ToolResult::error(format!("MCP tool call failed: {}", e)),
        }
    }
}

/// MCP 认证工具（简化版占位）
pub struct McpAuthTool;

#[async_trait]
impl Tool for McpAuthTool {
    fn name(&self) -> &str {
        "mcp_auth"
    }

    fn description(&self) -> &str {
        "Authenticate with an MCP server. \
         Note: OAuth flows are not fully implemented in this version."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Name of the MCP server to authenticate with"
                }
            },
            "required": ["server_name"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let server_name = params["server_name"].as_str().unwrap_or("");
        if server_name.is_empty() {
            return ToolResult::error("server_name is required");
        }

        ToolResult::success(format!(
            "MCP authentication for '{}' is not implemented. \
             Servers using stdio transport typically do not require OAuth.",
            server_name
        ))
    }
}

/// 列出 MCP 资源
pub struct ListMcpResourcesTool;

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        "list_mcp_resources"
    }

    fn description(&self) -> &str {
        "List available resources from all connected MCP servers, \
         or from a specific server if server_name is provided."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Optional: specific MCP server name"
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let manager = match &context.mcp_manager {
            Some(m) => m,
            None => return ToolResult::error("MCP manager not available"),
        };

        let server_name = params["server_name"].as_str();

        let resources = if let Some(name) = server_name {
            match manager.get_client(name) {
                Some(client) => match client.discover_resources().await {
                    Ok(r) => r,
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Failed to discover resources from {}: {}",
                            name, e
                        ))
                    }
                },
                None => return ToolResult::error(format!("MCP server '{}' not found", name)),
            }
        } else {
            manager.discover_all_resources().await
        };

        if resources.is_empty() {
            return ToolResult::success("No MCP resources available.".to_string());
        }

        let lines: Vec<String> = resources
            .iter()
            .map(|r| {
                format!(
                    "- {} ({}): {} [{}]",
                    r.name,
                    r.server_name,
                    r.uri,
                    r.mime_type.as_deref().unwrap_or("unknown")
                )
            })
            .collect();

        ToolResult::success_with_data(
            format!(
                "Available MCP resources ({}):\n{}",
                resources.len(),
                lines.join("\n")
            ),
            json!({
                "resources": resources,
                "count": resources.len()
            }),
        )
    }
}

/// 读取 MCP 资源
pub struct ReadMcpResourceTool;

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        "read_mcp_resource"
    }

    fn description(&self) -> &str {
        "Read a resource from an MCP server by its URI."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Name of the MCP server"
                },
                "uri": {
                    "type": "string",
                    "description": "URI of the resource to read"
                }
            },
            "required": ["server_name", "uri"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let server_name = params["server_name"].as_str().unwrap_or("");
        let uri = params["uri"].as_str().unwrap_or("");

        if server_name.is_empty() || uri.is_empty() {
            return ToolResult::error("server_name and uri are required");
        }

        let manager = match &context.mcp_manager {
            Some(m) => m,
            None => return ToolResult::error("MCP manager not available"),
        };

        match manager.read_resource(server_name, uri).await {
            Ok(result) => ToolResult::success_with_data(
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()),
                result,
            ),
            Err(e) => ToolResult::error(format!("Failed to read MCP resource: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_names() {
        assert_eq!(MCPTool.name(), "mcp_tool");
        assert_eq!(McpAuthTool.name(), "mcp_auth");
        assert_eq!(ListMcpResourcesTool.name(), "list_mcp_resources");
        assert_eq!(ReadMcpResourceTool.name(), "read_mcp_resource");
    }
}

//! MCP integration tools.
//!
//! These tools bridge the runtime to configured MCP servers for direct tool
//! calls, resource discovery, resource reads, and authentication. Agent-scoped
//! metadata can restrict which MCP servers are visible to a delegated agent.

use crate::tools::{
    Tool, ToolContext, ToolOperationKind, ToolResult, ToolSearchOrReadSemantics, ToolUiRenderKind,
};
use async_trait::async_trait;
use serde_json::json;

fn scoped_mcp_servers(context: &ToolContext) -> Option<Vec<String>> {
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
fn ensure_mcp_server_allowed(context: &ToolContext, server_name: &str) -> Result<(), ToolResult> {
    let Some(servers) = scoped_mcp_servers(context) else {
        return Ok(());
    };
    if servers.iter().any(|server| server == server_name) {
        Ok(())
    } else {
        Err(ToolResult::error(format!(
            "MCP server '{}' is outside this agent's allowed MCP scope: {}",
            server_name,
            servers.join(", ")
        )))
    }
}

/// Tool implementation for directly invoking a named MCP server tool.
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

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let server = params["server_name"].as_str().unwrap_or("");
        let tool = params["tool_name"].as_str().unwrap_or("");
        format!("mcp_tool: {server}/{tool}")
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Task
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_open_world(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn permission_matcher_input(&self, params: &serde_json::Value) -> Option<String> {
        Some(self.to_classifier_input(params))
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let server = params["server_name"].as_str()?.trim();
        let tool = params["tool_name"].as_str()?.trim();
        if server.is_empty() || tool.is_empty() {
            None
        } else {
            Some(format!("{server}/{tool}"))
        }
    }

    fn ui_render_kind(&self, _params: &serde_json::Value) -> ToolUiRenderKind {
        ToolUiRenderKind::Mcp
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let server_name = params["server_name"].as_str().unwrap_or("");
        let tool_name = params["tool_name"].as_str().unwrap_or("");
        let arguments = params["arguments"].clone();

        if server_name.is_empty() || tool_name.is_empty() {
            return ToolResult::error("server_name and tool_name are required");
        }
        if let Err(result) = ensure_mcp_server_allowed(&context, server_name) {
            return result;
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

    fn is_available(&self, context: &ToolContext) -> bool {
        context.mcp_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        Some("MCP manager not configured".to_string())
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
        "Authenticate with an MCP server using configured OAuth settings. \
         This stores token for subsequent MCP requests."
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

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let server = params["server_name"].as_str().unwrap_or("");
        format!("mcp_auth: {server}")
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Network
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_open_world(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn permission_matcher_input(&self, params: &serde_json::Value) -> Option<String> {
        Some(self.to_classifier_input(params))
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let server = params["server_name"].as_str()?.trim();
        (!server.is_empty()).then(|| server.to_string())
    }

    fn ui_render_kind(&self, _params: &serde_json::Value) -> ToolUiRenderKind {
        ToolUiRenderKind::Mcp
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let server_name = params["server_name"].as_str().unwrap_or("");
        if server_name.is_empty() {
            return ToolResult::error("server_name is required");
        }
        if let Err(result) = ensure_mcp_server_allowed(&context, server_name) {
            return result;
        }

        let manager = match &context.mcp_manager {
            Some(m) => m,
            None => return ToolResult::error("MCP manager not available"),
        };

        match manager.authenticate_server(server_name).await {
            Ok(_) => ToolResult::success(format!(
                "MCP authentication succeeded for '{}'.",
                server_name
            )),
            Err(e) => ToolResult::error(format!(
                "MCP authentication failed for '{}': {}",
                server_name, e
            )),
        }
    }

    fn is_available(&self, context: &ToolContext) -> bool {
        context.mcp_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        Some("MCP manager not configured".to_string())
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

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::List
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_open_world(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_search_or_read_command(&self, _params: &serde_json::Value) -> ToolSearchOrReadSemantics {
        ToolSearchOrReadSemantics {
            is_list: true,
            ..Default::default()
        }
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        params["server_name"]
            .as_str()
            .filter(|server| !server.trim().is_empty())
            .map(|server| format!("resources on {server}"))
            .or_else(|| Some("all resources".to_string()))
    }

    fn ui_render_kind(&self, _params: &serde_json::Value) -> ToolUiRenderKind {
        ToolUiRenderKind::Mcp
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let manager = match &context.mcp_manager {
            Some(m) => m,
            None => return ToolResult::error("MCP manager not available"),
        };

        let server_name = params["server_name"].as_str();
        if let Some(name) = server_name {
            if let Err(result) = ensure_mcp_server_allowed(&context, name) {
                return result;
            }
        } else if let Some(servers) = scoped_mcp_servers(&context) {
            return ToolResult::error(format!(
                "This agent is scoped to MCP servers: {}. Pass server_name explicitly.",
                servers.join(", ")
            ));
        }

        let resources = if let Some(name) = server_name {
            if !manager.is_server_approved(name) {
                return ToolResult::error(format!(
                    "MCP server '{}' is not approved. Use '/mcp approve {}' to approve it.",
                    name, name
                ));
            }
            if let Err(e) = manager.health_check(name).await {
                return ToolResult::error(format!(
                    "MCP server '{}' is not healthy enough to list resources: {}",
                    name, e
                ));
            }
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

    fn is_available(&self, context: &ToolContext) -> bool {
        context.mcp_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        Some("MCP manager not configured".to_string())
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

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let server = params["server_name"].as_str().unwrap_or("");
        let uri = params["uri"].as_str().unwrap_or("");
        format!("read_mcp_resource: {server}/{uri}")
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_open_world(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_search_or_read_command(&self, _params: &serde_json::Value) -> ToolSearchOrReadSemantics {
        ToolSearchOrReadSemantics {
            is_read: true,
            ..Default::default()
        }
    }

    fn permission_matcher_input(&self, params: &serde_json::Value) -> Option<String> {
        Some(self.to_classifier_input(params))
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let server = params["server_name"].as_str()?.trim();
        let uri = params["uri"].as_str()?.trim();
        if server.is_empty() || uri.is_empty() {
            None
        } else {
            Some(format!("{server}/{uri}"))
        }
    }

    fn ui_render_kind(&self, _params: &serde_json::Value) -> ToolUiRenderKind {
        ToolUiRenderKind::Mcp
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let server_name = params["server_name"].as_str().unwrap_or("");
        let uri = params["uri"].as_str().unwrap_or("");

        if server_name.is_empty() || uri.is_empty() {
            return ToolResult::error("server_name and uri are required");
        }
        if let Err(result) = ensure_mcp_server_allowed(&context, server_name) {
            return result;
        }

        let manager = match &context.mcp_manager {
            Some(m) => m,
            None => return ToolResult::error("MCP manager not available"),
        };

        match manager.read_resource(server_name, uri).await {
            Ok(result) => {
                let content =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                let retrieval_context =
                    crate::engine::retrieval_context::RetrievalContext::from_mcp_resource(
                        uri,
                        server_name,
                        uri,
                        &content,
                        crate::engine::intent_router::RetrievalPolicy::Full,
                    )
                    .map(|ctx| ctx.format_for_prompt());
                ToolResult::success_with_data(
                    content,
                    serde_json::json!({
                        "resource": result,
                        "retrieval_context": retrieval_context,
                    }),
                )
            }
            Err(e) => ToolResult::error(format!("Failed to read MCP resource: {}", e)),
        }
    }

    fn is_available(&self, context: &ToolContext) -> bool {
        context.mcp_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        Some("MCP manager not configured".to_string())
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

    #[test]
    fn mcp_scope_allows_only_declared_servers() {
        let mut context = ToolContext::new(".", "test");
        context.metadata.insert(
            "allowed_mcp_servers".to_string(),
            "filesystem,github".to_string(),
        );
        assert!(ensure_mcp_server_allowed(&context, "filesystem").is_ok());
        assert!(ensure_mcp_server_allowed(&context, "github").is_ok());
        assert!(ensure_mcp_server_allowed(&context, "slack").is_err());
    }
}

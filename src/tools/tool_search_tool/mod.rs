//! Tool Search 工具 - 按名称和描述搜索可用工具

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// 工具搜索工具
pub struct ToolSearchTool;

#[async_trait]
impl Tool for ToolSearchTool {
    fn name(&self) -> &str {
        "tool_search"
    }

    fn description(&self) -> &str {
        "Search for available tools by name or description keywords. Use this when you need to find a specific tool."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (keywords or exact tool name)"
                },
                "max_results": {
                    "type": "integer",
                    "default": 5,
                    "description": "Maximum number of results to return"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let query = params["query"].as_str().unwrap_or("").to_lowercase();
        let max_results = params["max_results"].as_u64().unwrap_or(5) as usize;

        if query.is_empty() {
            return ToolResult::error("query is required");
        }

        // Check for select: prefix — direct tool selection
        if let Some(name) = query.strip_prefix("select:") {
            let name = name.trim();
            let registry = crate::tools::ToolRegistry::default_registry();
            if registry.has(name) {
                return ToolResult::success_with_data(
                    format!("Selected tool: {}", name),
                    json!({ "matches": [name], "query": query }),
                );
            } else {
                return ToolResult::success_with_data(
                    format!("Tool '{}' not found", name),
                    json!({ "matches": [], "query": query }),
                );
            }
        }

        let registry = crate::tools::ToolRegistry::default_registry();
        let terms: Vec<&str> = query.split_whitespace().collect();

        let mut scored: Vec<(String, i32)> = Vec::new();
        for tool in registry.iter_tools() {
            let name = tool.name().to_lowercase();
            let desc = tool.description().to_lowercase();
            let mut score = 0;

            for term in &terms {
                if name == *term {
                    score += 20;
                } else if name.contains(term) {
                    score += 10;
                } else if desc.contains(term) {
                    score += 5;
                }
            }

            if score > 0 {
                scored.push((tool.name().to_string(), score));
            }
        }

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        let mut matches: Vec<String> = scored
            .into_iter()
            .map(|(name, _)| name)
            .take(max_results)
            .collect();

        let mut pending_mcp_servers: Vec<String> = Vec::new();
        let mut mcp_matches: Vec<String> = Vec::new();
        if let Some(mcp) = context.mcp_manager {
            let server_names = mcp.server_names();
            let approved = mcp.approved_server_names();
            pending_mcp_servers = server_names
                .into_iter()
                .filter(|s| !approved.iter().any(|a| a == s))
                .collect();
            mcp_matches = search_mcp_tools(&query, max_results, &mcp).await;
            for m in &mcp_matches {
                if matches.len() >= max_results {
                    break;
                }
                if !matches.iter().any(|x| x == m) {
                    matches.push(m.clone());
                }
            }
        }

        ToolResult::success_with_data(
            format!("Found {} matching tools", matches.len()),
            json!({
                "matches": matches,
                "query": query,
                "total_tools": registry.tool_names().len(),
                "mcp_matches": mcp_matches,
                "pending_mcp_servers": pending_mcp_servers
            }),
        )
    }
}

async fn search_mcp_tools(
    query: &str,
    max_results: usize,
    manager: &crate::engine::mcp::McpManager,
) -> Vec<String> {
    let terms: Vec<&str> = query.split_whitespace().collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let defs = manager.list_tools().await;
    let mut scored: Vec<(String, i32)> = Vec::new();

    for def in defs {
        let canonical = format!("mcp/{}/{}", def.server_name, def.name).to_lowercase();
        let desc = def.description.to_lowercase();
        let mut score = 0;
        for term in &terms {
            if canonical == *term {
                score += 30;
            } else if canonical.contains(term) {
                score += 12;
            } else if desc.contains(term) {
                score += 4;
            }
        }
        if score > 0 {
            scored.push((format!("mcp/{}/{}", def.server_name, def.name), score));
        }
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored
        .into_iter()
        .map(|(name, _)| name)
        .take(max_results)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_search_exact() {
        let tool = ToolSearchTool;
        let result = tool
            .execute(json!({"query": "bash"}), ToolContext::new(".", "test"))
            .await;
        assert!(result.success);
        let data = result.data.unwrap();
        let matches = data["matches"].as_array().unwrap();
        assert!(matches.iter().any(|m| m.as_str() == Some("bash")));
    }

    #[tokio::test]
    async fn test_tool_search_select() {
        let tool = ToolSearchTool;
        let result = tool
            .execute(
                json!({"query": "select:file_read"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        let data = result.data.unwrap();
        let matches = data["matches"].as_array().unwrap();
        assert_eq!(matches[0], "file_read");
    }
}

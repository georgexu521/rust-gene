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

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
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
        let matches: Vec<String> = scored
            .into_iter()
            .map(|(name, _)| name)
            .take(max_results)
            .collect();

        ToolResult::success_with_data(
            format!("Found {} matching tools", matches.len()),
            json!({
                "matches": matches,
                "query": query,
                "total_tools": registry.tool_names().len()
            }),
        )
    }
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

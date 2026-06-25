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

    fn search_hint(&self) -> Option<&'static str> {
        Some("load deferred tool schemas")
    }

    fn always_load(&self) -> bool {
        true
    }

    fn strict_schema(&self) -> bool {
        true
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let query = params["query"].as_str().unwrap_or("").to_lowercase();
        let max_results = params["max_results"].as_u64().unwrap_or(5) as usize;

        if query.is_empty() {
            return ToolResult::error("query is required");
        }

        // Check for select: prefix — direct tool selection
        if let Some(names) = query.strip_prefix("select:") {
            let requested: Vec<&str> = names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .collect();
            let registry = crate::tools::ToolRegistry::default_registry();
            let mut matches = Vec::new();
            for name in requested {
                if let Some(tool) = registry.get(name) {
                    let canonical = tool.name().to_string();
                    if !matches.iter().any(|item| item == &canonical) {
                        matches.push(canonical);
                    }
                }
            }
            let tools = tool_match_facts(&registry, &matches);
            return ToolResult::success_with_data(
                format!("Selected {} tool(s)", matches.len()),
                json!({
                    "matches": matches,
                    "query": query,
                    "tools": tools,
                }),
            );
        }

        let registry = crate::tools::ToolRegistry::default_registry();
        let terms: Vec<&str> = query.split_whitespace().collect();

        let mut scored: Vec<(String, i32)> = Vec::new();
        for tool in registry.iter_tools() {
            if !tool.is_available(&context) {
                continue;
            }
            let name = tool.name().to_lowercase();
            let desc = tool.description().to_lowercase();
            let aliases = tool.aliases().join(" ").to_lowercase();
            let search_hint = tool.search_hint().unwrap_or("").to_lowercase();
            let mut score = 0;

            for term in &terms {
                if name == *term {
                    score += 20;
                } else if name.contains(term) {
                    score += 10;
                } else if aliases.split_whitespace().any(|alias| alias == *term) {
                    score += 15;
                } else if aliases.contains(term) {
                    score += 8;
                } else if search_hint.contains(term) {
                    score += 7;
                } else if desc.contains(term) {
                    score += 5;
                }
            }

            if score > 0 {
                scored.push((tool.name().to_string(), score));
            }
        }

        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut matches: Vec<String> = scored
            .into_iter()
            .map(|(name, _)| name)
            .take(max_results)
            .collect();

        let mut pending_mcp_servers: Vec<String> = Vec::new();
        let mut unavailable_mcp_servers: Vec<String> = Vec::new();
        let mut available_mcp_servers: Vec<String> = Vec::new();
        let mut mcp_matches: Vec<String> = Vec::new();
        if let Some(mcp) = context.mcp_manager {
            for diagnostic in mcp.health_diagnostics() {
                if !diagnostic.approved {
                    pending_mcp_servers.push(diagnostic.name);
                } else if diagnostic.health == crate::engine::mcp::McpHealthStatus::Healthy {
                    available_mcp_servers.push(diagnostic.name);
                } else {
                    unavailable_mcp_servers
                        .push(format!("{}:{:?}", diagnostic.name, diagnostic.health));
                }
            }
            pending_mcp_servers.sort();
            available_mcp_servers.sort();
            unavailable_mcp_servers.sort();
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

        let tools = tool_match_facts(&registry, &matches);
        ToolResult::success_with_data(
            format!("Found {} matching tools", matches.len()),
            json!({
                "matches": matches,
                "query": query,
                "tools": tools,
                "total_tools": registry.tool_names().len(),
                "mcp_matches": mcp_matches,
                "pending_mcp_servers": pending_mcp_servers,
                "available_mcp_servers": available_mcp_servers,
                "unavailable_mcp_servers": unavailable_mcp_servers
            }),
        )
    }
}

fn tool_match_facts(
    registry: &crate::tools::ToolRegistry,
    matches: &[String],
) -> Vec<serde_json::Value> {
    matches
        .iter()
        .map(|name| match registry.get(name) {
            Some(tool) => json!({
                "name": tool.name(),
                "aliases": tool.aliases(),
                "search_hint": tool.search_hint(),
                "should_defer": tool.should_defer(),
                "always_load": tool.always_load(),
                "strict_schema": tool.strict_schema(),
            }),
            None => json!({ "name": name }),
        })
        .collect()
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
    let available_servers = manager.available_servers();
    if available_servers.is_empty() {
        return Vec::new();
    }
    let defs = manager.list_available_tools().await;
    let mut scored: Vec<(String, i32)> = Vec::new();

    for def in defs {
        if !available_servers
            .iter()
            .any(|name| name == &def.server_name)
        {
            continue;
        }
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

    scored.sort_by_key(|score| std::cmp::Reverse(score.1));
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
                json!({"query": "select:read,file_edit"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        let data = result.data.unwrap();
        let matches = data["matches"].as_array().unwrap();
        assert_eq!(matches[0], "file_read");
        assert!(matches
            .iter()
            .any(|value| value.as_str() == Some("file_edit")));
        assert_eq!(data["tools"][0]["aliases"], serde_json::json!(["read"]));
        assert_eq!(data["tools"][0]["strict_schema"], true);
    }

    #[tokio::test]
    async fn test_tool_search_uses_search_hints() {
        let tool = ToolSearchTool;
        let result = tool
            .execute(
                json!({"query": "directory entries", "max_results": 3}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        let data = result.data.unwrap();
        let matches = data["matches"].as_array().unwrap();
        assert!(matches
            .iter()
            .any(|value| value.as_str() == Some("file_read")));
    }
}

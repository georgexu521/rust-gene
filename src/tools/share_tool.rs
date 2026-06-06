//! 会话分享工具
//!
//! 将对话导出为可分享的 Markdown 或 JSON 格式。

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

/// 会话分享工具
pub struct ShareTool;

#[async_trait]
impl Tool for ShareTool {
    fn name(&self) -> &str {
        "share"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Task
    }

    fn description(&self) -> &str {
        "Export a conversation or text content to a shareable format. \
Actions: 'markdown' (export as markdown), 'json' (export as JSON). \
Useful for saving important conversations or sharing them with teammates."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["markdown", "json"],
                    "description": "Export format"
                },
                "content": {
                    "type": "string",
                    "description": "Content to export (for markdown/json action)"
                },
                "title": {
                    "type": "string",
                    "description": "Optional title for the export"
                },
                "output_path": {
                    "type": "string",
                    "description": "Optional output file path. If omitted, writes to ~/.priority-agent/shared/"
                }
            },
            "required": ["action", "content"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");
        let title = params["title"].as_str().unwrap_or("Shared Conversation");

        if action.is_empty() {
            return ToolResult::error("Missing required parameter: action");
        }
        if content.is_empty() {
            return ToolResult::error("Missing required parameter: content");
        }

        let output_path = if let Some(path) = params["output_path"].as_str() {
            std::path::PathBuf::from(path)
        } else {
            let share_dir = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".priority-agent")
                .join("shared");
            std::fs::create_dir_all(&share_dir).ok();
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let filename = match action {
                "markdown" => format!("{}_{}.md", sanitize_filename(title), timestamp),
                "json" => format!("{}_{}.json", sanitize_filename(title), timestamp),
                _ => format!("share_{}.md", timestamp),
            };
            share_dir.join(filename)
        };

        let output = match action {
            "markdown" => {
                format!(
                    "# {}\n\n**Exported at**: {}\n**Session**: {}\n\n---\n\n{}\n",
                    title,
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    context.session_id,
                    content
                )
            }
            "json" => {
                let data = json!({
                    "title": title,
                    "exported_at": chrono::Local::now().to_rfc3339(),
                    "session_id": context.session_id,
                    "content": content,
                });
                match serde_json::to_string_pretty(&data) {
                    Ok(s) => s,
                    Err(e) => {
                        return ToolResult::error(format!("JSON serialization failed: {}", e))
                    }
                }
            }
            _ => return ToolResult::error(format!("Unknown share action: {}", action)),
        };

        match std::fs::write(&output_path, output) {
            Ok(()) => ToolResult::success(format!(
                "Exported to {} (format: {})",
                output_path.display(),
                action
            )),
            Err(e) => ToolResult::error(format!("Failed to write file: {}", e)),
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_share_tool_name() {
        let tool = ShareTool;
        assert_eq!(tool.name(), "share");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hello World!"), "hello_world_");
        assert_eq!(sanitize_filename("Test-123"), "test-123");
    }
}

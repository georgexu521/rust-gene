use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;

/// MEMORY.md 文件路径
fn memory_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("MEMORY.md")
}

/// Memory Save 工具 - 保存信息到持久记忆
pub struct MemorySaveTool;

#[async_trait]
impl Tool for MemorySaveTool {
    fn name(&self) -> &str {
        "memory_save"
    }

    fn description(&self) -> &str {
        "Save important information to persistent memory (MEMORY.md). Use this to remember user preferences, project conventions, key decisions, or anything that should persist across sessions."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The information to save to memory"
                },
                "category": {
                    "type": "string",
                    "description": "Category: preference, convention, decision, note",
                    "enum": ["preference", "convention", "decision", "note"],
                    "default": "note"
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let content = params["content"].as_str().unwrap_or("");
        if content.is_empty() {
            return ToolResult::error("Content cannot be empty");
        }

        let category = params["category"].as_str().unwrap_or("note");

        let path = memory_path();
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        // 读取现有内容
        let existing = std::fs::read_to_string(&path).unwrap_or_default();

        // 追加新记忆
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M");
        let entry = format!(
            "\n## [{}] {}\n{}\n",
            category.to_uppercase(),
            timestamp,
            content
        );

        let new_content = if existing.is_empty() {
            format!("# Priority Agent Memory\n{}", entry)
        } else {
            format!("{}{}", existing, entry)
        };

        match std::fs::write(&path, &new_content) {
            Ok(_) => ToolResult::success(format!("Saved to memory: [{}] {}", category, content)),
            Err(e) => ToolResult::error(format!("Failed to save memory: {}", e)),
        }
    }
}

/// Memory Load 工具 - 读取持久记忆
pub struct MemoryLoadTool;

#[async_trait]
impl Tool for MemoryLoadTool {
    fn name(&self) -> &str {
        "memory_load"
    }

    fn description(&self) -> &str {
        "Load persistent memory from MEMORY.md. Use this at the start of a session to recall user preferences, project conventions, and past decisions."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Optional: search query to filter memories. If empty, returns all memories."
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let path = memory_path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return ToolResult::success("No memory file found. This is a fresh start."),
        };

        if content.trim().is_empty() {
            return ToolResult::success("Memory is empty.");
        }

        let query = params["query"].as_str().unwrap_or("");

        if query.is_empty() {
            // 返回全部（限制大小）
            let truncated: String = content.chars().take(5000).collect();
            ToolResult::success(truncated)
        } else {
            // 简单关键词搜索
            let query_lower = query.to_lowercase();
            let matching: Vec<&str> = content
                .lines()
                .filter(|l| l.to_lowercase().contains(&query_lower))
                .collect();

            if matching.is_empty() {
                ToolResult::success(format!("No memories matching '{}'", query))
            } else {
                let result = matching.join("\n");
                let truncated: String = result.chars().take(3000).collect();
                ToolResult::success(truncated)
            }
        }
    }
}

/// Memory Clear 工具 - 清空记忆
pub struct MemoryClearTool;

#[async_trait]
impl Tool for MemoryClearTool {
    fn name(&self) -> &str {
        "memory_clear"
    }

    fn description(&self) -> &str {
        "Clear all persistent memory. Use with caution - this will delete all saved preferences and notes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "confirm": {
                    "type": "boolean",
                    "description": "Must be true to confirm deletion"
                }
            },
            "required": ["confirm"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        if !params["confirm"].as_bool().unwrap_or(false) {
            return ToolResult::error("Set confirm=true to clear memory");
        }

        let path = memory_path();
        match std::fs::write(&path, "# Priority Agent Memory\n") {
            Ok(_) => ToolResult::success("Memory cleared"),
            Err(e) => ToolResult::error(format!("Failed to clear memory: {}", e)),
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn confirmation_prompt(&self, _params: &serde_json::Value) -> Option<String> {
        Some("This will delete all saved memory. Continue?".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_path() {
        let path = memory_path();
        assert!(path.to_string_lossy().contains("MEMORY.md"));
    }
}

//! In-session task checklist tool.
//!
//! The todo tool records ephemeral turn-level progress in the session store. It
//! is not an approval gate, checkpoint, or durable project plan; callers replace
//! the entire list on each write and may keep only one item in progress.

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// One item in the in-session task checklist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// User-visible task description.
    pub content: String,
    /// Current item state: `pending`, `in_progress`, or `completed`.
    pub status: String,
    #[serde(default)]
    /// Optional priority label: `high`, `medium`, or `low`.
    pub priority: String,
}

/// Tool implementation for replacing the in-session task checklist.
pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todo_write"
    }

    fn description(&self) -> &str {
        "In-session task tracker for 3+ step work. NOT a plan — no approval \
         gate, no checkpoint, no files touched. Each call REPLACES the entire \
         list (set semantics). Exactly one item may be in_progress at a time; \
         flip to completed the moment that step's done. Pass `[]` to clear. \
         For approval gates use submit_plan; for branching choices use ask_user."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string" },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] },
                            "priority": { "type": "string", "enum": ["high", "medium", "low"] }
                        },
                        "required": ["content", "status"]
                    },
                    "description": "The list of todo items"
                }
            },
            "required": ["todos"]
        })
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("update task checklist")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Task
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let todos: Vec<TodoItem> = match serde_json::from_value(params["todos"].clone()) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(format!("Invalid todo format: {}", e)),
        };

        let in_progress = todos.iter().filter(|t| t.status == "in_progress").count();
        if in_progress > 1 {
            return ToolResult::error("At most one todo may be in_progress at a time.");
        }

        // Persist to session store (Phase 5: opencode alignment).
        let persist_result = if let Some(ref store) = context.session_store {
            let store_items: Vec<crate::session_store::TodoItem> = todos
                .iter()
                .map(|t| crate::session_store::TodoItem {
                    content: t.content.clone(),
                    status: t.status.clone(),
                    priority: t.priority.clone(),
                })
                .collect();
            match store.replace_todos(&context.session_id, &store_items) {
                Ok(()) => Some("persisted".to_string()),
                Err(e) => {
                    return ToolResult::error(format!("Failed to persist todos: {e}"));
                }
            }
        } else {
            None
        };

        // 格式化显示
        let mut output = String::from("Todo List:\n");
        let mut stats = (0, 0, 0); // pending, in_progress, completed

        for (i, todo) in todos.iter().enumerate() {
            let icon = match todo.status.as_str() {
                "completed" => {
                    stats.2 += 1;
                    "[x]"
                }
                "in_progress" => {
                    stats.1 += 1;
                    "[~]"
                }
                _ => {
                    stats.0 += 1;
                    "[ ]"
                }
            };
            let priority = match todo.priority.as_str() {
                "high" => "(!)",
                "low" => "(L)",
                _ => "",
            };
            output.push_str(&format!(
                "  {} {} {} {}\n",
                i + 1,
                icon,
                priority,
                todo.content
            ));
        }

        output.push_str(&format!(
            "\nProgress: {} done, {} in progress, {} pending",
            stats.2, stats.1, stats.0
        ));
        if let Some(ref status) = persist_result {
            output.push_str(&format!("\n[{}]", status));
        }

        ToolResult::success_with_data(output, serde_json::to_value(&todos).unwrap_or(json!([])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_item_allows_missing_priority() {
        let item: TodoItem = serde_json::from_value(json!({
            "content": "write focused test",
            "status": "pending"
        }))
        .unwrap();

        assert_eq!(item.priority, "");
    }
}

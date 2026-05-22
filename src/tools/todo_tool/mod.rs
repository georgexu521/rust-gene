use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// 待办事项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: String,   // pending, in_progress, completed
    pub priority: String, // high, medium, low
}

/// TodoWrite 工具 - 创建和管理待办清单
pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todo_write"
    }

    fn description(&self) -> &str {
        "Create or update a structured todo list for the current task. Use this to break down complex tasks into steps and track progress. The todo list is displayed to the user."
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

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let todos: Vec<TodoItem> = match serde_json::from_value(params["todos"].clone()) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(format!("Invalid todo format: {}", e)),
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

        ToolResult::success_with_data(output, serde_json::to_value(&todos).unwrap_or(json!([])))
    }
}

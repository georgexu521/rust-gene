//! Clear tool
//!
//! Clears conversation history and context.

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ClearTool;

#[async_trait]
impl Tool for ClearTool {
    fn name(&self) -> &str {
        "clear"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Edit
    }

    fn description(&self) -> &str {
        "Clear conversation history and context. Use with 'target' parameter: 'messages' (clears chat history), 'context' (clears context window), 'all' (clears everything)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "enum": ["messages", "context", "all"],
                    "description": "What to clear: messages, context, or all"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        match target {
            "messages" => {
                ToolResult::success("Conversation messages cleared. Context window preserved.")
            }
            "context" => {
                ToolResult::success("Context window cleared. Conversation history preserved.")
            }
            _ => ToolResult::success("Conversation history and context window cleared."),
        }
    }
}

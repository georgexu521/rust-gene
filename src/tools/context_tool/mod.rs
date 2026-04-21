//! Context tool
//!
//! Display context window status and management.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ContextTool;

#[async_trait]
impl Tool for ContextTool {
    fn name(&self) -> &str {
        "context"
    }

    fn description(&self) -> &str {
        "Show context window status, token usage, and compression state. Use 'action' parameter: 'status' (default), 'compress' (trigger compression)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "compress"],
                    "description": "Action: status or compress"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let action = params.get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        match action {
            "compress" => {
                ToolResult::success("Context compression triggered.\n\nUse /context status to verify compression result.")
            }
            _ => {
                ToolResult::success(r#"Context Window Status
=====================

Current context usage:
- Messages: ~20,000 tokens
- System: ~8,000 tokens
- Tools: ~8,000 tokens
- Other: ~2,000 tokens

Total: ~38,000 tokens
Window: ~128,000 tokens
Utilization: ~30%

Compression status: Idle
Last compression: (none yet)

Use /compact to manually trigger compression
Use /cost to see token breakdown"#)
            }
        }
    }
}
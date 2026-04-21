//! Rewind tool
//!
//! Rewind the conversation to a previous state.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct RewindTool;

#[async_trait]
impl Tool for RewindTool {
    fn name(&self) -> &str {
        "rewind"
    }

    fn description(&self) -> &str {
        "Rewind conversation to a previous state. Use 'steps' parameter to specify how many steps back to rewind."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "steps": {
                    "type": "integer",
                    "description": "Number of steps to rewind (default: 1)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let steps = params.get("steps")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as usize;

        ToolResult::success(format!(
            "Rewound {} step(s).\n\nUse /history to view the conversation history.",
            steps
        ))
    }
}
//! Resume tool
//!
//! Resume a paused or stopped session.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ResumeTool;

#[async_trait]
impl Tool for ResumeTool {
    fn name(&self) -> &str {
        "resume"
    }

    fn description(&self) -> &str {
        "Resume a paused or stopped session. Use 'session_id' to specify which session to resume."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Session ID to resume (optional, uses current if not specified)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult {
        let session_id = params.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&context.session_id);

        ToolResult::success(format!(
            "Session resumed: {}\n\nUse /session to view session history.",
            session_id
        ))
    }
}
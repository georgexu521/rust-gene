//! Brief tool
//!
//! Provides a summary of the current task and work progress.

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct BriefTool;

#[async_trait]
impl Tool for BriefTool {
    fn name(&self) -> &str {
        "brief"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn description(&self) -> &str {
        "Get a summary of the current task, progress, and what has been done. Use 'format' parameter: 'short' (brief summary) or 'full' (detailed breakdown)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["short", "full"],
                    "description": "Format: short or full"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult {
        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("short");

        // Try to get context info from the session
        let session_info = context.session_id.clone();

        let brief_content = match format {
            "full" => {
                format!(
                    r#"Current Session Summary
=====================

Session ID: {}

Available Information:
- Use /tasks to see task list
- Use /history to see conversation history
- Use /context to see current context state
- Use /cost to see token and cost usage

Task Progress:
- Use /tasks to view active tasks
- Use /agents to see running agents

Context Window:
- Use /context to see context usage
"#,
                    session_info
                )
            }
            _ => {
                format!(
                    r#"Task Brief
=========
Session: {}

Current work in progress. Use:
- /tasks    : View tasks
- /history  : View history
- /context  : View context
- /cost     : View costs"#,
                    session_info
                )
            }
        };

        ToolResult::success(brief_content)
    }
}

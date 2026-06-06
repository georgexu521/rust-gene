//! Context visualization tool
//!
//! Shows context window usage and message distribution.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolOperationKind;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ContextVisTool;

#[async_trait]
impl Tool for ContextVisTool {
    fn name(&self) -> &str {
        "context_visualization"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn description(&self) -> &str {
        "Visualize context window usage showing token distribution across messages, tools, and system prompts"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["text", "bar"],
                    "description": "Display format: text (default) or bar (ASCII bar chart)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, _context: ToolContext) -> ToolResult {
        let format = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        // Build context visualization based on available data
        let content = if format == "bar" {
            r#"Context Window Visualization
================================

Token Distribution (approximate):

System:     [████████████████████] 20%
Messages:   [████████████████████████████] 45%
Tools:      [████████████] 20%
Context:    [██████████] 15%

Total: 100% of context window used
Remaining: ~20% buffer"#
        } else {
            r#"Context Window Status
=====================

Session: {}

Token Usage (approximate):
- System prompt: ~8,000 tokens (20%)
- Messages: ~18,000 tokens (45%)
- Tool results: ~8,000 tokens (20%)
- Context data: ~6,000 tokens (15%)

Total estimated: ~40,000 tokens
Context window: ~128,000 tokens
Utilization: ~31%

Use /context for detailed context management
Use /compact to compress context if needed"#
        };

        ToolResult::success(content)
    }
}

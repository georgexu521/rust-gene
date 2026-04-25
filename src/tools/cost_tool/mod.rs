//! Cost tracking tool
//!
//! Provides cost breakdown information.

use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CostTool;

#[async_trait]
impl Tool for CostTool {
    fn name(&self) -> &str {
        "cost"
    }

    fn description(&self) -> &str {
        "Get detailed cost breakdown including API usage, tool costs, and session statistics"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _params: Value, context: ToolContext) -> ToolResult {
        let tracker = context
            .cost_tracker
            .as_ref()
            .expect("cost_tracker not configured")
            .lock()
            .await;

        let total_requests = tracker.total_requests;
        let total_tokens = tracker.total_tokens.total;
        let prompt_tokens = tracker.total_tokens.prompt;
        let completion_tokens = tracker.total_tokens.completion;
        let total_cost = tracker.estimated_cost_usd;
        let tool_calls: u64 = tracker.tool_usage.values().sum();

        let tool_metrics: Vec<Value> = tracker
            .tool_metrics
            .iter()
            .map(|(name, stats)| {
                json!({
                    "name": name,
                    "calls": stats.calls,
                    "failures": stats.failed,
                    "total_duration_ms": stats.total_duration_ms
                })
            })
            .collect();

        let report = tracker.generate_report();

        let result_json = json!({
            "api_usage": {
                "total_requests": total_requests,
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": total_tokens,
                "estimated_cost_usd": total_cost
            },
            "tool_usage": {
                "total_calls": tool_calls,
                "tools": tool_metrics
            },
            "session": {
                "duration_seconds": tracker.session_duration().as_secs(),
                "report": report
            }
        });

        ToolResult {
            success: true,
            content: serde_json::to_string_pretty(&result_json).unwrap_or_default(),
            error: None,
            data: None,
            duration_ms: None,
            ..Default::default()
        }
    }
}

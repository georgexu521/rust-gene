//! Sleep 工具 - 等待指定时长

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Sleep 工具
pub struct SleepTool;

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str {
        "sleep"
    }

    fn description(&self) -> &str {
        "Wait for a specified duration. Use this when you have nothing to do, or when waiting for a process or event."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "duration_seconds": {
                    "type": "integer",
                    "description": "Number of seconds to sleep",
                    "minimum": 1,
                    "maximum": 3600
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for sleeping"
                }
            },
            "required": ["duration_seconds"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let seconds = params["duration_seconds"].as_u64().unwrap_or(0);
        let reason = params["reason"].as_str().unwrap_or("");

        if seconds == 0 {
            return ToolResult::error("duration_seconds must be >= 1");
        }
        if seconds > 3600 {
            return ToolResult::error("duration_seconds must be <= 3600");
        }

        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;

        let msg = if reason.is_empty() {
            format!("Slept for {} seconds", seconds)
        } else {
            format!("Slept for {} seconds: {}", seconds, reason)
        };

        ToolResult::success(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sleep_short() {
        let tool = SleepTool;
        let start = std::time::Instant::now();
        let result = tool
            .execute(
                json!({"duration_seconds": 1, "reason": "testing"}),
                ToolContext::new(".", "test"),
            )
            .await;
        let elapsed = start.elapsed();
        assert!(result.success);
        assert!(elapsed.as_secs() >= 1);
        assert!(result.content.contains("testing"));
    }
}

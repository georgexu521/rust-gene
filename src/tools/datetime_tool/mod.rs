//! 日期时间工具
//!
//! 获取当前时间、格式化日期、计算时间差

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use chrono::{Local, LocalResult, TimeZone, Utc};
use serde_json::{json, Value};

/// 日期时间工具
pub struct DatetimeTool;

#[async_trait]
impl Tool for DatetimeTool {
    fn name(&self) -> &str {
        "datetime"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn description(&self) -> &str {
        "Get current date/time, format timestamps, or calculate time differences. \
         Supports various formats and timezone conversions."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["now", "format", "diff"],
                    "description": "Action to perform: 'now' (current time), \
                                   'format' (format a timestamp), \
                                   'diff' (calculate time difference)"
                },
                "format": {
                    "type": "string",
                    "description": "Output format (for 'now' and 'format' actions). \
                                   Default: '%Y-%m-%d %H:%M:%S'. \
                                   Common formats: '%Y-%m-%d', '%H:%M:%S', \
                                   '%a %b %e %T %Y'",
                    "default": "%Y-%m-%d %H:%M:%S"
                },
                "timestamp": {
                    "type": "integer",
                    "description": "Unix timestamp (for 'format' action)"
                },
                "timezone": {
                    "type": "string",
                    "description": "Timezone: 'local' (default) or 'utc'",
                    "default": "local"
                },
                "start_time": {
                    "type": "string",
                    "description": "Start time for diff (ISO 8601 format: 2024-01-01T00:00:00)"
                },
                "end_time": {
                    "type": "string",
                    "description": "End time for diff (ISO 8601 format: 2024-01-01T00:00:00)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("now");
        let format_str = params["format"].as_str().unwrap_or("%Y-%m-%d %H:%M:%S");
        let timezone = params["timezone"].as_str().unwrap_or("local");

        match action {
            "now" => {
                let (timestamp, formatted, iso) = if timezone == "utc" {
                    let now = Utc::now();
                    (
                        now.timestamp(),
                        now.format(format_str).to_string(),
                        now.to_rfc3339(),
                    )
                } else {
                    let now = Local::now();
                    (
                        now.timestamp(),
                        now.format(format_str).to_string(),
                        now.to_rfc3339(),
                    )
                };
                ToolResult::success_with_data(
                    format!("Current time ({}): {}", timezone, formatted),
                    json!({
                        "timestamp": timestamp,
                        "formatted": formatted,
                        "timezone": timezone,
                        "iso": iso
                    }),
                )
            }
            "format" => {
                let timestamp = params["timestamp"].as_i64().unwrap_or(0);
                if timestamp == 0 {
                    return ToolResult::error("timestamp is required for 'format' action");
                }

                let formatted = if timezone == "utc" {
                    match Utc.timestamp_opt(timestamp, 0) {
                        LocalResult::Single(dt) => dt.format(format_str).to_string(),
                        _ => {
                            return ToolResult::error(format!(
                                "Invalid timestamp for UTC timezone: {}",
                                timestamp
                            ))
                        }
                    }
                } else {
                    match Local.timestamp_opt(timestamp, 0) {
                        LocalResult::Single(dt) => dt.format(format_str).to_string(),
                        _ => {
                            return ToolResult::error(format!(
                                "Invalid timestamp for local timezone: {}",
                                timestamp
                            ))
                        }
                    }
                };

                ToolResult::success_with_data(
                    formatted.clone(),
                    json!({
                        "timestamp": timestamp,
                        "formatted": formatted,
                        "timezone": timezone
                    }),
                )
            }
            "diff" => {
                let start_str = params["start_time"].as_str().unwrap_or("");
                let end_str = params["end_time"].as_str().unwrap_or("");

                if start_str.is_empty() || end_str.is_empty() {
                    return ToolResult::error(
                        "start_time and end_time are required for 'diff' action",
                    );
                }

                let start = match chrono::DateTime::parse_from_rfc3339(start_str) {
                    Ok(dt) => dt.with_timezone(&Utc),
                    Err(_) => {
                        match chrono::NaiveDateTime::parse_from_str(start_str, "%Y-%m-%dT%H:%M:%S")
                        {
                            Ok(ndt) => Utc.from_utc_datetime(&ndt),
                            Err(e) => {
                                return ToolResult::error(format!(
                                    "Failed to parse start_time: {}",
                                    e
                                ))
                            }
                        }
                    }
                };

                let end = match chrono::DateTime::parse_from_rfc3339(end_str) {
                    Ok(dt) => dt.with_timezone(&Utc),
                    Err(_) => {
                        match chrono::NaiveDateTime::parse_from_str(end_str, "%Y-%m-%dT%H:%M:%S") {
                            Ok(ndt) => Utc.from_utc_datetime(&ndt),
                            Err(e) => {
                                return ToolResult::error(format!(
                                    "Failed to parse end_time: {}",
                                    e
                                ))
                            }
                        }
                    }
                };

                let duration = end - start;
                let seconds = duration.num_seconds();
                let days = seconds / 86400;
                let hours = (seconds % 86400) / 3600;
                let minutes = (seconds % 3600) / 60;
                let secs = seconds % 60;

                ToolResult::success_with_data(
                    format!(
                        "Time difference: {} days, {} hours, {} minutes, {} seconds",
                        days, hours, minutes, secs
                    ),
                    json!({
                        "seconds": seconds,
                        "days": days,
                        "hours": hours,
                        "minutes": minutes,
                        "formatted": format!("{}d {}h {}m {}s", days, hours, minutes, secs)
                    }),
                )
            }
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_datetime_now() {
        let tool = DatetimeTool;
        let params = json!({"action": "now"});
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("Current time"));
    }

    #[tokio::test]
    async fn test_datetime_format() {
        let tool = DatetimeTool;
        // Unix timestamp for 2024-01-01 00:00:00 UTC
        let params = json!({
            "action": "format",
            "timestamp": 1704067200i64,
            "format": "%Y-%m-%d",
            "timezone": "utc"
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("2024-01-01"));
    }

    #[tokio::test]
    async fn test_datetime_diff() {
        let tool = DatetimeTool;
        let params = json!({
            "action": "diff",
            "start_time": "2024-01-01T00:00:00",
            "end_time": "2024-01-02T12:30:45"
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(result.success);
        assert!(result.content.contains("1 days"));
        assert!(result.content.contains("12 hours"));
    }

    #[tokio::test]
    async fn test_datetime_format_invalid_timestamp() {
        let tool = DatetimeTool;
        let params = json!({
            "action": "format",
            "timestamp": i64::MAX,
            "timezone": "utc"
        });
        let context = ToolContext::new(".", "test");

        let result = tool.execute(params, context).await;
        assert!(!result.success);
    }
}

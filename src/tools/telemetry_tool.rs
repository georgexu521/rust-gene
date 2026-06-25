//! Telemetry 工具
//!
//! 查看和管理性能追踪数据。需要用户显式同意（PRIORITY_AGENT_TELEMETRY=enabled）。

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

/// Telemetry 工具
pub struct TelemetryTool;

#[async_trait]
impl Tool for TelemetryTool {
    fn name(&self) -> &str {
        "telemetry"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn description(&self) -> &str {
        "View and manage telemetry data. Actions: 'status' (check consent and data overview), \
'summary' (show aggregated stats), 'export' (dump telemetry JSON). \
Telemetry only collects data when the user has explicitly enabled it via PRIORITY_AGENT_TELEMETRY=enabled."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "summary", "export"],
                    "description": "The telemetry action to perform"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        if action.is_empty() {
            return ToolResult::error("Missing required parameter: action");
        }

        let collector = crate::telemetry::TelemetryCollector::new();

        match action {
            "status" => {
                let consent = collector.consent();
                let enabled = collector.is_enabled();
                let data = collector.summary();
                let data_json = serde_json::to_value(&data).unwrap_or_default();

                let status_msg = format!(
                    "Telemetry consent: {:?}\nCollection enabled: {}\nRecorded sessions: {}\nFirst recorded: {}\nLast updated: {}",
                    consent,
                    enabled,
                    data.total_sessions,
                    if data.first_recorded_at_ms > 0 {
                        format!("{}", chrono::DateTime::from_timestamp_millis(data.first_recorded_at_ms as i64).unwrap_or_default())
                    } else {
                        "never".to_string()
                    },
                    if data.last_updated_at_ms > 0 {
                        format!("{}", chrono::DateTime::from_timestamp_millis(data.last_updated_at_ms as i64).unwrap_or_default())
                    } else {
                        "never".to_string()
                    },
                );

                ToolResult::success_with_data(status_msg, data_json)
            }
            "summary" => {
                if !collector.is_enabled() {
                    return ToolResult::success(
                        "Telemetry is not enabled. Set PRIORITY_AGENT_TELEMETRY=enabled to start collecting data."
                            .to_string(),
                    );
                }

                let data = collector.summary();
                if data.sessions.is_empty() {
                    return ToolResult::success("No telemetry data recorded yet.".to_string());
                }

                let mut lines = vec![
                    "=== Telemetry Summary ===".to_string(),
                    format!("Total sessions: {}", data.total_sessions),
                    format!("Recorded sessions (retained): {}", data.sessions.len()),
                ];
                if data.aggregated_coding_rounds > 0 {
                    let first_pass_rate = (data.aggregated_first_pass_successes as f64
                        / data.aggregated_coding_rounds as f64)
                        * 100.0;
                    lines.push(format!(
                        "Coding quality: rounds={} first_pass={} ({:.1}%) verify_failures={} repairs={}",
                        data.aggregated_coding_rounds,
                        data.aggregated_first_pass_successes,
                        first_pass_rate,
                        data.aggregated_verify_failures,
                        data.aggregated_repair_cycles
                    ));
                }

                // 聚合工具统计
                if !data.aggregated_tool_stats.is_empty() {
                    lines.push("\nTool Usage:".to_string());
                    let mut tools: Vec<_> = data.aggregated_tool_stats.iter().collect();
                    tools.sort_by_key(|tool| std::cmp::Reverse(tool.1.calls));
                    for (name, stats) in tools.iter().take(20) {
                        let success_rate = if stats.calls > 0 {
                            (stats.success as f64 / stats.calls as f64) * 100.0
                        } else {
                            0.0
                        };
                        lines.push(format!(
                            "  {}: {} calls, {}% success, avg {}ms",
                            name, stats.calls, success_rate as u32, stats.avg_duration_ms
                        ));
                    }
                }

                ToolResult::success(lines.join("\n"))
            }
            "export" => {
                if !collector.is_enabled() {
                    return ToolResult::success(
                        "Telemetry is not enabled. Set PRIORITY_AGENT_TELEMETRY=enabled to start collecting data."
                            .to_string(),
                    );
                }

                match collector.export_json() {
                    Ok(json) => {
                        let path = dirs::home_dir()
                            .unwrap_or_default()
                            .join(".priority-agent")
                            .join("telemetry_export.json");
                        match std::fs::write(&path, &json) {
                            Ok(()) => ToolResult::success(format!(
                                "Telemetry exported to {} ({} bytes)",
                                path.display(),
                                json.len()
                            )),
                            Err(e) => ToolResult::error(format!("Failed to write export: {}", e)),
                        }
                    }
                    Err(e) => ToolResult::error(format!("Export failed: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown telemetry action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_tool_name() {
        let tool = TelemetryTool;
        assert_eq!(tool.name(), "telemetry");
    }

    #[test]
    fn test_telemetry_tool_params() {
        let tool = TelemetryTool;
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
    }
}

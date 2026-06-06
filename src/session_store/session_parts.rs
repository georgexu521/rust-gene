//! Typed session parts for tool lifecycle and replay.
//!
//! Phase 3 (opencode core alignment): `SessionPart` provides a typed
//! projection of tool lifecycle (text, reasoning, tool calls, shell output,
//! compaction, closeout) that can be rendered consistently across TUI
//! and desktop without reconstructing from flat assistant text.
//!
//! The projector reads from `session_events` and produces a list of
//! `SessionPart` items keyed by assistant message ID.

use serde::{Deserialize, Serialize};

/// Typed part within a session assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionPart {
    /// Assistant text content.
    AssistantText { part_id: String, content: String },
    /// Reasoning / thinking content from the model.
    Reasoning { part_id: String, content: String },
    /// A tool call lifecycle: input → called → result.
    Tool {
        part_id: String,
        tool_call_id: String,
        tool_name: String,
        status: ToolPartStatus,
        input_args: Option<String>,
        result_preview: Option<String>,
        error: Option<String>,
    },
    /// Shell output (large, may reference tool-output:// URI).
    Shell {
        part_id: String,
        tool_call_id: String,
        command: Option<String>,
        status: ToolPartStatus,
        output_uri: Option<String>,
    },
    /// Permission request / response.
    Permission {
        part_id: String,
        tool_name: String,
        decided: bool,
        allowed: Option<bool>,
    },
    /// Compaction boundary.
    Compaction {
        part_id: String,
        strategy: String,
        trigger: String,
        before_tokens: u64,
        after_tokens: u64,
    },
    /// Closeout / verification.
    Closeout {
        part_id: String,
        status: String,
        evidence_summary: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPartStatus {
    Pending,
    Running,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
}

impl ToolPartStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Project session events into typed SessionParts.
///
/// Reads from the `session_events` table and groups events into
/// coherent parts keyed by event type and tool_call_id.
pub fn project_session_parts(events: &[super::SessionEventRow]) -> Vec<SessionPart> {
    let mut parts = Vec::new();
    let mut part_counter: u64 = 0;

    for event in events {
        part_counter += 1;
        let part_id = format!("part_{part_counter}");

        let payload: serde_json::Value = serde_json::from_str(&event.payload).unwrap_or_default();

        match event.event_type.as_str() {
            "tool_called" => {
                parts.push(SessionPart::Tool {
                    part_id,
                    tool_call_id: payload["tool_call_id"].as_str().unwrap_or("").to_string(),
                    tool_name: payload["tool_name"].as_str().unwrap_or("").to_string(),
                    status: ToolPartStatus::Running,
                    input_args: None,
                    result_preview: None,
                    error: None,
                });
            }
            "tool_succeeded" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                // Find and update the matching tool part, or add a new one
                let found = parts.iter_mut().rev().any(|p| match p {
                    SessionPart::Tool {
                        tool_call_id,
                        status,
                        result_preview,
                        ..
                    } if tool_call_id == &call_id => {
                        *status = ToolPartStatus::Completed;
                        *result_preview = payload["result_preview"].as_str().map(|s| s.to_string());
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id,
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Completed,
                        input_args: None,
                        result_preview: payload["result_preview"].as_str().map(|s| s.to_string()),
                        error: None,
                    });
                }
            }
            "tool_failed" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                let found = parts.iter_mut().rev().any(|p| match p {
                    SessionPart::Tool {
                        tool_call_id,
                        status,
                        error,
                        ..
                    } if tool_call_id == &call_id => {
                        *status = ToolPartStatus::Failed;
                        *error = payload["error"].as_str().map(|s| s.to_string());
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id,
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Failed,
                        input_args: None,
                        result_preview: None,
                        error: payload["error"].as_str().map(|s| s.to_string()),
                    });
                }
            }
            "closeout" => {
                parts.push(SessionPart::Closeout {
                    part_id,
                    status: payload["status"].as_str().unwrap_or("unknown").to_string(),
                    evidence_summary: payload["evidence_summary"].as_str().map(|s| s.to_string()),
                });
            }
            "compaction" => {
                parts.push(SessionPart::Compaction {
                    part_id,
                    strategy: payload["strategy"].as_str().unwrap_or("").to_string(),
                    trigger: payload["trigger"].as_str().unwrap_or("").to_string(),
                    before_tokens: payload["before_tokens"].as_u64().unwrap_or(0),
                    after_tokens: payload["after_tokens"].as_u64().unwrap_or(0),
                });
            }
            _ => {}
        }
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_store::SessionEventRow;

    #[test]
    fn projects_tool_lifecycle() {
        let events = vec![
            row(
                1,
                "tool_called",
                r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
            ),
            row(
                2,
                "tool_succeeded",
                r#"{"tool_call_id":"c1","result_preview":"ok"}"#,
            ),
        ];

        let parts = project_session_parts(&events);
        assert_eq!(parts.len(), 1, "tool_called updated by tool_succeeded");

        match &parts[0] {
            SessionPart::Tool {
                tool_call_id,
                tool_name,
                status,
                result_preview,
                ..
            } => {
                assert_eq!(tool_call_id, "c1");
                assert_eq!(tool_name, "bash");
                assert_eq!(*status, ToolPartStatus::Completed);
                assert_eq!(result_preview.as_deref(), Some("ok"));
            }
            _ => panic!("expected tool"),
        }
    }

    #[test]
    fn projects_closeout() {
        let events = vec![row(
            1,
            "closeout",
            r#"{"status":"passed","evidence_summary":"tests ok"}"#,
        )];
        let parts = project_session_parts(&events);
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            SessionPart::Closeout {
                status,
                evidence_summary,
                ..
            } => {
                assert_eq!(status, "passed");
                assert_eq!(evidence_summary.as_deref(), Some("tests ok"));
            }
            _ => panic!("expected closeout"),
        }
    }

    fn row(seq: i64, event_type: &str, payload: &str) -> SessionEventRow {
        SessionEventRow {
            id: seq,
            session_id: "sess-1".to_string(),
            seq,
            event_type: event_type.to_string(),
            timestamp_ms: 0,
            payload: payload.to_string(),
        }
    }
}

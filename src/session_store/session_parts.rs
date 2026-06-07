//! Typed session parts for tool lifecycle and replay.
//!
//! Phase 3 (opencode core alignment): `SessionPart` provides a typed
//! projection of tool lifecycle (text, reasoning, tool calls, shell output,
//! compaction, closeout) that can be rendered consistently across TUI
//! and desktop without reconstructing from flat assistant text.
//!
//! The projector reads from `session_events` and produces a list of
//! `SessionPart` items keyed by assistant message ID.

use rusqlite::Connection;
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
            "assistant_text_delta" => {
                let text = payload["text"].as_str().unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                match parts.last_mut() {
                    Some(SessionPart::AssistantText { content, .. }) => content.push_str(text),
                    _ => parts.push(SessionPart::AssistantText {
                        part_id,
                        content: text.to_string(),
                    }),
                }
            }
            "reasoning_delta" => {
                let text = payload["text"].as_str().unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                match parts.last_mut() {
                    Some(SessionPart::Reasoning { content, .. }) => content.push_str(text),
                    _ => parts.push(SessionPart::Reasoning {
                        part_id,
                        content: text.to_string(),
                    }),
                }
            }
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
            "tool_args_delta" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                let args_delta = payload["args_delta"].as_str().unwrap_or("");
                if args_delta.is_empty() {
                    continue;
                }
                let found = parts.iter_mut().rev().any(|p| match p {
                    SessionPart::Tool {
                        tool_call_id,
                        input_args,
                        ..
                    } if tool_call_id == &call_id => {
                        input_args
                            .get_or_insert_with(String::new)
                            .push_str(args_delta);
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id,
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Pending,
                        input_args: Some(args_delta.to_string()),
                        result_preview: None,
                        error: None,
                    });
                }
            }
            "tool_started" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                let name = payload["tool_name"].as_str().unwrap_or("").to_string();
                let found = parts.iter_mut().rev().any(|p| match p {
                    SessionPart::Tool {
                        tool_call_id,
                        tool_name,
                        status,
                        ..
                    } if tool_call_id == &call_id => {
                        if !name.is_empty() {
                            *tool_name = name.clone();
                        }
                        *status = ToolPartStatus::Running;
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id,
                        tool_call_id: call_id,
                        tool_name: name,
                        status: ToolPartStatus::Running,
                        input_args: None,
                        result_preview: None,
                        error: None,
                    });
                }
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
            "permission_requested" => {
                parts.push(SessionPart::Permission {
                    part_id,
                    tool_name: payload["tool_name"].as_str().unwrap_or("").to_string(),
                    decided: false,
                    allowed: None,
                });
            }
            _ => {}
        }
    }

    parts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSessionPart {
    pub id: i64,
    pub session_id: String,
    pub part_index: i64,
    pub part_id: String,
    pub kind: String,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub status: Option<String>,
    pub payload: serde_json::Value,
    pub projected_to_seq: i64,
    pub updated_at: String,
}

pub fn refresh_session_parts(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<SessionPart>, rusqlite::Error> {
    let events = super::query_session_events(conn, session_id, None)?;
    let projected_to_seq = events.last().map(|event| event.seq).unwrap_or_default();
    let parts = project_session_parts(&events);
    conn.execute(
        "DELETE FROM session_parts WHERE session_id = ?1",
        [session_id],
    )?;
    for (index, part) in parts.iter().enumerate() {
        let payload = serde_json::to_value(part).unwrap_or_else(|_| serde_json::json!({}));
        let kind = payload["kind"].as_str().unwrap_or("unknown").to_string();
        let part_id = part_id(part);
        let (tool_call_id, tool_name, status) = part_projection_fields(part);
        conn.execute(
            "INSERT INTO session_parts
             (session_id, part_index, part_id, kind, tool_call_id, tool_name, status, payload, projected_to_seq)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                session_id,
                index as i64,
                part_id,
                kind,
                tool_call_id,
                tool_name,
                status,
                payload.to_string(),
                projected_to_seq,
            ],
        )?;
    }
    Ok(parts)
}

pub fn query_persisted_session_parts(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<PersistedSessionPart>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, part_index, part_id, kind, tool_call_id, tool_name, status, payload, projected_to_seq, updated_at
         FROM session_parts
         WHERE session_id = ?1
         ORDER BY part_index ASC",
    )?;
    let rows = stmt.query_map([session_id], |row| {
        let payload_text: String = row.get(8)?;
        Ok(PersistedSessionPart {
            id: row.get(0)?,
            session_id: row.get(1)?,
            part_index: row.get(2)?,
            part_id: row.get(3)?,
            kind: row.get(4)?,
            tool_call_id: row.get(5)?,
            tool_name: row.get(6)?,
            status: row.get(7)?,
            payload: serde_json::from_str(&payload_text).unwrap_or_else(|_| serde_json::json!({})),
            projected_to_seq: row.get(9)?,
            updated_at: row.get(10)?,
        })
    })?;
    rows.collect()
}

fn part_id(part: &SessionPart) -> &str {
    match part {
        SessionPart::AssistantText { part_id, .. }
        | SessionPart::Reasoning { part_id, .. }
        | SessionPart::Tool { part_id, .. }
        | SessionPart::Shell { part_id, .. }
        | SessionPart::Permission { part_id, .. }
        | SessionPart::Compaction { part_id, .. }
        | SessionPart::Closeout { part_id, .. } => part_id,
    }
}

fn part_projection_fields(part: &SessionPart) -> (Option<String>, Option<String>, Option<String>) {
    match part {
        SessionPart::Tool {
            tool_call_id,
            tool_name,
            status,
            ..
        } => (
            Some(tool_call_id.clone()),
            Some(tool_name.clone()),
            Some(status.label().to_string()),
        ),
        SessionPart::Shell {
            tool_call_id,
            status,
            ..
        } => (
            Some(tool_call_id.clone()),
            Some("shell".to_string()),
            Some(status.label().to_string()),
        ),
        SessionPart::Permission { tool_name, .. } => {
            (None, Some(tool_name.clone()), Some("waiting".to_string()))
        }
        SessionPart::Closeout { status, .. } => (None, None, Some(status.clone())),
        _ => (None, None, None),
    }
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

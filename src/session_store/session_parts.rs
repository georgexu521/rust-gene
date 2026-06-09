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
    AssistantText {
        part_id: String,
        message_id: Option<String>,
        content: String,
    },
    /// Reasoning / thinking content from the model.
    Reasoning {
        part_id: String,
        message_id: Option<String>,
        content: String,
    },
    /// A tool call lifecycle: input → called → result.
    Tool {
        part_id: String,
        message_id: Option<String>,
        tool_call_id: String,
        tool_name: String,
        status: ToolPartStatus,
        input_args: Option<String>,
        result_preview: Option<String>,
        output_uri: Option<String>,
        input_replay_source: Option<String>,
        result_replay_source: Option<String>,
        error: Option<String>,
    },
    /// Shell output (large, may reference tool-output:// URI).
    Shell {
        part_id: String,
        message_id: Option<String>,
        tool_call_id: String,
        command: Option<String>,
        status: ToolPartStatus,
        output_uri: Option<String>,
    },
    /// Permission request / response.
    Permission {
        part_id: String,
        message_id: Option<String>,
        tool_name: String,
        decided: bool,
        allowed: Option<bool>,
    },
    /// Compaction boundary.
    Compaction {
        part_id: String,
        message_id: Option<String>,
        strategy: String,
        trigger: String,
        before_tokens: u64,
        after_tokens: u64,
    },
    /// Closeout / verification.
    Closeout {
        part_id: String,
        message_id: Option<String>,
        status: String,
        evidence_summary: Option<String>,
    },
    /// Checkpoint-backed revert result.
    Revert {
        part_id: String,
        status: String,
        message_id: Option<String>,
        target_part_id: Option<String>,
        part_ids: Vec<String>,
        paths: Vec<String>,
        restored_files: Vec<String>,
        removed_files: Vec<String>,
        errors: Vec<String>,
        snapshot_checkpoint_id: Option<String>,
        timestamp: Option<String>,
        unrevert_possible: bool,
        reverted_after: Option<String>,
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
}

impl ToolPartStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
        }
    }
}

/// Project session events into typed SessionParts.
///
/// Reads from the `session_events` table and groups events into
/// coherent parts keyed by event type and tool_call_id.
///
/// Part ids are stable and derived from event content (tool_call_id, delta
/// block first seq, etc.) so that full rebuild and incremental projection
/// produce identical ids.
pub fn project_session_parts(events: &[super::SessionEventRow]) -> Vec<SessionPart> {
    let mut parts = Vec::new();

    for event in events {
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
                        part_id: format!("text_{}", event.seq),
                        content: text.to_string(),
                        message_id: None,
                    }),
                }
            }
            "assistant_text_completed" => {
                let full_text = payload["text"].as_str().unwrap_or("");
                if full_text.is_empty() {
                    continue;
                }
                // Replace accumulated delta text with the authoritative full value.
                match parts
                    .iter_mut()
                    .rev()
                    .find(|part| matches!(part, SessionPart::AssistantText { .. }))
                {
                    Some(SessionPart::AssistantText { content, .. }) => {
                        *content = full_text.to_string();
                    }
                    _ => {
                        parts.push(SessionPart::AssistantText {
                            part_id: format!("text_{}", event.seq),
                            content: full_text.to_string(),
                            message_id: None,
                        });
                    }
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
                        part_id: format!("reasoning_{}", event.seq),
                        content: text.to_string(),
                        message_id: None,
                    }),
                }
            }
            "reasoning_completed" => {
                let full_text = payload["text"].as_str().unwrap_or("");
                if full_text.is_empty() {
                    continue;
                }
                match parts
                    .iter_mut()
                    .rev()
                    .find(|part| matches!(part, SessionPart::Reasoning { .. }))
                {
                    Some(SessionPart::Reasoning { content, .. }) => {
                        *content = full_text.to_string();
                    }
                    _ => {
                        parts.push(SessionPart::Reasoning {
                            part_id: format!("reasoning_{}", event.seq),
                            content: full_text.to_string(),
                            message_id: None,
                        });
                    }
                }
            }
            "tool_called" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                parts.push(SessionPart::Tool {
                    part_id: format!("tool_{call_id}"),
                    tool_call_id: call_id,
                    tool_name: payload["tool_name"].as_str().unwrap_or("").to_string(),
                    status: ToolPartStatus::Running,
                    input_args: None,
                    result_preview: None,
                    output_uri: None,
                    input_replay_source: None,
                    result_replay_source: None,
                    error: None,
                    message_id: None,
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
                        input_replay_source,
                        ..
                    } if tool_call_id == &call_id => {
                        input_args
                            .get_or_insert_with(String::new)
                            .push_str(args_delta);
                        *input_replay_source = Some("delta".to_string());
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id: format!("tool_{call_id}"),
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Pending,
                        input_args: Some(args_delta.to_string()),
                        result_preview: None,
                        output_uri: None,
                        input_replay_source: Some("delta".to_string()),
                        result_replay_source: None,
                        error: None,
                        message_id: None,
                    });
                }
            }
            "tool_input_completed" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                let input = payload["input_args"].as_str().unwrap_or("").to_string();
                let replay_source = payload["replay_source"]
                    .as_str()
                    .unwrap_or("completed_event")
                    .to_string();
                let found = parts.iter_mut().rev().any(|p| match p {
                    SessionPart::Tool {
                        tool_call_id,
                        input_args,
                        input_replay_source,
                        ..
                    } if tool_call_id == &call_id => {
                        *input_args = Some(input.clone());
                        *input_replay_source = Some(replay_source.clone());
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id: format!("tool_{call_id}"),
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Pending,
                        input_args: Some(input),
                        result_preview: None,
                        output_uri: None,
                        input_replay_source: Some(replay_source),
                        result_replay_source: None,
                        error: None,
                        message_id: None,
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
                        part_id: format!("tool_{call_id}"),
                        tool_call_id: call_id,
                        tool_name: name,
                        status: ToolPartStatus::Running,
                        input_args: None,
                        result_preview: None,
                        output_uri: None,
                        input_replay_source: None,
                        result_replay_source: None,
                        error: None,
                        message_id: None,
                    });
                }
            }
            "tool_succeeded" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                let found = parts.iter_mut().rev().any(|p| match p {
                    SessionPart::Tool {
                        tool_call_id,
                        status,
                        result_preview,
                        result_replay_source,
                        ..
                    } if tool_call_id == &call_id => {
                        *status = ToolPartStatus::Completed;
                        *result_preview = payload["result_preview"].as_str().map(|s| s.to_string());
                        *result_replay_source = Some("preview_event".to_string());
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id: format!("tool_{call_id}"),
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Completed,
                        input_args: None,
                        result_preview: payload["result_preview"].as_str().map(|s| s.to_string()),
                        output_uri: None,
                        input_replay_source: None,
                        result_replay_source: Some("preview_event".to_string()),
                        error: None,
                        message_id: None,
                    });
                }
            }
            "tool_result_completed" => {
                let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                let result_preview = payload["result_preview"]
                    .as_str()
                    .or_else(|| payload["result"].as_str())
                    .map(str::to_string);
                let output_uri = payload["output_uri"].as_str().map(str::to_string);
                let replay_source = payload["replay_source"]
                    .as_str()
                    .unwrap_or("completed_event")
                    .to_string();
                let found = parts.iter_mut().rev().any(|p| match p {
                    SessionPart::Tool {
                        tool_call_id,
                        status,
                        result_preview: current_preview,
                        output_uri: current_uri,
                        result_replay_source,
                        ..
                    } if tool_call_id == &call_id => {
                        *status = ToolPartStatus::Completed;
                        *current_preview = result_preview.clone();
                        *current_uri = output_uri.clone();
                        *result_replay_source = Some(replay_source.clone());
                        true
                    }
                    _ => false,
                });
                if !found {
                    parts.push(SessionPart::Tool {
                        part_id: format!("tool_{call_id}"),
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Completed,
                        input_args: None,
                        result_preview,
                        output_uri,
                        input_replay_source: None,
                        result_replay_source: Some(replay_source),
                        error: None,
                        message_id: None,
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
                        part_id: format!("tool_{call_id}"),
                        tool_call_id: call_id,
                        tool_name: String::new(),
                        status: ToolPartStatus::Failed,
                        input_args: None,
                        result_preview: None,
                        output_uri: None,
                        input_replay_source: None,
                        result_replay_source: None,
                        error: payload["error"].as_str().map(|s| s.to_string()),
                        message_id: None,
                    });
                }
            }
            // Non-merge events reset text/reasoning block tracking so the
            // next delta starts a fresh block with a new part_id.
            other => match other {
                "shell_output_completed" => {
                    let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
                    parts.push(SessionPart::Shell {
                        part_id: format!("shell_{call_id}"),
                        tool_call_id: call_id,
                        command: payload["command"].as_str().map(str::to_string),
                        status: ToolPartStatus::Completed,
                        output_uri: payload["output_uri"].as_str().map(str::to_string),
                        message_id: None,
                    });
                }
                "closeout" => parts.push(SessionPart::Closeout {
                    part_id: format!("closeout_{}", event.seq),
                    status: payload["status"].as_str().unwrap_or("unknown").to_string(),
                    evidence_summary: payload["evidence_summary"].as_str().map(|s| s.to_string()),
                    message_id: None,
                }),
                "compaction" => parts.push(SessionPart::Compaction {
                    part_id: format!("compaction_{}", event.seq),
                    strategy: payload["strategy"].as_str().unwrap_or("").to_string(),
                    trigger: payload["trigger"].as_str().unwrap_or("").to_string(),
                    before_tokens: payload["before_tokens"].as_u64().unwrap_or(0),
                    after_tokens: payload["after_tokens"].as_u64().unwrap_or(0),
                    message_id: None,
                }),
                "permission_requested" => parts.push(SessionPart::Permission {
                    part_id: format!("perm_{}", event.seq),
                    tool_name: payload["tool_name"].as_str().unwrap_or("").to_string(),
                    decided: false,
                    allowed: None,
                    message_id: None,
                }),
                "revert" => parts.push(SessionPart::Revert {
                    part_id: format!("revert_{}", event.seq),
                    status: payload["status"].as_str().unwrap_or("unknown").to_string(),
                    message_id: payload["message_id"].as_str().map(str::to_string),
                    target_part_id: payload["target_part_id"].as_str().map(str::to_string),
                    part_ids: string_array(&payload["part_ids"]),
                    paths: string_array(&payload["paths"]),
                    restored_files: string_array(&payload["restored_files"]),
                    removed_files: string_array(&payload["removed_files"]),
                    errors: string_array(&payload["errors"]),
                    snapshot_checkpoint_id: payload["snapshot_checkpoint_id"]
                        .as_str()
                        .map(str::to_string),
                    timestamp: payload["timestamp"].as_str().map(str::to_string),
                    unrevert_possible: payload["unrevert_possible"].as_bool().unwrap_or(false),
                    reverted_after: reverted_after_marker(&payload),
                }),
                "unrevert" => parts.push(SessionPart::Revert {
                    part_id: format!("unrevert_{}", event.seq),
                    status: "unreverted".to_string(),
                    message_id: payload["message_id"].as_str().map(str::to_string),
                    target_part_id: payload["target_part_id"].as_str().map(str::to_string),
                    part_ids: string_array(&payload["part_ids"]),
                    paths: string_array(&payload["paths"]),
                    restored_files: string_array(&payload["restored_files"]),
                    removed_files: string_array(&payload["removed_files"]),
                    errors: string_array(&payload["errors"]),
                    snapshot_checkpoint_id: payload["snapshot_checkpoint_id"]
                        .as_str()
                        .map(str::to_string),
                    timestamp: payload["timestamp"].as_str().map(str::to_string),
                    unrevert_possible: false,
                    reverted_after: reverted_after_marker(&payload),
                }),
                _ => {}
            },
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
    pub message_id: Option<String>,
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
        let (tool_call_id, tool_name, status, message_id) = part_projection_fields(part);
        conn.execute(
            "INSERT INTO session_parts
             (session_id, part_index, part_id, kind, tool_call_id, tool_name, status, payload, projected_to_seq, message_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                message_id,
            ],
        )?;
    }
    Ok(parts)
}

/// Get the highest projected sequence for a session (0 if empty).
pub fn get_max_projected_seq(conn: &Connection, session_id: &str) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "SELECT COALESCE(MAX(projected_to_seq), 0) FROM session_parts WHERE session_id = ?1",
        [session_id],
        |row| row.get(0),
    )
}

/// Incrementally apply new session events to session_parts without a full
/// DELETE + re-insert.  Reads events after the last projected sequence and
/// applies each one into existing or new rows.
pub fn incremental_refresh_session_parts(
    conn: &Connection,
    session_id: &str,
) -> Result<(), rusqlite::Error> {
    let max_seq = get_max_projected_seq(conn, session_id)?;
    let new_events = super::query_session_events_after(conn, session_id, max_seq)?;
    if new_events.is_empty() {
        return Ok(());
    }
    let new_projected_to_seq = new_events.last().map(|e| e.seq).unwrap_or(max_seq);
    for event in &new_events {
        apply_event_to_session_parts(conn, session_id, event)?;
    }
    conn.execute(
        "UPDATE session_parts SET projected_to_seq = ?1, updated_at = datetime('now') WHERE session_id = ?2",
        rusqlite::params![new_projected_to_seq, session_id],
    )?;
    Ok(())
}

/// Apply one event to the session_parts table, creating or updating rows.
fn apply_event_to_session_parts(
    conn: &Connection,
    session_id: &str,
    event: &super::SessionEventRow,
) -> Result<(), rusqlite::Error> {
    let payload: serde_json::Value = serde_json::from_str(&event.payload).unwrap_or_default();
    match event.event_type.as_str() {
        "assistant_text_delta" => {
            let text = payload["text"].as_str().unwrap_or("");
            if text.is_empty() {
                return Ok(());
            }
            append_text_part(conn, session_id, "assistant_text", text, event.seq)
        }
        "reasoning_delta" => {
            let text = payload["text"].as_str().unwrap_or("");
            if text.is_empty() {
                return Ok(());
            }
            append_text_part(conn, session_id, "reasoning", text, event.seq)
        }
        "assistant_text_completed" => {
            let full_text = payload["text"].as_str().unwrap_or("");
            if full_text.is_empty() {
                return Ok(());
            }
            replace_text_with_completed(conn, session_id, "assistant_text", full_text, event.seq)
        }
        "reasoning_completed" => {
            let full_text = payload["text"].as_str().unwrap_or("");
            if full_text.is_empty() {
                return Ok(());
            }
            replace_text_with_completed(conn, session_id, "reasoning", full_text, event.seq)
        }
        "tool_called" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let part_id = format!("tool_{call_id}");
            let part = SessionPart::Tool {
                part_id: part_id.clone(),
                tool_call_id: call_id,
                tool_name: payload["tool_name"].as_str().unwrap_or("").to_string(),
                status: ToolPartStatus::Running,
                input_args: None,
                result_preview: None,
                output_uri: None,
                input_replay_source: None,
                result_replay_source: None,
                error: None,
                message_id: None,
            };
            insert_session_part(conn, session_id, part_id, part, event.seq)
        }
        "tool_args_delta" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let args_delta = payload["args_delta"].as_str().unwrap_or("");
            if args_delta.is_empty() {
                return Ok(());
            }
            upsert_tool_field(conn, session_id, &call_id, "input_args", args_delta, true)?;
            upsert_tool_field(
                conn,
                session_id,
                &call_id,
                "input_replay_source",
                "delta",
                false,
            )
        }
        "tool_input_completed" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let input = payload["input_args"].as_str().unwrap_or("");
            let replay_source = payload["replay_source"]
                .as_str()
                .unwrap_or("completed_event");
            ensure_tool_part(conn, session_id, &call_id, event.seq)?;
            upsert_tool_field(conn, session_id, &call_id, "input_args", input, false)?;
            upsert_tool_field(
                conn,
                session_id,
                &call_id,
                "input_replay_source",
                replay_source,
                false,
            )
        }
        "tool_started" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let name = payload["tool_name"].as_str().unwrap_or("").to_string();
            let part_id = format!("tool_{call_id}");
            match find_part_id_by_part_id(conn, session_id, &part_id) {
                Some(_) => {
                    let _ = conn.execute(
                        "UPDATE session_parts SET tool_name = ?1, status = ?2, updated_at = datetime('now') WHERE session_id = ?3 AND part_id = ?4",
                        rusqlite::params![name, ToolPartStatus::Running.label(), session_id, part_id],
                    );
                    update_payload_field_direct(
                        conn,
                        session_id,
                        &part_id,
                        "tool_name",
                        &serde_json::Value::String(name),
                    )?;
                    update_payload_field_direct(
                        conn,
                        session_id,
                        &part_id,
                        "status",
                        &serde_json::Value::String(ToolPartStatus::Running.label().to_string()),
                    )?;
                }
                None => {
                    let part = SessionPart::Tool {
                        part_id: part_id.clone(),
                        tool_call_id: call_id,
                        tool_name: name,
                        status: ToolPartStatus::Running,
                        input_args: None,
                        result_preview: None,
                        output_uri: None,
                        input_replay_source: None,
                        result_replay_source: None,
                        error: None,
                        message_id: None,
                    };
                    insert_session_part(conn, session_id, part_id, part, event.seq)?;
                }
            }
            Ok(())
        }
        "tool_succeeded" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let result = payload["result_preview"].as_str().map(|s| s.to_string());
            let part_id = format!("tool_{call_id}");
            if let Some(v) = result.as_ref() {
                upsert_tool_field(conn, session_id, &call_id, "result_preview", v, false)?;
                upsert_tool_field(
                    conn,
                    session_id,
                    &call_id,
                    "result_replay_source",
                    "preview_event",
                    false,
                )?;
            }
            let _ = conn.execute(
                "UPDATE session_parts SET status = ?1, updated_at = datetime('now') WHERE session_id = ?2 AND part_id = ?3",
                rusqlite::params![ToolPartStatus::Completed.label(), session_id, part_id],
            );
            update_payload_field_direct(
                conn,
                session_id,
                &part_id,
                "status",
                &serde_json::Value::String(ToolPartStatus::Completed.label().to_string()),
            )
        }
        "tool_result_completed" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let result_preview = payload["result_preview"]
                .as_str()
                .or_else(|| payload["result"].as_str())
                .unwrap_or("");
            let replay_source = payload["replay_source"]
                .as_str()
                .unwrap_or("completed_event");
            let part_id = format!("tool_{call_id}");
            ensure_tool_part(conn, session_id, &call_id, event.seq)?;
            upsert_tool_field(
                conn,
                session_id,
                &call_id,
                "result_preview",
                result_preview,
                false,
            )?;
            if let Some(output_uri) = payload["output_uri"].as_str() {
                upsert_tool_field(conn, session_id, &call_id, "output_uri", output_uri, false)?;
            }
            upsert_tool_field(
                conn,
                session_id,
                &call_id,
                "result_replay_source",
                replay_source,
                false,
            )?;
            let _ = conn.execute(
                "UPDATE session_parts SET status = ?1, updated_at = datetime('now') WHERE session_id = ?2 AND part_id = ?3",
                rusqlite::params![ToolPartStatus::Completed.label(), session_id, part_id],
            );
            update_payload_field_direct(
                conn,
                session_id,
                &part_id,
                "status",
                &serde_json::Value::String(ToolPartStatus::Completed.label().to_string()),
            )
        }
        "tool_failed" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let error_text = payload["error"].as_str().map(|s| s.to_string());
            let part_id = format!("tool_{call_id}");
            if let Some(v) = error_text.as_ref() {
                upsert_tool_field(conn, session_id, &call_id, "error", v, false)?;
            }
            let _ = conn.execute(
                "UPDATE session_parts SET status = ?1, updated_at = datetime('now') WHERE session_id = ?2 AND part_id = ?3",
                rusqlite::params![ToolPartStatus::Failed.label(), session_id, part_id],
            );
            update_payload_field_direct(
                conn,
                session_id,
                &part_id,
                "status",
                &serde_json::Value::String(ToolPartStatus::Failed.label().to_string()),
            )
        }
        "closeout" => {
            let part_id = format!("closeout_{}", event.seq);
            let part = SessionPart::Closeout {
                part_id: part_id.clone(),
                status: payload["status"].as_str().unwrap_or("unknown").to_string(),
                evidence_summary: payload["evidence_summary"].as_str().map(|s| s.to_string()),
                message_id: None,
            };
            insert_session_part(conn, session_id, part_id, part, event.seq)
        }
        "compaction" => {
            let part_id = format!("compaction_{}", event.seq);
            let part = SessionPart::Compaction {
                part_id: part_id.clone(),
                strategy: payload["strategy"].as_str().unwrap_or("").to_string(),
                trigger: payload["trigger"].as_str().unwrap_or("").to_string(),
                before_tokens: payload["before_tokens"].as_u64().unwrap_or(0),
                after_tokens: payload["after_tokens"].as_u64().unwrap_or(0),
                message_id: None,
            };
            insert_session_part(conn, session_id, part_id, part, event.seq)
        }
        "permission_requested" => {
            let part_id = format!("perm_{}", event.seq);
            let part = SessionPart::Permission {
                part_id: part_id.clone(),
                tool_name: payload["tool_name"].as_str().unwrap_or("").to_string(),
                decided: false,
                allowed: None,
                message_id: None,
            };
            insert_session_part(conn, session_id, part_id, part, event.seq)
        }
        "shell_output_completed" => {
            let call_id = payload["tool_call_id"].as_str().unwrap_or("").to_string();
            let part_id = format!("shell_{call_id}");
            let part = SessionPart::Shell {
                part_id: part_id.clone(),
                tool_call_id: call_id,
                command: payload["command"].as_str().map(str::to_string),
                status: ToolPartStatus::Completed,
                output_uri: payload["output_uri"].as_str().map(str::to_string),
                message_id: None,
            };
            insert_session_part(conn, session_id, part_id, part, event.seq)
        }
        "revert" => {
            let part_id = format!("revert_{}", event.seq);
            let part = SessionPart::Revert {
                part_id: part_id.clone(),
                status: payload["status"].as_str().unwrap_or("unknown").to_string(),
                message_id: payload["message_id"].as_str().map(str::to_string),
                target_part_id: payload["target_part_id"].as_str().map(str::to_string),
                part_ids: string_array(&payload["part_ids"]),
                paths: string_array(&payload["paths"]),
                restored_files: string_array(&payload["restored_files"]),
                removed_files: string_array(&payload["removed_files"]),
                errors: string_array(&payload["errors"]),
                snapshot_checkpoint_id: payload["snapshot_checkpoint_id"]
                    .as_str()
                    .map(str::to_string),
                timestamp: payload["timestamp"].as_str().map(str::to_string),
                unrevert_possible: payload["unrevert_possible"].as_bool().unwrap_or(false),
                reverted_after: reverted_after_marker(&payload),
            };
            insert_session_part(conn, session_id, part_id, part, event.seq)
        }
        "unrevert" => {
            let part_id = format!("unrevert_{}", event.seq);
            let part = SessionPart::Revert {
                part_id: part_id.clone(),
                status: "unreverted".to_string(),
                message_id: payload["message_id"].as_str().map(str::to_string),
                target_part_id: payload["target_part_id"].as_str().map(str::to_string),
                part_ids: string_array(&payload["part_ids"]),
                paths: string_array(&payload["paths"]),
                restored_files: string_array(&payload["restored_files"]),
                removed_files: string_array(&payload["removed_files"]),
                errors: string_array(&payload["errors"]),
                snapshot_checkpoint_id: payload["snapshot_checkpoint_id"]
                    .as_str()
                    .map(str::to_string),
                timestamp: payload["timestamp"].as_str().map(str::to_string),
                unrevert_possible: false,
                reverted_after: reverted_after_marker(&payload),
            };
            insert_session_part(conn, session_id, part_id, part, event.seq)
        }
        // Non-delta events that aren't directly projected are fine to skip.
        _ => Ok(()),
    }
}

/// Cursor API: query persisted parts after a given part_index.
pub fn query_session_parts_after(
    conn: &Connection,
    session_id: &str,
    after_part_index: i64,
    limit: usize,
) -> Result<Vec<PersistedSessionPart>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, part_index, part_id, kind, tool_call_id, tool_name, status, payload, projected_to_seq, updated_at, message_id
         FROM session_parts
         WHERE session_id = ?1 AND part_index > ?2
         ORDER BY part_index ASC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![session_id, after_part_index, limit as i64],
        map_persisted_part,
    )?;
    rows.collect()
}

/// Cursor API: query session events after a given sequence.
pub fn query_session_events_page(
    conn: &Connection,
    session_id: &str,
    after_seq: i64,
    limit: usize,
) -> Result<Vec<super::SessionEventRow>, rusqlite::Error> {
    super::event_store::query_session_events_after(conn, session_id, after_seq)
        .map(|events| events.into_iter().take(limit).collect())
}

// --- internal helpers for incremental projection ---

/// Append text to the last text/reasoning part, or create a new one.
fn append_text_part(
    conn: &Connection,
    session_id: &str,
    kind: &str,
    text: &str,
    seq: i64,
) -> Result<(), rusqlite::Error> {
    match conn.query_row(
        "SELECT id, kind, payload FROM session_parts WHERE session_id = ?1 ORDER BY part_index DESC LIMIT 1",
        rusqlite::params![session_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    ) {
        Ok((row_id, last_kind, existing_payload)) if last_kind == kind => {
            let mut value: serde_json::Value =
                serde_json::from_str(&existing_payload).unwrap_or_default();
            if let Some(content) = value.get_mut("content") {
                if let Some(s) = content.as_str() {
                    *content = serde_json::Value::String(format!("{s}{text}"));
                }
            }
            conn.execute(
                "UPDATE session_parts SET payload = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![value.to_string(), row_id],
            )?;
        }
        Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {
            insert_text_part(conn, session_id, kind, text, seq)?
        }
        Err(err) => return Err(err),
    }
    Ok(())
}

/// Replace the content of the last text/reasoning part with the completed
/// full text, or create a new part if none exists.
fn replace_text_with_completed(
    conn: &Connection,
    session_id: &str,
    kind: &str,
    full_text: &str,
    seq: i64,
) -> Result<(), rusqlite::Error> {
    match conn.query_row(
        "SELECT id, payload FROM session_parts WHERE session_id = ?1 AND kind = ?2 ORDER BY part_index DESC LIMIT 1",
        rusqlite::params![session_id, kind],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    ) {
        Ok((row_id, existing_payload)) => {
            let mut value: serde_json::Value =
                serde_json::from_str(&existing_payload).unwrap_or_default();
            value["content"] = serde_json::Value::String(full_text.to_string());
            conn.execute(
                "UPDATE session_parts SET payload = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![value.to_string(), row_id],
            )?;
        }
        Err(_) => {
            let part_id = text_part_id(kind, seq);
            let part = match kind {
                "reasoning" => SessionPart::Reasoning {
                    part_id: part_id.clone(),
                    content: full_text.to_string(),
                    message_id: None,
                },
                _ => SessionPart::AssistantText {
                    part_id: part_id.clone(),
                    content: full_text.to_string(),
                    message_id: None,
                },
            };
            insert_session_part(conn, session_id, part_id, part, seq)?;
        }
    }
    Ok(())
}

fn insert_text_part(
    conn: &Connection,
    session_id: &str,
    kind: &str,
    text: &str,
    seq: i64,
) -> Result<(), rusqlite::Error> {
    let part_id = text_part_id(kind, seq);
    let part = match kind {
        "reasoning" => SessionPart::Reasoning {
            part_id: part_id.clone(),
            content: text.to_string(),
            message_id: None,
        },
        _ => SessionPart::AssistantText {
            part_id: part_id.clone(),
            content: text.to_string(),
            message_id: None,
        },
    };
    insert_session_part(conn, session_id, part_id, part, seq)
}

fn text_part_id(kind: &str, seq: i64) -> String {
    match kind {
        "reasoning" => format!("reasoning_{seq}"),
        _ => format!("text_{seq}"),
    }
}

/// Find the row id for a given part_id in a session.
fn find_part_id_by_part_id(conn: &Connection, session_id: &str, part_id: &str) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM session_parts WHERE session_id = ?1 AND part_id = ?2",
        rusqlite::params![session_id, part_id],
        |row| row.get(0),
    )
    .ok()
}

fn ensure_tool_part(
    conn: &Connection,
    session_id: &str,
    tool_call_id: &str,
    seq: i64,
) -> Result<(), rusqlite::Error> {
    let part_id = format!("tool_{tool_call_id}");
    if find_part_id_by_part_id(conn, session_id, &part_id).is_some() {
        return Ok(());
    }
    let part = SessionPart::Tool {
        part_id: part_id.clone(),
        tool_call_id: tool_call_id.to_string(),
        tool_name: String::new(),
        status: ToolPartStatus::Pending,
        input_args: None,
        result_preview: None,
        output_uri: None,
        input_replay_source: None,
        result_replay_source: None,
        error: None,
        message_id: None,
    };
    insert_session_part(conn, session_id, part_id, part, seq)
}

/// Upsert a field in a tool part's JSON payload.
fn upsert_tool_field(
    conn: &Connection,
    session_id: &str,
    tool_call_id: &str,
    field: &str,
    value: &str,
    append: bool,
) -> Result<(), rusqlite::Error> {
    let part_id = format!("tool_{tool_call_id}");
    if let Some(row_id) = find_part_id_by_part_id(conn, session_id, &part_id) {
        let existing: String = conn.query_row(
            "SELECT payload FROM session_parts WHERE id = ?1",
            [row_id],
            |row| row.get(0),
        )?;
        let mut payload: serde_json::Value = serde_json::from_str(&existing).unwrap_or_default();
        if append {
            if let Some(current) = payload.get_mut(field) {
                if let Some(s) = current.as_str() {
                    *current = serde_json::Value::String(format!("{s}{value}"));
                }
            } else {
                payload[field] = serde_json::Value::String(value.to_string());
            }
        } else {
            payload[field] = serde_json::Value::String(value.to_string());
        }
        conn.execute(
            "UPDATE session_parts SET payload = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![payload.to_string(), row_id],
        )?;
    }
    Ok(())
}

/// Set a field in a part's JSON payload directly.
fn update_payload_field_direct(
    conn: &Connection,
    session_id: &str,
    part_id: &str,
    field: &str,
    value: &serde_json::Value,
) -> Result<(), rusqlite::Error> {
    if let Some(row_id) = find_part_id_by_part_id(conn, session_id, part_id) {
        let existing: String = conn.query_row(
            "SELECT payload FROM session_parts WHERE id = ?1",
            [row_id],
            |row| row.get(0),
        )?;
        let mut payload: serde_json::Value = serde_json::from_str(&existing).unwrap_or_default();
        payload[field] = value.clone();
        conn.execute(
            "UPDATE session_parts SET payload = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![payload.to_string(), row_id],
        )?;
    }
    Ok(())
}

/// Insert a new session part.
fn insert_session_part(
    conn: &Connection,
    session_id: &str,
    part_id: String,
    part: SessionPart,
    seq: i64,
) -> Result<(), rusqlite::Error> {
    let part_index = next_part_index(conn, session_id)?;
    let payload = serde_json::to_value(&part).unwrap_or_else(|_| serde_json::json!({}));
    let kind = payload["kind"].as_str().unwrap_or("unknown").to_string();
    let (tool_call_id, tool_name, status, message_id) = part_projection_fields(&part);
    conn.execute(
        "INSERT INTO session_parts
         (session_id, part_index, part_id, kind, tool_call_id, tool_name, status, payload, projected_to_seq, message_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            session_id,
            part_index,
            part_id,
            kind,
            tool_call_id,
            tool_name,
            status,
            payload.to_string(),
            seq,
            message_id,
        ],
    )?;
    Ok(())
}

fn next_part_index(conn: &Connection, session_id: &str) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "SELECT COALESCE(MAX(part_index), -1) + 1 FROM session_parts WHERE session_id = ?1",
        [session_id],
        |row| row.get(0),
    )
}

fn map_persisted_part(row: &rusqlite::Row<'_>) -> rusqlite::Result<PersistedSessionPart> {
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
        message_id: row.get(11)?,
    })
}

pub fn query_persisted_session_parts(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<PersistedSessionPart>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, part_index, part_id, kind, tool_call_id, tool_name, status, payload, projected_to_seq, updated_at, message_id
         FROM session_parts
         WHERE session_id = ?1
         ORDER BY part_index ASC",
    )?;
    let rows = stmt.query_map([session_id], map_persisted_part)?;
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
        | SessionPart::Closeout { part_id, .. }
        | SessionPart::Revert { part_id, .. } => part_id,
    }
}

fn part_projection_fields(
    part: &SessionPart,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    match part {
        SessionPart::Tool {
            tool_call_id,
            tool_name,
            status,
            message_id,
            ..
        } => (
            Some(tool_call_id.clone()),
            Some(tool_name.clone()),
            Some(status.label().to_string()),
            message_id.clone(),
        ),
        SessionPart::Shell {
            tool_call_id,
            status,
            message_id,
            ..
        } => (
            Some(tool_call_id.clone()),
            Some("shell".to_string()),
            Some(status.label().to_string()),
            message_id.clone(),
        ),
        SessionPart::Permission {
            tool_name,
            message_id,
            ..
        } => (
            None,
            Some(tool_name.clone()),
            Some("waiting".to_string()),
            message_id.clone(),
        ),
        SessionPart::Closeout {
            status, message_id, ..
        } => (None, None, Some(status.clone()), message_id.clone()),
        SessionPart::Revert { status, .. } => {
            (None, Some("revert".to_string()), Some(status.clone()), None)
        }
        SessionPart::AssistantText { message_id, .. }
        | SessionPart::Reasoning { message_id, .. }
        | SessionPart::Compaction { message_id, .. } => (None, None, None, message_id.clone()),
    }
}

fn string_array(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn reverted_after_marker(payload: &serde_json::Value) -> Option<String> {
    payload["reverted_after"]
        .as_str()
        .or_else(|| payload["target_part_id"].as_str())
        .map(str::to_string)
        .or_else(|| string_array(&payload["part_ids"]).last().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_store::SessionEventRow;
    use rusqlite::Connection;

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

    #[test]
    fn projects_separate_text_blocks_around_tool_parts() {
        let events = vec![
            row(1, "assistant_text_delta", r#"{"text":"before"}"#),
            row(
                2,
                "tool_called",
                r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
            ),
            row(
                3,
                "tool_succeeded",
                r#"{"tool_call_id":"c1","result_preview":"ok"}"#,
            ),
            row(4, "assistant_text_delta", r#"{"text":"after"}"#),
        ];

        let parts = project_session_parts(&events);
        assert_eq!(parts.len(), 3);
        assert!(matches!(
            &parts[0],
            SessionPart::AssistantText { part_id, content, .. }
                if part_id == "text_1" && content == "before"
        ));
        assert!(matches!(&parts[1], SessionPart::Tool { .. }));
        assert!(matches!(
            &parts[2],
            SessionPart::AssistantText { part_id, content, .. }
                if part_id == "text_4" && content == "after"
        ));
    }

    #[test]
    fn projects_completed_tool_input_without_delta() {
        let events = vec![
            row(
                1,
                "tool_input_completed",
                r#"{"tool_call_id":"c1","input_args":"{\"command\":\"cargo test\"}","replay_source":"completed_event"}"#,
            ),
            row(
                2,
                "tool_started",
                r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
            ),
        ];

        let parts = project_session_parts(&events);
        match &parts[0] {
            SessionPart::Tool {
                input_args,
                input_replay_source,
                tool_name,
                ..
            } => {
                assert_eq!(tool_name, "bash");
                assert_eq!(input_args.as_deref(), Some(r#"{"command":"cargo test"}"#));
                assert_eq!(input_replay_source.as_deref(), Some("completed_event"));
            }
            _ => panic!("expected tool"),
        }
    }

    #[test]
    fn projects_completed_tool_result_with_output_uri() {
        let events = vec![
            row(
                1,
                "tool_called",
                r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
            ),
            row(
                2,
                "tool_result_completed",
                r#"{"tool_call_id":"c1","result_preview":"tail","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
            ),
            row(
                3,
                "shell_output_completed",
                r#"{"tool_call_id":"c1","command":"cargo test","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
            ),
        ];

        let parts = project_session_parts(&events);
        assert_eq!(parts.len(), 2);
        match &parts[0] {
            SessionPart::Tool {
                status,
                result_preview,
                output_uri,
                result_replay_source,
                ..
            } => {
                assert_eq!(*status, ToolPartStatus::Completed);
                assert_eq!(result_preview.as_deref(), Some("tail"));
                assert_eq!(output_uri.as_deref(), Some("tool-output://bash_c1"));
                assert_eq!(result_replay_source.as_deref(), Some("completed_event"));
            }
            _ => panic!("expected tool"),
        }
        assert!(matches!(
            &parts[1],
            SessionPart::Shell {
                command,
                output_uri,
                ..
            } if command.as_deref() == Some("cargo test")
                && output_uri.as_deref() == Some("tool-output://bash_c1")
        ));
    }

    #[test]
    fn projects_revert_marker_from_target_part() {
        let events = vec![row(
            1,
            "revert",
            r#"{"status":"completed","target_part_id":"tool_c1","part_ids":["tool_c1"],"unrevert_possible":true}"#,
        )];

        let parts = project_session_parts(&events);
        assert!(matches!(
            &parts[0],
            SessionPart::Revert {
                reverted_after,
                unrevert_possible,
                ..
            } if reverted_after.as_deref() == Some("tool_c1") && *unrevert_possible
        ));
    }

    #[test]
    fn incremental_projection_matches_full_projection_for_text_tool_text() {
        let conn = test_conn();
        let events = vec![
            row(1, "assistant_text_delta", r#"{"text":"before"}"#),
            row(
                2,
                "tool_called",
                r#"{"tool_call_id":"c1","tool_name":"bash"}"#,
            ),
            row(
                3,
                "tool_input_completed",
                r#"{"tool_call_id":"c1","input_args":"{\"command\":\"cargo test\"}","replay_source":"completed_event"}"#,
            ),
            row(
                4,
                "tool_succeeded",
                r#"{"tool_call_id":"c1","result_preview":"ok"}"#,
            ),
            row(
                5,
                "tool_result_completed",
                r#"{"tool_call_id":"c1","result_preview":"ok full","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
            ),
            row(
                6,
                "shell_output_completed",
                r#"{"tool_call_id":"c1","command":"cargo test","output_uri":"tool-output://bash_c1","replay_source":"completed_event"}"#,
            ),
            row(7, "assistant_text_delta", r#"{"text":"after"}"#),
            row(8, "reasoning_delta", r#"{"text":"think"}"#),
            row(9, "reasoning_completed", r#"{"text":"think done"}"#),
        ];

        for event in &events {
            insert_event(&conn, event);
            incremental_refresh_session_parts(&conn, "sess-1").unwrap();
        }

        let full_payloads = project_session_parts(&events)
            .iter()
            .map(|part| serde_json::to_value(part).unwrap())
            .collect::<Vec<_>>();
        let incremental_payloads = query_persisted_session_parts(&conn, "sess-1")
            .unwrap()
            .into_iter()
            .map(|part| part.payload)
            .collect::<Vec<_>>();

        assert_eq!(incremental_payloads, full_payloads);
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

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE session_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                payload TEXT NOT NULL DEFAULT '{}'
            );
            CREATE INDEX IF NOT EXISTS idx_session_events_session ON session_events(session_id, seq);
            CREATE TABLE session_parts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                part_index INTEGER NOT NULL,
                part_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                tool_call_id TEXT,
                tool_name TEXT,
                status TEXT,
                payload TEXT NOT NULL DEFAULT '{}',
                projected_to_seq INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                message_id TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_session_parts_session_part
                ON session_parts(session_id, part_id);",
        )
        .unwrap();
        conn
    }

    fn insert_event(conn: &Connection, event: &SessionEventRow) {
        conn.execute(
            "INSERT INTO session_events (session_id, seq, event_type, timestamp_ms, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                event.session_id,
                event.seq,
                event.event_type,
                event.timestamp_ms,
                event.payload
            ],
        )
        .unwrap();
    }
}

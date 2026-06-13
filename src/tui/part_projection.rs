//! Pure-ish reducer for TUI message/part projection state.
//!
//! `sync_store` owns snapshots and sequence bookkeeping; this module owns the
//! event-to-part projection rules so renderer state has one reducer path.

use crate::{
    session_store::SessionProjectionEvent,
    tui::{
        app::StreamUsageSnapshot,
        sync_store::{
            part_id_for, TuiMessagePart, TuiMessageProjection, TuiMessageRole, TuiPartKind,
            TuiSessionPhase, TuiSyncSnapshot,
        },
        tool_view::{upsert_tool_run, with_tool_run, ToolRunStatus},
    },
};

pub(crate) fn project_event(snapshot: &mut TuiSyncSnapshot, event: &SessionProjectionEvent) {
    match event {
        SessionProjectionEvent::RunStarted => {
            snapshot.phase = TuiSessionPhase::Running;
        }
        SessionProjectionEvent::TurnStarted {
            user_message_id,
            assistant_message_id,
        } => {
            let messages = vec![
                TuiMessageProjection {
                    id: user_message_id.clone(),
                    role: TuiMessageRole::User,
                    part_ids: Vec::new(),
                },
                TuiMessageProjection {
                    id: assistant_message_id.clone(),
                    role: TuiMessageRole::Assistant,
                    part_ids: Vec::new(),
                },
            ];
            *snapshot = TuiSyncSnapshot {
                phase: TuiSessionPhase::Running,
                active_user_message_id: Some(user_message_id.clone()),
                active_assistant_message_id: Some(assistant_message_id.clone()),
                messages,
                ..TuiSyncSnapshot::default()
            };
        }
        SessionProjectionEvent::AssistantTextDelta { message_id, text } => {
            if !text.is_empty() {
                let Some(message_id) = assistant_message_id(snapshot, message_id.as_deref()) else {
                    return;
                };
                if let Some(part) =
                    assistant_part_for_message(snapshot, &message_id, TuiPartKind::Text)
                {
                    part.text.push_str(text);
                    part.streaming = true;
                }
                snapshot.rebuild_assistant_projection_for(&message_id);
                snapshot.assistant_streaming = true;
            }
        }
        SessionProjectionEvent::AssistantTextUpdated {
            message_id,
            text,
            streaming,
        } => {
            let Some(message_id) = assistant_message_id(snapshot, message_id.as_deref()) else {
                return;
            };
            snapshot.set_message_text_part(
                &message_id,
                TuiMessageRole::Assistant,
                TuiPartKind::Text,
                text.clone(),
                *streaming,
            );
            snapshot.assistant_streaming = *streaming;
        }
        SessionProjectionEvent::ThinkingStarted { message_id } => {
            let Some(message_id) = assistant_message_id(snapshot, message_id.as_deref()) else {
                return;
            };
            if let Some(part) =
                assistant_part_for_message(snapshot, &message_id, TuiPartKind::Thinking)
            {
                part.streaming = true;
            }
            snapshot.thinking_streaming = true;
            snapshot.rebuild_assistant_projection_for(&message_id);
        }
        SessionProjectionEvent::ThinkingDelta { message_id, text } => {
            let Some(message_id) = assistant_message_id(snapshot, message_id.as_deref()) else {
                return;
            };
            if let Some(part) =
                assistant_part_for_message(snapshot, &message_id, TuiPartKind::Thinking)
            {
                part.text.push_str(text);
                part.streaming = true;
            }
            snapshot.rebuild_assistant_projection_for(&message_id);
            snapshot.thinking_streaming = true;
        }
        SessionProjectionEvent::ThinkingCompleted { message_id } => {
            let Some(message_id) = assistant_message_id(snapshot, message_id.as_deref()) else {
                return;
            };
            if let Some(part) =
                assistant_part_for_message(snapshot, &message_id, TuiPartKind::Thinking)
            {
                part.streaming = false;
            }
            snapshot.thinking_streaming = false;
            snapshot.rebuild_assistant_projection_for(&message_id);
        }
        SessionProjectionEvent::ThinkingUpdated {
            message_id,
            text,
            streaming,
        } => {
            let Some(message_id) = assistant_message_id(snapshot, message_id.as_deref()) else {
                return;
            };
            snapshot.set_message_text_part(
                &message_id,
                TuiMessageRole::Assistant,
                TuiPartKind::Thinking,
                text.clone(),
                *streaming,
            );
            snapshot.thinking_streaming = *streaming;
        }
        SessionProjectionEvent::ToolCallStarted {
            message_id,
            tool_call_id,
            tool_name,
        } => {
            snapshot.assistant_streaming = false;
            upsert_tool_run(
                &mut snapshot.tool_run_render_cache,
                tool_call_id.clone(),
                tool_name.clone(),
            );
            upsert_tool_part_for_message(snapshot, message_id.as_deref(), tool_call_id, tool_name);
        }
        SessionProjectionEvent::ToolArgumentsDelta {
            tool_call_id,
            arguments_delta,
        } => {
            with_tool_run(&mut snapshot.tool_run_render_cache, tool_call_id, |run| {
                run.push_args_delta(arguments_delta)
            });
            snapshot.sync_tool_part(tool_call_id);
        }
        SessionProjectionEvent::ToolCallAccepted { tool_call_id } => {
            snapshot.sync_tool_part(tool_call_id);
        }
        SessionProjectionEvent::ToolExecutionStarted {
            message_id,
            tool_call_id,
            tool_name,
            ..
        } => {
            with_tool_run(&mut snapshot.tool_run_render_cache, tool_call_id, |run| {
                run.mark_running(tool_name.clone())
            });
            upsert_tool_part_for_message(snapshot, message_id.as_deref(), tool_call_id, tool_name);
            snapshot.sync_tool_part(tool_call_id);
        }
        SessionProjectionEvent::ToolExecutionProgress {
            tool_call_id,
            progress,
        } => {
            with_tool_run(&mut snapshot.tool_run_render_cache, tool_call_id, |run| {
                run.push_progress(progress.clone())
            });
            snapshot.sync_tool_part(tool_call_id);
        }
        SessionProjectionEvent::ToolExecutionCompleted {
            tool_call_id,
            result,
            metadata,
            result_data,
        } => {
            with_tool_run(&mut snapshot.tool_run_render_cache, tool_call_id, |run| {
                run.mark_complete_with_metadata(result.clone(), metadata.clone());
                if let Some(data) = result_data.clone() {
                    run.result_data = Some(data);
                }
            });
            snapshot.sync_tool_part(tool_call_id);
        }
        SessionProjectionEvent::ToolPartUpdated {
            message_id,
            tool_call_id,
            tool_name,
            status,
            input_args,
            result,
            metadata,
            result_data,
        } => {
            upsert_tool_run_snapshot(
                snapshot,
                message_id.as_deref(),
                tool_call_id,
                tool_name,
                status.as_deref(),
                input_args.as_deref(),
                result.as_deref(),
                metadata.clone(),
                result_data.clone(),
            );
        }
        SessionProjectionEvent::PermissionRequested {
            message_id,
            tool_call_id,
            tool_name,
            arguments,
            ..
        } => {
            upsert_tool_run(
                &mut snapshot.tool_run_render_cache,
                tool_call_id.clone(),
                tool_name.clone(),
            );
            with_tool_run(&mut snapshot.tool_run_render_cache, tool_call_id, |run| {
                run.mark_waiting_permission(tool_name.clone(), arguments.clone())
            });
            upsert_tool_part_for_message(snapshot, message_id.as_deref(), tool_call_id, tool_name);
            snapshot.sync_tool_part(tool_call_id);
        }
        SessionProjectionEvent::Usage {
            prompt_tokens,
            completion_tokens,
            reasoning_tokens,
            cached_tokens,
        } => {
            snapshot.usage = Some(StreamUsageSnapshot {
                prompt_tokens: *prompt_tokens,
                completion_tokens: *completion_tokens,
                reasoning_tokens: *reasoning_tokens,
                cached_tokens: *cached_tokens,
            });
        }
        SessionProjectionEvent::RuntimeDiagnostic { .. }
        | SessionProjectionEvent::Closeout { .. }
        | SessionProjectionEvent::ToolResultsReadyForModel { .. }
        | SessionProjectionEvent::OutputTruncated => {}
        SessionProjectionEvent::Completed => {
            snapshot.phase = TuiSessionPhase::Completed;
            snapshot.assistant_streaming = false;
            snapshot.thinking_streaming = false;
            finalize_streaming_parts(snapshot);
        }
        SessionProjectionEvent::Error { message } => {
            snapshot.phase = TuiSessionPhase::Failed;
            snapshot.assistant_streaming = false;
            snapshot.thinking_streaming = false;
            mark_active_parts_not_streaming(snapshot);
            snapshot.last_error = Some(message.clone());
            if let Some(assistant_id) = snapshot.active_assistant_message_id.clone() {
                snapshot.upsert_message_projection(&assistant_id, TuiMessageRole::Assistant);
                let parts = snapshot
                    .parts_by_message_id
                    .entry(assistant_id.clone())
                    .or_default();
                let error_text = format!("[Error: {message}]");
                if let Some(text_part) = parts.iter_mut().find(|p| p.kind == TuiPartKind::Text) {
                    if !text_part.text.ends_with(&error_text) {
                        if !text_part.text.is_empty() {
                            text_part.text.push('\n');
                        }
                        text_part.text.push_str(&error_text);
                    }
                } else {
                    let part_id = part_id_for(&assistant_id, TuiPartKind::Text);
                    parts.push(TuiMessagePart {
                        id: part_id.clone(),
                        message_id: assistant_id.clone(),
                        kind: TuiPartKind::Text,
                        text: error_text,
                        tool_run: None,
                        streaming: false,
                    });
                    snapshot.push_message_part_id(&assistant_id, part_id);
                }
                snapshot.rebuild_assistant_projection_for(&assistant_id);
            }
        }
    }
}

pub(crate) fn finalize_streaming_parts(snapshot: &mut TuiSyncSnapshot) {
    mark_active_parts_not_streaming(snapshot);
    rebuild_active_assistant_projection(snapshot);
}

fn assistant_message_id(snapshot: &TuiSyncSnapshot, message_id: Option<&str>) -> Option<String> {
    message_id
        .map(str::to_string)
        .or_else(|| snapshot.active_assistant_message_id.clone())
}

fn assistant_part_for_message<'a>(
    snapshot: &'a mut TuiSyncSnapshot,
    message_id: &str,
    kind: TuiPartKind,
) -> Option<&'a mut TuiMessagePart> {
    snapshot.upsert_message_projection(message_id, TuiMessageRole::Assistant);
    let part_id = part_id_for(message_id, kind);
    let parts = snapshot
        .parts_by_message_id
        .entry(message_id.to_string())
        .or_default();
    if let Some(index) = parts.iter().position(|part| part.kind == kind) {
        return parts.get_mut(index);
    }
    parts.push(TuiMessagePart {
        id: part_id.clone(),
        message_id: message_id.to_string(),
        kind,
        text: String::new(),
        tool_run: None,
        streaming: false,
    });
    if let Some(message) = snapshot
        .messages
        .iter_mut()
        .find(|message| message.id == message_id)
    {
        if !message.part_ids.iter().any(|id| id == &part_id) {
            message.part_ids.push(part_id);
        }
    }
    parts.last_mut()
}

fn upsert_tool_part_for_message(
    snapshot: &mut TuiSyncSnapshot,
    message_id: Option<&str>,
    tool_id: &str,
    name: &str,
) {
    let Some(message_id) = message_id
        .map(str::to_string)
        .or_else(|| snapshot.active_user_message_id.clone())
    else {
        return;
    };
    snapshot.upsert_tool_part_for_message(&message_id, tool_id, name);
    snapshot.sync_tool_part(tool_id);
}

#[allow(clippy::too_many_arguments)]
fn upsert_tool_run_snapshot(
    snapshot: &mut TuiSyncSnapshot,
    message_id: Option<&str>,
    tool_id: &str,
    tool_name: &str,
    status: Option<&str>,
    input_args: Option<&str>,
    result: Option<&str>,
    metadata: Option<serde_json::Value>,
    result_data: Option<serde_json::Value>,
) {
    upsert_tool_run(
        &mut snapshot.tool_run_render_cache,
        tool_id.to_string(),
        tool_name.to_string(),
    );
    with_tool_run(&mut snapshot.tool_run_render_cache, tool_id, |run| {
        if let Some(input_args) = input_args {
            run.args_buffer = input_args.to_string();
            run.arguments = serde_json::from_str(input_args).ok();
        }
        match status {
            Some("failed") => {
                run.mark_complete_with_metadata(
                    result.unwrap_or_default().to_string(),
                    metadata.or_else(|| Some(serde_json::json!({"success": false}))),
                );
                run.status = ToolRunStatus::Failed;
            }
            Some("timed_out") => {
                run.mark_complete_with_metadata(
                    result.unwrap_or_default().to_string(),
                    metadata.or_else(|| Some(serde_json::json!({"status": "timed_out"}))),
                );
                run.status = ToolRunStatus::TimedOut;
            }
            Some("cancelled") => {
                run.mark_complete_with_metadata(
                    result.unwrap_or_default().to_string(),
                    metadata.or_else(|| Some(serde_json::json!({"status": "cancelled"}))),
                );
                run.status = ToolRunStatus::Cancelled;
            }
            Some("completed") => {
                run.mark_complete_with_metadata(
                    result.unwrap_or_default().to_string(),
                    metadata.or_else(|| Some(serde_json::json!({"success": true}))),
                );
            }
            Some("running") => run.mark_running(tool_name.to_string()),
            _ if result.is_some() => {
                run.mark_complete_with_metadata(result.unwrap_or_default().to_string(), metadata)
            }
            _ => {}
        }
        run.result_data = result_data;
    });
    upsert_tool_part_for_message(snapshot, message_id, tool_id, tool_name);
}

fn mark_active_parts_not_streaming(snapshot: &mut TuiSyncSnapshot) {
    let Some(message_id) = snapshot.active_assistant_message_id.clone() else {
        return;
    };
    if let Some(parts) = snapshot.parts_by_message_id.get_mut(&message_id) {
        for part in parts {
            part.streaming = false;
        }
    }
}

fn rebuild_active_assistant_projection(snapshot: &mut TuiSyncSnapshot) {
    let Some(message_id) = snapshot.active_assistant_message_id.clone() else {
        return;
    };
    snapshot.rebuild_assistant_projection_for(&message_id);
}

//! Event mirror — wires SessionEventWriter into StreamEvent pipeline.
//!
//! Mirrors live `StreamEvent` values into the `session_events` table
//! without changing frontend behavior. Used by TUI and desktop to
//! provide durable replay data.

use crate::engine::streaming::StreamEvent;
use crate::session_store::event_store::SessionEventWriter;
use crate::session_store::SessionProjectionEvent;
use crate::session_store::SessionStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Wraps a `SessionEventWriter` for use in the StreamEvent pipeline.
///
/// Call `mirror(event)` for each received `StreamEvent` to persist
/// tool lifecycle, usage, and closeout events.
///
/// Tracks accumulated assistant text so that when `Complete` arrives
/// the mirror can write an `assistant_text_completed` final-value
/// event for durable replay without concatenating every delta.
pub struct StreamEventMirror {
    writer: SessionEventWriter,
    accumulated_text: String,
    accumulated_reasoning: String,
    tool_args: HashMap<String, String>,
    tool_names: HashMap<String, String>,
}

impl StreamEventMirror {
    /// Create a mirror from a SessionStore.
    pub fn new(store: &SessionStore, session_id: &str) -> Option<Self> {
        let conn = store.shared_conn();
        let writer = SessionEventWriter::new(conn, session_id);
        Some(Self {
            writer,
            accumulated_text: String::new(),
            accumulated_reasoning: String::new(),
            tool_args: HashMap::new(),
            tool_names: HashMap::new(),
        })
    }

    /// Create a shared mirror for use across async boundaries.
    pub fn shared(store: &SessionStore, session_id: &str) -> Option<Arc<Mutex<Self>>> {
        Self::new(store, session_id).map(|m| Arc::new(Mutex::new(m)))
    }

    /// Mirror a StreamEvent to the session_events table.
    pub fn mirror(&mut self, event: &StreamEvent) {
        let projection_event = SessionProjectionEvent::from_stream_event(event, None, None);
        self.mirror_projection_event(&projection_event);
    }

    /// Mirror a SessionProjectionEvent to the session_events table.
    pub fn mirror_projection_event(&mut self, event: &SessionProjectionEvent) {
        let _ = match event {
            SessionProjectionEvent::RunStarted => self.writer.step_started(),
            SessionProjectionEvent::TurnStarted { .. } => Ok(()),
            SessionProjectionEvent::AssistantTextDelta { text, .. } => {
                self.accumulated_text.push_str(text);
                self.writer.text_delta(text)
            }
            SessionProjectionEvent::AssistantTextUpdated { text, .. } => {
                self.accumulated_text.clear();
                self.writer.text_completed(text)
            }
            SessionProjectionEvent::ThinkingStarted { .. } => {
                self.writer.write_event("reasoning_started", "{}")
            }
            SessionProjectionEvent::ThinkingDelta { text, .. } => {
                self.accumulated_reasoning.push_str(text);
                self.writer.reasoning_delta(text)
            }
            SessionProjectionEvent::ThinkingCompleted { .. } => {
                if self.accumulated_reasoning.is_empty() {
                    self.writer.write_event("reasoning_completed", "{}")
                } else {
                    let result = self.writer.reasoning_completed(&self.accumulated_reasoning);
                    self.accumulated_reasoning.clear();
                    result
                }
            }
            SessionProjectionEvent::ThinkingUpdated { text, .. } => {
                self.accumulated_reasoning.clear();
                self.writer.reasoning_completed(text)
            }
            SessionProjectionEvent::ToolCallStarted {
                tool_call_id,
                tool_name,
                ..
            } => {
                self.tool_names
                    .insert(tool_call_id.clone(), tool_name.clone());
                self.writer.tool_called(tool_call_id, tool_name)
            }
            SessionProjectionEvent::ToolArgumentsDelta {
                tool_call_id,
                arguments_delta,
            } => {
                self.tool_args
                    .entry(tool_call_id.clone())
                    .or_default()
                    .push_str(arguments_delta);
                self.writer.tool_args_delta(tool_call_id, arguments_delta)
            }
            SessionProjectionEvent::ToolCallAccepted { tool_call_id } => {
                let result = self.writer.tool_call_ready(tool_call_id);
                if let Some(args) = self.tool_args.get(tool_call_id) {
                    let _ = self.writer.tool_input_completed(tool_call_id, args);
                }
                result
            }
            SessionProjectionEvent::ToolExecutionStarted {
                tool_call_id,
                tool_name,
                ..
            } => {
                self.tool_names
                    .insert(tool_call_id.clone(), tool_name.clone());
                self.writer.tool_started(tool_call_id, tool_name)
            }
            SessionProjectionEvent::ToolExecutionProgress {
                tool_call_id,
                progress,
            } => self.writer.tool_progress(tool_call_id, progress),
            SessionProjectionEvent::ToolExecutionCompleted {
                tool_call_id,
                result,
                metadata,
                result_data,
            } => {
                let tool_succeeded = tool_execution_succeeded(metadata, result_data, result);
                let _ = self.writer.tool_result_completed(tool_call_id, result);
                if is_shell_tool(self.tool_names.get(tool_call_id).map(String::as_str)) {
                    let command = self
                        .tool_args
                        .get(tool_call_id)
                        .and_then(|args| extract_command(args));
                    let _ = self.writer.shell_output_completed(
                        tool_call_id,
                        command.as_deref(),
                        result,
                    );
                }
                let status_event = if tool_succeeded {
                    self.writer
                        .tool_succeeded(tool_call_id, &safe_preview(result, 256))
                } else {
                    self.writer
                        .tool_failed(tool_call_id, &safe_preview(result, 512))
                };
                self.tool_args.remove(tool_call_id);
                self.tool_names.remove(tool_call_id);
                status_event
            }
            SessionProjectionEvent::ToolPartUpdated {
                tool_call_id,
                tool_name,
                status,
                input_args,
                result,
                ..
            } => {
                let _ = self.writer.tool_called(tool_call_id, tool_name);
                if let Some(input_args) = input_args {
                    let _ = self.writer.tool_input_completed(tool_call_id, input_args);
                }
                if let Some(result) = result {
                    let _ = self.writer.tool_result_completed(tool_call_id, result);
                    if matches!(
                        status.as_deref(),
                        Some("failed" | "timed_out" | "cancelled")
                    ) {
                        self.writer
                            .tool_failed(tool_call_id, &safe_preview(result, 512))
                    } else {
                        self.writer
                            .tool_succeeded(tool_call_id, &safe_preview(result, 256))
                    }
                } else {
                    self.writer.tool_started(tool_call_id, tool_name)
                }
            }
            SessionProjectionEvent::ToolResultsReadyForModel { tool_call_ids } => {
                self.writer.write_event(
                    "tool_results_ready_for_model",
                    &serde_json::json!({ "tool_call_ids": tool_call_ids }).to_string(),
                )
            }
            SessionProjectionEvent::Closeout {
                status,
                evidence_summary,
            } => self.writer.closeout(status, evidence_summary.as_deref()),
            SessionProjectionEvent::Usage {
                prompt_tokens,
                completion_tokens,
                cached_tokens,
                ..
            } => self.writer.usage(
                *prompt_tokens as u64,
                *completion_tokens as u64,
                cached_tokens.unwrap_or(0) as u64,
            ),
            SessionProjectionEvent::RuntimeDiagnostic { diagnostic } => {
                let payload = serde_json::json!({ "diagnostic": diagnostic }).to_string();
                self.writer.write_event("runtime_diagnostic", &payload)
            }
            SessionProjectionEvent::PermissionRequested {
                tool_call_id,
                tool_name,
                arguments,
                prompt,
                ..
            } => self
                .writer
                .permission_requested(tool_call_id, tool_name, arguments, prompt),
            SessionProjectionEvent::OutputTruncated => {
                self.writer.write_event("output_truncated", "{}")
            }
            SessionProjectionEvent::Completed => {
                if !self.accumulated_text.is_empty() {
                    let _ = self.writer.text_completed(&self.accumulated_text);
                }
                self.accumulated_text.clear();
                self.accumulated_reasoning.clear();
                self.tool_args.clear();
                self.tool_names.clear();
                self.writer.step_ended()
            }
            SessionProjectionEvent::Error { message } => self.writer.runtime_error(message),
        };
    }
}

fn tool_execution_succeeded(
    metadata: &Option<serde_json::Value>,
    result_data: &Option<serde_json::Value>,
    result: &str,
) -> bool {
    for value in [result_data.as_ref(), metadata.as_ref()]
        .into_iter()
        .flatten()
    {
        if let Some(success) = value
            .get("success")
            .or_else(|| value.pointer("/tool_summary/success"))
            .and_then(serde_json::Value::as_bool)
        {
            return success;
        }
        if let Some(status) = value
            .get("status")
            .or_else(|| value.pointer("/tool_observation/status"))
            .and_then(serde_json::Value::as_str)
        {
            return matches!(status, "success" | "passed" | "completed" | "ok");
        }
    }
    !result.contains("Result: ERROR")
}

fn safe_preview(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_bytes).collect();
        truncated.push_str("...");
        truncated
    }
}

fn is_shell_tool(tool_name: Option<&str>) -> bool {
    matches!(tool_name, Some("bash" | "shell" | "run_shell_command"))
}

fn extract_command(args: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(args)
        .ok()
        .and_then(|value| {
            value
                .get("command")
                .or_else(|| value.get("cmd"))
                .and_then(|command| command.as_str())
                .map(str::to_string)
        })
}

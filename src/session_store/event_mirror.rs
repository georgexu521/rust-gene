//! Event mirror — wires SessionEventWriter into StreamEvent pipeline.
//!
//! Mirrors live `StreamEvent` values into the `session_events` table
//! without changing frontend behavior. Used by TUI and desktop to
//! provide durable replay data.

use crate::engine::streaming::StreamEvent;
use crate::session_store::event_store::SessionEventWriter;
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
        let _ = match event {
            StreamEvent::Start => self.writer.step_started(),
            StreamEvent::TextChunk(text) => {
                self.accumulated_text.push_str(text);
                self.writer.text_delta(text)
            }
            StreamEvent::ThinkingStart => self.writer.write_event("reasoning_started", "{}"),
            StreamEvent::ThinkingChunk(text) => {
                self.accumulated_reasoning.push_str(text);
                self.writer.reasoning_delta(text)
            }
            StreamEvent::ThinkingComplete => {
                if self.accumulated_reasoning.is_empty() {
                    self.writer.write_event("reasoning_completed", "{}")
                } else {
                    let result = self.writer.reasoning_completed(&self.accumulated_reasoning);
                    self.accumulated_reasoning.clear();
                    result
                }
            }
            StreamEvent::ToolCallStart { id, name } => {
                self.tool_names.insert(id.clone(), name.clone());
                self.writer.tool_called(id, name)
            }
            StreamEvent::ToolCallArgs { id, args_delta } => {
                self.tool_args
                    .entry(id.clone())
                    .or_default()
                    .push_str(args_delta);
                self.writer.tool_args_delta(id, args_delta)
            }
            StreamEvent::ToolCallComplete { id } => {
                let result = self.writer.tool_call_ready(id);
                if let Some(args) = self.tool_args.get(id) {
                    let _ = self.writer.tool_input_completed(id, args);
                }
                result
            }
            StreamEvent::ToolExecutionStart { id, name, .. } => {
                self.tool_names.insert(id.clone(), name.clone());
                self.writer.tool_started(id, name)
            }
            StreamEvent::ToolExecutionProgress { id, progress } => {
                self.writer.tool_progress(id, progress)
            }
            StreamEvent::ToolExecutionComplete { id, result, .. } => {
                let succeeded = self.writer.tool_succeeded(id, &safe_preview(result, 256));
                let _ = self.writer.tool_result_completed(id, result);
                if is_shell_tool(self.tool_names.get(id).map(String::as_str)) {
                    let command = self
                        .tool_args
                        .get(id)
                        .and_then(|args| extract_command(args));
                    let _ = self
                        .writer
                        .shell_output_completed(id, command.as_deref(), result);
                }
                self.tool_args.remove(id);
                self.tool_names.remove(id);
                succeeded
            }
            StreamEvent::Closeout {
                status,
                evidence_summary,
            } => self.writer.closeout(status, evidence_summary.as_deref()),
            StreamEvent::Usage {
                prompt_tokens,
                completion_tokens,
                cached_tokens,
                ..
            } => self.writer.usage(
                *prompt_tokens as u64,
                *completion_tokens as u64,
                cached_tokens.unwrap_or(0) as u64,
            ),
            StreamEvent::RuntimeDiagnostic { diagnostic } => {
                let payload = serde_json::json!({ "diagnostic": diagnostic }).to_string();
                self.writer.write_event("runtime_diagnostic", &payload)
            }
            StreamEvent::PermissionRequest {
                id,
                tool_name,
                arguments,
                prompt,
                ..
            } => self
                .writer
                .permission_requested(id, tool_name, arguments, prompt),
            StreamEvent::OutputTruncated => self.writer.write_event("output_truncated", "{}"),
            StreamEvent::Complete => {
                if !self.accumulated_text.is_empty() {
                    let _ = self.writer.text_completed(&self.accumulated_text);
                }
                self.accumulated_text.clear();
                self.accumulated_reasoning.clear();
                self.tool_args.clear();
                self.tool_names.clear();
                self.writer.step_ended()
            }
            StreamEvent::Error(message) => self.writer.runtime_error(message),
        };
    }
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

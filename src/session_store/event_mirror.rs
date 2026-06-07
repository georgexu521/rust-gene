//! Event mirror — wires SessionEventWriter into StreamEvent pipeline.
//!
//! Mirrors live `StreamEvent` values into the `session_events` table
//! without changing frontend behavior. Used by TUI and desktop to
//! provide durable replay data.

use crate::engine::streaming::StreamEvent;
use crate::session_store::event_store::SessionEventWriter;
use crate::session_store::SessionStore;
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
            StreamEvent::ToolCallStart { id, name } => self.writer.tool_called(id, name),
            StreamEvent::ToolCallArgs { id, args_delta } => {
                self.writer.tool_args_delta(id, args_delta)
            }
            StreamEvent::ToolCallComplete { id } => self.writer.tool_call_ready(id),
            StreamEvent::ToolExecutionStart { id, name, .. } => self.writer.tool_started(id, name),
            StreamEvent::ToolExecutionProgress { id, progress } => {
                self.writer.tool_progress(id, progress)
            }
            StreamEvent::ToolExecutionComplete { id, result, .. } => {
                self.writer.tool_succeeded(id, &safe_preview(result, 256))
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

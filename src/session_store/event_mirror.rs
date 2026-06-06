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
pub struct StreamEventMirror {
    writer: SessionEventWriter,
}

impl StreamEventMirror {
    /// Create a mirror from a SessionStore.
    pub fn new(store: &SessionStore, session_id: &str) -> Option<Self> {
        let conn = store.shared_conn();
        let writer = SessionEventWriter::new(conn, session_id);
        Some(Self { writer })
    }

    /// Create a shared mirror for use across async boundaries.
    pub fn shared(store: &SessionStore, session_id: &str) -> Option<Arc<Mutex<Self>>> {
        Self::new(store, session_id).map(|m| Arc::new(Mutex::new(m)))
    }

    /// Mirror a StreamEvent to the session_events table.
    pub fn mirror(&self, event: &StreamEvent) {
        let _ = match event {
            StreamEvent::ToolExecutionComplete { id, result, .. } => {
                let _ = self.writer.tool_called(id, "completed");
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
            _ => Ok(()),
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

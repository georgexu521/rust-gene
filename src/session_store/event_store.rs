//! Session event writer for durable replay.
//!
//! Phase 2 (opencode core alignment): mirrors StreamEvent variants into
//! the `session_events` table without changing frontend behavior. Events
//! are append-only and keyed by per-session sequence for deterministic replay.

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// Writes session events to the `session_events` table.
pub struct SessionEventWriter {
    conn: Arc<Mutex<Connection>>,
    session_id: String,
}

impl SessionEventWriter {
    pub fn new(conn: Arc<Mutex<Connection>>, session_id: &str) -> Self {
        Self {
            conn,
            session_id: session_id.to_string(),
        }
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    /// Write a single typed event with payload JSON.
    pub fn write_event(&self, event_type: &str, payload: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let timestamp_ms = now_ms();
        let seq: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM session_events WHERE session_id = ?1",
                [&self.session_id],
                |row| row.get(0),
            )
            .unwrap_or(1);

        conn.execute(
            "INSERT INTO session_events (session_id, seq, event_type, timestamp_ms, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![self.session_id, seq, event_type, timestamp_ms, payload],
        )?;
        super::session_parts::incremental_refresh_session_parts(&conn, &self.session_id)?;
        Ok(())
    }

    /// Mirror a provider step lifecycle event.
    pub fn step_started(&self) -> Result<(), rusqlite::Error> {
        self.write_event("step_started", "{}")
    }

    pub fn step_ended(&self) -> Result<(), rusqlite::Error> {
        self.write_event("step_ended", "{}")
    }

    pub fn step_failed(&self, error: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({ "error": error }).to_string();
        self.write_event("step_failed", &payload)
    }

    pub fn text_delta(&self, text: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({ "text": preview_bytes(text, 4096) }).to_string();
        self.write_event("assistant_text_delta", &payload)
    }

    pub fn reasoning_delta(&self, text: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({ "text": preview_bytes(text, 4096) }).to_string();
        self.write_event("reasoning_delta", &payload)
    }

    /// Write final complete reasoning text for durable replay.
    pub fn reasoning_completed(&self, text: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "text": text,
            "length": text.len(),
        })
        .to_string();
        self.write_event("reasoning_completed", &payload)
    }

    /// Mirror a tool lifecycle event.
    pub fn tool_called(&self, tool_call_id: &str, tool_name: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
        })
        .to_string();
        self.write_event("tool_called", &payload)
    }

    pub fn tool_args_delta(
        &self,
        tool_call_id: &str,
        args_delta: &str,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "args_delta": preview_bytes(args_delta, 4096),
        })
        .to_string();
        self.write_event("tool_args_delta", &payload)
    }

    pub fn tool_call_ready(&self, tool_call_id: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({ "tool_call_id": tool_call_id }).to_string();
        self.write_event("tool_call_ready", &payload)
    }

    /// Write the authoritative completed tool input for durable replay.
    pub fn tool_input_completed(
        &self,
        tool_call_id: &str,
        input_args: &str,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "input_args": input_args,
            "replay_source": "completed_event",
            "length": input_args.len(),
        })
        .to_string();
        self.write_event("tool_input_completed", &payload)
    }

    pub fn tool_started(&self, tool_call_id: &str, tool_name: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
        })
        .to_string();
        self.write_event("tool_started", &payload)
    }

    pub fn tool_progress(&self, tool_call_id: &str, progress: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "progress": preview_bytes(progress, 1024),
        })
        .to_string();
        self.write_event("tool_progress", &payload)
    }

    pub fn tool_succeeded(
        &self,
        tool_call_id: &str,
        result_preview: &str,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "result_preview": preview_bytes(result_preview, 512),
        })
        .to_string();
        self.write_event("tool_succeeded", &payload)
    }

    /// Write the authoritative completed tool result for durable replay.
    pub fn tool_result_completed(
        &self,
        tool_call_id: &str,
        result: &str,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "result": result,
            "result_preview": preview_bytes(result, 512),
            "output_uri": extract_tool_output_uri(result),
            "replay_source": "completed_event",
            "length": result.len(),
        })
        .to_string();
        self.write_event("tool_result_completed", &payload)
    }

    /// Write a shell-specific completed output marker for shell replay views.
    pub fn shell_output_completed(
        &self,
        tool_call_id: &str,
        command: Option<&str>,
        output: &str,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "command": command,
            "output": output,
            "output_preview": preview_bytes(output, 512),
            "output_uri": extract_tool_output_uri(output),
            "replay_source": "completed_event",
            "length": output.len(),
        })
        .to_string();
        self.write_event("shell_output_completed", &payload)
    }

    pub fn tool_failed(&self, tool_call_id: &str, error: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "tool_call_id": tool_call_id,
            "error": error,
        })
        .to_string();
        self.write_event("tool_failed", &payload)
    }

    pub fn permission_requested(
        &self,
        request_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
        prompt: &str,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "request_id": request_id,
            "tool_name": tool_name,
            "arguments": arguments,
            "prompt": preview_bytes(prompt, 1024),
        })
        .to_string();
        self.write_event("permission_requested", &payload)
    }

    pub fn runtime_error(&self, error: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({ "error": preview_bytes(error, 2048) }).to_string();
        self.write_event("error", &payload)
    }

    /// Mirror a usage event.
    pub fn usage(
        &self,
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_tokens: u64,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "cached_tokens": cached_tokens,
        })
        .to_string();
        self.write_event("usage", &payload)
    }

    /// Mirror a closeout event.
    pub fn closeout(&self, status: &str, summary: Option<&str>) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "status": status,
            "evidence_summary": summary,
        })
        .to_string();
        self.write_event("closeout", &payload)
    }

    /// Write final complete assistant text for durable replay.
    pub fn text_completed(&self, text: &str) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "text": text,
            "length": text.len(),
        })
        .to_string();
        self.write_event("assistant_text_completed", &payload)
    }

    /// Mirror a compaction event.
    pub fn compaction(
        &self,
        strategy: &str,
        trigger: &str,
        before_tokens: u64,
        after_tokens: u64,
    ) -> Result<(), rusqlite::Error> {
        let payload = serde_json::json!({
            "strategy": strategy,
            "trigger": trigger,
            "before_tokens": before_tokens,
            "after_tokens": after_tokens,
        })
        .to_string();
        self.write_event("compaction", &payload)
    }
}

impl Drop for SessionEventWriter {
    fn drop(&mut self) {
        let _ = self.write_event("writer_closed", "{}");
    }
}

/// Query session events for a session, ordered by sequence.
pub fn query_session_events(
    conn: &Connection,
    session_id: &str,
    limit: Option<usize>,
) -> Result<Vec<SessionEventRow>, rusqlite::Error> {
    let limit_clause = limit.map(|n| format!(" LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT id, session_id, seq, event_type, timestamp_ms, payload FROM session_events WHERE session_id = ?1 ORDER BY seq ASC{limit_clause}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([session_id], |row| {
        Ok(SessionEventRow {
            id: row.get(0)?,
            session_id: row.get(1)?,
            seq: row.get(2)?,
            event_type: row.get(3)?,
            timestamp_ms: row.get(4)?,
            payload: row.get(5)?,
        })
    })?;
    rows.collect()
}

/// Query session events after a given sequence (for incremental projection).
pub fn query_session_events_after(
    conn: &Connection,
    session_id: &str,
    after_seq: i64,
) -> Result<Vec<SessionEventRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, seq, event_type, timestamp_ms, payload FROM session_events WHERE session_id = ?1 AND seq > ?2 ORDER BY seq ASC",
    )?;
    let rows = stmt.query_map(rusqlite::params![session_id, after_seq], |row| {
        Ok(SessionEventRow {
            id: row.get(0)?,
            session_id: row.get(1)?,
            seq: row.get(2)?,
            event_type: row.get(3)?,
            timestamp_ms: row.get(4)?,
            payload: row.get(5)?,
        })
    })?;
    rows.collect()
}

#[derive(Debug, Clone)]
pub struct SessionEventRow {
    pub id: i64,
    pub session_id: String,
    pub seq: i64,
    pub event_type: String,
    pub timestamp_ms: i64,
    pub payload: String,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn preview_bytes(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        text.to_string()
    } else {
        let mut truncated = text.chars().take(max_bytes).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

fn extract_tool_output_uri(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.starts_with("tool-output://"))
        .map(|token| {
            token
                .trim_matches(|ch: char| {
                    matches!(ch, '.' | ',' | ';' | ':' | ')' | ']' | '}' | '"' | '\'')
                })
                .to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_conn() -> Arc<Mutex<Connection>> {
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
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn writes_and_queries_events_in_sequence() {
        let conn = test_conn();
        let writer = SessionEventWriter::new(conn.clone(), "sess-1");

        writer.step_started().unwrap();
        writer.tool_called("call-1", "bash").unwrap();
        writer.tool_succeeded("call-1", "test output").unwrap();
        writer.usage(1000, 500, 800).unwrap();
        writer.closeout("passed", Some("verified")).unwrap();

        let conn_guard = conn.lock().unwrap();
        let events = query_session_events(&conn_guard, "sess-1", None).unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].event_type, "step_started");
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[1].event_type, "tool_called");
        assert_eq!(events[2].event_type, "tool_succeeded");
        assert_eq!(events[3].event_type, "usage");
        assert_eq!(events[4].event_type, "closeout");
    }

    #[test]
    fn sequences_are_monotonic() {
        let conn = test_conn();
        let writer = SessionEventWriter::new(conn.clone(), "sess-2");

        for i in 0..10 {
            writer
                .write_event("test", &serde_json::json!({ "n": i }).to_string())
                .unwrap();
        }

        let conn_guard = conn.lock().unwrap();
        let events = query_session_events(&conn_guard, "sess-2", None).unwrap();
        assert_eq!(events.len(), 10);
        for (i, event) in events.iter().enumerate() {
            assert_eq!(event.seq, (i + 1) as i64);
        }
    }
}

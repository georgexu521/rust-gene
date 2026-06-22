//! Session-store support module.
//!
//! Owns one slice of durable session persistence so message, trace, learning, revert, and compact state stay separated.

use rusqlite::{params, Result as SqlResult};

use super::SessionStore;

impl SessionStore {
    /// Persist a completed or running turn trace.
    pub fn add_turn_trace(&self, trace: &crate::engine::trace::TurnTrace) -> SqlResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let status = serde_json::to_value(&trace.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "running".to_string());
        tx.execute(
            "INSERT OR REPLACE INTO turn_traces
             (trace_id, session_id, turn_index, user_message_preview, status, started_at, finished_at, duration_ms, event_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                trace.trace_id,
                trace.session_id,
                trace.turn_index as i64,
                trace.user_message_preview,
                status,
                trace.started_at.to_rfc3339(),
                trace.finished_at.map(|dt| dt.to_rfc3339()),
                trace.duration_ms(),
                trace.events.len() as i64,
            ],
        )?;
        tx.execute(
            "DELETE FROM trace_events WHERE trace_id = ?1",
            params![trace.trace_id],
        )?;
        for (idx, event) in trace.events.iter().enumerate() {
            let payload = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
            tx.execute(
                "INSERT INTO trace_events (trace_id, event_index, event_type, summary, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    trace.trace_id,
                    idx as i64,
                    event.label(),
                    event.summary(),
                    payload
                ],
            )?;
        }
        tx.commit()
    }

    /// Load the latest trace for a session.
    pub fn latest_turn_trace(
        &self,
        session_id: &str,
    ) -> SqlResult<Option<crate::engine::trace::TurnTrace>> {
        let trace_id: Option<String> = {
            let conn = self.conn();
            match conn.query_row(
                "SELECT trace_id FROM turn_traces WHERE session_id = ?1 ORDER BY turn_index DESC, started_at DESC LIMIT 1",
                params![session_id],
                |row| row.get(0),
            ) {
                Ok(id) => Some(id),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(e),
            }
        };
        match trace_id {
            Some(id) => self.get_turn_trace(&id),
            None => Ok(None),
        }
    }

    /// Load recent traces for a session, newest first.
    pub fn recent_turn_traces(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<crate::engine::trace::TurnTrace>> {
        let limit = limit.max(1);
        let trace_ids: Vec<String> = {
            let conn = self.conn();
            let mut stmt = conn.prepare(
                "SELECT trace_id FROM turn_traces
                 WHERE session_id = ?1
                 ORDER BY turn_index DESC, started_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![session_id, limit], |row| row.get(0))?;
            rows.collect::<SqlResult<Vec<_>>>()?
        };

        let mut traces = Vec::with_capacity(trace_ids.len());
        for trace_id in trace_ids {
            if let Some(trace) = self.get_turn_trace(&trace_id)? {
                traces.push(trace);
            }
        }
        Ok(traces)
    }

    /// Load one trace by ID.
    pub fn get_turn_trace(
        &self,
        trace_id: &str,
    ) -> SqlResult<Option<crate::engine::trace::TurnTrace>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT trace_id, session_id, turn_index, user_message_preview, status, started_at, finished_at
             FROM turn_traces WHERE trace_id = ?1",
            params![trace_id],
            |row| {
                let status_text: String = row.get(4)?;
                let status = match status_text.as_str() {
                    "completed" => crate::engine::trace::TurnStatus::Completed,
                    "failed" => crate::engine::trace::TurnStatus::Failed,
                    _ => crate::engine::trace::TurnStatus::Running,
                };
                let started_at: String = row.get(5)?;
                let finished_at: Option<String> = row.get(6)?;
                Ok(crate::engine::trace::TurnTrace {
                    trace_id: row.get(0)?,
                    session_id: row.get(1)?,
                    turn_index: row.get::<_, i64>(2)? as u64,
                    user_message_preview: row.get(3)?,
                    started_at: chrono::DateTime::parse_from_rfc3339(&started_at)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    finished_at: finished_at.and_then(|value| {
                        chrono::DateTime::parse_from_rfc3339(&value)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .ok()
                    }),
                    status,
                    events: Vec::new(),
                })
            },
        );

        let mut trace = match result {
            Ok(trace) => trace,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };

        let mut stmt = conn.prepare(
            "SELECT payload FROM trace_events WHERE trace_id = ?1 ORDER BY event_index ASC",
        )?;
        let rows = stmt.query_map(params![trace_id], |row| row.get::<_, String>(0))?;
        let mut events = Vec::new();
        for payload in rows.flatten() {
            if let Ok(event) = serde_json::from_str::<crate::engine::trace::TraceEvent>(&payload) {
                events.push(event);
            }
        }
        trace.events = events;
        Ok(Some(trace))
    }
}

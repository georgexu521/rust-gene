//! Session-store support module.
//!
//! Owns one slice of durable session persistence so message, trace, learning, revert, and compact state stay separated.

use rusqlite::{params, Result as SqlResult, Row};

use super::{LearningEventRecord, SessionStore};

impl SessionStore {
    /// Persist a durable learning event extracted from runtime behavior.
    pub fn add_learning_event(
        &self,
        session_id: &str,
        kind: &str,
        source: &str,
        summary: &str,
        confidence: f64,
        payload: &serde_json::Value,
    ) -> SqlResult<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO learning_events (session_id, kind, source, summary, confidence, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                kind,
                source,
                summary,
                confidence.clamp(0.0, 1.0),
                payload.to_string()
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Load recent learning events for a session.
    pub fn recent_learning_events(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<LearningEventRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, kind, source, summary, confidence, payload, created_at
             FROM learning_events
             WHERE session_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], learning_event_from_row)?;
        rows.collect()
    }

    /// Load recent context ledger events for a session.
    pub fn recent_context_ledger_events(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<LearningEventRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, kind, source, summary, confidence, payload, created_at
             FROM learning_events
             WHERE session_id = ?1 AND kind LIKE 'context_ledger.%'
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], learning_event_from_row)?;
        rows.collect()
    }

    /// Load the most recent file-read context ledger fact for a path in a session.
    pub fn latest_file_read_context_event(
        &self,
        session_id: &str,
        resolved_path: &str,
    ) -> SqlResult<Option<LearningEventRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, kind, source, summary, confidence, payload, created_at
             FROM learning_events
             WHERE session_id = ?1
               AND kind = ?2
               AND json_extract(payload, '$.resolved_path') = ?3
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
            params![
                session_id,
                crate::engine::context_ledger::CONTEXT_LEDGER_FILE_READ_KIND,
                resolved_path
            ],
            learning_event_from_row,
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Load one learning event by id within a session.
    pub fn learning_event(
        &self,
        session_id: &str,
        id: i64,
    ) -> SqlResult<Option<LearningEventRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, kind, source, summary, confidence, payload, created_at
             FROM learning_events
             WHERE session_id = ?1 AND id = ?2",
            params![session_id, id],
            learning_event_from_row,
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

fn learning_event_from_row(row: &Row<'_>) -> SqlResult<LearningEventRecord> {
    let payload_text: String = row.get(6)?;
    Ok(LearningEventRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        kind: row.get(2)?,
        source: row.get(3)?,
        summary: row.get(4)?,
        confidence: row.get(5)?,
        payload: serde_json::from_str(&payload_text).unwrap_or_else(|_| serde_json::json!({})),
        created_at: row.get(7)?,
    })
}

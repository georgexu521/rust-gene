//! Session-store support module.
//!
//! Owns one slice of durable session persistence so message, trace, learning, revert, and compact state stay separated.

use super::{fts_phrase_terms, session_from_row, MessageRecord, SessionRecord, SessionStore};
use rusqlite::{params, Result as SqlResult};

impl SessionStore {
    // ==================== 搜索 ====================

    /// 全文搜索消息
    pub fn search_messages(&self, query: &str, limit: i64) -> SqlResult<Vec<MessageRecord>> {
        let conn = self.conn();

        // FTS5 搜索
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.content, m.tool_calls, m.tool_call_id, m.reasoning, m.metadata, m.created_at
             FROM messages_fts fts
             JOIN messages m ON m.id = fts.rowid
             WHERE messages_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;

        let messages = stmt.query_map(params![query, limit], |row| {
            let tool_calls_str: Option<String> = row.get(4)?;
            let tool_calls = tool_calls_str.and_then(|s| serde_json::from_str(&s).ok());
            let metadata_str: Option<String> = row.get(7)?;
            let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(MessageRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_calls,
                tool_call_id: row.get(5)?,
                reasoning: row.get(6)?,
                metadata,
                created_at: row.get(8)?,
            })
        })?;

        messages.collect()
    }

    /// Search sessions by title and by message content through the FTS index.
    pub fn search_sessions(&self, query: &str, limit: i64) -> SqlResult<Vec<SessionRecord>> {
        let query = query.trim();
        if query.is_empty() {
            return self.list_sessions(limit);
        }

        let conn = self.conn();
        let clamped_limit = limit.clamp(1, 100);
        let title_query = format!("%{query}%");
        let fts_query = fts_phrase_terms(query);

        let mut sessions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        {
            let mut stmt = conn.prepare(
                "SELECT id, title, parent_session_id, created_at, updated_at, model, total_input_tokens, total_output_tokens, workspace_root
                 FROM sessions
                 WHERE title LIKE ?1
                 ORDER BY updated_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![title_query, clamped_limit], session_from_row)?;
            for row in rows {
                let session = row?;
                seen.insert(session.id.clone());
                sessions.push(session);
            }
        }

        if sessions.len() < clamped_limit as usize {
            let remaining = clamped_limit - sessions.len() as i64;
            let mut stmt = conn.prepare(
                "SELECT DISTINCT s.id, s.title, s.parent_session_id, s.created_at, s.updated_at, s.model, s.total_input_tokens, s.total_output_tokens, s.workspace_root
                 FROM messages_fts fts
                 JOIN messages m ON m.id = fts.rowid
                 JOIN sessions s ON s.id = m.session_id
                 WHERE messages_fts MATCH ?1
                 ORDER BY s.updated_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![fts_query, remaining], session_from_row)?;
            for row in rows {
                let session = row?;
                if seen.insert(session.id.clone()) {
                    sessions.push(session);
                }
            }
        }

        Ok(sessions)
    }
}

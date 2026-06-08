use super::{MessageInsert, MessageRecord, SessionStore};
use crate::services::api::Message;
use rusqlite::{params, Result as SqlResult};
use tracing::debug;

impl SessionStore {
    // ==================== 消息操作 ====================

    /// 添加消息
    pub fn add_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        tool_calls: Option<&serde_json::Value>,
        tool_call_id: Option<&str>,
    ) -> SqlResult<i64> {
        let conn = self.conn();
        let tool_calls_str = tool_calls.map(|v| v.to_string());

        conn.execute(
            "INSERT INTO messages (session_id, role, content, tool_calls, tool_call_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, role, content, tool_calls_str, tool_call_id],
        )?;

        let id = conn.last_insert_rowid();

        // 更新会话时间
        conn.execute(
            "UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1",
            params![session_id],
        )?;

        Ok(id)
    }

    /// 获取会话的所有消息
    pub fn get_messages(&self, session_id: &str) -> SqlResult<Vec<MessageRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, tool_calls, tool_call_id, reasoning, created_at
             FROM messages WHERE session_id = ?1 ORDER BY id ASC",
        )?;

        let messages = stmt.query_map(params![session_id], |row| {
            let tool_calls_str: Option<String> = row.get(4)?;
            let tool_calls = tool_calls_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(MessageRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_calls,
                tool_call_id: row.get(5)?,
                reasoning: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        messages.collect()
    }

    /// 获取消息数量
    pub fn message_count(&self, session_id: &str) -> SqlResult<i64> {
        let conn = self.conn();
        conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )
    }

    /// 删除会话中指定 ID 之前的消息（用于上下文压缩）
    pub fn delete_messages_before(&self, session_id: &str, before_id: i64) -> SqlResult<usize> {
        let conn = self.conn();
        let count = conn.execute(
            "DELETE FROM messages WHERE session_id = ?1 AND id < ?2",
            params![session_id, before_id],
        )?;
        if count > 0 {
            debug!("Deleted {} old messages from session {}", count, session_id);
        }
        Ok(count)
    }

    /// 删除会话中的全部消息（用于会话重写）
    pub fn delete_messages(&self, session_id: &str) -> SqlResult<usize> {
        let conn = self.conn();
        let count = conn.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        if count > 0 {
            debug!(
                "Deleted {} messages from session {} for rewrite",
                count, session_id
            );
        }
        Ok(count)
    }

    /// Rewrite the model-visible message set after context compaction.
    ///
    /// Raw transcript details should stay available through trace/artifact
    /// records; this table is the runtime continuation surface.
    pub fn rewrite_session_messages_after_compact(
        &self,
        session_id: &str,
        messages: &[MessageInsert],
    ) -> SqlResult<usize> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        for message in messages {
            let tool_calls = message
                .tool_calls
                .as_ref()
                .map(serde_json::Value::to_string);
            tx.execute(
                "INSERT INTO messages (session_id, role, content, tool_calls, tool_call_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    session_id,
                    message.role,
                    message.content,
                    tool_calls,
                    message.tool_call_id
                ],
            )?;
        }
        tx.execute(
            "UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1",
            params![session_id],
        )?;
        tx.commit()?;
        Ok(messages.len())
    }

    /// Replace all messages for a session with a new set (used after compression).
    pub fn replace_session_messages(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> SqlResult<usize> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM messages WHERE session_id = ?1", params![session_id])?;
        let mut count = 0usize;
        for msg in messages {
            let (role, content, tool_calls, tool_call_id) = match msg {
                Message::System { content } => ("system", content.as_str(), None, None),
                Message::User { content } => ("user", content.as_str(), None, None),
                Message::Assistant { content, tool_calls } => (
                    "assistant",
                    content.as_str(),
                    tool_calls.as_ref().and_then(|tc| serde_json::to_value(tc).ok()),
                    None,
                ),
                Message::Tool { tool_call_id: id, content } => {
                    ("tool", content.as_str(), None, Some(id.as_str()))
                }
            };
            tx.execute(
                "INSERT INTO messages (session_id, role, content, tool_calls, tool_call_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![session_id, role, content, tool_calls, tool_call_id],
            )?;
            count += 1;
        }
        tx.commit()?;
        debug!("Replaced {} messages for session {} after compression", count, session_id);
        Ok(count)
    }

    /// Deprecated: use restore_history instead
    pub fn restore_compacted_messages(&self, session_id: &str) -> SqlResult<Vec<MessageRecord>> {
        self.get_messages(session_id)
    }

    /// Load conversation history from DB as API Message objects.
    /// Used to restore conversation state after process restart.
    pub fn restore_history(&self, session_id: &str) -> SqlResult<Vec<Message>> {
        let records = self.get_messages(session_id)?;
        Ok(records.into_iter().map(|r| r.into_api_message()).collect())
    }
}

impl MessageRecord {
    /// Convert a persisted MessageRecord back into the API Message type.
    pub fn into_api_message(self) -> Message {
        match self.role.as_str() {
            "system" => Message::system(&self.content),
            "user" => Message::user(&self.content),
            "assistant" => {
                let tool_calls = self
                    .tool_calls
                    .and_then(|v| serde_json::from_value(v).ok());
                Message::Assistant {
                    content: self.content,
                    tool_calls,
                }
            }
            "tool" => Message::Tool {
                tool_call_id: self.tool_call_id.unwrap_or_default(),
                content: self.content,
            },
            _ => Message::user(&self.content),
        }
    }
}

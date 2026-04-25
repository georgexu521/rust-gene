//! SQLite 会话存储
//!
//! 参考 hermes-agent 的 SessionDB 设计：
//! - WAL 模式支持并发读写
//! - FTS5 全文搜索
//! - 会话链（parent_session_id 用于上下文压缩）
//! - Token 统计

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

/// 消息记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub reasoning: Option<String>,
    pub created_at: String,
}

/// 会话记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub parent_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
}

/// Durable event extracted from completed turns for future routing/tool tuning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningEventRecord {
    pub id: i64,
    pub session_id: String,
    pub kind: String,
    pub source: String,
    pub summary: String,
    pub confidence: f64,
    pub payload: serde_json::Value,
    pub created_at: String,
}

/// 会话存储
pub struct SessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SessionStore {
    /// 安全获取连接（处理 Mutex poison）
    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// 打开或创建数据库
    pub fn open(path: impl AsRef<Path>) -> SqlResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;

        // 启用 WAL 模式（并发读写）
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        //  busy_timeout 避免短暂锁竞争导致立即失败（5 秒）
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        // 运行迁移
        let mut runner = crate::migrations::MigrationRunner::new();
        runner.register(std::sync::Arc::new(
            crate::migrations::v1_initial::V1Initial,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v2_add_tasks::V2AddTasks,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v3_add_traces::V3AddTraces,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v4_add_learning_events::V4AddLearningEvents,
        ));
        runner.run(&conn)?;

        info!("SessionStore opened at {:?}", path);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 内存数据库（用于测试）
    pub fn in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        // 运行迁移
        let mut runner = crate::migrations::MigrationRunner::new();
        runner.register(std::sync::Arc::new(
            crate::migrations::v1_initial::V1Initial,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v2_add_tasks::V2AddTasks,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v3_add_traces::V3AddTraces,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v4_add_learning_events::V4AddLearningEvents,
        ));
        runner.run(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // ==================== 会话操作 ====================

    /// 创建会话
    pub fn create_session(&self, id: &str, title: &str, model: &str) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES (?1, ?2, ?3)",
            params![id, title, model],
        )?;
        debug!("Created session: {}", id);
        Ok(())
    }

    /// 获取会话
    pub fn get_session(&self, id: &str) -> SqlResult<Option<SessionRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, title, parent_session_id, created_at, updated_at, model, total_input_tokens, total_output_tokens
             FROM sessions WHERE id = ?1",
            params![id],
            |row| {
                Ok(SessionRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    parent_session_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    model: row.get(5)?,
                    total_input_tokens: row.get(6)?,
                    total_output_tokens: row.get(7)?,
                })
            },
        );

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 列出会话（最近的在前）
    pub fn list_sessions(&self, limit: i64) -> SqlResult<Vec<SessionRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, title, parent_session_id, created_at, updated_at, model, total_input_tokens, total_output_tokens
             FROM sessions ORDER BY updated_at DESC LIMIT ?1"
        )?;

        let sessions = stmt.query_map(params![limit], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                parent_session_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                model: row.get(5)?,
                total_input_tokens: row.get(6)?,
                total_output_tokens: row.get(7)?,
            })
        })?;

        sessions.collect()
    }

    /// 更新会话标题
    pub fn update_session_title(&self, id: &str, title: &str) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE sessions SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![title, id],
        )?;
        Ok(())
    }

    /// 更新 token 统计
    pub fn update_tokens(&self, id: &str, input_tokens: i64, output_tokens: i64) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE sessions SET total_input_tokens = total_input_tokens + ?1, total_output_tokens = total_output_tokens + ?2, updated_at = datetime('now') WHERE id = ?3",
            params![input_tokens, output_tokens, id],
        )?;
        Ok(())
    }

    /// 创建子会话（上下文压缩时用）
    pub fn create_child_session(
        &self,
        id: &str,
        title: &str,
        model: &str,
        parent_id: &str,
    ) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model, parent_session_id) VALUES (?1, ?2, ?3, ?4)",
            params![id, title, model, parent_id],
        )?;
        debug!("Created child session: {} (parent: {})", id, parent_id);
        Ok(())
    }

    /// 删除会话及其消息
    pub fn delete_session(&self, id: &str) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM messages WHERE session_id = ?1", params![id])?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        debug!("Deleted session: {}", id);
        Ok(())
    }

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

    // ==================== 搜索 ====================

    /// 全文搜索消息
    pub fn search_messages(&self, query: &str, limit: i64) -> SqlResult<Vec<MessageRecord>> {
        let conn = self.conn();

        // FTS5 搜索
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.content, m.tool_calls, m.tool_call_id, m.reasoning, m.created_at
             FROM messages_fts fts
             JOIN messages m ON m.id = fts.rowid
             WHERE messages_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;

        let messages = stmt.query_map(params![query, limit], |row| {
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

    // ==================== 统计 ====================

    /// 获取数据库统计
    pub fn stats(&self) -> SqlResult<DbStats> {
        let conn = self.conn();

        let session_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;

        let message_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;

        let total_input: i64 = conn.query_row(
            "SELECT COALESCE(SUM(total_input_tokens), 0) FROM sessions",
            [],
            |r| r.get(0),
        )?;

        let total_output: i64 = conn.query_row(
            "SELECT COALESCE(SUM(total_output_tokens), 0) FROM sessions",
            [],
            |r| r.get(0),
        )?;

        Ok(DbStats {
            session_count,
            message_count,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
        })
    }

    // ==================== Trace 操作 ====================

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
        for row in rows {
            if let Ok(payload) = row {
                if let Ok(event) =
                    serde_json::from_str::<crate::engine::trace::TraceEvent>(&payload)
                {
                    events.push(event);
                }
            }
        }
        trace.events = events;
        Ok(Some(trace))
    }

    // ==================== Learning Event 操作 ====================

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
        let rows = stmt.query_map(params![session_id, limit], |row| {
            let payload_text: String = row.get(6)?;
            Ok(LearningEventRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                kind: row.get(2)?,
                source: row.get(3)?,
                summary: row.get(4)?,
                confidence: row.get(5)?,
                payload: serde_json::from_str(&payload_text)
                    .unwrap_or_else(|_| serde_json::json!({})),
                created_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }
}

/// 数据库统计
#[derive(Debug, Clone)]
pub struct DbStats {
    pub session_count: i64,
    pub message_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_crud() {
        let store = SessionStore::in_memory().unwrap();

        // 创建
        store
            .create_session("s1", "Test Session", "kimi-k2.5")
            .unwrap();

        // 获取
        let session = store.get_session("s1").unwrap().unwrap();
        assert_eq!(session.title, "Test Session");
        assert_eq!(session.model, "kimi-k2.5");

        // 更新标题
        store.update_session_title("s1", "Updated Title").unwrap();
        let session = store.get_session("s1").unwrap().unwrap();
        assert_eq!(session.title, "Updated Title");

        // 列出
        let sessions = store.list_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_message_crud() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        // 添加消息
        let id = store
            .add_message("s1", "user", "Hello", None, None)
            .unwrap();
        assert!(id > 0);

        store
            .add_message("s1", "assistant", "Hi there!", None, None)
            .unwrap();

        // 获取消息
        let messages = store.get_messages("s1").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].role, "assistant");

        // 消息数量
        assert_eq!(store.message_count("s1").unwrap(), 2);
    }

    #[test]
    fn test_message_with_tool_calls() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        let tool_calls = serde_json::json!([{
            "id": "call_1",
            "name": "bash",
            "arguments": {"command": "ls"}
        }]);

        store
            .add_message(
                "s1",
                "assistant",
                "Running command...",
                Some(&tool_calls),
                None,
            )
            .unwrap();

        let messages = store.get_messages("s1").unwrap();
        assert!(messages[0].tool_calls.is_some());
    }

    #[test]
    fn test_turn_trace_persistence() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        let mut trace = crate::engine::trace::TurnTrace::new("s1", 1, "hello trace");
        trace
            .events
            .push(crate::engine::trace::TraceEvent::ToolCompleted {
                tool: "bash".to_string(),
                call_id: "call_123456".to_string(),
                success: true,
                duration_ms: Some(12),
                output_chars: 5,
            });
        trace.finish(crate::engine::trace::TurnStatus::Completed);

        store.add_turn_trace(&trace).unwrap();
        let loaded = store.latest_turn_trace("s1").unwrap().unwrap();

        assert_eq!(loaded.trace_id, trace.trace_id);
        assert_eq!(loaded.status, crate::engine::trace::TurnStatus::Completed);
        assert_eq!(loaded.events.len(), trace.events.len());
        assert_eq!(loaded.events[1].label(), "tool.done");
    }

    #[test]
    fn test_learning_event_persistence() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        let id = store
            .add_learning_event(
                "s1",
                "turn_outcome",
                "test",
                "Turn completed",
                1.2,
                &serde_json::json!({"intent": "CodeChange"}),
            )
            .unwrap();
        assert!(id > 0);

        let events = store.recent_learning_events("s1", 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "turn_outcome");
        assert_eq!(events[0].confidence, 1.0);
        assert_eq!(events[0].payload["intent"], "CodeChange");
    }

    #[test]
    fn test_child_session() {
        let store = SessionStore::in_memory().unwrap();

        store.create_session("parent", "Parent", "model").unwrap();
        store
            .add_message("parent", "user", "Old message", None, None)
            .unwrap();

        // 创建子会话（压缩后）
        store
            .create_child_session("child", "Child (compressed)", "model", "parent")
            .unwrap();

        let child = store.get_session("child").unwrap().unwrap();
        assert_eq!(child.parent_session_id, Some("parent".to_string()));
    }

    #[test]
    fn test_delete_messages_before() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        let id1 = store.add_message("s1", "user", "msg1", None, None).unwrap();
        let _id2 = store.add_message("s1", "user", "msg2", None, None).unwrap();
        let _id3 = store.add_message("s1", "user", "msg3", None, None).unwrap();

        // 删除 id1 之前的消息（实际上没有，因为 id1 是第一个）
        let deleted = store.delete_messages_before("s1", id1).unwrap();
        assert_eq!(deleted, 0);

        // 删除 id2 之前的消息（删除 id1）
        let deleted = store.delete_messages_before("s1", id1 + 1).unwrap();
        assert_eq!(deleted, 1);

        let messages = store.get_messages("s1").unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_search_messages() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        store
            .add_message(
                "s1",
                "user",
                "How do I implement authentication?",
                None,
                None,
            )
            .unwrap();
        store
            .add_message(
                "s1",
                "assistant",
                "You can use JWT tokens for auth",
                None,
                None,
            )
            .unwrap();
        store
            .add_message("s1", "user", "What about database migrations?", None, None)
            .unwrap();

        // FTS5 搜索需要一点时间来索引
        let results = store.search_messages("authentication", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_token_tracking() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        store.update_tokens("s1", 100, 50).unwrap();
        store.update_tokens("s1", 200, 80).unwrap();

        let session = store.get_session("s1").unwrap().unwrap();
        assert_eq!(session.total_input_tokens, 300);
        assert_eq!(session.total_output_tokens, 130);
    }

    #[test]
    fn test_stats() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test1", "model").unwrap();
        store.create_session("s2", "Test2", "model").unwrap();
        store
            .add_message("s1", "user", "hello", None, None)
            .unwrap();

        let stats = store.stats().unwrap();
        assert_eq!(stats.session_count, 2);
        assert_eq!(stats.message_count, 1);
    }
}

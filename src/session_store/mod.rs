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

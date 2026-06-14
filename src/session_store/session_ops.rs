use super::{SessionRecord, SessionStore};
use rusqlite::{params, Result as SqlResult};
use tracing::debug;

impl SessionStore {
    // ==================== 会话操作 ====================

    /// 创建会话
    pub fn create_session(
        &self,
        id: &str,
        title: &str,
        model: &str,
        workspace_root: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model, workspace_root) VALUES (?1, ?2, ?3, ?4)",
            params![id, title, model, workspace_root],
        )?;
        debug!("Created session: {}", id);
        Ok(())
    }

    /// 获取会话
    pub fn get_session(&self, id: &str) -> SqlResult<Option<SessionRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, title, parent_session_id, created_at, updated_at, model, total_input_tokens, total_output_tokens, workspace_root
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
                    workspace_root: row.get(8)?,
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
            "SELECT id, title, parent_session_id, created_at, updated_at, model, total_input_tokens, total_output_tokens, workspace_root
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
                workspace_root: row.get(8)?,
            })
        })?;

        sessions.collect()
    }

    /// 列出所有非空 workspace_root（按最近使用时间排序）。
    pub fn list_workspaces(&self) -> SqlResult<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT workspace_root FROM sessions
             WHERE workspace_root IS NOT NULL AND workspace_root != ''
             ORDER BY MAX(updated_at) OVER (PARTITION BY workspace_root) DESC",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    /// 列出指定 workspace 下的会话（最近的在前）。
    pub fn list_sessions_by_workspace(
        &self,
        workspace_root: &str,
        limit: i64,
    ) -> SqlResult<Vec<SessionRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, title, parent_session_id, created_at, updated_at, model, total_input_tokens, total_output_tokens, workspace_root
             FROM sessions
             WHERE workspace_root = ?1
                OR (workspace_root IS NULL OR workspace_root = '')
             ORDER BY updated_at DESC LIMIT ?2"
        )?;

        let sessions = stmt.query_map(params![workspace_root, limit], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                parent_session_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                model: row.get(5)?,
                total_input_tokens: row.get(6)?,
                total_output_tokens: row.get(7)?,
                workspace_root: row.get(8)?,
            })
        })?;

        sessions.collect()
    }

    /// 回填缺失的 workspace_root 为给定值。
    pub fn backfill_workspace_root(&self, workspace_root: &str) -> SqlResult<usize> {
        let conn = self.conn();
        conn.execute(
            "UPDATE sessions SET workspace_root = ?1
             WHERE workspace_root IS NULL OR workspace_root = ''",
            params![workspace_root],
        )
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
        workspace_root: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model, parent_session_id, workspace_root) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, model, parent_id, workspace_root],
        )?;
        debug!("Created child session: {} (parent: {})", id, parent_id);
        Ok(())
    }

    /// 删除会话及其消息
    pub fn delete_session(&self, id: &str) -> SqlResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM trace_events WHERE trace_id IN (
                SELECT trace_id FROM turn_traces WHERE session_id = ?1
            )",
            params![id],
        )?;
        tx.execute("DELETE FROM turn_traces WHERE session_id = ?1", params![id])?;
        tx.execute(
            "DELETE FROM learning_events WHERE session_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM agent_task_states WHERE session_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM agent_artifacts WHERE session_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM compact_boundaries WHERE session_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM messages WHERE session_id = ?1", params![id])?;
        tx.execute(
            "UPDATE sessions SET parent_session_id = NULL WHERE parent_session_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        tx.commit()?;
        debug!("Deleted session: {}", id);
        Ok(())
    }
}

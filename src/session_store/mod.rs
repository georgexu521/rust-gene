//! SQLite 会话存储
//!
//! 参考 hermes-agent 的 SessionDB 设计：
//! - WAL 模式支持并发读写
//! - FTS5 全文搜索
//! - 会话链（parent_session_id 用于上下文压缩）
//! - Token 统计

use rusqlite::{params, Connection, Result as SqlResult, Row};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInsert {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
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

/// Durable compact boundary produced when earlier context is summarized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundaryRecord {
    pub id: i64,
    pub session_id: String,
    pub boundary_id: String,
    pub sequence: Option<i64>,
    pub strategy: String,
    pub trigger: Option<String>,
    pub before_tokens: i64,
    pub after_tokens: i64,
    pub messages_before: i64,
    pub messages_after: i64,
    pub preserved_tail_count: Option<i64>,
    pub retained_items: serde_json::Value,
    pub provenance: serde_json::Value,
    pub summary: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundaryInsert {
    pub session_id: String,
    pub boundary_id: String,
    pub sequence: Option<i64>,
    pub strategy: String,
    pub trigger: Option<String>,
    pub before_tokens: i64,
    pub after_tokens: i64,
    pub messages_before: i64,
    pub messages_after: i64,
    pub preserved_tail_count: Option<i64>,
    pub retained_items: serde_json::Value,
    pub provenance: serde_json::Value,
    pub summary: String,
    pub payload: serde_json::Value,
}

/// Durable subagent result artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArtifactRecord {
    pub id: i64,
    pub session_id: String,
    pub agent_id: String,
    pub profile: Option<String>,
    pub role: String,
    pub status: String,
    pub description: String,
    pub output: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

/// Durable subagent task state for background/runtime panels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskStateRecord {
    pub id: i64,
    pub session_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub profile: Option<String>,
    pub role: String,
    pub status: String,
    pub description: String,
    pub transcript_path: Option<String>,
    pub tool_ids_in_progress: Vec<String>,
    pub permission_requests: Vec<String>,
    pub result_artifact_id: Option<i64>,
    pub cleanup_hooks: Vec<String>,
    pub payload: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

/// Upsert payload for durable subagent task state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskStateUpsert {
    pub session_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub profile: Option<String>,
    pub role: String,
    pub status: String,
    pub description: String,
    pub transcript_path: Option<String>,
    pub tool_ids_in_progress: Vec<String>,
    pub permission_requests: Vec<String>,
    pub result_artifact_id: Option<i64>,
    pub cleanup_hooks: Vec<String>,
    pub payload: serde_json::Value,
}

/// 会话存储
#[derive(Clone)]
pub struct SessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SessionStore {
    /// 安全获取连接（处理 Mutex poison）
    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 默认会话数据库路径。
    ///
    /// CLI、引擎和会话面板必须使用同一个路径，否则 `/sessions`、
    /// trace/learning events 与实际对话历史会被拆到不同数据库。
    pub fn default_path() -> std::path::PathBuf {
        dirs::data_dir()
            .map(|d| d.join("priority-agent").join("sessions.db"))
            .unwrap_or_else(|| std::path::PathBuf::from(".priority-agent/sessions.db"))
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
        runner.register(std::sync::Arc::new(
            crate::migrations::v5_add_agent_artifacts::V5AddAgentArtifacts,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v6_add_agent_task_states::V6AddAgentTaskStates,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v7_add_compact_boundaries::V7AddCompactBoundaries,
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
        runner.register(std::sync::Arc::new(
            crate::migrations::v5_add_agent_artifacts::V5AddAgentArtifacts,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v6_add_agent_task_states::V6AddAgentTaskStates,
        ));
        runner.register(std::sync::Arc::new(
            crate::migrations::v7_add_compact_boundaries::V7AddCompactBoundaries,
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

    /// Restore the compacted runtime continuation surface for a session.
    pub fn restore_compacted_messages(&self, session_id: &str) -> SqlResult<Vec<MessageRecord>> {
        self.get_messages(session_id)
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
                "SELECT id, title, parent_session_id, created_at, updated_at, model, total_input_tokens, total_output_tokens
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
                "SELECT DISTINCT s.id, s.title, s.parent_session_id, s.created_at, s.updated_at, s.model, s.total_input_tokens, s.total_output_tokens
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

    // ==================== Compact Boundary 操作 ====================

    pub fn add_compact_boundary(&self, boundary: &CompactBoundaryInsert) -> SqlResult<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO compact_boundaries
             (session_id, boundary_id, sequence, strategy, trigger, before_tokens, after_tokens,
              messages_before, messages_after, preserved_tail_count, retained_items, provenance,
              summary, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(session_id, boundary_id) DO UPDATE SET
               sequence = excluded.sequence,
               strategy = excluded.strategy,
               trigger = excluded.trigger,
               before_tokens = excluded.before_tokens,
               after_tokens = excluded.after_tokens,
               messages_before = excluded.messages_before,
               messages_after = excluded.messages_after,
               preserved_tail_count = excluded.preserved_tail_count,
               retained_items = excluded.retained_items,
               provenance = excluded.provenance,
               summary = excluded.summary,
               payload = excluded.payload",
            params![
                &boundary.session_id,
                &boundary.boundary_id,
                boundary.sequence,
                &boundary.strategy,
                &boundary.trigger,
                boundary.before_tokens,
                boundary.after_tokens,
                boundary.messages_before,
                boundary.messages_after,
                boundary.preserved_tail_count,
                boundary.retained_items.to_string(),
                boundary.provenance.to_string(),
                boundary.summary,
                boundary.payload.to_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_compact_boundaries(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<CompactBoundaryRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, boundary_id, sequence, strategy, trigger,
                    before_tokens, after_tokens, messages_before, messages_after,
                    preserved_tail_count, retained_items, provenance, summary, payload, created_at
             FROM compact_boundaries
             WHERE session_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], compact_boundary_from_row)?;
        rows.collect()
    }

    pub fn latest_compact_boundary(
        &self,
        session_id: &str,
    ) -> SqlResult<Option<CompactBoundaryRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, boundary_id, sequence, strategy, trigger,
                    before_tokens, after_tokens, messages_before, messages_after,
                    preserved_tail_count, retained_items, provenance, summary, payload, created_at
             FROM compact_boundaries
             WHERE session_id = ?1
             ORDER BY id DESC
             LIMIT 1",
            params![session_id],
            compact_boundary_from_row,
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn add_compact_boundary_from_runtime_record(
        &self,
        session_id: &str,
        record: &crate::engine::context_collapse::CompactionRuntimeRecord,
        trigger: Option<&str>,
        summary: &str,
    ) -> SqlResult<i64> {
        let boundary_id = record
            .boundary_id
            .clone()
            .unwrap_or_else(|| format!("compact-{}", uuid::Uuid::new_v4()));
        self.add_compact_boundary(&CompactBoundaryInsert {
            session_id: session_id.to_string(),
            boundary_id,
            sequence: record.sequence.map(i64::from),
            strategy: record.strategy.label().to_string(),
            trigger: trigger
                .map(str::to_string)
                .or_else(|| record.trigger.clone()),
            before_tokens: i64::try_from(record.tokens_before).unwrap_or(i64::MAX),
            after_tokens: i64::try_from(record.tokens_after).unwrap_or(i64::MAX),
            messages_before: i64::try_from(record.messages_before).unwrap_or(i64::MAX),
            messages_after: i64::try_from(record.messages_after).unwrap_or(i64::MAX),
            preserved_tail_count: record
                .preserved_tail_count
                .and_then(|count| i64::try_from(count).ok()),
            retained_items: serde_json::json!(record.retained_items),
            provenance: serde_json::json!(record.provenance),
            summary: summary.to_string(),
            payload: serde_json::to_value(record).unwrap_or_else(|_| serde_json::json!({})),
        })
    }

    // ==================== Agent Artifact 操作 ====================

    #[allow(clippy::too_many_arguments)]
    pub fn add_agent_artifact(
        &self,
        session_id: &str,
        agent_id: &str,
        profile: Option<&str>,
        role: &str,
        status: &str,
        description: &str,
        output: &str,
        payload: &serde_json::Value,
    ) -> SqlResult<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO agent_artifacts
             (session_id, agent_id, profile, role, status, description, output, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session_id,
                agent_id,
                profile,
                role,
                status,
                description,
                output,
                payload.to_string()
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn recent_agent_artifacts(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<AgentArtifactRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, agent_id, profile, role, status, description, output, payload, created_at
             FROM agent_artifacts
             WHERE session_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], |row| {
            let payload_text: String = row.get(8)?;
            Ok(AgentArtifactRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                agent_id: row.get(2)?,
                profile: row.get(3)?,
                role: row.get(4)?,
                status: row.get(5)?,
                description: row.get(6)?,
                output: row.get(7)?,
                payload: serde_json::from_str(&payload_text)
                    .unwrap_or_else(|_| serde_json::json!({})),
                created_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    pub fn agent_artifact(
        &self,
        session_id: &str,
        artifact_id: i64,
    ) -> SqlResult<Option<AgentArtifactRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, agent_id, profile, role, status, description, output, payload, created_at
             FROM agent_artifacts
             WHERE session_id = ?1 AND id = ?2
             LIMIT 1",
            params![session_id, artifact_id],
            |row| {
                let payload_text: String = row.get(8)?;
                Ok(AgentArtifactRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    agent_id: row.get(2)?,
                    profile: row.get(3)?,
                    role: row.get(4)?,
                    status: row.get(5)?,
                    description: row.get(6)?,
                    output: row.get(7)?,
                    payload: serde_json::from_str(&payload_text)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    created_at: row.get(9)?,
                })
            },
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }

    // ==================== Agent Task State 操作 ====================

    pub fn upsert_agent_task_state(&self, state: &AgentTaskStateUpsert) -> SqlResult<()> {
        let conn = self.conn();
        let tool_ids =
            serde_json::to_string(&state.tool_ids_in_progress).unwrap_or_else(|_| "[]".to_string());
        let permission_requests =
            serde_json::to_string(&state.permission_requests).unwrap_or_else(|_| "[]".to_string());
        let cleanup_hooks =
            serde_json::to_string(&state.cleanup_hooks).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO agent_task_states
             (session_id, task_id, agent_id, profile, role, status, description, transcript_path,
              tool_ids_in_progress, permission_requests, result_artifact_id, cleanup_hooks, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(session_id, task_id) DO UPDATE SET
                agent_id = excluded.agent_id,
                profile = excluded.profile,
                role = excluded.role,
                status = excluded.status,
                description = excluded.description,
                transcript_path = excluded.transcript_path,
                tool_ids_in_progress = excluded.tool_ids_in_progress,
                permission_requests = excluded.permission_requests,
                result_artifact_id = excluded.result_artifact_id,
                cleanup_hooks = excluded.cleanup_hooks,
                payload = excluded.payload,
                updated_at = datetime('now')",
            params![
                &state.session_id,
                &state.task_id,
                &state.agent_id,
                state.profile.as_deref(),
                &state.role,
                &state.status,
                &state.description,
                state.transcript_path.as_deref(),
                tool_ids,
                permission_requests,
                state.result_artifact_id,
                cleanup_hooks,
                state.payload.to_string()
            ],
        )?;
        Ok(())
    }

    pub fn recent_agent_task_states(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<AgentTaskStateRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, task_id, agent_id, profile, role, status, description,
                    transcript_path, tool_ids_in_progress, permission_requests,
                    result_artifact_id, cleanup_hooks, payload, created_at, updated_at
             FROM agent_task_states
             WHERE session_id = ?1
             ORDER BY updated_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], |row| {
            let tool_ids: String = row.get(9)?;
            let permission_requests: String = row.get(10)?;
            let cleanup_hooks: String = row.get(12)?;
            let payload_text: String = row.get(13)?;
            Ok(AgentTaskStateRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                task_id: row.get(2)?,
                agent_id: row.get(3)?,
                profile: row.get(4)?,
                role: row.get(5)?,
                status: row.get(6)?,
                description: row.get(7)?,
                transcript_path: row.get(8)?,
                tool_ids_in_progress: serde_json::from_str(&tool_ids).unwrap_or_default(),
                permission_requests: serde_json::from_str(&permission_requests).unwrap_or_default(),
                result_artifact_id: row.get(11)?,
                cleanup_hooks: serde_json::from_str(&cleanup_hooks).unwrap_or_default(),
                payload: serde_json::from_str(&payload_text)
                    .unwrap_or_else(|_| serde_json::json!({})),
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })?;
        rows.collect()
    }

    pub fn agent_task_state(
        &self,
        session_id: &str,
        agent_id_or_task_id: &str,
    ) -> SqlResult<Option<AgentTaskStateRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, task_id, agent_id, profile, role, status, description,
                    transcript_path, tool_ids_in_progress, permission_requests,
                    result_artifact_id, cleanup_hooks, payload, created_at, updated_at
             FROM agent_task_states
             WHERE session_id = ?1 AND (agent_id = ?2 OR task_id = ?2)
             ORDER BY updated_at DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![session_id, agent_id_or_task_id])?;
        if let Some(row) = rows.next()? {
            let tool_ids: String = row.get(9)?;
            let permission_requests: String = row.get(10)?;
            let cleanup_hooks: String = row.get(12)?;
            let payload_text: String = row.get(13)?;
            Ok(Some(AgentTaskStateRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                task_id: row.get(2)?,
                agent_id: row.get(3)?,
                profile: row.get(4)?,
                role: row.get(5)?,
                status: row.get(6)?,
                description: row.get(7)?,
                transcript_path: row.get(8)?,
                tool_ids_in_progress: serde_json::from_str(&tool_ids).unwrap_or_default(),
                permission_requests: serde_json::from_str(&permission_requests).unwrap_or_default(),
                result_artifact_id: row.get(11)?,
                cleanup_hooks: serde_json::from_str(&cleanup_hooks).unwrap_or_default(),
                payload: serde_json::from_str(&payload_text).unwrap_or_default(),
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            }))
        } else {
            Ok(None)
        }
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

fn session_from_row(row: &Row<'_>) -> SqlResult<SessionRecord> {
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

fn compact_boundary_from_row(row: &Row<'_>) -> SqlResult<CompactBoundaryRecord> {
    let retained_items_text: String = row.get(11)?;
    let provenance_text: String = row.get(12)?;
    let payload_text: String = row.get(14)?;
    Ok(CompactBoundaryRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        boundary_id: row.get(2)?,
        sequence: row.get(3)?,
        strategy: row.get(4)?,
        trigger: row.get(5)?,
        before_tokens: row.get(6)?,
        after_tokens: row.get(7)?,
        messages_before: row.get(8)?,
        messages_after: row.get(9)?,
        preserved_tail_count: row.get(10)?,
        retained_items: serde_json::from_str(&retained_items_text)
            .unwrap_or_else(|_| serde_json::json!([])),
        provenance: serde_json::from_str(&provenance_text)
            .unwrap_or_else(|_| serde_json::json!([])),
        summary: row.get(13)?,
        payload: serde_json::from_str(&payload_text).unwrap_or_else(|_| serde_json::json!({})),
        created_at: row.get(15)?,
    })
}

fn fts_phrase_terms(query: &str) -> String {
    let terms = query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    if terms.is_empty() {
        "\"\"".to_string()
    } else {
        terms.join(" ")
    }
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
    fn test_recent_turn_traces_are_session_scoped_newest_first() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();
        store.create_session("s2", "Other", "model").unwrap();

        for turn_index in 1..=3 {
            let mut trace = crate::engine::trace::TurnTrace::new("s1", turn_index, "session one");
            trace
                .events
                .push(crate::engine::trace::TraceEvent::FinalCloseoutPrepared {
                    status: "passed".to_string(),
                    terminal_status: Some("completed".to_string()),
                    stop_reason: None,
                    stop_action: None,
                    failure_type: None,
                    recovery_plan_id: None,
                    rollback_status: None,
                    changed_files: 1,
                    validation_items: 1,
                    tool_records: turn_index as usize,
                    tool_evidence: Some(format!("tool evidence: records={}", turn_index)),
                    verification_proof_status: Some("verified".to_string()),
                    verification_proof_summary: Some("validation passed".to_string()),
                    acceptance_items: 1,
                    residual_risks: 0,
                });
            trace.finish(crate::engine::trace::TurnStatus::Completed);
            store.add_turn_trace(&trace).unwrap();
        }

        let mut other_trace = crate::engine::trace::TurnTrace::new("s2", 10, "session two");
        other_trace.finish(crate::engine::trace::TurnStatus::Completed);
        store.add_turn_trace(&other_trace).unwrap();

        let traces = store.recent_turn_traces("s1", 2).unwrap();

        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].session_id, "s1");
        assert_eq!(traces[0].turn_index, 3);
        assert_eq!(traces[1].turn_index, 2);
        assert!(
            crate::engine::trace::format_trace_recent_line(&traces[0]).contains("tool_records=3")
        );
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
    fn test_context_ledger_event_queries() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();
        store.create_session("s2", "Other", "model").unwrap();

        store
            .add_learning_event(
                "s1",
                crate::engine::context_ledger::CONTEXT_LEDGER_FILE_READ_KIND,
                "file_read",
                "Read README.md",
                1.0,
                &serde_json::json!({
                    "path": "README.md",
                    "resolved_path": "/tmp/project/README.md",
                    "content_hash": "a",
                    "size_bytes": 12,
                    "total_lines": 2,
                    "displayed_lines": 2,
                    "line_start": 1,
                    "line_end": 2,
                    "targeted_read": false,
                    "truncated": false
                }),
            )
            .unwrap();
        store
            .add_learning_event(
                "s2",
                crate::engine::context_ledger::CONTEXT_LEDGER_FILE_READ_KIND,
                "file_read",
                "Read README.md elsewhere",
                1.0,
                &serde_json::json!({"resolved_path": "/tmp/project/README.md"}),
            )
            .unwrap();

        let ledger = store.recent_context_ledger_events("s1", 10).unwrap();
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].summary, "Read README.md");

        let latest = store
            .latest_file_read_context_event("s1", "/tmp/project/README.md")
            .unwrap()
            .expect("latest file read");
        assert_eq!(latest.session_id, "s1");
        assert_eq!(latest.payload["content_hash"], "a");
    }

    #[test]
    fn test_agent_artifact_persistence() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        let id = store
            .add_agent_artifact(
                "s1",
                "agent_123",
                Some("verifier"),
                "Verifier",
                "completed",
                "check the patch",
                "looks good",
                &serde_json::json!({"confidence": 0.9}),
            )
            .unwrap();
        assert!(id > 0);

        let artifacts = store.recent_agent_artifacts("s1", 10).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].agent_id, "agent_123");
        assert_eq!(artifacts[0].profile.as_deref(), Some("verifier"));
        assert_eq!(artifacts[0].payload["confidence"], 0.9);

        let artifact = store.agent_artifact("s1", id).unwrap().unwrap();
        assert_eq!(artifact.id, id);
        assert_eq!(artifact.output, "looks good");
        assert!(store.agent_artifact("s1", id + 1).unwrap().is_none());
    }

    #[test]
    fn test_agent_task_state_upsert() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        let state = AgentTaskStateUpsert {
            session_id: "s1".to_string(),
            task_id: "task_123".to_string(),
            agent_id: "agent_123".to_string(),
            profile: Some("implementer".to_string()),
            role: "specialist".to_string(),
            status: "running".to_string(),
            description: "edit focused files".to_string(),
            transcript_path: Some("/tmp/a2a.jsonl".to_string()),
            tool_ids_in_progress: vec!["bash_1".to_string()],
            permission_requests: vec!["file_write".to_string()],
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: serde_json::json!({"context_mode": "full_fork"}),
        };
        store.upsert_agent_task_state(&state).unwrap();

        let artifact_id = store
            .add_agent_artifact(
                "s1",
                "agent_123",
                Some("implementer"),
                "specialist",
                "completed",
                "edit focused files",
                "done",
                &serde_json::json!({}),
            )
            .unwrap();
        let mut completed = state.clone();
        completed.status = "completed".to_string();
        completed.tool_ids_in_progress = Vec::new();
        completed.result_artifact_id = Some(artifact_id);
        store.upsert_agent_task_state(&completed).unwrap();

        let states = store.recent_agent_task_states("s1", 10).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].status, "completed");
        assert_eq!(states[0].profile.as_deref(), Some("implementer"));
        assert_eq!(states[0].result_artifact_id, Some(artifact_id));
        assert_eq!(states[0].cleanup_hooks, vec!["worktree_cleanup"]);
        assert_eq!(states[0].payload["context_mode"], "full_fork");

        let by_agent = store.agent_task_state("s1", "agent_123").unwrap().unwrap();
        assert_eq!(by_agent.status, "completed");
        let by_task = store.agent_task_state("s1", "task_123").unwrap().unwrap();
        assert_eq!(by_task.agent_id, "agent_123");
        assert!(store.agent_task_state("s1", "missing").unwrap().is_none());
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
    fn test_delete_session_removes_related_runtime_records() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();
        store
            .create_child_session("child", "Child", "model", "s1")
            .unwrap();
        store
            .add_message("s1", "user", "hello", None, None)
            .unwrap();

        let mut trace = crate::engine::trace::TurnTrace::new("s1", 1, "delete me");
        trace.finish(crate::engine::trace::TurnStatus::Completed);
        store.add_turn_trace(&trace).unwrap();
        store
            .add_learning_event(
                "s1",
                "turn_outcome",
                "test",
                "summary",
                1.0,
                &serde_json::json!({}),
            )
            .unwrap();
        let artifact_id = store
            .add_agent_artifact(
                "s1",
                "agent_123",
                None,
                "worker",
                "completed",
                "desc",
                "output",
                &serde_json::json!({}),
            )
            .unwrap();
        store
            .upsert_agent_task_state(&AgentTaskStateUpsert {
                session_id: "s1".to_string(),
                task_id: "task_123".to_string(),
                agent_id: "agent_123".to_string(),
                profile: None,
                role: "worker".to_string(),
                status: "completed".to_string(),
                description: "desc".to_string(),
                transcript_path: None,
                tool_ids_in_progress: Vec::new(),
                permission_requests: Vec::new(),
                result_artifact_id: Some(artifact_id),
                cleanup_hooks: Vec::new(),
                payload: serde_json::json!({}),
            })
            .unwrap();

        store.delete_session("s1").unwrap();

        assert!(store.get_session("s1").unwrap().is_none());
        assert_eq!(store.get_messages("s1").unwrap().len(), 0);
        assert!(store.latest_turn_trace("s1").unwrap().is_none());
        assert_eq!(store.recent_learning_events("s1", 10).unwrap().len(), 0);
        assert_eq!(store.recent_agent_artifacts("s1", 10).unwrap().len(), 0);
        assert_eq!(store.recent_agent_task_states("s1", 10).unwrap().len(), 0);
        assert_eq!(store.list_compact_boundaries("s1", 10).unwrap().len(), 0);
        assert!(store
            .get_session("child")
            .unwrap()
            .unwrap()
            .parent_session_id
            .is_none());
    }

    #[test]
    fn test_compact_boundary_persistence() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();

        let id = store
            .add_compact_boundary(&CompactBoundaryInsert {
                session_id: "s1".to_string(),
                boundary_id: "boundary-1".to_string(),
                sequence: Some(1),
                strategy: "auto_compact".to_string(),
                trigger: Some("preflight".to_string()),
                before_tokens: 90_000,
                after_tokens: 20_000,
                messages_before: 30,
                messages_after: 5,
                preserved_tail_count: Some(4),
                retained_items: serde_json::json!(["changed_files:1"]),
                provenance: serde_json::json!(["trigger:preflight"]),
                summary: "Compacted previous work".to_string(),
                payload: serde_json::json!({"pressure": "high"}),
            })
            .unwrap();
        assert!(id >= 0);

        let latest = store.latest_compact_boundary("s1").unwrap().unwrap();
        assert_eq!(latest.boundary_id, "boundary-1");
        assert_eq!(latest.strategy, "auto_compact");
        assert_eq!(latest.before_tokens, 90_000);
        assert_eq!(
            latest.retained_items,
            serde_json::json!(["changed_files:1"])
        );

        let listed = store.list_compact_boundaries("s1", 10).unwrap();
        assert_eq!(listed.len(), 1);
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
    fn test_rewrite_and_restore_compacted_messages() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();
        store.add_message("s1", "user", "old", None, None).unwrap();
        store
            .add_compact_boundary(&CompactBoundaryInsert {
                session_id: "s1".to_string(),
                boundary_id: "boundary-restore".to_string(),
                sequence: Some(1),
                strategy: "auto_compact".to_string(),
                trigger: Some("test".to_string()),
                before_tokens: 1_000,
                after_tokens: 100,
                messages_before: 4,
                messages_after: 2,
                preserved_tail_count: Some(1),
                retained_items: serde_json::json!(["README.md"]),
                provenance: serde_json::json!({}),
                summary: "summary".to_string(),
                payload: serde_json::json!({}),
            })
            .unwrap();

        let count = store
            .rewrite_session_messages_after_compact(
                "s1",
                &[
                    MessageInsert {
                        role: "system".to_string(),
                        content: "compact boundary summary".to_string(),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    MessageInsert {
                        role: "user".to_string(),
                        content: "continue".to_string(),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                ],
            )
            .unwrap();
        assert_eq!(count, 2);

        let restored = store.restore_compacted_messages("s1").unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].content, "compact boundary summary");
        assert_eq!(
            store
                .latest_compact_boundary("s1")
                .unwrap()
                .unwrap()
                .boundary_id,
            "boundary-restore"
        );
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
    fn test_search_sessions_matches_title_and_message_fts() {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Auth Plan", "model").unwrap();
        store
            .create_session("s2", "Migration Notes", "model")
            .unwrap();
        store
            .add_message("s2", "user", "How should I implement oauth?", None, None)
            .unwrap();

        let title_results = store.search_sessions("Auth", 10).unwrap();
        assert_eq!(title_results[0].id, "s1");

        let message_results = store.search_sessions("oauth", 10).unwrap();
        assert_eq!(message_results[0].id, "s2");
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

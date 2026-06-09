//! TUI 会话管理器
//!
//! 管理当前会话的持久化和恢复

pub use crate::session_store::SessionRecord;
use crate::session_store::{PersistedSessionPart, SessionEventRow, SessionStore};
use crate::state::{MessageItem, MessageRole};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

/// 编辑记录
#[derive(Debug, Clone)]
pub struct EditRecord {
    pub timestamp: String,
    pub file_path: String,
    pub tool_name: String,
    pub snapshot_dir: String,
    pub snapshot_file: String,
}

impl EditRecord {
    /// 获取快照的完整路径
    pub fn snapshot_path(&self) -> PathBuf {
        PathBuf::from(&self.snapshot_dir).join(&self.snapshot_file)
    }
}

/// 会话管理器
pub struct TuiSessionManager {
    store: Arc<SessionStore>,
    current_session_id: Option<String>,
    current_session_title: String,
}

impl TuiSessionManager {
    /// 创建新的会话管理器
    pub fn new() -> anyhow::Result<Self> {
        let db_path = Self::db_path()?;
        let store = Arc::new(SessionStore::open(&db_path)?);

        info!("SessionManager initialized at {:?}", db_path);

        Ok(Self {
            store,
            current_session_id: None,
            current_session_title: String::new(),
        })
    }

    /// 内存模式（用于测试）
    pub fn in_memory() -> anyhow::Result<Self> {
        let store = Arc::new(SessionStore::in_memory()?);
        Ok(Self {
            store,
            current_session_id: None,
            current_session_title: String::new(),
        })
    }

    /// 使用已有 SessionStore 和会话 ID 创建管理器。
    ///
    /// 这用于 Priority Agent CLI 复用 StreamingQueryEngine 的持久化会话，
    /// 避免 UI 历史、trace、learning events 写入不同会话。
    pub fn from_store(
        store: Arc<SessionStore>,
        session_id: impl Into<String>,
        title: impl Into<String>,
        model: &str,
    ) -> anyhow::Result<Self> {
        let session_id = session_id.into();
        let title = title.into();
        if store.get_session(&session_id)?.is_none() {
            store.create_session(&session_id, &title, model)?;
        }
        Ok(Self {
            store,
            current_session_id: Some(session_id),
            current_session_title: title,
        })
    }

    /// 当前管理器是否绑定到给定会话。
    pub fn is_current_session(&self, session_id: &str) -> bool {
        self.current_session_id.as_deref() == Some(session_id)
    }

    /// 获取数据库路径
    fn db_path() -> anyhow::Result<PathBuf> {
        Ok(SessionStore::default_path())
    }

    /// 开始新会话
    pub fn start_session(
        &mut self,
        title: impl Into<String>,
        model: &str,
    ) -> anyhow::Result<String> {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        let title = title.into();

        self.store.create_session(&session_id, &title, model)?;
        self.current_session_id = Some(session_id.clone());
        self.current_session_title = title;

        info!("Started new session: {}", session_id);
        Ok(session_id)
    }

    /// 获取当前会话 ID
    pub fn current_session_id(&self) -> Option<&str> {
        self.current_session_id.as_deref()
    }

    /// 获取底层会话存储，供工具上下文复用当前 TUI 会话数据。
    pub fn store(&self) -> Arc<SessionStore> {
        self.store.clone()
    }

    /// 获取当前会话标题
    pub fn current_session_title(&self) -> &str {
        &self.current_session_title
    }

    /// 更新当前会话标题
    pub fn update_title(&mut self, title: impl Into<String>) -> anyhow::Result<()> {
        let title = title.into();
        if let Some(ref id) = self.current_session_id {
            self.store.update_session_title(id, &title)?;
            self.current_session_title = title;
        }
        Ok(())
    }

    /// 更新指定会话标题
    pub fn update_session_title(&self, session_id: &str, title: &str) -> anyhow::Result<()> {
        self.store.update_session_title(session_id, title)?;
        Ok(())
    }

    /// 添加消息到当前会话
    pub fn add_message(&self, role: MessageRole, content: &str) -> anyhow::Result<i64> {
        let session_id = self
            .current_session_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        let role_str = match role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        };

        let id = self
            .store
            .add_message(session_id, role_str, content, None, None)?;
        debug!("Added message {} to session {}", id, session_id);
        Ok(id)
    }

    /// 保存消息列表到当前会话
    pub fn save_messages(&self, messages: &[MessageItem]) -> anyhow::Result<()> {
        let session_id = self
            .current_session_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        // 清空现有消息（如果存在）
        // 注意：这里我们不删除消息，而是追加新的
        // 如果需要完全替换，可以先删除旧消息

        for msg in messages {
            let role_str = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
                MessageRole::Tool => "tool",
            };

            // 检查消息是否已存在（通过内容简单判断）
            // 实际应用中可能需要更好的去重策略
            self.store
                .add_message(session_id, role_str, &msg.content, None, None)?;
        }

        info!(
            "Saved {} messages to session {}",
            messages.len(),
            session_id
        );
        Ok(())
    }

    /// 用给定消息完整替换会话消息（先删后写）
    pub fn replace_messages(
        &self,
        session_id: &str,
        messages: &[MessageItem],
    ) -> anyhow::Result<()> {
        self.store.delete_messages(session_id)?;
        for msg in messages {
            let role_str = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
                MessageRole::Tool => "tool",
            };
            self.store
                .add_message(session_id, role_str, &msg.content, None, None)?;
        }
        Ok(())
    }

    /// 加载会话消息
    pub fn load_messages(&self, session_id: &str) -> anyhow::Result<Vec<MessageItem>> {
        let records = self.store.get_messages(session_id)?;

        let messages: Vec<MessageItem> = records
            .into_iter()
            .map(|record| {
                let role = match record.role.as_str() {
                    "user" => MessageRole::User,
                    "assistant" => MessageRole::Assistant,
                    "system" => MessageRole::System,
                    "tool" => MessageRole::Tool,
                    _ => MessageRole::System,
                };

                MessageItem {
                    id: format!("msg_{}", record.id),
                    role,
                    content: record.content,
                    timestamp: std::time::SystemTime::now(), // 简化处理
                    metadata: Default::default(),
                }
            })
            .collect();

        Ok(messages)
    }

    /// 加载会话消息为 API 消息格式（用于恢复引擎对话历史）
    pub fn load_api_messages(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Vec<crate::services::api::Message>> {
        let records = self.store.get_messages(session_id)?;
        let mut messages = Vec::with_capacity(records.len());

        for record in records {
            let msg = match record.role.as_str() {
                "user" => crate::services::api::Message::user(record.content),
                "assistant" => {
                    let tool_calls = record.tool_calls.and_then(|v| {
                        if v.is_array() {
                            serde_json::from_value::<Vec<crate::services::api::ToolCall>>(v).ok()
                        } else {
                            None
                        }
                    });
                    if let Some(tool_calls) = tool_calls {
                        crate::services::api::Message::assistant_with_tools(
                            record.content,
                            tool_calls,
                        )
                    } else {
                        crate::services::api::Message::assistant(record.content)
                    }
                }
                "tool" => crate::services::api::Message::tool(
                    record.tool_call_id.unwrap_or_default(),
                    record.content,
                ),
                _ => crate::services::api::Message::system(record.content),
            };
            messages.push(msg);
        }

        Ok(messages)
    }

    pub fn load_session_parts(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Vec<PersistedSessionPart>> {
        Ok(self.store.get_session_parts(session_id)?)
    }

    pub fn write_session_event(
        &self,
        session_id: &str,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let writer =
            crate::session_store::SessionEventWriter::new(self.store.shared_conn(), session_id);
        writer.write_event(event_type, &payload.to_string())?;
        Ok(())
    }

    pub fn load_session_events(&self, session_id: &str) -> anyhow::Result<Vec<SessionEventRow>> {
        let conn = self.store.shared_conn();
        let conn = conn.lock().unwrap_or_else(|err| err.into_inner());
        Ok(crate::session_store::query_session_events(
            &conn, session_id, None,
        )?)
    }

    pub fn pending_session_inputs(
        &self,
    ) -> anyhow::Result<Vec<crate::engine::run_coordinator::SessionInputRecord>> {
        let session_id = self
            .current_session_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;
        let conn = self.store.shared_conn();
        let conn = conn.lock().unwrap_or_else(|err| err.into_inner());
        Ok(crate::engine::run_coordinator::list_pending_session_inputs(
            &conn, session_id, 20,
        )?)
    }

    pub fn cancel_session_input(&self, id_or_prompt_id: &str) -> anyhow::Result<bool> {
        let session_id = self
            .current_session_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;
        let conn = self.store.shared_conn();
        let conn = conn.lock().unwrap_or_else(|err| err.into_inner());
        Ok(crate::engine::run_coordinator::cancel_session_input(
            &conn,
            session_id,
            id_or_prompt_id,
        )?)
    }

    pub fn record_session_revert(
        &self,
        insert: &crate::session_store::SessionRevertInsert,
    ) -> anyhow::Result<crate::session_store::SessionRevertRecord> {
        Ok(self.store.record_session_revert(insert)?)
    }

    pub fn mark_latest_revert_unreverted(&self, session_id: &str) -> anyhow::Result<bool> {
        Ok(self.store.mark_latest_revert_unreverted(session_id)?)
    }

    /// 切换到指定会话
    pub fn switch_to_session(&mut self, session_id: &str) -> anyhow::Result<Vec<MessageItem>> {
        // 验证会话存在
        let session = self
            .store
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // 加载消息
        let messages = self.load_messages(session_id)?;

        // 更新当前会话
        self.current_session_id = Some(session_id.to_string());
        self.current_session_title = session.title;

        info!("Switched to session: {}", session_id);
        Ok(messages)
    }

    /// 列出会话
    pub fn list_sessions(&self, limit: i64) -> anyhow::Result<Vec<SessionRecord>> {
        Ok(self.store.list_sessions(limit)?)
    }

    /// 列出有消息的可恢复会话
    pub fn list_resumable_sessions(&self, limit: i64) -> anyhow::Result<Vec<SessionRecord>> {
        let sessions = self.store.list_sessions(limit)?;
        Ok(sessions
            .into_iter()
            .filter(|session| self.message_count(&session.id).unwrap_or_default() > 0)
            .collect())
    }

    /// 解析 /resume 选择输入：序号、完整/前缀 id、标题/模型关键词或消息搜索词。
    pub fn resolve_resume_selection(
        &self,
        query: &str,
        limit: i64,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let sessions = self.list_resumable_sessions(limit)?;
        resolve_session_selection_with_store(&self.store, &sessions, query)
    }

    /// 搜索会话
    pub fn search_sessions(&self, query: &str, limit: i64) -> anyhow::Result<Vec<SessionRecord>> {
        let query_lower = query.to_lowercase();
        let mut sessions = self
            .store
            .list_sessions(limit * 2)?
            .into_iter()
            .filter(|session| {
                session.id.starts_with(query)
                    || session.title.to_lowercase().contains(&query_lower)
                    || session.model.to_lowercase().contains(&query_lower)
            })
            .collect::<Vec<_>>();

        let message_results = self.store.search_messages(query, limit * 2)?;
        for message in message_results {
            if sessions
                .iter()
                .any(|session| session.id == message.session_id)
            {
                continue;
            }
            if let Some(session) = self.store.get_session(&message.session_id)? {
                sessions.push(session);
            }
            if sessions.len() >= limit as usize {
                break;
            }
        }

        sessions.truncate(limit as usize);
        Ok(sessions)
    }

    /// 删除会话
    pub fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        self.store.delete_session(session_id)?;
        debug!("Deleted session: {}", session_id);
        Ok(())
    }

    /// 导出会话到 JSON
    pub fn export_session(&self, session_id: &str) -> anyhow::Result<String> {
        let export = self.build_session_export(
            session_id,
            crate::session_store::export::SessionExportPrivacy::Full,
            crate::session_store::export::SessionExportFormat::Json,
        )?;
        crate::session_store::export::serialize(
            &export,
            crate::session_store::export::SessionExportFormat::Json,
        )
    }

    pub fn write_session_export(
        &self,
        session_id: &str,
        format: crate::session_store::export::SessionExportFormat,
        privacy: crate::session_store::export::SessionExportPrivacy,
    ) -> anyhow::Result<std::path::PathBuf> {
        let export = self.build_session_export(session_id, privacy, format)?;
        crate::session_store::export::write_export(
            &export,
            &crate::session_store::export::default_export_dir(),
            format,
        )
    }

    fn build_session_export(
        &self,
        session_id: &str,
        privacy: crate::session_store::export::SessionExportPrivacy,
        format: crate::session_store::export::SessionExportFormat,
    ) -> anyhow::Result<crate::session_store::export::SessionExport> {
        let session = self
            .store
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        let messages = self.store.get_messages(session_id)?;
        let messages = messages
            .into_iter()
            .map(|message| crate::session_store::export::ExportMessage {
                role: message.role,
                content: message.content,
                timestamp: Some(message.created_at),
            })
            .collect();
        let (events, warnings) = match self.load_session_events(session_id) {
            Ok(events) => (events, Vec::new()),
            Err(err) => (
                Vec::new(),
                vec![format!(
                    "Session events could not be loaded; event-derived metadata may be incomplete: {}",
                    err
                )],
            ),
        };
        let export_events = summarize_export_events(&events);
        let reverts = self
            .store
            .list_session_reverts(session_id, 50)?
            .into_iter()
            .map(|revert| crate::session_store::export::ExportRevert {
                operation: revert.operation,
                status: revert.status,
                paths: revert.paths,
                diff_summary: revert.diff_summary,
                unreverted: revert.unreverted,
                created_at: revert.created_at,
            })
            .collect();

        Ok(crate::session_store::export::build_export(
            crate::session_store::export::SessionExportInput {
                session_id: session.id,
                title: Some(session.title),
                model: Some(session.model),
                messages,
                changed_files: export_events.changed_files,
                reverts,
                diagnostics: export_events.diagnostics,
                tool_stats: export_events.tool_stats,
                warnings,
            },
            privacy,
            format,
        ))
    }

    /// 获取数据库统计
    pub fn stats(&self) -> anyhow::Result<crate::session_store::DbStats> {
        Ok(self.store.stats()?)
    }

    /// 获取当前会话的最近一条运行轨迹。
    pub fn latest_trace(&self) -> anyhow::Result<Option<crate::engine::trace::TurnTrace>> {
        let Some(session_id) = self.current_session_id.as_deref() else {
            return Ok(None);
        };
        Ok(self.store.latest_turn_trace(session_id)?)
    }

    /// 获取当前会话的最近运行轨迹，按新到旧排序。
    pub fn recent_traces(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::engine::trace::TurnTrace>> {
        let Some(session_id) = self.current_session_id.as_deref() else {
            return Ok(Vec::new());
        };
        Ok(self.store.recent_turn_traces(session_id, limit)?)
    }

    /// 获取当前会话的最近学习事件。
    pub fn recent_learning_events(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::session_store::LearningEventRecord>> {
        let Some(session_id) = self.current_session_id.as_deref() else {
            return Ok(Vec::new());
        };
        Ok(self.store.recent_learning_events(session_id, limit)?)
    }

    /// 获取当前会话最近的 subagent 结果 artifact。
    pub fn recent_agent_artifacts(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::session_store::AgentArtifactRecord>> {
        let Some(session_id) = self.current_session_id.as_deref() else {
            return Ok(Vec::new());
        };
        Ok(self.store.recent_agent_artifacts(session_id, limit)?)
    }

    /// 获取当前会话最近的 durable subagent task state。
    pub fn recent_agent_task_states(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::session_store::AgentTaskStateRecord>> {
        let Some(session_id) = self.current_session_id.as_deref() else {
            return Ok(Vec::new());
        };
        Ok(self.store.recent_agent_task_states(session_id, limit)?)
    }

    /// 获取当前会话的指定学习事件。
    pub fn learning_event(
        &self,
        id: i64,
    ) -> anyhow::Result<Option<crate::session_store::LearningEventRecord>> {
        let Some(session_id) = self.current_session_id.as_deref() else {
            return Ok(None);
        };
        Ok(self.store.learning_event(session_id, id)?)
    }

    pub fn add_learning_event(
        &self,
        kind: &str,
        source: &str,
        summary: &str,
        confidence: f64,
        payload: &serde_json::Value,
    ) -> anyhow::Result<i64> {
        let Some(session_id) = self.current_session_id.as_deref() else {
            anyhow::bail!("No active session");
        };
        Ok(self
            .store
            .add_learning_event(session_id, kind, source, summary, confidence, payload)?)
    }

    /// 获取会话消息数量
    pub fn message_count(&self, session_id: &str) -> anyhow::Result<i64> {
        Ok(self.store.message_count(session_id)?)
    }

    pub fn recent_preview_lines(
        &self,
        session_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<String>> {
        let records = self.store.get_messages(session_id)?;
        Ok(recent_preview_from_records(&records, limit))
    }

    /// 生成会话标题（基于第一条用户消息）
    pub fn generate_title(&self, messages: &[MessageItem]) -> String {
        // 找到第一条用户消息
        let first_user_msg = messages
            .iter()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.trim());

        if let Some(content) = first_user_msg {
            // 取前 50 个字符作为标题
            let title: String = content.chars().take(50).collect();

            if title.len() < content.len() {
                format!("{}...", title)
            } else {
                title
            }
        } else {
            "New Session".to_string()
        }
    }

    /// 更新 token 统计
    pub fn update_tokens(&self, input_tokens: i64, output_tokens: i64) -> anyhow::Result<()> {
        if let Some(ref id) = self.current_session_id {
            self.store.update_tokens(id, input_tokens, output_tokens)?;
        }
        Ok(())
    }

    /// 获取会话的编辑历史
    pub fn list_edits(&self, session_id: &str) -> anyhow::Result<Vec<EditRecord>> {
        self.load_edit_records(&self.edits_path(session_id))
    }

    /// 回滚最后一次编辑
    pub fn rewind_last_edit(&self, session_id: &str) -> anyhow::Result<String> {
        let mut edits = self.list_edits(session_id)?;
        if edits.is_empty() {
            return Err(anyhow::anyhow!("No edits to rewind"));
        }

        let last_edit = edits
            .pop()
            .expect("edits must be non-empty after is_empty check");
        let snap_path = last_edit.snapshot_path();

        if !snap_path.exists() {
            return Err(anyhow::anyhow!(
                "Snapshot not found: {}",
                snap_path.display()
            ));
        }

        let current_content = std::fs::read_to_string(&last_edit.file_path)?;
        let content = std::fs::read_to_string(&snap_path)?;

        // Save current state into redo stack before rewinding.
        self.push_redo_record(session_id, &last_edit, &current_content)?;
        std::fs::write(&last_edit.file_path, content)?;
        self.save_edit_records(&self.edits_path(session_id), &edits)?;

        Ok(format!(
            "Rewound {} on {}",
            last_edit.tool_name, last_edit.file_path
        ))
    }

    /// 回滚指定文件的最后一次编辑
    pub fn rewind_file(&self, session_id: &str, file_path: &str) -> anyhow::Result<String> {
        let edits = self.list_edits(session_id)?;
        if edits.is_empty() {
            return Err(anyhow::anyhow!("No edits to rewind"));
        }

        // 找到指定文件的最后一次编辑
        let file_edit_idx = edits
            .iter()
            .rposition(|e| e.file_path == file_path)
            .ok_or_else(|| anyhow::anyhow!("No edits found for file: {}", file_path))?;

        let target_edit = edits[file_edit_idx].clone();
        let snap_path = target_edit.snapshot_path();

        if !snap_path.exists() {
            return Err(anyhow::anyhow!(
                "Snapshot not found: {}",
                snap_path.display()
            ));
        }

        let current_content = std::fs::read_to_string(&target_edit.file_path)?;
        let content = std::fs::read_to_string(&snap_path)?;

        // Save current state into redo stack before rewinding.
        self.push_redo_record(session_id, &target_edit, &current_content)?;
        std::fs::write(&target_edit.file_path, content)?;

        // 移除该条记录并更新 edits.json
        let mut remaining = edits;
        remaining.remove(file_edit_idx);

        self.save_edit_records(&self.edits_path(session_id), &remaining)?;

        Ok(format!(
            "Rewound {} on {}",
            target_edit.tool_name, target_edit.file_path
        ))
    }

    /// 重做最后一次被撤销的编辑
    pub fn redo_last_edit(&self, session_id: &str) -> anyhow::Result<String> {
        let redo_path = self.redo_edits_path(session_id);
        let mut redo_edits = self.load_edit_records(&redo_path)?;
        if redo_edits.is_empty() {
            return Err(anyhow::anyhow!("No edits to redo"));
        }

        let redo_edit = redo_edits
            .pop()
            .expect("redo edits must be non-empty after is_empty check");
        let redo_snap_path = redo_edit.snapshot_path();
        if !redo_snap_path.exists() {
            return Err(anyhow::anyhow!(
                "Redo snapshot not found: {}",
                redo_snap_path.display()
            ));
        }

        let current_content = std::fs::read_to_string(&redo_edit.file_path)?;
        let redo_content = std::fs::read_to_string(&redo_snap_path)?;

        // Re-add an undo record so /undo can reverse this /redo.
        let undo_record = self.create_runtime_snapshot_record(
            session_id,
            &redo_edit.file_path,
            &format!("redo:{}", redo_edit.tool_name),
            &current_content,
            "runtime_undo",
        )?;
        let mut edits = self.list_edits(session_id)?;
        edits.push(undo_record);
        self.save_edit_records(&self.edits_path(session_id), &edits)?;

        // Apply redone content and consume redo stack top.
        std::fs::write(&redo_edit.file_path, redo_content)?;
        self.save_edit_records(&redo_path, &redo_edits)?;

        Ok(format!(
            "Redid {} on {}",
            redo_edit.tool_name, redo_edit.file_path
        ))
    }

    fn snapshots_session_dir(&self, session_id: &str) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("snapshots")
            .join(session_id)
    }

    fn edits_path(&self, session_id: &str) -> PathBuf {
        self.snapshots_session_dir(session_id).join("edits.json")
    }

    fn redo_edits_path(&self, session_id: &str) -> PathBuf {
        self.snapshots_session_dir(session_id)
            .join("redo_edits.json")
    }

    fn load_edit_records(&self, path: &PathBuf) -> anyhow::Result<Vec<EditRecord>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(path)?;
        let records: Vec<serde_json::Value> = serde_json::from_str(&content)?;

        Ok(records
            .into_iter()
            .map(|r| EditRecord {
                timestamp: r["timestamp"].as_str().unwrap_or("").to_string(),
                file_path: r["file_path"].as_str().unwrap_or("").to_string(),
                tool_name: r["tool_name"].as_str().unwrap_or("").to_string(),
                snapshot_dir: r["snapshot_dir"].as_str().unwrap_or("").to_string(),
                snapshot_file: r["snapshot_file"].as_str().unwrap_or("").to_string(),
            })
            .collect())
    }

    fn save_edit_records(&self, path: &PathBuf, edits: &[EditRecord]) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let records: Vec<serde_json::Value> = edits
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "file_path": e.file_path,
                    "tool_name": e.tool_name,
                    "snapshot_dir": e.snapshot_dir,
                    "snapshot_file": e.snapshot_file,
                })
            })
            .collect();

        std::fs::write(path, serde_json::to_string_pretty(&records)?)?;
        Ok(())
    }

    fn create_runtime_snapshot_record(
        &self,
        session_id: &str,
        file_path: &str,
        tool_name: &str,
        content: &str,
        sub_dir: &str,
    ) -> anyhow::Result<EditRecord> {
        let snapshot_dir = self.snapshots_session_dir(session_id).join(sub_dir);
        std::fs::create_dir_all(&snapshot_dir)?;
        let snapshot_file = format!(
            "{}_{}.txt",
            chrono::Utc::now().timestamp_millis(),
            Uuid::new_v4().simple()
        );
        std::fs::write(snapshot_dir.join(&snapshot_file), content)?;

        Ok(EditRecord {
            timestamp: chrono::Utc::now().to_rfc3339(),
            file_path: file_path.to_string(),
            tool_name: tool_name.to_string(),
            snapshot_dir: snapshot_dir.to_string_lossy().to_string(),
            snapshot_file,
        })
    }

    fn push_redo_record(
        &self,
        session_id: &str,
        undone_edit: &EditRecord,
        content_before_rewind: &str,
    ) -> anyhow::Result<()> {
        let redo_record = self.create_runtime_snapshot_record(
            session_id,
            &undone_edit.file_path,
            &undone_edit.tool_name,
            content_before_rewind,
            "runtime_redo",
        )?;
        let redo_path = self.redo_edits_path(session_id);
        let mut redo_edits = self.load_edit_records(&redo_path)?;
        redo_edits.push(redo_record);
        self.save_edit_records(&redo_path, &redo_edits)
    }
}

fn resolve_session_selection_with_store(
    store: &SessionStore,
    sessions: &[SessionRecord],
    query: &str,
) -> anyhow::Result<Option<SessionRecord>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(None);
    }
    if matches!(query, "latest" | "last" | "continue") {
        return Ok(sessions.first().cloned());
    }
    if let Ok(index) = query.parse::<usize>() {
        if (1..=sessions.len()).contains(&index) {
            return Ok(Some(sessions[index - 1].clone()));
        }
    }

    let query_lower = query.to_lowercase();
    if let Some(session) = sessions.iter().find(|session| {
        session.id.starts_with(query)
            || session.title.to_lowercase().contains(&query_lower)
            || session.model.to_lowercase().contains(&query_lower)
    }) {
        return Ok(Some(session.clone()));
    }

    for message in store.search_messages(query, 8).unwrap_or_default() {
        if let Some(session) = store.get_session(&message.session_id)? {
            return Ok(Some(session));
        }
    }

    Ok(None)
}

fn recent_preview_from_records(
    records: &[crate::session_store::MessageRecord],
    limit: usize,
) -> Vec<String> {
    let mut recent = records
        .iter()
        .rev()
        .filter(|record| matches!(record.role.as_str(), "user" | "assistant"))
        .take(limit)
        .map(|record| {
            let label = if record.role == "user" {
                "you"
            } else {
                "agent"
            };
            format!(
                "  {:<5} {}",
                label,
                compact_preview_line(&record.content, 96)
            )
        })
        .collect::<Vec<_>>();
    recent.reverse();
    recent
}

fn compact_preview_line(input: &str, max_chars: usize) -> String {
    let one_line = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    for ch in one_line.chars() {
        if out.chars().count() >= max_chars {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
}

struct ExportEventSummary {
    changed_files: Vec<String>,
    diagnostics: Vec<crate::session_store::export::ExportDiagnosticRecord>,
    tool_stats: Value,
}

fn summarize_export_events(events: &[SessionEventRow]) -> ExportEventSummary {
    let mut call_tools = BTreeMap::<String, String>::new();
    let mut tool_calls = BTreeMap::<String, usize>::new();
    let mut tool_successes = BTreeMap::<String, usize>::new();
    let mut tool_failures = BTreeMap::<String, usize>::new();
    let mut counted_calls = BTreeSet::<String>::new();
    let mut counted_successes = BTreeSet::<String>::new();
    let mut counted_failures = BTreeSet::<String>::new();
    let mut changed_files = BTreeSet::<String>::new();
    let mut diagnostics = Vec::new();

    for event in events {
        let payload = serde_json::from_str::<Value>(&event.payload).unwrap_or(Value::Null);
        match event.event_type.as_str() {
            "tool_called" | "tool_started" => {
                if let (Some(id), Some(name)) = (
                    payload.get("tool_call_id").and_then(Value::as_str),
                    payload.get("tool_name").and_then(Value::as_str),
                ) {
                    call_tools.insert(id.to_string(), name.to_string());
                    if counted_calls.insert(id.to_string()) {
                        *tool_calls.entry(name.to_string()).or_default() += 1;
                    }
                }
            }
            "tool_input_completed" => {
                let Some(call_id) = payload.get("tool_call_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(tool) = call_tools.get(call_id) else {
                    continue;
                };
                if !matches!(tool.as_str(), "file_write" | "file_edit" | "file_patch") {
                    continue;
                }
                if let Some(input) = payload
                    .get("input_args")
                    .and_then(Value::as_str)
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                {
                    collect_changed_paths_from_tool_input(&input, &mut changed_files);
                }
            }
            "tool_succeeded" | "tool_result_completed" => {
                if let Some((call_id, tool)) = tool_call_and_name_for_event(&payload, &call_tools) {
                    if !counted_successes.insert(call_id) {
                        continue;
                    }
                    *tool_successes.entry(tool).or_default() += 1;
                }
            }
            "tool_failed" => {
                if let Some((call_id, tool)) = tool_call_and_name_for_event(&payload, &call_tools) {
                    if !counted_failures.insert(call_id) {
                        continue;
                    }
                    *tool_failures.entry(tool).or_default() += 1;
                }
            }
            "runtime_diagnostic" => {
                diagnostics.push(crate::session_store::export::ExportDiagnosticRecord {
                    source: "runtime".to_string(),
                    status: "recorded".to_string(),
                    path: None,
                    error_count: 0,
                    warning_count: 0,
                    detail: payload
                        .get("schema")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
            _ => {}
        }
    }

    ExportEventSummary {
        changed_files: changed_files.into_iter().collect(),
        diagnostics,
        tool_stats: serde_json::json!({
            "calls": tool_calls,
            "successes": tool_successes,
            "failures": tool_failures,
        }),
    }
}

fn tool_call_and_name_for_event(
    payload: &Value,
    call_tools: &BTreeMap<String, String>,
) -> Option<(String, String)> {
    let call_id = payload.get("tool_call_id").and_then(Value::as_str)?;
    let tool = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| call_tools.get(call_id).cloned())?;
    Some((call_id.to_string(), tool))
}

fn collect_changed_paths_from_tool_input(input: &Value, changed_files: &mut BTreeSet<String>) {
    if let Some(path) = input.get("path").and_then(Value::as_str) {
        if !path.trim().is_empty() {
            changed_files.insert(path.trim().to_string());
        }
    }
    if let Some(paths) = input.get("written_paths").and_then(Value::as_array) {
        for path in paths.iter().filter_map(Value::as_str) {
            if !path.trim().is_empty() {
                changed_files.insert(path.trim().to_string());
            }
        }
    }
    if let Some(operations) = input.get("operations").and_then(Value::as_array) {
        for operation in operations {
            if let Some(path) = operation.get("path").and_then(Value::as_str) {
                if !path.trim().is_empty() {
                    changed_files.insert(path.trim().to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let mut manager = TuiSessionManager::in_memory().unwrap();

        // 开始新会话
        let session_id = manager.start_session("Test Session", "gpt-4").unwrap();
        assert!(manager.current_session_id().is_some());

        // 添加消息
        manager.add_message(MessageRole::User, "Hello").unwrap();
        manager
            .add_message(MessageRole::Assistant, "Hi there!")
            .unwrap();

        // 验证消息数
        let count = manager.message_count(&session_id).unwrap();
        assert_eq!(count, 2);

        // 加载消息
        let messages = manager.load_messages(&session_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "Hello");
    }

    #[test]
    fn test_list_sessions() {
        let mut manager = TuiSessionManager::in_memory().unwrap();

        manager.start_session("Session 1", "gpt-4").unwrap();
        manager.start_session("Session 2", "gpt-4").unwrap();
        manager.start_session("Session 3", "gpt-4").unwrap();

        // 需要为每个会话创建新的 manager 实例才能看到独立的会话
        // 这里只是测试接口可用
    }

    #[test]
    fn test_resume_selection_accepts_index_id_title_and_message_search() {
        let mut manager = TuiSessionManager::in_memory().unwrap();

        let alpha = manager.start_session("Fix login bug", "model-a").unwrap();
        manager
            .add_message(MessageRole::User, "Need authentication callback repair")
            .unwrap();
        manager.start_session("Build dashboard", "model-b").unwrap();
        manager
            .add_message(MessageRole::Assistant, "Dashboard session ready")
            .unwrap();

        let expected_latest = manager.list_resumable_sessions(10).unwrap()[0].id.clone();
        let by_latest = manager
            .resolve_resume_selection("latest", 10)
            .unwrap()
            .unwrap();
        assert_eq!(by_latest.id, expected_latest);

        let by_id = manager
            .resolve_resume_selection(&alpha[..8], 10)
            .unwrap()
            .unwrap();
        assert_eq!(by_id.id, alpha);

        let by_title = manager
            .resolve_resume_selection("dashboard", 10)
            .unwrap()
            .unwrap();
        assert_eq!(by_title.title, "Build dashboard");

        let by_message = manager
            .resolve_resume_selection("authentication", 10)
            .unwrap()
            .unwrap();
        assert_eq!(by_message.id, alpha);
    }

    #[test]
    fn test_recent_preview_lines_show_conversation_context() {
        let mut manager = TuiSessionManager::in_memory().unwrap();
        let session_id = manager.start_session("Preview", "model").unwrap();
        manager
            .add_message(MessageRole::User, "Hello there")
            .unwrap();
        manager
            .add_message(MessageRole::Assistant, "Hi, I can help")
            .unwrap();

        let preview = manager.recent_preview_lines(&session_id, 4).unwrap();
        assert_eq!(preview.len(), 2);
        assert!(preview[0].contains("you"));
        assert!(preview[0].contains("Hello there"));
        assert!(preview[1].contains("agent"));
        assert!(preview[1].contains("Hi, I can help"));
    }

    #[test]
    fn test_generate_title() {
        let manager = TuiSessionManager::in_memory().unwrap();

        let messages = vec![
            MessageItem {
                id: "1".to_string(),
                role: MessageRole::System,
                content: "Welcome".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
            MessageItem {
                id: "2".to_string(),
                role: MessageRole::User,
                content: "How do I implement authentication in Rust?".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
        ];

        let title = manager.generate_title(&messages);
        assert!(title.contains("How do I implement authentication"));
    }

    #[test]
    fn test_export_session() {
        let mut manager = TuiSessionManager::in_memory().unwrap();
        let session_id = manager.start_session("Export Test", "gpt-4").unwrap();

        manager
            .add_message(MessageRole::User, "Test message")
            .unwrap();

        let export = manager.export_session(&session_id).unwrap();
        assert!(export.contains("Export Test"));
        assert!(export.contains("Test message"));
    }

    #[test]
    fn test_list_edits_empty() {
        let mut manager = TuiSessionManager::in_memory().unwrap();
        let session_id = manager.start_session("Rewind Test", "gpt-4").unwrap();

        // 新会话没有编辑记录
        let edits = manager.list_edits(&session_id).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn test_rewind_edit_record_roundtrip() {
        let mut manager = TuiSessionManager::in_memory().unwrap();
        let session_id = manager.start_session("Rewind Test", "gpt-4").unwrap();

        // 创建测试文件和快照
        let test_file = std::env::temp_dir().join("test_rewind_file.txt");
        std::fs::write(&test_file, "original content").unwrap();

        let snap_dir = std::env::temp_dir().join("test_snapshot");
        std::fs::create_dir_all(&snap_dir).unwrap();
        std::fs::write(snap_dir.join("test_rewind_file.txt"), "original content").unwrap();

        // 手动写入 edits.json
        let edit_record = serde_json::json!([{
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "file_path": test_file.to_string_lossy().to_string(),
            "tool_name": "file_edit",
            "snapshot_dir": snap_dir.to_string_lossy().to_string(),
            "snapshot_file": "test_rewind_file.txt",
        }]);

        let edits_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("snapshots")
            .join(&session_id)
            .join("edits.json");
        std::fs::create_dir_all(edits_path.parent().unwrap()).unwrap();
        std::fs::write(
            &edits_path,
            serde_json::to_string_pretty(&edit_record).unwrap(),
        )
        .unwrap();

        // 验证列出编辑
        let edits = manager.list_edits(&session_id).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].tool_name, "file_edit");
        assert_eq!(edits[0].file_path, test_file.to_string_lossy().to_string());

        // 修改文件
        std::fs::write(&test_file, "modified content").unwrap();
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "modified content"
        );

        // 回滚
        let result = manager.rewind_last_edit(&session_id).unwrap();
        assert!(result.contains("Rewound file_edit"));
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "original content"
        );

        // 验证 edits.json 已更新
        let edits = manager.list_edits(&session_id).unwrap();
        assert!(edits.is_empty());

        // 清理
        let _ = std::fs::remove_file(&test_file);
        let _ = std::fs::remove_dir_all(&snap_dir);
        let _ = std::fs::remove_file(&edits_path);
    }

    #[test]
    fn test_rewind_file_specific() {
        let mut manager = TuiSessionManager::in_memory().unwrap();
        let session_id = manager.start_session("Rewind Test", "gpt-4").unwrap();

        // 创建两个测试文件
        let file_a = std::env::temp_dir().join("test_rewind_a.txt");
        let file_b = std::env::temp_dir().join("test_rewind_b.txt");
        std::fs::write(&file_a, "A original").unwrap();
        std::fs::write(&file_b, "B original").unwrap();

        let snap_dir = std::env::temp_dir().join("test_snapshot_multi");
        std::fs::create_dir_all(&snap_dir).unwrap();
        std::fs::write(snap_dir.join("file_a.txt"), "A original").unwrap();
        std::fs::write(snap_dir.join("file_b.txt"), "B original").unwrap();

        // 手动写入多条编辑记录
        let edit_records = serde_json::json!([
            {
                "timestamp": "2024-01-01T00:00:00Z",
                "file_path": file_a.to_string_lossy().to_string(),
                "tool_name": "file_edit",
                "snapshot_dir": snap_dir.to_string_lossy().to_string(),
                "snapshot_file": "file_a.txt",
            },
            {
                "timestamp": "2024-01-01T00:01:00Z",
                "file_path": file_b.to_string_lossy().to_string(),
                "tool_name": "file_write",
                "snapshot_dir": snap_dir.to_string_lossy().to_string(),
                "snapshot_file": "file_b.txt",
            }
        ]);

        let edits_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("snapshots")
            .join(&session_id)
            .join("edits.json");
        std::fs::create_dir_all(edits_path.parent().unwrap()).unwrap();
        std::fs::write(
            &edits_path,
            serde_json::to_string_pretty(&edit_records).unwrap(),
        )
        .unwrap();

        // 修改两个文件
        std::fs::write(&file_a, "A modified").unwrap();
        std::fs::write(&file_b, "B modified").unwrap();

        // 回滚 file_b
        let result = manager
            .rewind_file(&session_id, file_b.to_string_lossy().as_ref())
            .unwrap();
        assert!(result.contains("Rewound file_write"));
        assert_eq!(std::fs::read_to_string(&file_b).unwrap(), "B original");
        assert_eq!(std::fs::read_to_string(&file_a).unwrap(), "A modified"); // A 未变

        // 验证剩余记录
        let edits = manager.list_edits(&session_id).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].file_path, file_a.to_string_lossy().to_string());

        // 清理
        let _ = std::fs::remove_file(&file_a);
        let _ = std::fs::remove_file(&file_b);
        let _ = std::fs::remove_dir_all(&snap_dir);
        let _ = std::fs::remove_file(&edits_path);
    }

    #[test]
    fn test_redo_roundtrip_after_rewind() {
        let mut manager = TuiSessionManager::in_memory().unwrap();
        let session_id = manager.start_session("Redo Test", "gpt-4").unwrap();

        let test_file = std::env::temp_dir().join("test_redo_file.txt");
        std::fs::write(&test_file, "original content").unwrap();

        let snap_dir = std::env::temp_dir().join("test_redo_snapshot");
        std::fs::create_dir_all(&snap_dir).unwrap();
        std::fs::write(snap_dir.join("test_redo_file.txt"), "original content").unwrap();

        let edit_record = serde_json::json!([{
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "file_path": test_file.to_string_lossy().to_string(),
            "tool_name": "file_edit",
            "snapshot_dir": snap_dir.to_string_lossy().to_string(),
            "snapshot_file": "test_redo_file.txt",
        }]);

        let edits_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("snapshots")
            .join(&session_id)
            .join("edits.json");
        std::fs::create_dir_all(edits_path.parent().unwrap()).unwrap();
        std::fs::write(
            &edits_path,
            serde_json::to_string_pretty(&edit_record).unwrap(),
        )
        .unwrap();

        // Simulate file changed by edit tool.
        std::fs::write(&test_file, "modified content").unwrap();

        // Undo -> file returns to original.
        let undo_result = manager.rewind_last_edit(&session_id).unwrap();
        assert!(undo_result.contains("Rewound"));
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "original content"
        );

        // Redo -> file returns to modified.
        let redo_result = manager.redo_last_edit(&session_id).unwrap();
        assert!(redo_result.contains("Redid"));
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "modified content"
        );

        // Cleanup.
        let _ = std::fs::remove_file(&test_file);
        let _ = std::fs::remove_dir_all(&snap_dir);
        let _ = std::fs::remove_dir_all(
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".priority-agent")
                .join("snapshots")
                .join(&session_id),
        );
    }

    #[test]
    fn test_redo_empty_stack_fails() {
        let mut manager = TuiSessionManager::in_memory().unwrap();
        let session_id = manager.start_session("Redo Empty", "gpt-4").unwrap();

        let err = manager
            .redo_last_edit(&session_id)
            .expect_err("redo without undo should fail");
        assert!(err.to_string().contains("No edits to redo"));
    }

    #[test]
    fn test_from_store_reuses_existing_session() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        store
            .create_session("shared-session", "Shared", "mock-model")
            .unwrap();

        let manager =
            TuiSessionManager::from_store(store.clone(), "shared-session", "Shared", "mock-model")
                .unwrap();

        assert_eq!(manager.current_session_id(), Some("shared-session"));
        assert!(manager.is_current_session("shared-session"));
        manager.add_message(MessageRole::User, "hello").unwrap();
        assert_eq!(store.get_messages("shared-session").unwrap().len(), 1);
    }

    #[test]
    fn test_recent_traces_use_current_session_scope() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        store
            .create_session("shared-session", "Shared", "mock-model")
            .unwrap();
        store
            .create_session("other-session", "Other", "mock-model")
            .unwrap();

        for turn_index in 1..=2 {
            let mut trace =
                crate::engine::trace::TurnTrace::new("shared-session", turn_index, "shared trace");
            trace.finish(crate::engine::trace::TurnStatus::Completed);
            store.add_turn_trace(&trace).unwrap();
        }
        let mut other_trace =
            crate::engine::trace::TurnTrace::new("other-session", 9, "other trace");
        other_trace.finish(crate::engine::trace::TurnStatus::Completed);
        store.add_turn_trace(&other_trace).unwrap();

        let manager =
            TuiSessionManager::from_store(store, "shared-session", "Shared", "mock-model").unwrap();
        let traces = manager.recent_traces(10).unwrap();

        assert_eq!(traces.len(), 2);
        assert!(traces
            .iter()
            .all(|trace| trace.session_id == "shared-session"));
        assert_eq!(traces[0].turn_index, 2);
        assert_eq!(traces[1].turn_index, 1);
    }
}

//! TUI 会话管理器
//!
//! 管理当前会话的持久化和恢复

pub use crate::session_store::SessionRecord;
use crate::session_store::{PersistedSessionPart, SessionEventRow, SessionStore};
use crate::state::{MessageItem, MessageRole};
use std::collections::HashMap;
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
    /// Workspace root per session id (in-memory tag until schema migration).
    session_workspaces: HashMap<String, String>,
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
            session_workspaces: HashMap::new(),
        })
    }

    /// 内存模式（用于测试）
    pub fn in_memory() -> anyhow::Result<Self> {
        let store = Arc::new(SessionStore::in_memory()?);
        Ok(Self {
            store,
            current_session_id: None,
            current_session_title: String::new(),
            session_workspaces: HashMap::new(),
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
            session_workspaces: HashMap::new(),
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

    /// Tag a session with a workspace root.
    pub fn tag_session_workspace(&mut self, session_id: &str, workspace_root: &str) {
        self.session_workspaces
            .insert(session_id.to_string(), workspace_root.to_string());
    }

    /// Get the workspace root for a session, falling back to the current workspace.
    pub fn session_workspace(&self, session_id: &str, current_workspace: &str) -> String {
        self.session_workspaces
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| current_workspace.to_string())
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
        self.add_message_with_metadata(role, content, &HashMap::new())
    }

    /// 添加带 UI/runtime metadata 的消息到当前会话。
    pub fn add_message_with_metadata(
        &self,
        role: MessageRole,
        content: &str,
        metadata: &HashMap<String, String>,
    ) -> anyhow::Result<i64> {
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

        let metadata_json = metadata_map_to_value(metadata);
        let id = self.store.add_message_with_metadata(
            session_id,
            role_str,
            content,
            None,
            None,
            metadata_json.as_ref(),
        )?;
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
            let metadata_json = metadata_map_to_value(&msg.metadata);
            self.store.add_message_with_metadata(
                session_id,
                role_str,
                &msg.content,
                None,
                None,
                metadata_json.as_ref(),
            )?;
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
            let metadata_json = metadata_map_to_value(&msg.metadata);
            self.store.add_message_with_metadata(
                session_id,
                role_str,
                &msg.content,
                None,
                None,
                metadata_json.as_ref(),
            )?;
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
                    metadata: metadata_value_to_map(record.metadata),
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

    pub fn settle_unfinished_tool_parts(
        &self,
        session_id: &str,
        reason: &str,
    ) -> anyhow::Result<usize> {
        self.store.refresh_session_parts(session_id)?;
        let parts = self.store.get_session_parts(session_id)?;
        let writer =
            crate::session_store::SessionEventWriter::new(self.store.shared_conn(), session_id);
        let mut settled = 0usize;
        let mut seen = std::collections::HashSet::new();
        for part in parts.iter().filter(|part| {
            (part.kind == "tool" || part.kind == "shell")
                && matches!(part.status.as_deref(), Some("running" | "pending"))
        }) {
            let tool_call_id = part.tool_call_id.as_deref().unwrap_or(&part.part_id);
            if !seen.insert(tool_call_id.to_string()) {
                continue;
            }
            let tool_name = part.tool_name.as_deref().unwrap_or("unknown");
            writer.tool_failed(
                tool_call_id,
                &format!("{reason} before settlement ({tool_name}:{tool_call_id})"),
            )?;
            settled += 1;
        }
        Ok(settled)
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

    /// Fork the current session into a new child session and switch to it.
    pub async fn fork_current_session(
        &mut self,
        title: &str,
        workspace_root: &str,
    ) -> anyhow::Result<String> {
        let parent_id = self
            .current_session_id()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?
            .to_string();
        let parent = self
            .store
            .get_session(&parent_id)?
            .ok_or_else(|| anyhow::anyhow!("Current session not found: {}", parent_id))?;

        let child_id = format!("sess_{}", Uuid::new_v4().simple());
        self.store
            .create_child_session(&child_id, title, &parent.model, &parent_id)?;

        let records = self.store.get_messages(&parent_id)?;
        for record in records {
            self.store.add_message_with_metadata(
                &child_id,
                &record.role,
                &record.content,
                record.tool_calls.as_ref(),
                record.tool_call_id.as_deref(),
                record.metadata.as_ref(),
            )?;
        }

        self.tag_session_workspace(&child_id, workspace_root);
        self.current_session_id = Some(child_id.clone());
        self.current_session_title = title.to_string();

        info!("Forked session {} into {}", parent_id, child_id);
        Ok(child_id)
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
        let (events, mut warnings) = match self.load_session_events(session_id) {
            Ok(events) => (events, Vec::new()),
            Err(err) => (
                Vec::new(),
                vec![format!(
                    "Session events could not be loaded; event-derived metadata may be incomplete: {}",
                    err
                )],
            ),
        };
        let export_events = export_builder::summarize_export_events(&events);
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

        // Session parts (lightweight projection) — query once and reuse.
        let (parts, unresolved_settlement) = match self.store.get_session_parts(session_id) {
            Ok(all_parts) => {
                let parts = all_parts
                    .iter()
                    .cloned()
                    .map(|part| crate::session_store::export::ExportPart {
                        part_id: part.part_id,
                        kind: part.kind,
                        tool_call_id: part.tool_call_id,
                        tool_name: part.tool_name,
                        status: part.status,
                        message_id: part.message_id,
                        projected_to_seq: part.projected_to_seq,
                        updated_at: part.updated_at,
                    })
                    .collect();
                let unresolved_settlement = all_parts
                    .into_iter()
                    .filter(|part| {
                        (part.kind == "tool" || part.kind == "shell")
                            && matches!(part.status.as_deref(), Some("running" | "pending"))
                    })
                    .map(|part| {
                        format!(
                            "{}:{}:{}",
                            part.part_id,
                            part.tool_name.as_deref().unwrap_or("unknown"),
                            part.status.as_deref().unwrap_or("unknown")
                        )
                    })
                    .collect();
                (parts, unresolved_settlement)
            }
            Err(err) => {
                warnings.push(format!(
                    "Session parts could not be loaded; parts-derived metadata may be incomplete: {}",
                    err
                ));
                (Vec::new(), Vec::new())
            }
        };

        // Extract closeout status and compaction count from events
        let mut closeout_status = None;
        let mut compaction_count = 0;
        for event in &events {
            match event.event_type.as_str() {
                "closeout" => {
                    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&event.payload) {
                        closeout_status = payload
                            .get("status")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                    }
                }
                "compaction" => {
                    compaction_count += 1;
                }
                _ => {}
            }
        }

        // Tool output index (if available)
        let tool_outputs = {
            let output_store = crate::tool_output_store::ToolOutputStore::new();
            match output_store.list_for_session(session_id) {
                Ok(list) => list
                    .into_iter()
                    .map(|meta| crate::session_store::export::ExportToolOutput {
                        id: meta.id,
                        tool_name: meta.tool_name,
                        original_bytes: meta.original_bytes,
                    })
                    .collect(),
                Err(err) => {
                    warnings.push(format!("Tool output index could not be loaded: {}", err));
                    Vec::new()
                }
            }
        };

        let session_id_owned = session.id.clone();
        Ok(crate::session_store::export::build_export(
            crate::session_store::export::SessionExportInput {
                session_id: session_id_owned.clone(),
                title: Some(session.title),
                model: Some(session.model),
                messages,
                parts,
                changed_files: export_events.changed_files,
                reverts,
                diagnostics: export_events.diagnostics,
                tool_stats: export_events.tool_stats,
                warnings,
                closeout_status,
                compaction_count,
                unresolved_settlement,
                tool_outputs,
                goal_summary: build_goal_export_summary(&self.store, &session_id_owned),
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

fn metadata_map_to_value(metadata: &HashMap<String, String>) -> Option<serde_json::Value> {
    if metadata.is_empty() {
        return None;
    }
    serde_json::to_value(metadata).ok()
}

fn metadata_value_to_map(value: Option<serde_json::Value>) -> HashMap<String, String> {
    let Some(serde_json::Value::Object(object)) = value else {
        return HashMap::new();
    };

    object
        .into_iter()
        .filter_map(|(key, value)| value.as_str().map(|value| (key, value.to_string())))
        .collect()
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

fn build_goal_export_summary(
    store: &Arc<SessionStore>,
    session_id: &str,
) -> Option<serde_json::Value> {
    let db = store.as_ref();
    let active = db.get_current_goal_run(session_id).ok().flatten()?;
    let steps = db.list_goal_steps(&active.id, 50).ok().unwrap_or_default();
    let steps_json: Vec<serde_json::Value> = steps
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "turn_index": s.turn_index,
                "decision": s.decision,
                "closeout_status": s.closeout_status,
                "verification_status": s.verification_status,
                "changed_files": s.changed_files,
                "validation_items": s.validation_items,
                "score": s.score,
                "summary": s.summary,
                "created_at": s.created_at,
            })
        })
        .collect();
    Some(serde_json::json!({
        "goal_id": active.id,
        "objective": active.objective,
        "status": active.status,
        "turn_count": active.turn_count,
        "last_closeout_status": active.last_closeout_status,
        "last_blocker": active.last_blocker,
        "created_at": active.created_at,
        "updated_at": active.updated_at,
        "step_count": steps.len(),
        "steps": steps_json,
    }))
}

pub mod export_builder;

#[cfg(test)]
mod tests;

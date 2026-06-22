//! 流式查询引擎
//!
//! 提供与 Claude Code 类似的流式响应体验

use crate::engine::context_collapse::{CompactionDecision, ContextCompactionStrategy};
use crate::engine::context_compressor::CompactionAttemptInput;
use crate::services::api::{LlmProvider, Message};
use crate::tools::ToolRegistry;
use anyhow::Result;
use futures::Stream;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};

mod context_scrubber;
mod fallback;
mod text_progress;
mod turn_messages;

pub mod config;
pub use config::StreamingConfig;

use fallback::{ErrorType, FallbackState};
pub use text_progress::emit_text_progressively;
use text_progress::flush_session_end_memory_best_effort;
#[cfg(test)]
use text_progress::progressive_text_chunks;
use turn_messages::{
    build_messages_for_turn, estimate_registry_tool_schema_tokens, reactive_context_retry_messages,
    route_wants_agent_manager, session_title_from_user_message,
};

/// 流式查询事件
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 开始处理
    Start,
    /// 文本块（增量内容）
    TextChunk(String),
    /// 工具调用开始
    ToolCallStart { id: String, name: String },
    /// 工具调用参数（增量）
    ToolCallArgs { id: String, args_delta: String },
    /// 工具调用完成
    ToolCallComplete { id: String },
    /// 工具执行开始
    ToolExecutionStart {
        id: String,
        name: String,
        metadata: Option<serde_json::Value>,
    },
    /// 工具执行进度
    ToolExecutionProgress { id: String, progress: String },
    /// 工具执行完成
    ToolExecutionComplete {
        id: String,
        result: String,
        metadata: Option<serde_json::Value>,
        /// Structured JSON data from the tool result (e.g. mutation_result).
        result_data: Option<serde_json::Value>,
    },
    /// Tool results have been appended to the next model context.
    ToolResultsReadyForModel { ids: Vec<String> },
    /// 思考开始（extended thinking 模型）
    ThinkingStart,
    /// 思考内容块（增量）
    ThinkingChunk(String),
    /// 思考完成
    ThinkingComplete,
    /// 使用量统计
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        reasoning_tokens: Option<u32>,
        cached_tokens: Option<u32>,
        cache_write_tokens: Option<u32>,
    },
    /// Runtime diagnostic snapshot for clients that render run state.
    RuntimeDiagnostic { diagnostic: serde_json::Value },
    /// Final verification/closeout status for frontend rendering.
    Closeout {
        status: String,
        evidence_summary: Option<String>,
    },
    /// 完成
    Complete,
    /// 输出被截断（达到 max_tokens 限制）
    OutputTruncated,
    /// 错误
    Error(String),
    /// 工具执行需要用户授权
    PermissionRequest {
        id: String,
        tool_name: String,
        arguments: serde_json::Value,
        prompt: String,
        metadata: Option<serde_json::Value>,
        #[allow(dead_code)]
        review: Option<Box<crate::engine::human_review::HumanReviewAuditRecord>>,
    },
}

#[derive(Debug, Clone)]
pub struct ContextUsageReport {
    pub prompt: crate::engine::prompt_context::PromptContextReport,
    pub history_messages: usize,
    pub history_tokens: u64,
    pub tool_count: usize,
    pub tool_schema_tokens: u64,
    pub tool_schema_fingerprint: String,
    pub memory_snapshot_tokens: u64,
    pub relevant_memories: Vec<crate::memory::manager::MemoryMatch>,
    pub max_context_tokens: u64,
    pub total_estimated_tokens: u64,
    pub stable_prefix_fingerprint: String,
}

/// 流式查询引擎
pub struct StreamingQueryEngine {
    /// 配置
    config: StreamingConfig,
}

impl StreamingQueryEngine {
    /// 创建新的流式查询引擎
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            config: StreamingConfig::new(provider, tool_registry, model),
        }
    }

    /// 设置任务管理器
    pub fn with_task_manager(mut self, manager: Arc<crate::task_manager::TaskManager>) -> Self {
        self.config.task_manager = Some(manager);
        self
    }

    /// 获取成本追踪器的引用
    pub fn cost_tracker(&self) -> &Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>> {
        &self.config.cost_tracker
    }

    /// 设置会话存储（覆盖 lazy init）
    pub fn with_session_store(
        self,
        store: Arc<crate::session_store::SessionStore>,
        session_id: String,
    ) -> Self {
        let _ = self.config.session_store.set(Some(store));
        *self.config.session_id.write() = Some(session_id);
        self
    }

    /// 禁止 session_store 自动初始化（测试用）
    pub fn with_disable_session_auto_init(mut self) -> Self {
        self.config.disable_session_auto_init = true;
        self
    }

    pub fn trace_store(&self) -> Arc<crate::engine::trace::TraceStore> {
        self.config.trace_store.clone()
    }

    pub fn goal_manager(&self) -> Arc<crate::engine::session_goal::SessionGoalManager> {
        self.config.goal_manager.clone()
    }

    /// 返回当前持久化会话绑定。
    ///
    /// UI 层用这个绑定复用同一个 SessionStore/session_id，避免一轮对话
    /// 同时写入 CLI 会话和引擎会话两套历史。
    pub fn session_binding(&self) -> Option<(Arc<crate::session_store::SessionStore>, String)> {
        if self.config.disable_session_auto_init {
            return self
                .config
                .session_store
                .get()
                .and_then(|opt| {
                    opt.as_ref().map(|store| {
                        let sid = self.current_session_id()?;
                        Some((store.clone(), sid))
                    })
                })
                .flatten();
        }
        let store = self
            .config
            .session_store
            .get_or_init(|| {
                let db_path = crate::session_store::SessionStore::default_path();
                match crate::session_store::SessionStore::open(&db_path) {
                    Ok(s) => {
                        tracing::info!("SessionStore opened at {:?}", db_path);
                        Some(Arc::new(s))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to open SessionStore: {}", e);
                        None
                    }
                }
            })
            .as_ref()?;
        // Ensure we have a session id; create one if missing
        if self.config.session_id.read().is_none() {
            let sid = format!("session-{}", uuid::Uuid::new_v4());
            let model = self.model_name();
            let _ = store.create_session(&sid, "Session", &model, None);
            *self.config.session_id.write() = Some(sid);
        }
        let session_id = self.current_session_id()?;
        if let Ok(recovered) = store.recover_interrupted_agent_task_states(Some(&session_id)) {
            if recovered > 0 {
                tracing::warn!(
                    "Recovered {} interrupted sub-agent task(s) for session {} as paused_restart",
                    recovered,
                    session_id
                );
            }
        }
        Some((store.clone(), session_id))
    }

    /// 当前持久化会话 ID。
    pub fn current_session_id(&self) -> Option<String> {
        self.config.session_id.read().clone()
    }

    /// 切换当前持久化会话 ID。
    pub fn set_session_id(&self, session_id: impl Into<String>) {
        *self.config.session_id.write() = Some(session_id.into());
    }

    /// 设置记忆快照（在 system prompt 中注入冻结的记忆）
    pub fn with_memory_snapshot(mut self, snapshot: String) -> Self {
        if !snapshot.is_empty() {
            self.config.system_prompt = format!("{}\n{}", snapshot, self.config.system_prompt);
        }
        self
    }

    /// 设置最大上下文长度
    pub fn with_max_context(mut self, tokens: u64) -> Self {
        let model = self.model_name();
        self.config.compressor = Arc::new(tokio::sync::Mutex::new(
            crate::engine::context_compressor::ContextCompressor::new(tokens)
                .with_llm_provider(self.provider(), &model),
        ));
        self
    }

    /// 清除对话历史
    pub async fn clear_history(&self) {
        self.flush_memory_for_current_history(crate::memory::MemoryFlushReason::Clear)
            .await;
        let mut history = self.config.conversation_history.lock().await;
        history.clear();
    }

    /// 获取对话历史
    pub async fn get_history(&self) -> Vec<Message> {
        self.config.conversation_history.lock().await.clone()
    }

    /// 设置对话历史
    pub async fn set_history(&self, messages: Vec<Message>) {
        let mut history = self.config.conversation_history.lock().await;
        *history = messages;
    }

    pub async fn compact_context_manually(
        &self,
    ) -> Option<crate::engine::context_compressor::CompactionAttemptRecord> {
        let history_before = self.get_history().await;
        if history_before.is_empty() {
            return None;
        }

        self.flush_memory_for_current_history(crate::memory::MemoryFlushReason::Manual)
            .await;

        let before_tokens =
            crate::engine::context_compressor::estimate_messages_tokens(&history_before);
        let before_messages = history_before.len();
        let compressed = {
            let mut compressor = self.config.compressor.lock().await;
            compressor.set_llm_summary_stable_prefix(self.config.system_prompt.clone());
            if compressor.compaction_circuit_open() {
                return Some(
                    compressor.record_compaction_decision(CompactionAttemptInput::new(
                        "manual compact",
                        ContextCompactionStrategy::SessionMemoryCompact,
                        CompactionDecision::CircuitOpen,
                        before_tokens,
                        before_messages,
                        "compaction circuit open before manual compact",
                    )),
                );
            }
            compressor.record_compaction_decision(CompactionAttemptInput::new(
                "manual compact",
                ContextCompactionStrategy::SessionMemoryCompact,
                CompactionDecision::Considered,
                before_tokens,
                before_messages,
                "manual compact requested",
            ));
            compressor
                .compress_async_with_strategy(
                    &history_before,
                    ContextCompactionStrategy::SessionMemoryCompact,
                )
                .await
        };

        let after_tokens = crate::engine::context_compressor::estimate_messages_tokens(&compressed);
        let (compaction_record, runtime_record) = {
            let mut compressor = self.config.compressor.lock().await;
            let decision = if after_tokens < before_tokens {
                CompactionDecision::Compacted
            } else {
                CompactionDecision::NoGain
            };
            let runtime_record = compressor.latest_compaction_record().cloned();
            let boundary_id = compressor
                .latest_compaction_record()
                .and_then(|record| record.boundary_id.clone());
            let attempt = compressor.record_compaction_decision(
                CompactionAttemptInput::new(
                    "manual compact",
                    ContextCompactionStrategy::SessionMemoryCompact,
                    decision,
                    before_tokens,
                    before_messages,
                    if decision == CompactionDecision::Compacted {
                        "manual compact reduced estimated tokens"
                    } else {
                        "manual compact did not reduce estimated tokens"
                    },
                )
                .with_after(Some(after_tokens), Some(compressed.len()))
                .with_boundary_id(boundary_id),
            );
            (attempt, runtime_record)
        };

        let compacted_for_db = compressed.clone();
        self.set_history(compressed).await;
        if compaction_record.decision == CompactionDecision::Compacted {
            let binding = self.session_binding();
            if let (Some((store, session_id)), Some(record)) =
                (binding.as_ref(), runtime_record.as_ref())
            {
                let _ = store.add_compact_boundary_from_runtime_record(
                    session_id,
                    record,
                    Some("manual compact"),
                    "manual compact requested",
                );
                // Write compaction event to session_events for durable replay.
                let writer =
                    crate::session_store::SessionEventWriter::new(store.shared_conn(), session_id);
                if let Err(err) = writer.compaction(
                    record.strategy.label(),
                    "manual compact",
                    record.tokens_before,
                    record.tokens_after,
                ) {
                    tracing::warn!(
                        "Failed to write compaction event for session {}: {}",
                        session_id,
                        err
                    );
                }
                // Rewrite messages table to the compacted continuation surface
                // so resume/export see the compacted history, not the pre-compaction one.
                if let Err(e) = store.rewrite_session_messages_after_compact(
                    session_id,
                    &compacted_for_db
                        .into_iter()
                        .map(crate::session_store::MessageInsert::from)
                        .collect::<Vec<_>>(),
                ) {
                    tracing::warn!(
                        "Failed to rewrite compacted messages for session {}: {}",
                        session_id,
                        e
                    );
                }
            }
        }
        self.clear_post_compact_transient_state();
        Some(compaction_record)
    }

    fn clear_post_compact_transient_state(&self) {
        crate::tools::file_cache::GLOBAL_FILE_CACHE.clear();
        if let Some(session_id) = self.current_session_id() {
            crate::tools::file_tool::clear_read_files(&session_id);
        }
        // ReadTracker must be cleared on fold — the model's byte-level
        // view of the elided history is gone, so prior reads are stale.
        if let Some(ref tracker) = self.config.read_tracker {
            tracker.reset();
        }
    }

    /// Flush memory extraction for the current conversation history with an explicit lifecycle reason.
    pub async fn flush_memory_for_current_history(&self, reason: crate::memory::MemoryFlushReason) {
        let Some(mem_mutex) = self.memory_manager() else {
            return;
        };
        let Some(session_id) = self.current_session_id() else {
            debug!("Skipping memory flush: no persistent session id is bound");
            return;
        };
        let messages = self.get_history().await;
        if messages.is_empty() {
            return;
        }
        let mut mem = mem_mutex.lock().await;
        mem.flush_session_with_reason_async(session_id, reason, &messages)
            .await;
    }

    /// 设置模型
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.config.model = Arc::new(RwLock::new(model.into()));
        self
    }

    pub fn provider(&self) -> Arc<dyn LlmProvider> {
        self.config.provider.read().clone()
    }

    pub fn set_provider(&self, provider: Arc<dyn LlmProvider>, model: impl Into<String>) {
        *self.config.provider.write() = provider.clone();
        self.set_model(model);
        let model = self.model_name();
        let profile =
            crate::engine::model_context::ModelContextProfile::detect(provider.base_url(), &model);
        if let Ok(mut compressor) = self.config.compressor.try_lock() {
            *compressor =
                crate::engine::context_compressor::ContextCompressor::from_model_context_profile(
                    &profile,
                )
                .with_llm_provider(provider, &model);
        }
    }

    /// 运行时切换模型；下一次请求立即生效。
    pub fn set_model(&self, model: impl Into<String>) {
        *self.config.model.write() = model.into();
    }

    /// 设置系统提示词
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.system_prompt = prompt.into();
        self
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.config.max_iterations = max;
        self
    }

    /// 设置 Agent 管理器
    pub fn with_agent_manager(self, manager: Arc<crate::agent::AgentManager>) -> Self {
        let _ = self.config.agent_manager.set(manager);
        self
    }

    /// 设置 AgentManager 的 lazy 构造依赖。
    pub fn with_agent_query_engine(mut self, query_engine: Arc<super::QueryEngine>) -> Self {
        self.config.agent_manager_query_engine = Some(query_engine);
        self
    }

    /// 设置 MCP 管理器
    pub fn with_mcp_manager(mut self, manager: Arc<crate::engine::mcp::McpManager>) -> Self {
        self.config.mcp_manager = Some(manager);
        self
    }

    /// 获取 MCP 管理器
    pub fn mcp_manager(&self) -> Option<Arc<crate::engine::mcp::McpManager>> {
        self.config.mcp_manager.clone()
    }

    /// 设置 LSP 管理器
    pub fn with_lsp_manager(mut self, manager: Arc<crate::engine::lsp::LspManager>) -> Self {
        self.config.lsp_manager = Some(manager);
        self
    }

    /// 设置 Worktree 管理器
    pub fn with_worktree_manager(
        mut self,
        manager: Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.config.worktree_manager = Some(manager);
        self
    }

    pub fn with_working_dir(mut self, working_dir: impl Into<PathBuf>) -> Self {
        self.config.working_dir_override = Some(working_dir.into());
        self
    }

    pub fn with_lab_context(self, enabled: bool) -> Self {
        self.config
            .lab_context_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
        self
    }

    pub fn set_lab_context_enabled(&self, enabled: bool) {
        self.config
            .lab_context_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn lab_context_enabled(&self) -> bool {
        self.config
            .lab_context_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 设置记忆管理器（覆盖 lazy init）
    pub fn with_memory_manager(
        self,
        manager: Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        let _ = self.config.memory_manager.set(Some(manager));
        self
    }

    /// 设置权限模式
    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.config.permission_mode = Arc::new(RwLock::new(mode));
        self
    }

    /// 设置是否启用 LLM 驱动的记忆提取
    pub fn with_llm_memory_extraction(self, enabled: bool) -> Self {
        self.config
            .llm_memory_extraction
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// Set memory use on/off at runtime (Phase 1)
    pub fn set_memory_use(&self, enabled: bool) {
        self.config
            .memory_use
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    /// Set whether this session can generate future memory proposals.
    pub fn set_memory_generate(&self, enabled: bool) {
        self.config
            .memory_generate
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn memory_use_enabled(&self) -> bool {
        self.config
            .memory_use
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn memory_generate_enabled(&self) -> bool {
        self.config
            .memory_generate
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_memory_recall_mode(&self, mode: impl Into<String>) {
        *self.config.memory_recall_mode.write() = mode.into();
    }

    pub fn memory_recall_mode(&self) -> String {
        self.config.memory_recall_mode.read().clone()
    }

    /// 设置工具授权通道
    pub fn with_approval_channel(
        mut self,
        channel: Arc<crate::engine::conversation_loop::ToolApprovalChannel>,
    ) -> Self {
        self.config.approval_channel = Some(channel);
        self
    }

    /// 设置 fallback 模型
    /// Attach the ReadTracker so it can be cleared on context fold.
    pub fn with_read_tracker(
        mut self,
        tracker: Arc<crate::engine::read_tracker::ReadTracker>,
    ) -> Self {
        self.config.read_tracker = Some(tracker);
        self
    }

    /// Get a reference to the ReadTracker, if attached.
    pub fn read_tracker(&self) -> Option<&Arc<crate::engine::read_tracker::ReadTracker>> {
        self.config.read_tracker.as_ref()
    }

    pub fn with_fallback_model(mut self, model: impl Into<String>) -> Self {
        self.config.fallback_model = Some(model.into());
        self
    }

    /// 获取 fallback 模型名称
    pub fn fallback_model(&self) -> Option<&str> {
        self.config.fallback_model.as_deref()
    }

    /// 运行时更新权限模式（供 TUI 命令调用）
    pub fn set_permission_mode(&self, mode: crate::permissions::PermissionMode) {
        *self.config.permission_mode.write() = mode;
    }

    pub fn permission_mode(&self) -> crate::permissions::PermissionMode {
        *self.config.permission_mode.read()
    }

    pub fn add_session_permission_rule(&self, decision: &str, pattern: &str) {
        let mut rules = self.config.session_permission_rules.write();
        let rule =
            crate::permissions::SourcedRule::new(pattern, crate::permissions::RuleSource::User);
        let target = match decision {
            "allow" => &mut rules.always_allow,
            "deny" => &mut rules.always_deny,
            "ask" => &mut rules.always_ask,
            _ => return,
        };
        if !target.iter().any(|existing| existing.pattern == pattern) {
            target.push(rule);
        }
    }

    pub fn remove_session_permission_rule(&self, decision: &str, pattern: &str) {
        let mut rules = self.config.session_permission_rules.write();
        let target = match decision {
            "allow" => &mut rules.always_allow,
            "deny" => &mut rules.always_deny,
            "ask" => &mut rules.always_ask,
            _ => return,
        };
        target.retain(|existing| {
            !(existing.pattern == pattern
                && existing.source == crate::permissions::RuleSource::User)
        });
    }

    pub fn session_permission_rules(&self) -> crate::permissions::PermissionRules {
        self.config.session_permission_rules.read().clone()
    }

    /// 获取已初始化的记忆管理器，不触发初始化。
    pub fn memory_manager(&self) -> Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>> {
        self.config
            .memory_manager
            .get()
            .and_then(|manager| manager.as_ref().cloned())
    }

    /// 获取或初始化记忆管理器（自动触发 freeze_snapshot）。
    pub fn memory_manager_or_init(
        &self,
    ) -> Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>> {
        self.config
            .memory_manager
            .get_or_init(|| {
                let mut mgr = crate::memory::MemoryManager::new();
                mgr.freeze_snapshot();
                Some(Arc::new(tokio::sync::Mutex::new(mgr)))
            })
            .clone()
    }

    /// 获取上下文压缩器
    pub fn compressor(
        &self,
    ) -> Option<Arc<tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>>> {
        Some(self.config.compressor.clone())
    }

    /// 获取任务管理器
    pub fn task_manager(&self) -> Option<Arc<crate::task_manager::TaskManager>> {
        self.config.task_manager.clone()
    }

    /// 获取 Agent 管理器
    pub fn agent_manager(&self) -> Option<Arc<crate::agent::AgentManager>> {
        self.config.agent_manager.get().cloned()
    }

    /// 获取或按需初始化 Agent 管理器。
    pub fn agent_manager_or_init(&self) -> Option<Arc<crate::agent::AgentManager>> {
        if let Some(manager) = self.agent_manager() {
            return Some(manager);
        }
        let query_engine = self.config.agent_manager_query_engine.as_ref()?.clone();
        Some(
            self.config
                .agent_manager
                .get_or_init(|| {
                    Arc::new(crate::agent::AgentManager::new().with_query_engine(query_engine))
                })
                .clone(),
        )
    }

    /// 获取工具授权通道
    pub fn approval_channel(
        &self,
    ) -> Option<Arc<crate::engine::conversation_loop::ToolApprovalChannel>> {
        self.config.approval_channel.clone()
    }

    /// 获取工具注册表
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.config.tool_registry
    }

    pub async fn context_usage_report(&self) -> ContextUsageReport {
        let history = self.get_history().await;
        let last_user = history
            .iter()
            .rev()
            .find_map(|m| match m {
                Message::User { content } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or("");
        let assembler = if let Some(ref working_dir) = self.config.working_dir_override {
            crate::engine::prompt_context::PromptContextAssembler::new(
                &self.config.system_prompt,
                working_dir,
            )
        } else {
            crate::engine::prompt_context::PromptContextAssembler::from_current_dir(
                &self.config.system_prompt,
            )
        };
        let prompt = assembler.report_for_turn(last_user, &history);
        let history_tokens = crate::engine::context_compressor::estimate_messages_tokens(&history);
        let (tool_count, tool_schema_tokens, tool_schema_fingerprint) =
            estimate_registry_tool_schema_tokens(&self.config.tool_registry);
        let (memory_snapshot_tokens, relevant_memories) =
            if let Some(mem_mutex) = self.memory_manager() {
                let mem = mem_mutex.lock().await;
                (
                    crate::engine::context_compressor::estimate_tokens(&mem.get_snapshot()),
                    mem.preview_relevant_memories(last_user, 5),
                )
            } else {
                (0, Vec::new())
            };
        let max_context_tokens = {
            let comp = self.config.compressor.lock().await;
            comp.stats().max_context_tokens
        };
        let total_estimated_tokens =
            prompt.total_tokens + history_tokens + tool_schema_tokens + memory_snapshot_tokens;

        ContextUsageReport {
            stable_prefix_fingerprint: crate::engine::prompt_context::stable_fingerprint(&format!(
                "{}:{}",
                prompt.stable_prefix_fingerprint, tool_schema_fingerprint
            )),
            prompt,
            history_messages: history.len(),
            history_tokens,
            tool_count,
            tool_schema_tokens,
            tool_schema_fingerprint,
            memory_snapshot_tokens,
            relevant_memories,
            max_context_tokens,
            total_estimated_tokens,
        }
    }

    /// 获取当前模型名
    pub fn model_name(&self) -> String {
        self.config.model.read().clone()
    }

    /// 获取当前 Provider 的 base URL（用于状态展示）
    pub fn provider_base_url(&self) -> String {
        self.provider().base_url().to_string()
    }

    /// 执行流式查询（支持多轮对话）
    ///
    /// 返回一个事件流，调用者可以实时接收响应内容
    /// 自动维护对话历史，上下文不够时自动压缩
    pub async fn query_stream(
        &self,
        user_message: impl Into<String>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        self.query_stream_with_agent_mode(user_message, crate::engine::agent_mode::AgentMode::Auto)
            .await
    }

    pub async fn query_stream_with_agent_mode(
        &self,
        user_message: impl Into<String>,
        agent_mode: crate::engine::agent_mode::AgentMode,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        let user_msg = user_message.into();
        let (tx, rx) = mpsc::channel(100);

        // 准备共享资源
        let history = self.config.conversation_history.clone();
        let compressor = self.config.compressor.clone();
        let binding = self.session_binding();
        let session_store: Option<Arc<crate::session_store::SessionStore>> =
            binding.as_ref().map(|(s, _)| s.clone());
        let session_id = binding.as_ref().map(|(_, id)| id.clone());
        let trace_store = self.config.trace_store.clone();
        let route = crate::engine::intent_router::IntentRouter::new().route(&user_msg);
        let agent_manager = if route_wants_agent_manager(&route) {
            self.agent_manager_or_init()
        } else {
            self.agent_manager()
        };
        let memory_manager = if self.memory_use_enabled() || self.memory_generate_enabled() {
            self.memory_manager_or_init()
        } else {
            None
        };

        let mut engine = StreamingEngineInner {
            provider: self.provider(),
            tool_registry: self.config.tool_registry.clone(),
            model: self.model_name(),
            system_prompt: self.config.system_prompt.clone(),
            max_iterations: self.config.max_iterations,
            agent_manager,
            task_manager: self.config.task_manager.clone(),
            mcp_manager: self.config.mcp_manager.clone(),
            lsp_manager: self.config.lsp_manager.clone(),
            worktree_manager: self.config.worktree_manager.clone(),
            working_dir_override: self.config.working_dir_override.clone(),
            memory_manager,
            compressor: self.config.compressor.clone(),
            session_store: binding.as_ref().map(|(s, _)| s.clone()),
            session_id: binding.as_ref().map(|(_, id)| id.clone()),
            trace_store: trace_store.clone(),
            goal_manager: self.config.goal_manager.clone(),
            cost_tracker: self.config.cost_tracker.clone(),
            permission_mode: self.permission_mode(),
            session_permission_rules: self.config.session_permission_rules.clone(),
            memory_use_enabled: self.memory_use_enabled(),
            memory_generate_enabled: self.memory_generate_enabled(),
            memory_recall_mode: self.memory_recall_mode(),
            llm_memory_extraction: self.memory_generate_enabled()
                && self
                    .config
                    .llm_memory_extraction
                    .load(std::sync::atomic::Ordering::Relaxed),
            approval_channel: self.config.approval_channel.clone(),
            fallback_model: self.config.fallback_model.clone(),
            lab_context_enabled: self.lab_context_enabled(),
            fallback_state: None,
        };

        tokio::spawn(async move {
            // 1. 添加用户消息到历史
            {
                let mut hist = history.lock().await;

                // 持久化用户消息
                if let (Some(ref store), Some(ref sid)) = (&session_store, &session_id) {
                    let msg_count = store.message_count(sid).unwrap_or_default();
                    if msg_count == 0 {
                        // 新会话: 设置标题
                        if let Ok(Some(session)) = store.get_session(sid) {
                            if session.title.trim().is_empty()
                                || matches!(session.title.as_str(), "CLI Session" | "New Session")
                            {
                                let title = session_title_from_user_message(&user_msg);
                                let _ = store.update_session_title(sid, &title);
                            }
                        }
                    } else if hist.is_empty() {
                        // 恢复会话: 从 DB 加载已有的对话历史
                        match store.restore_history(sid) {
                            Ok(restored) if !restored.is_empty() => {
                                debug!(
                                    "Restored {} messages from session {} history",
                                    restored.len(),
                                    sid,
                                );
                                *hist = restored;
                            }
                            Ok(_) => {
                                debug!("Session {} exists but has no stored messages", sid);
                            }
                            Err(e) => {
                                warn!("Failed to restore session {} history: {}", sid, e);
                            }
                        }
                    }
                    if let Err(e) = store.add_message(sid, "user", &user_msg, None, None) {
                        warn!("Failed to persist user message: {}", e);
                    }
                }

                hist.push(Message::user(&user_msg));
            }
            let _ = tx
                .send(StreamEvent::RuntimeDiagnostic {
                    diagnostic: serde_json::json!({
                        "schema": "streaming_stage.v1",
                        "stage": "user_message_recorded",
                    }),
                })
                .await;

            // 2. 检查是否需要压缩
            {
                let hist = history.lock().await;
                let mut comp = compressor.lock().await;
                if comp.needs_compression(&hist) {
                    let before_tokens =
                        crate::engine::context_compressor::estimate_messages_tokens(&hist);
                    if comp.compaction_circuit_open() {
                        comp.record_compaction_decision(CompactionAttemptInput::new(
                            "streaming_history_preflight",
                            ContextCompactionStrategy::AutoCompact,
                            CompactionDecision::CircuitOpen,
                            before_tokens,
                            hist.len(),
                            "compaction circuit open before streaming pre-query compression",
                        ));
                    } else {
                        comp.record_compaction_decision(CompactionAttemptInput::new(
                            "streaming_history_preflight",
                            ContextCompactionStrategy::AutoCompact,
                            CompactionDecision::Considered,
                            before_tokens,
                            hist.len(),
                            "streaming history exceeded compression threshold",
                        ));
                        drop(comp);
                        let pre_compress_flush = engine
                            .memory_manager
                            .as_ref()
                            .zip(engine.session_id.as_ref())
                            .map(|(mem_mutex, sid)| (mem_mutex.clone(), sid.clone(), hist.clone()));
                        drop(hist);
                        if let Some((mem_mutex, sid, pre_compress_history)) = pre_compress_flush {
                            let mut mem = mem_mutex.lock().await;
                            mem.flush_session_with_reason_async(
                                sid,
                                crate::memory::MemoryFlushReason::PreCompress,
                                &pre_compress_history,
                            )
                            .await;
                        }
                        let mut hist = history.lock().await;
                        let mut comp = compressor.lock().await;
                        comp.set_llm_summary_stable_prefix(engine.system_prompt.clone());
                        let compressed = comp.compress_async(&hist).await;
                        let compaction_record = comp.latest_compaction_record().cloned();
                        let after_tokens =
                            crate::engine::context_compressor::estimate_messages_tokens(
                                &compressed,
                            );
                        let decision = if after_tokens < before_tokens {
                            CompactionDecision::Compacted
                        } else {
                            CompactionDecision::NoGain
                        };
                        comp.record_compaction_decision(
                            CompactionAttemptInput::new(
                                "streaming_history_preflight",
                                ContextCompactionStrategy::AutoCompact,
                                decision,
                                before_tokens,
                                hist.len(),
                                if decision == CompactionDecision::Compacted {
                                    "streaming pre-query compression reduced estimated tokens"
                                } else {
                                    "streaming pre-query compression did not reduce estimated tokens"
                                },
                            )
                            .with_after(Some(after_tokens), Some(compressed.len()))
                            .with_boundary_id(
                                compaction_record
                                    .as_ref()
                                    .and_then(|record| record.boundary_id.clone()),
                            ),
                        );
                        *hist = compressed;
                        if let Some(sid) = &engine.session_id {
                            crate::tools::file_tool::clear_read_files(sid);
                        }
                        if let (Some(store), Some(sid), Some(record)) = (
                            &engine.session_store,
                            &engine.session_id,
                            compaction_record.as_ref(),
                        ) {
                            let _ = store.add_compact_boundary_from_runtime_record(
                                sid,
                                record,
                                Some("streaming_history_preflight"),
                                "streaming history compacted before request",
                            );
                            // Write compaction event to session_events for durable replay.
                            let writer = crate::session_store::SessionEventWriter::new(
                                store.shared_conn(),
                                sid,
                            );
                            if let Err(err) = writer.compaction(
                                record.strategy.label(),
                                "streaming_history_preflight",
                                record.tokens_before,
                                record.tokens_after,
                            ) {
                                tracing::warn!(
                                    "Failed to write compaction event for session {}: {}",
                                    sid,
                                    err
                                );
                            }
                        }
                    }
                } else {
                    comp.record_compaction_decision(CompactionAttemptInput::new(
                        "streaming_history_preflight",
                        ContextCompactionStrategy::AutoCompact,
                        CompactionDecision::Skipped,
                        crate::engine::context_compressor::estimate_messages_tokens(&hist),
                        hist.len(),
                        "streaming history below compression threshold",
                    ));
                }
            }
            let _ = tx
                .send(StreamEvent::RuntimeDiagnostic {
                    diagnostic: serde_json::json!({
                        "schema": "streaming_stage.v1",
                        "stage": "preflight_compression_checked",
                    }),
                })
                .await;

            // 3. 获取当前历史用于查询
            let messages_for_query = {
                let hist = history.lock().await;
                build_messages_for_turn(
                    &engine.system_prompt,
                    &user_msg,
                    &hist,
                    agent_mode,
                    engine.working_dir_override.as_deref(),
                )
            };
            let _ = tx
                .send(StreamEvent::RuntimeDiagnostic {
                    diagnostic: serde_json::json!({
                        "schema": "streaming_stage.v1",
                        "stage": "messages_built",
                        "messages": messages_for_query.len(),
                    }),
                })
                .await;

            // 4. 执行查询（带 fallback 支持）
            let mut assistant_content = String::new();
            let mut assistant_tool_calls_made = false;

            let turn_timeout = config::turn_execution_timeout();
            let _ = tx
                .send(StreamEvent::RuntimeDiagnostic {
                    diagnostic: serde_json::json!({
                        "schema": "streaming_stage.v1",
                        "stage": "conversation_loop_starting",
                        "timeout_secs": turn_timeout.as_secs(),
                    }),
                })
                .await;
            let run_result = match tokio::time::timeout(
                turn_timeout,
                engine.run_query_with_messages(messages_for_query.clone(), &tx, agent_mode),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(anyhow::anyhow!(
                    "turn execution timed out after {}s",
                    turn_timeout.as_secs()
                )),
            };

            match run_result {
                Ok((content, tool_calls)) => {
                    assistant_content = content;
                    assistant_tool_calls_made = tool_calls;
                }
                Err(e) => {
                    let mut err_message = e.to_string();
                    let err_str = err_message.to_lowercase();
                    let error_type = ErrorType::from_error_str(&err_str);
                    let mut recovered_by_context_retry = false;

                    if error_type == ErrorType::ContextTooLong {
                        if let Some(retry_messages) = reactive_context_retry_messages(
                            history.clone(),
                            compressor.clone(),
                            &engine.system_prompt,
                            &user_msg,
                            agent_mode,
                            engine.working_dir_override.as_deref(),
                            engine.session_id.as_deref(),
                        )
                        .await
                        {
                            match tokio::time::timeout(
                                turn_timeout,
                                engine.run_query_with_messages(retry_messages, &tx, agent_mode),
                            )
                            .await
                            {
                                Ok(Ok((content, tool_calls))) => {
                                    assistant_content = content;
                                    assistant_tool_calls_made = tool_calls;
                                    recovered_by_context_retry = true;
                                }
                                Ok(Err(retry_err)) => {
                                    err_message = retry_err.to_string();
                                }
                                Err(_) => {
                                    err_message = format!(
                                        "context retry turn execution timed out after {}s",
                                        turn_timeout.as_secs()
                                    );
                                }
                            }
                        }
                    }

                    if !recovered_by_context_retry {
                        let err_str = err_message.to_lowercase();
                        let error_type = ErrorType::from_error_str(&err_str);

                        // 初始化 fallback_state（如果是第一次错误）
                        let fb_state = engine
                            .fallback_state
                            .take()
                            .unwrap_or_else(FallbackState::new);
                        let mut fb_state = fb_state;

                        // 记录错误
                        fb_state.record_error(error_type);

                        // 检查是否应触发 fallback（连续 3 次 529 或特定错误类型）
                        let should_try_fallback = if fb_state.fallback_triggered {
                            // 已触发过 fallback，检查是否还有尝试次数
                            !fb_state.max_attempts_reached()
                        } else {
                            // 检查是否应该触发 fallback
                            fb_state.should_trigger_fallback()
                                || error_type == ErrorType::RateLimit
                                || error_type == ErrorType::ContextTooLong
                                || error_type == ErrorType::ServerError
                        };

                        if should_try_fallback && engine.fallback_model.is_some() {
                            // 如果还没触发过 fallback，标记已触发
                            if !fb_state.fallback_triggered {
                                fb_state.fallback_triggered = true;
                                warn!(
                                "Fallback triggered after {} consecutive errors (type: {:?}), trying fallback model",
                                fb_state.consecutive_529_count,
                                error_type
                            );
                            }
                            fb_state.fallback_attempts += 1;

                            // Fallback: 重新执行，stream 事件会继续发送到 tx
                            let fb_model = engine
                                .fallback_model
                                .clone()
                                .expect("fallback model must be configured for retry");
                            let recovery_plan =
                                crate::engine::recovery_plan::RecoveryPlan::fallback_model(
                                    "streaming_engine",
                                    &err_message,
                                    &fb_model,
                                );
                            if let (Some(ref store), Some(ref sid)) =
                                (&engine.session_store, &engine.session_id)
                            {
                                let _ = store.add_learning_event(
                                    sid,
                                    "recovery_plan",
                                    &recovery_plan.source,
                                    &recovery_plan.summary(),
                                    0.8,
                                    &serde_json::to_value(&recovery_plan)
                                        .unwrap_or_else(|_| serde_json::json!({})),
                                );
                            }
                            let fb_engine = StreamingEngineInner {
                                provider: engine.provider.clone(),
                                tool_registry: engine.tool_registry.clone(),
                                model: fb_model,
                                system_prompt: engine.system_prompt.clone(),
                                max_iterations: engine.max_iterations,
                                agent_manager: engine.agent_manager.clone(),
                                task_manager: engine.task_manager.clone(),
                                mcp_manager: engine.mcp_manager.clone(),
                                lsp_manager: engine.lsp_manager.clone(),
                                worktree_manager: engine.worktree_manager.clone(),
                                working_dir_override: engine.working_dir_override.clone(),
                                memory_manager: engine.memory_manager.clone(),
                                compressor: engine.compressor.clone(),
                                session_store: engine.session_store.clone(),
                                session_id: engine.session_id.clone(),
                                trace_store: engine.trace_store.clone(),
                                goal_manager: engine.goal_manager.clone(),
                                cost_tracker: engine.cost_tracker.clone(),
                                permission_mode: engine.permission_mode,
                                session_permission_rules: engine.session_permission_rules.clone(),
                                memory_use_enabled: engine.memory_use_enabled,
                                memory_generate_enabled: engine.memory_generate_enabled,
                                memory_recall_mode: engine.memory_recall_mode.clone(),
                                llm_memory_extraction: engine.llm_memory_extraction,
                                approval_channel: engine.approval_channel.clone(),
                                fallback_model: None, // 防止无限 fallback
                                lab_context_enabled: engine.lab_context_enabled,
                                fallback_state: Some(fb_state),
                            };
                            let turn_timeout = config::turn_execution_timeout();
                            match tokio::time::timeout(
                                turn_timeout,
                                fb_engine.run_query_with_messages(
                                    messages_for_query.clone(),
                                    &tx,
                                    agent_mode,
                                ),
                            )
                            .await
                            {
                                Ok(Ok((content, tool_calls))) => {
                                    assistant_content = content;
                                    assistant_tool_calls_made = tool_calls;
                                }
                                Ok(Err(fb_err)) => {
                                    let _ = tx.send(StreamEvent::Error(fb_err.to_string())).await;
                                }
                                Err(_) => {
                                    let _ = tx
                                        .send(StreamEvent::Error(format!(
                                            "fallback turn execution timed out after {}s",
                                            turn_timeout.as_secs()
                                        )))
                                        .await;
                                }
                            }
                        } else {
                            let _ = tx.send(StreamEvent::Error(err_message)).await;
                        }
                    }
                }
            }

            // 5. 添加助手回复到历史
            {
                let mut hist = history.lock().await;
                if !assistant_content.is_empty() {
                    let assistant_msg = Message::assistant(&assistant_content);
                    hist.push(assistant_msg.clone());

                    // 持久化助手消息 + 压缩后的完整历史
                    if let (Some(ref store), Some(ref sid)) = (&session_store, &session_id) {
                        if let Err(e) =
                            store.add_message(sid, "assistant", &assistant_content, None, None)
                        {
                            warn!("Failed to persist assistant message: {}", e);
                        }
                        // Sync compressed history to DB so restarted sessions see compressed state.
                        if crate::engine::message_compression::selective_compression_enabled() {
                            if let Err(e) = store.replace_session_messages(sid, &hist) {
                                warn!("Failed to sync compressed history to DB: {}", e);
                            }
                        }
                    }
                } else if assistant_tool_calls_made {
                    warn!("Tool calls were executed but produced no final assistant content to persist");
                }
            }

            // 6. 自动 flush 记忆（每次查询结束后自动写入）
            if let Some(ref mem_mutex) = engine.memory_manager {
                let Some(session_id) = engine.session_id.clone() else {
                    debug!("Skipping memory flush: no persistent session id is bound");
                    return;
                };
                let flush_history = {
                    let hist = history.lock().await;
                    hist.clone()
                };
                flush_session_end_memory_best_effort(
                    mem_mutex.clone(),
                    session_id,
                    flush_history,
                    tx.clone(),
                )
                .await;
            }
        });

        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    /// 执行非流式查询（兼容旧接口）
    pub async fn query(&self, user_message: impl Into<String>) -> Result<String> {
        let mut result = String::new();
        let mut stream = self.query_stream(user_message).await;

        use futures::StreamExt;
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextChunk(text) => result.push_str(&text),
                StreamEvent::Complete => break,
                StreamEvent::Error(e) => return Err(anyhow::anyhow!(e)),
                _ => {}
            }
        }

        Ok(result)
    }
}

/// 内部执行引擎
struct StreamingEngineInner {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    model: String,
    system_prompt: String,
    max_iterations: usize,
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    task_manager: Option<Arc<crate::task_manager::TaskManager>>,
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    working_dir_override: Option<PathBuf>,
    memory_manager: Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>>,
    compressor: Arc<tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>>,
    session_store: Option<Arc<crate::session_store::SessionStore>>,
    session_id: Option<String>,
    trace_store: Arc<crate::engine::trace::TraceStore>,
    goal_manager: Arc<crate::engine::session_goal::SessionGoalManager>,
    cost_tracker: Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    permission_mode: crate::permissions::PermissionMode,
    session_permission_rules: Arc<RwLock<crate::permissions::PermissionRules>>,
    memory_use_enabled: bool,
    memory_generate_enabled: bool,
    memory_recall_mode: String,
    llm_memory_extraction: bool,
    approval_channel: Option<Arc<crate::engine::conversation_loop::ToolApprovalChannel>>,
    fallback_model: Option<String>,
    lab_context_enabled: bool,
    /// Fallback 状态追踪（连续错误计数）
    fallback_state: Option<FallbackState>,
}

impl StreamingEngineInner {
    /// 使用预构建的消息列表执行查询，委托给统一对话循环
    async fn run_query_with_messages(
        &self,
        messages: Vec<Message>,
        tx: &mpsc::Sender<StreamEvent>,
        agent_mode: crate::engine::agent_mode::AgentMode,
    ) -> Result<(String, bool)> {
        let mut builder = super::ConversationLoopBuilder::new(
            self.provider.clone(),
            self.tool_registry.clone(),
            self.cost_tracker.clone(),
            &self.model,
        )
        .with_max_iterations(self.max_iterations)
        .with_permission_mode(self.permission_mode)
        .with_session_permission_rules(self.session_permission_rules.read().clone())
        .with_memory_use(self.memory_use_enabled)
        .with_memory_generate(self.memory_generate_enabled)
        .with_memory_recall_mode(self.memory_recall_mode.clone())
        .with_llm_memory_extraction(self.llm_memory_extraction)
        .with_compressor(self.compressor.clone())
        .with_trace_store(self.trace_store.clone())
        .with_session_goal_manager(self.goal_manager.clone())
        .with_agent_mode(agent_mode)
        .with_lab_context(self.lab_context_enabled);

        if let (Some(ref store), Some(ref session_id)) = (&self.session_store, &self.session_id) {
            builder = builder.with_session_store(store.clone(), session_id.clone());
        }

        if let Some(ref manager) = self.agent_manager {
            builder = builder.with_agent_manager(manager.clone());
        }
        if let Some(ref mcp) = self.mcp_manager {
            builder = builder.with_mcp_manager(mcp.clone());
        }
        if let Some(ref lsp) = self.lsp_manager {
            builder = builder.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            builder = builder.with_worktree_manager(wt.clone());
        }
        if let Some(ref working_dir) = self.working_dir_override {
            builder = builder.with_working_dir(working_dir.clone());
        }
        if let Some(ref mem) = self.memory_manager {
            builder = builder.with_memory_manager(mem.clone());
        }
        if let Some(ref channel) = self.approval_channel {
            builder = builder.with_approval_channel(channel.clone());
        }

        let result = builder.build().run_streaming(messages, tx).await?;
        Ok((result.content, result.tool_calls_made))
    }
}

#[cfg(test)]
mod tests;

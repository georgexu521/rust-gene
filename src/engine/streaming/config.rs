//! StreamingQueryEngine 配置模块
//!
//! 提取 StreamingQueryEngine 的配置字段，消除与 StreamingEngineInner 的重复。

use crate::engine::default_system_prompt;
use crate::engine::QueryEngine;
use crate::services::api::LlmProvider;
use crate::tools::ToolRegistry;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

/// StreamingQueryEngine 配置
///
/// 包含所有可配置的字段，用于构建 StreamingQueryEngine 和 StreamingEngineInner。
pub struct StreamingConfig {
    /// LLM 提供商
    pub provider: Arc<RwLock<Arc<dyn LlmProvider>>>,
    /// 工具注册表
    pub tool_registry: Arc<ToolRegistry>,
    /// 模型名称
    pub model: Arc<RwLock<String>>,
    /// 系统提示词
    pub system_prompt: String,
    /// 最大工具调用迭代次数
    pub max_iterations: usize,
    /// Agent 管理器（按需用于子 Agent 创建）
    pub agent_manager: std::sync::OnceLock<Arc<crate::agent::AgentManager>>,
    /// QueryEngine dependency used to lazily construct AgentManager.
    pub agent_manager_query_engine: Option<Arc<QueryEngine>>,
    /// 任务管理器（可选，用于 task_tool 等）
    pub task_manager: Option<Arc<crate::task_manager::TaskManager>>,
    /// MCP 管理器（可选，用于调用外部 MCP 工具）
    pub mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    /// LSP 管理器（可选，用于 lsp_tool 等）
    pub lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器（可选，用于 worktree_tool 等）
    pub worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    /// Optional working directory override for desktop/worktree runs.
    pub working_dir_override: Option<PathBuf>,
    /// 记忆管理器（lazy init，首次 memory 操作时创建）
    pub memory_manager:
        std::sync::OnceLock<Option<Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>>>,
    /// 对话历史（多轮对话支持）
    pub conversation_history: Arc<tokio::sync::Mutex<Vec<crate::services::api::Message>>>,
    /// 上下文压缩器
    pub compressor: Arc<tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>>,
    /// 会话存储（lazy init，首次 query 时创建）
    pub session_store: std::sync::OnceLock<Option<Arc<crate::session_store::SessionStore>>>,
    /// 禁止 session_store 自动初始化（测试用）
    pub disable_session_auto_init: bool,
    /// Recent runtime traces for `/trace`.
    pub trace_store: Arc<crate::engine::trace::TraceStore>,
    /// Current session goal shown in `/goal` and `/quick`.
    pub goal_manager: Arc<crate::engine::session_goal::SessionGoalManager>,
    /// 当前会话 ID（可运行时切换）
    pub session_id: Arc<RwLock<Option<String>>>,
    /// 成本追踪器
    pub cost_tracker: Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    /// 当前权限模式（可在运行时通过 TUI 命令切换）
    pub permission_mode: Arc<RwLock<crate::permissions::PermissionMode>>,
    /// 当前 CLI 会话内临时权限规则
    pub session_permission_rules: Arc<RwLock<crate::permissions::PermissionRules>>,
    /// Whether existing memory may be used for request context in this session.
    pub memory_use: std::sync::atomic::AtomicBool,
    /// Whether this session may generate future memory proposals/sync output.
    pub memory_generate: std::sync::atomic::AtomicBool,
    /// Dynamic memory recall mode for this session.
    pub memory_recall_mode: Arc<RwLock<String>>,
    /// 是否启用 LLM 驱动的记忆提取（可运行时切换）
    pub llm_memory_extraction: std::sync::atomic::AtomicBool,
    /// 工具授权通道（用于交互式 MCP 授权）
    pub approval_channel: Option<Arc<crate::engine::conversation_loop::ToolApprovalChannel>>,
    /// Fallback 模型名称（当主模型失败时使用）
    pub fallback_model: Option<String>,
    /// Read-before-edit guard — cleared on context fold so stale
    /// read-tracking doesn't survive across compacted history.
    pub read_tracker: Option<Arc<crate::engine::read_tracker::ReadTracker>>,
}

impl StreamingConfig {
    /// 创建新的配置
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        model: impl Into<String>,
    ) -> Self {
        let provider_clone = provider.clone();
        let model = model.into();
        let profile =
            crate::engine::model_context::ModelContextProfile::detect(provider.base_url(), &model);
        Self {
            provider: Arc::new(RwLock::new(provider)),
            tool_registry,
            model: Arc::new(RwLock::new(model.clone())),
            system_prompt: default_system_prompt(),
            max_iterations: 50,
            agent_manager: std::sync::OnceLock::new(),
            agent_manager_query_engine: None,
            task_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            working_dir_override: None,
            memory_manager: std::sync::OnceLock::new(),
            conversation_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            compressor: Arc::new(tokio::sync::Mutex::new(
                crate::engine::context_compressor::ContextCompressor::from_model_context_profile(
                    &profile,
                )
                .with_llm_provider(provider_clone, &model),
            )),
            session_store: std::sync::OnceLock::new(),
            disable_session_auto_init: false,
            trace_store: Arc::new(crate::engine::trace::TraceStore::default()),
            goal_manager: Arc::new(crate::engine::session_goal::SessionGoalManager::new()),
            session_id: Arc::new(RwLock::new(None)),
            cost_tracker: Arc::new(tokio::sync::Mutex::new(
                crate::cost_tracker::CostTracker::new(),
            )),
            permission_mode: Arc::new(RwLock::new(crate::permissions::PermissionMode::AutoAll)),
            session_permission_rules: Arc::new(RwLock::new(
                crate::permissions::PermissionRules::new(),
            )),
            memory_use: std::sync::atomic::AtomicBool::new(true),
            memory_generate: std::sync::atomic::AtomicBool::new(true),
            memory_recall_mode: Arc::new(RwLock::new("balanced".to_string())),
            llm_memory_extraction: std::sync::atomic::AtomicBool::new(false),
            approval_channel: None,
            fallback_model: std::env::var("PRIORITY_AGENT_FALLBACK_MODEL").ok(),
            read_tracker: None,
        }
    }

    /// 获取 Provider
    pub fn provider(&self) -> Arc<dyn LlmProvider> {
        self.provider.read().clone()
    }

    /// 获取模型名称
    pub fn model_name(&self) -> String {
        self.model.read().clone()
    }

    /// 获取当前权限模式
    pub fn permission_mode(&self) -> crate::permissions::PermissionMode {
        *self.permission_mode.read()
    }

    /// 获取记忆使用状态
    pub fn memory_use_enabled(&self) -> bool {
        self.memory_use.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 获取记忆生成状态
    pub fn memory_generate_enabled(&self) -> bool {
        self.memory_generate
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 获取记忆召回模式
    pub fn memory_recall_mode(&self) -> String {
        self.memory_recall_mode.read().clone()
    }

    /// 获取会话权限规则
    pub fn session_permission_rules(&self) -> crate::permissions::PermissionRules {
        self.session_permission_rules.read().clone()
    }
}

/// Turn execution timeout from runtime config.
pub fn turn_execution_timeout() -> std::time::Duration {
    crate::services::config::runtime_config().turn_timeout()
}

/// Session-end memory flush timeout from runtime config.
pub fn session_end_memory_flush_timeout() -> std::time::Duration {
    crate::services::config::runtime_config().session_end_memory_flush_timeout()
}

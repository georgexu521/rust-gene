//! Tool trait 与执行上下文
//!
//! 从 `tools/mod.rs` 拆分出来的 Tool trait、JSON schema 校验函数、
//! ToolContext 及关联类型。

use super::result::{ToolPermissionLevel, ToolResult};
use super::schema::{
    ToolFamily, ToolInterruptBehavior, ToolKind, ToolOperationKind, ToolSchema,
    ToolSearchOrReadSemantics, ToolUiRenderKind,
};
use crate::services::api::ToolCall;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 工具执行上下文
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolContextRetentionItem {
    pub source: String,
    pub title: String,
    pub provenance: String,
    pub reason: String,
    pub trust: String,
    pub conflict: bool,
    pub token_estimate: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolContextSkillTrigger {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub context: Option<String>,
    pub provenance: String,
}

/// Per-turn context retained for tools, hooks, permissions, and subagents.
///
/// This is intentionally metadata-only: prompt-sized memory/skill bodies stay
/// in prompt assembly, while tools get stable provenance about what was kept.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolContextRetainedContext {
    pub query: String,
    pub retrieval_policy: Option<String>,
    pub retrieval_items: Vec<ToolContextRetentionItem>,
    pub skill_triggers: Vec<ToolContextSkillTrigger>,
    pub token_estimate: usize,
    pub provenance: Vec<String>,
}

impl ToolContextRetainedContext {
    pub fn from_retrieval_context(
        query: impl Into<String>,
        context: Option<&crate::engine::retrieval_context::RetrievalContext>,
    ) -> Self {
        let query = query.into();
        let Some(context) = context else {
            return Self {
                query,
                ..Self::default()
            };
        };

        let retrieval_items = context
            .items
            .iter()
            .map(|item| ToolContextRetentionItem {
                source: format!("{:?}", item.source),
                title: item.title.clone(),
                provenance: item.provenance.clone(),
                reason: item.reason.clone(),
                trust: format!("{:?}", item.trust),
                conflict: item.conflict,
                token_estimate: item.token_estimate,
            })
            .collect::<Vec<_>>();
        let mut provenance = context.provenance_summaries();
        provenance.push(format!("retrieval_items={}", retrieval_items.len()));

        Self {
            query,
            retrieval_policy: Some(format!("{:?}", context.policy)),
            retrieval_items,
            skill_triggers: Vec::new(),
            token_estimate: context.token_estimate,
            provenance,
        }
    }

    pub fn with_skill_triggers(mut self, skill_triggers: Vec<ToolContextSkillTrigger>) -> Self {
        if !skill_triggers.is_empty() {
            self.provenance
                .push(format!("skill_triggers={}", skill_triggers.len()));
        }
        self.skill_triggers = skill_triggers;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.retrieval_items.is_empty() && self.skill_triggers.is_empty()
    }
}

#[derive(Clone)]
pub struct ToolContext {
    // ── 核心字段 ──
    /// 当前工作目录
    pub working_dir: std::path::PathBuf,
    /// 会话 ID
    pub session_id: String,
    /// 当前模型名称
    pub model: String,

    // ── 权限 ──
    /// 用户设置（如是否总是允许某类操作）
    pub permissions: ToolPermissions,
    /// 权限上下文（细粒度权限控制）
    pub permission_context: crate::permissions::PermissionContext,

    // ── 额外数据 ──
    /// 额外上下文数据
    pub metadata: HashMap<String, String>,
    /// Per-turn retained memory/skill context visible to tools and hooks.
    pub retained_context: ToolContextRetainedContext,
    /// Tool calls from the parent assistant message that produced this tool round.
    pub parent_assistant_tool_calls: Vec<ToolCall>,
    /// Text content from that parent assistant message, when available.
    pub parent_assistant_content: String,

    // ── 子系统管理器（按需注入） ──
    /// LLM Provider（socratic_analyze、swarm 等需要调用 LLM 的工具）
    pub llm_provider: Option<std::sync::Arc<dyn crate::services::api::LlmProvider>>,
    /// Agent 管理器（agent_tool、send_message_tool 创建子 Agent）
    pub agent_manager: Option<std::sync::Arc<crate::agent::AgentManager>>,
    /// 当前 turn trace（用于工具记录内部生命周期事件）
    pub trace_collector: Option<crate::engine::trace::TraceCollector>,
    /// 会话存储（用于工具持久化运行时 artifact）
    pub session_store: Option<std::sync::Arc<crate::session_store::SessionStore>>,
    /// MCP 管理器（mcp_tool 调用外部 MCP 工具）
    pub mcp_manager: Option<std::sync::Arc<crate::engine::mcp::McpManager>>,
    /// LSP 管理器（lsp_tool 查询语言服务器）
    pub lsp_manager: Option<std::sync::Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器（worktree_tool 管理 git worktree）
    pub worktree_manager: Option<std::sync::Arc<crate::engine::worktree::WorktreeManager>>,
    /// Task 管理器（task_tool 创建和管理任务）
    pub task_manager: Option<std::sync::Arc<crate::internal::task_manager::TaskManager>>,
    /// 成本追踪器（cost_tool 查询 token 和费用统计）
    pub cost_tracker: Option<std::sync::Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>>,
    /// 文件状态缓存（file_read/file_edit 优化与变更检测）
    pub file_cache: Option<std::sync::Arc<crate::tools::file_cache::FileStateCache>>,
    /// 诊断跟踪器（用于 diagnostic tracking 功能）
    pub diagnostic_tracker: Option<std::sync::Arc<crate::engine::DiagnosticTracker>>,
    /// Checkpoint 管理器（文件修改快照）
    pub checkpoint_manager:
        Option<std::sync::Arc<tokio::sync::Mutex<crate::engine::checkpoint::CheckpointManager>>>,
    /// Persistent memory manager shared with the active conversation.
    pub memory_manager: Option<std::sync::Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>>,
    /// Read-before-edit guard — tracks which files the model has read
    /// so edit_file / multi_edit can validate SEARCH text grounding.
    pub read_tracker: Option<std::sync::Arc<crate::engine::read_tracker::ReadTracker>>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("working_dir", &self.working_dir)
            .field("session_id", &self.session_id)
            .field("model", &self.model)
            .field("permissions", &self.permissions)
            .field("metadata", &self.metadata)
            .field("retained_context", &self.retained_context)
            .field(
                "parent_assistant_tool_calls",
                &self.parent_assistant_tool_calls.len(),
            )
            .field(
                "llm_provider",
                &self.llm_provider.as_ref().map(|_| "<LlmProvider>"),
            )
            .field(
                "agent_manager",
                &self.agent_manager.as_ref().map(|_| "<AgentManager>"),
            )
            .field(
                "trace_collector",
                &self.trace_collector.as_ref().map(|_| "<TraceCollector>"),
            )
            .field(
                "session_store",
                &self.session_store.as_ref().map(|_| "<SessionStore>"),
            )
            .field(
                "mcp_manager",
                &self.mcp_manager.as_ref().map(|_| "<McpManager>"),
            )
            .field(
                "lsp_manager",
                &self.lsp_manager.as_ref().map(|_| "<LspManager>"),
            )
            .field(
                "worktree_manager",
                &self.worktree_manager.as_ref().map(|_| "<WorktreeManager>"),
            )
            .field(
                "task_manager",
                &self.task_manager.as_ref().map(|_| "<TaskManager>"),
            )
            .field(
                "file_cache",
                &self.file_cache.as_ref().map(|_| "<FileStateCache>"),
            )
            .field(
                "memory_manager",
                &self.memory_manager.as_ref().map(|_| "<MemoryManager>"),
            )
            .field(
                "read_tracker",
                &self.read_tracker.as_ref().map(|_| "<ReadTracker>"),
            )
            .finish()
    }
}

impl ToolContext {
    pub fn new(working_dir: impl Into<std::path::PathBuf>, session_id: impl Into<String>) -> Self {
        let wd = working_dir.into();
        Self {
            permission_context: crate::permissions::PermissionContext::new(&wd),
            working_dir: wd,
            session_id: session_id.into(),
            model: String::new(),
            permissions: ToolPermissions::default(),
            metadata: HashMap::new(),
            retained_context: ToolContextRetainedContext::default(),
            parent_assistant_tool_calls: Vec::new(),
            parent_assistant_content: String::new(),
            llm_provider: None,
            agent_manager: None,
            trace_collector: None,
            session_store: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            task_manager: None,
            cost_tracker: None,
            file_cache: None,
            diagnostic_tracker: None,
            checkpoint_manager: None,
            memory_manager: None,
            read_tracker: None,
        }
    }

    /// 设置权限模式
    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.permission_context.mode = mode;
        self
    }

    /// 设置 Agent 管理器
    pub fn with_agent_manager(
        mut self,
        manager: std::sync::Arc<crate::agent::AgentManager>,
    ) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    /// 设置当前 turn trace collector
    pub fn with_trace_collector(mut self, trace: crate::engine::trace::TraceCollector) -> Self {
        self.trace_collector = Some(trace);
        self
    }

    /// 设置会话存储
    pub fn with_session_store(
        mut self,
        store: std::sync::Arc<crate::session_store::SessionStore>,
    ) -> Self {
        self.session_store = Some(store);
        self
    }

    /// 设置 LLM Provider
    pub fn with_llm_provider(
        mut self,
        provider: std::sync::Arc<dyn crate::services::api::LlmProvider>,
    ) -> Self {
        self.llm_provider = Some(provider);
        self
    }

    /// 设置模型名称
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// 设置 MCP 管理器
    pub fn with_mcp_manager(
        mut self,
        manager: std::sync::Arc<crate::engine::mcp::McpManager>,
    ) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    /// 设置 LSP 管理器
    pub fn with_lsp_manager(
        mut self,
        manager: std::sync::Arc<crate::engine::lsp::LspManager>,
    ) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    /// 设置 Worktree 管理器
    pub fn with_worktree_manager(
        mut self,
        manager: std::sync::Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    /// 设置 Task 管理器
    pub fn with_task_manager(
        mut self,
        manager: std::sync::Arc<crate::internal::task_manager::TaskManager>,
    ) -> Self {
        self.task_manager = Some(manager);
        self
    }

    /// 设置成本追踪器
    pub fn with_cost_tracker(
        mut self,
        tracker: std::sync::Arc<tokio::sync::Mutex<crate::cost_tracker::CostTracker>>,
    ) -> Self {
        self.cost_tracker = Some(tracker);
        self
    }

    /// 设置文件状态缓存
    pub fn with_file_cache(
        mut self,
        cache: std::sync::Arc<crate::tools::file_cache::FileStateCache>,
    ) -> Self {
        self.file_cache = Some(cache);
        self
    }

    /// 设置诊断跟踪器
    pub fn with_diagnostic_tracker(
        mut self,
        tracker: std::sync::Arc<crate::engine::DiagnosticTracker>,
    ) -> Self {
        self.diagnostic_tracker = Some(tracker);
        self
    }

    /// 设置 Checkpoint 管理器
    pub fn with_checkpoint_manager(
        mut self,
        manager: std::sync::Arc<tokio::sync::Mutex<crate::engine::checkpoint::CheckpointManager>>,
    ) -> Self {
        self.checkpoint_manager = Some(manager);
        self
    }

    /// 设置共享记忆管理器
    pub fn with_memory_manager(
        mut self,
        manager: std::sync::Arc<tokio::sync::Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        self.memory_manager = Some(manager);
        self
    }

    /// Attach the read-before-edit guard so file_read / file_edit / file_write
    /// can participate in the ReadTracker lifecycle.
    pub fn with_read_tracker(
        mut self,
        tracker: std::sync::Arc<crate::engine::read_tracker::ReadTracker>,
    ) -> Self {
        self.read_tracker = Some(tracker);
        self
    }

    /// Attach per-turn retained memory/skill context to downstream tools.
    pub fn with_retained_context(mut self, retained: ToolContextRetainedContext) -> Self {
        self.retained_context = retained;
        self
    }

    /// Attach the current provider tool-call identifiers to downstream tools.
    pub fn with_tool_call_metadata(
        mut self,
        tool_name: impl Into<String>,
        tool_call_id: impl Into<String>,
    ) -> Self {
        self.metadata.insert("tool_name".into(), tool_name.into());
        self.metadata
            .insert("tool_call_id".into(), tool_call_id.into());
        self
    }

    /// Attach the parent assistant tool-use round for forked subagent context.
    pub fn with_parent_assistant_tool_calls(
        mut self,
        tool_calls: Vec<ToolCall>,
        assistant_content: impl Into<String>,
    ) -> Self {
        self.parent_assistant_tool_calls = tool_calls;
        self.parent_assistant_content = assistant_content.into();
        self
    }
}

/// 工具权限设置
#[derive(Debug, Clone, Default)]
pub struct ToolPermissions {
    /// 总是允许读文件
    pub allow_all_reads: bool,
    /// 总是允许写文件
    pub allow_all_writes: bool,
    /// 总是允许执行命令
    pub allow_all_bash: bool,
    /// 只读模式（禁止任何写入）
    pub read_only: bool,
}

// ── Tool trait ──

#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 工具参数 JSON Schema
    fn parameters(&self) -> Value;

    /// 执行工具
    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult;

    /// Whether this tool is currently usable with the provided runtime context.
    ///
    /// Tools that depend on optional managers should return false here so they
    /// are not advertised to the model or command UI when the backing subsystem
    /// is not wired for the current session.
    fn is_available(&self, _context: &ToolContext) -> bool {
        true
    }

    /// Human-readable reason for an unavailable tool.
    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        None
    }

    /// 是否需要用户确认
    fn requires_confirmation(&self, _params: &Value) -> bool {
        false
    }

    /// 获取确认提示信息
    fn confirmation_prompt(&self, _params: &Value) -> Option<String> {
        None
    }

    /// 为安全分类器提供精简输入
    ///
    /// 返回工具的精简参数摘要，用于 LLM 分类器 transcript。
    /// 返回空字符串表示该工具不需要分类器审查。
    fn to_classifier_input(&self, params: &Value) -> String {
        // 默认实现：返回工具名 + 参数键列表
        let keys: Vec<String> = params
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        format!("{}({})", self.name(), keys.join(", "))
    }

    /// Optional JSON schema for the structured result payload (`ToolResult.data`).
    fn output_schema(&self) -> Option<Value> {
        None
    }

    /// Runtime operation category for this invocation.
    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        let name = self.name().to_ascii_lowercase();
        if name.contains("grep") || name.contains("glob") || name.contains("search") {
            ToolOperationKind::Search
        } else if name.contains("read") || name.contains("get") {
            ToolOperationKind::Read
        } else if name.contains("list") {
            ToolOperationKind::List
        } else if name.contains("write") || name.contains("create") {
            ToolOperationKind::Write
        } else if name.contains("edit") || name.contains("update") {
            ToolOperationKind::Edit
        } else if name.contains("patch") {
            ToolOperationKind::Patch
        } else if name.contains("bash") || name.contains("shell") || name.contains("exec") {
            ToolOperationKind::Shell
        } else if name.contains("task") || name.contains("agent") {
            ToolOperationKind::Task
        } else if name.contains("web") || name.contains("browser") || name.contains("http") {
            ToolOperationKind::Network
        } else {
            ToolOperationKind::Other
        }
    }

    /// Provider/UI-facing tool kind for protocol adapters and compact traces.
    fn tool_kind(&self, params: &Value) -> ToolKind {
        let name = self.name().to_ascii_lowercase();
        match self.operation_kind(params) {
            ToolOperationKind::Shell => ToolKind::Execute,
            ToolOperationKind::Write | ToolOperationKind::Edit | ToolOperationKind::Patch => {
                ToolKind::Edit
            }
            ToolOperationKind::Search | ToolOperationKind::List => ToolKind::Search,
            ToolOperationKind::Read => ToolKind::Read,
            ToolOperationKind::Task => ToolKind::Think,
            ToolOperationKind::Network if name.contains("fetch") => ToolKind::Fetch,
            ToolOperationKind::Network if name.contains("search") => ToolKind::Search,
            ToolOperationKind::Network => ToolKind::Fetch,
            ToolOperationKind::Other => ToolKind::Other,
        }
    }

    /// Broad permission/product family. This is intentionally separate from
    /// side-effect policy, which can classify individual invocations more deeply.
    fn tool_family(&self, params: &Value) -> ToolFamily {
        let name = self.name().to_ascii_lowercase();
        if name.starts_with("mcp") || name.contains("mcp_") {
            return ToolFamily::Mcp;
        }
        if name.starts_with("plugin") || name.contains("plugin_") {
            return ToolFamily::Plugin;
        }

        match self.operation_kind(params) {
            ToolOperationKind::Write | ToolOperationKind::Edit | ToolOperationKind::Patch => {
                ToolFamily::Edit
            }
            ToolOperationKind::Shell => ToolFamily::Shell,
            ToolOperationKind::Read | ToolOperationKind::List => ToolFamily::Read,
            ToolOperationKind::Search => ToolFamily::Search,
            ToolOperationKind::Task => ToolFamily::Task,
            ToolOperationKind::Network => ToolFamily::Network,
            ToolOperationKind::Other => ToolFamily::Other,
        }
    }

    /// Backward-compatible names that should resolve to this tool.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Short keyword phrase used by tool_search when this tool is deferred.
    fn search_hint(&self) -> Option<&'static str> {
        None
    }

    /// Whether this tool should be hidden behind tool_search when available.
    fn should_defer(&self) -> bool {
        false
    }

    /// Whether this tool must always be sent even when tool search is active.
    fn always_load(&self) -> bool {
        false
    }

    /// Whether compatible providers should request strict schema adherence.
    fn strict_schema(&self) -> bool {
        false
    }

    /// How to handle user interruption while this tool is running.
    fn interrupt_behavior(&self) -> ToolInterruptBehavior {
        ToolInterruptBehavior::Block
    }

    /// Whether execution requires a user-facing interaction.
    fn requires_user_interaction(&self) -> bool {
        false
    }

    /// Whether this invocation can reach outside a bounded local context.
    fn is_open_world(&self, params: &Value) -> bool {
        matches!(self.operation_kind(params), ToolOperationKind::Network)
    }

    /// Whether this invocation should be treated as search/read/list UI evidence.
    fn is_search_or_read_command(&self, params: &Value) -> ToolSearchOrReadSemantics {
        match self.operation_kind(params) {
            ToolOperationKind::Search => ToolSearchOrReadSemantics {
                is_search: true,
                ..Default::default()
            },
            ToolOperationKind::Read => ToolSearchOrReadSemantics {
                is_read: true,
                ..Default::default()
            },
            ToolOperationKind::List => ToolSearchOrReadSemantics {
                is_list: true,
                ..Default::default()
            },
            _ => ToolSearchOrReadSemantics::default(),
        }
    }

    /// Paths or path-like arguments referenced by this invocation.
    fn input_paths(&self, params: &Value) -> Vec<String> {
        ["path", "file_path", "directory", "working_dir"]
            .iter()
            .filter_map(|key| params.get(*key).and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Stable input used by permission matchers and permission summaries.
    fn permission_matcher_input(&self, params: &Value) -> Option<String> {
        let paths = self.input_paths(params);
        if paths.is_empty() {
            let classifier_input = self.to_classifier_input(params);
            (!classifier_input.trim().is_empty()).then_some(classifier_input)
        } else {
            Some(paths.join(","))
        }
    }

    /// Mutates an observer-only copy of input before hooks/transcript metadata.
    fn backfill_observable_input(&self, _input: &mut Value) {}

    /// Tool invocation text for transcript search and compact history.
    fn transcript_summary(&self, params: &Value) -> Option<String> {
        self.tool_use_summary(params)
    }

    /// Preferred UI rendering lane for this invocation.
    fn ui_render_kind(&self, params: &Value) -> ToolUiRenderKind {
        match self.operation_kind(params) {
            ToolOperationKind::Read
            | ToolOperationKind::Write
            | ToolOperationKind::Edit
            | ToolOperationKind::Patch => ToolUiRenderKind::File,
            ToolOperationKind::Shell => ToolUiRenderKind::Shell,
            ToolOperationKind::Search | ToolOperationKind::List => ToolUiRenderKind::Search,
            ToolOperationKind::Task => ToolUiRenderKind::Task,
            ToolOperationKind::Network => ToolUiRenderKind::Network,
            ToolOperationKind::Other => ToolUiRenderKind::Generic,
        }
    }

    /// Whether this invocation only observes state and can avoid write budget.
    fn is_read_only(&self, params: &Value) -> bool {
        matches!(
            self.operation_kind(params),
            ToolOperationKind::Read | ToolOperationKind::Search | ToolOperationKind::List
        ) || self.permission_level() == ToolPermissionLevel::ReadOnly
    }

    /// Whether this invocation can run while a model response is still streaming.
    fn is_concurrency_safe(&self, params: &Value) -> bool {
        self.is_read_only(params)
    }

    /// Whether the invocation can destroy or overwrite user data.
    fn is_destructive(&self, params: &Value) -> bool {
        self.requires_confirmation(params)
            && matches!(
                self.permission_level(),
                ToolPermissionLevel::HighRisk | ToolPermissionLevel::Critical
            )
    }

    /// Preferred maximum provider-visible result size for this tool.
    fn max_result_size_chars(&self) -> Option<usize> {
        None
    }

    /// Human-facing display name for this invocation.
    fn user_facing_name(&self, _params: &Value) -> String {
        self.name().to_string()
    }

    /// Short invocation summary suitable for progress events and ledgers.
    fn tool_use_summary(&self, _params: &Value) -> Option<String> {
        None
    }

    /// Progress text for active invocation state.
    fn activity_description(&self, params: &Value) -> Option<String> {
        self.tool_use_summary(params)
            .map(|summary| format!("{}: {}", self.user_facing_name(params), summary))
    }

    /// Provider-visible payload for the result. Existing normalizers still own
    /// formatting, but this hook gives tools a Claude-like escape hatch.
    fn provider_payload(&self, result: &ToolResult) -> String {
        if result.content.trim().is_empty() {
            result
                .error
                .clone()
                .unwrap_or_else(|| "Tool returned no output".to_string())
        } else {
            result.content.clone()
        }
    }

    /// 验证参数是否符合 schema（返回 None 表示验证通过，Some(msg) 表示错误）
    fn validate_params(&self, params: &Value) -> Option<String> {
        validate_json_schema_value(params, &self.parameters(), "")
    }

    /// 渲染工具结果（用于 TUI 展示）
    fn render_result(&self, result: &ToolResult) -> String {
        if result.success {
            // 成功结果：截断长输出，保留关键部分
            let content = &result.content;
            if content.len() > 2000 {
                let safe: String = content.chars().take(2000).collect();
                format!(
                    "{}\n\n[Output truncated - {} bytes total]",
                    safe,
                    content.len()
                )
            } else {
                content.clone()
            }
        } else {
            // 错误结果：显示完整错误
            result.content.clone()
        }
    }

    /// 获取支持的错误码列表
    fn error_codes(&self) -> Vec<String> {
        vec![
            "unknown".to_string(),
            "invalid_params".to_string(),
            "permission_denied".to_string(),
            "timeout".to_string(),
            "execution_failed".to_string(),
        ]
    }

    /// 获取权限等级（默认 LowRisk）
    fn permission_level(&self) -> ToolPermissionLevel {
        ToolPermissionLevel::LowRisk
    }

    /// 是否幂等（相同参数重复执行结果相同）
    fn is_idempotent(&self) -> bool {
        false
    }

    /// 是否可重试（默认可重试）
    fn is_retryable(&self) -> bool {
        true
    }

    /// 预估执行时间（毫秒）
    fn estimated_duration_ms(&self) -> Option<u64> {
        None
    }

    /// 获取工具 schema
    fn schema(&self) -> ToolSchema
    where
        Self: Sized,
    {
        ToolSchema::from_tool(self)
    }
}

// ── JSON Schema 校验函数 ──

pub fn validate_json_schema_value(value: &Value, schema: &Value, path: &str) -> Option<String> {
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        if branches
            .iter()
            .any(|branch| validate_json_schema_value(value, branch, path).is_none())
        {
            return None;
        }
        return Some(format!(
            "{} does not match any allowed schema",
            parameter_label(path)
        ));
    }

    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        let matches = branches
            .iter()
            .filter(|branch| validate_json_schema_value(value, branch, path).is_none())
            .count();
        if matches == 1 {
            return None;
        }
        return Some(format!(
            "{} must match exactly one allowed schema, matched {}",
            parameter_label(path),
            matches
        ));
    }

    let allowed_types = schema_allowed_types(schema);
    if !allowed_types.is_empty()
        && !allowed_types
            .iter()
            .any(|type_name| type_name == "any" || schema_type_matches(type_name, value))
    {
        return Some(format!(
            "{} must be of type {}, got {}",
            parameter_label(path),
            allowed_types.join("|"),
            json_value_type(value)
        ));
    }

    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
        if !enum_values.iter().any(|allowed| allowed == value) {
            return Some(format!(
                "{} must be one of {}, got {}",
                parameter_label(path),
                enum_values
                    .iter()
                    .map(json_value_preview)
                    .collect::<Vec<_>>()
                    .join(", "),
                json_value_preview(value)
            ));
        }
    }

    if let Some(const_value) = schema.get("const") {
        if const_value != value {
            return Some(format!(
                "{} must be {}, got {}",
                parameter_label(path),
                json_value_preview(const_value),
                json_value_preview(value)
            ));
        }
    }

    match value {
        Value::Object(object) => validate_json_schema_object(object, schema, path),
        Value::Array(items) => validate_json_schema_array(items, schema, path),
        Value::String(text) => validate_json_schema_string(text, schema, path),
        Value::Number(number) => validate_json_schema_number(number, schema, path),
        Value::Null | Value::Bool(_) => None,
    }
}

pub fn validate_json_schema_object(
    object: &serde_json::Map<String, Value>,
    schema: &Value,
    path: &str,
) -> Option<String> {
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for key in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(key) {
                return Some(format!(
                    "Missing required parameter: {}",
                    nested_path(path, key)
                ));
            }
        }
    }

    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (key, property_schema) in properties {
            if let Some(property_value) = object.get(key) {
                let property_path = nested_path(path, key);
                if let Some(error) =
                    validate_json_schema_value(property_value, property_schema, &property_path)
                {
                    return Some(error);
                }
            }
        }
    }

    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {
            if let Some(properties) = properties {
                if let Some(unknown) = object.keys().find(|key| !properties.contains_key(*key)) {
                    return Some(format!("Unknown parameter: {}", nested_path(path, unknown)));
                }
            }
        }
        Some(additional_schema) if additional_schema.is_object() => {
            if let Some(properties) = properties {
                for (key, item) in object
                    .iter()
                    .filter(|(key, _)| !properties.contains_key(*key))
                {
                    let property_path = nested_path(path, key);
                    if let Some(error) =
                        validate_json_schema_value(item, additional_schema, &property_path)
                    {
                        return Some(error);
                    }
                }
            }
        }
        _ => {}
    }

    if let Some(min) = schema.get("minProperties").and_then(Value::as_u64) {
        if (object.len() as u64) < min {
            return Some(format!(
                "{} must have at least {} properties, got {}",
                parameter_label(path),
                min,
                object.len()
            ));
        }
    }
    if let Some(max) = schema.get("maxProperties").and_then(Value::as_u64) {
        if (object.len() as u64) > max {
            return Some(format!(
                "{} must have at most {} properties, got {}",
                parameter_label(path),
                max,
                object.len()
            ));
        }
    }

    None
}

pub fn validate_json_schema_array(items: &[Value], schema: &Value, path: &str) -> Option<String> {
    if let Some(min) = schema.get("minItems").and_then(Value::as_u64) {
        if (items.len() as u64) < min {
            return Some(format!(
                "{} must contain at least {} items, got {}",
                parameter_label(path),
                min,
                items.len()
            ));
        }
    }
    if let Some(max) = schema.get("maxItems").and_then(Value::as_u64) {
        if (items.len() as u64) > max {
            return Some(format!(
                "{} must contain at most {} items, got {}",
                parameter_label(path),
                max,
                items.len()
            ));
        }
    }

    match schema.get("items") {
        Some(item_schema) if item_schema.is_object() => {
            for (index, item) in items.iter().enumerate() {
                if let Some(error) =
                    validate_json_schema_value(item, item_schema, &format!("{path}[{index}]"))
                {
                    return Some(error);
                }
            }
        }
        Some(Value::Array(tuple_schemas)) => {
            for (index, item_schema) in tuple_schemas.iter().enumerate() {
                if let Some(item) = items.get(index) {
                    if let Some(error) =
                        validate_json_schema_value(item, item_schema, &format!("{path}[{index}]"))
                    {
                        return Some(error);
                    }
                }
            }
        }
        _ => {}
    }

    None
}

pub fn validate_json_schema_string(text: &str, schema: &Value, path: &str) -> Option<String> {
    let char_count = text.chars().count() as u64;
    if let Some(min) = schema.get("minLength").and_then(Value::as_u64) {
        if char_count < min {
            return Some(format!(
                "{} must be at least {} characters, got {}",
                parameter_label(path),
                min,
                char_count
            ));
        }
    }
    if let Some(max) = schema.get("maxLength").and_then(Value::as_u64) {
        if char_count > max {
            return Some(format!(
                "{} must be at most {} characters, got {}",
                parameter_label(path),
                max,
                char_count
            ));
        }
    }
    None
}

pub fn validate_json_schema_number(
    number: &serde_json::Number,
    schema: &Value,
    path: &str,
) -> Option<String> {
    let value = number.as_f64()?;
    if let Some(min) = schema.get("minimum").and_then(Value::as_f64) {
        if value < min {
            return Some(format!(
                "{} must be >= {}, got {}",
                parameter_label(path),
                min,
                number
            ));
        }
    }
    if let Some(max) = schema.get("maximum").and_then(Value::as_f64) {
        if value > max {
            return Some(format!(
                "{} must be <= {}, got {}",
                parameter_label(path),
                max,
                number
            ));
        }
    }
    if let Some(exclusive_min) = schema.get("exclusiveMinimum").and_then(Value::as_f64) {
        if value <= exclusive_min {
            return Some(format!(
                "{} must be > {}, got {}",
                parameter_label(path),
                exclusive_min,
                number
            ));
        }
    }
    if let Some(exclusive_max) = schema.get("exclusiveMaximum").and_then(Value::as_f64) {
        if value >= exclusive_max {
            return Some(format!(
                "{} must be < {}, got {}",
                parameter_label(path),
                exclusive_max,
                number
            ));
        }
    }
    None
}

fn schema_allowed_types(schema: &Value) -> Vec<String> {
    match schema.get("type") {
        Some(Value::String(type_name)) => vec![type_name.clone()],
        Some(Value::Array(types)) => types
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn schema_type_matches(type_name: &str, value: &Value) -> bool {
    match (type_name, value) {
        ("null", Value::Null) => true,
        ("boolean", Value::Bool(_)) => true,
        ("string", Value::String(_)) => true,
        ("array", Value::Array(_)) => true,
        ("object", Value::Object(_)) => true,
        ("number", Value::Number(_)) => true,
        ("integer", Value::Number(number)) => number.is_i64() || number.is_u64(),
        _ => false,
    }
}

fn json_value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn json_value_preview(value: &Value) -> String {
    match value {
        Value::String(text) => format!("\"{}\"", text.chars().take(40).collect::<String>()),
        _ => value.to_string(),
    }
}

fn parameter_label(path: &str) -> String {
    if path.is_empty() {
        "Parameters".to_string()
    } else {
        format!("Parameter '{path}'")
    }
}

fn nested_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_string()
    } else {
        format!("{parent}.{key}")
    }
}

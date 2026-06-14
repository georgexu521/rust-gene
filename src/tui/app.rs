//! 交互式终端 CLI 应用状态管理
//!
//! 对应 Claude Code 中的 AppState 概念

use crate::engine::agent_mode::AgentMode;
use crate::engine::conversation_loop::ToolApprovalResponse;
use crate::engine::runtime_controller::RuntimeController;
use crate::engine::runtime_facade::{RuntimeFacadeState, RuntimeStateSnapshot, ToolTurnPhase};
use crate::engine::streaming::StreamingQueryEngine;
use crate::permissions::RuleSource;
use crate::state::{
    select_runtime_status, select_tool_viewer_tool_id, AppContext, AppState, MessageItem,
    MessageRole, RuntimeAppState, RuntimeBridgeState, RuntimeMcpState, RuntimePermissionState,
    RuntimeStatusSnapshot, RuntimeToolStatus, TaskItem,
};
use crate::tui::components::input::InputState;
use crate::tui::sync_store::{TuiSyncSnapshot, TuiSyncStore};
use crate::tui::tool_view::{ToolRunStatus, ToolRunView};
use crate::workspace::Workspace;
use futures::StreamExt;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::commands::{default_command_registry, CommandRegistry};

mod actions;
mod memory;
mod palette;
mod permission_diff;
mod runtime;
mod slash_commands;
mod status_tools;
use memory::*;
pub use runtime::StreamUsageSnapshot;
use runtime::*;
pub(crate) use runtime::{
    parse_permission_mode, permission_mode_name, permission_rule_pattern, persist_permission_rule,
};

/// Auto-dismissing toast notification (Reasonix-style)
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub glyph: &'static str,
    pub color: ratatui::style::Color,
    pub expires_at_tick: usize,
}

const LONG_PASTE_CHAR_THRESHOLD: usize = 600;
const LONG_PASTE_LINE_THRESHOLD: usize = 12;

#[derive(Debug, Clone)]
struct PastedBlock {
    placeholder: String,
    content: String,
}

#[derive(Debug, Clone)]
struct PendingSkillInvocation {
    name: String,
    version: String,
    started_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Chat,
    Settings,
    PlanApproval,
    PermissionApproval,
    AskUser,
    DiffViewer,
    ToolViewer,
    VimNormal,
    Onboarding,
    MessageSearch,
    CommandPalette,
    ShortcutHelp,
    PromptHistory,
    ModelSelect,
    ProviderSelect,
    FilePicker,
}

/// Pending leader-key sequence state.
#[derive(Debug, Clone)]
pub struct LeaderState {
    pub started_at: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelChoice {
    pub provider: String,
    pub model: String,
    pub note: String,
    pub active: bool,
}

/// 侧边栏面板类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarPanel {
    Sessions,
    Context,
}

impl SidebarPanel {
    pub fn next(self) -> Self {
        match self {
            Self::Sessions => Self::Context,
            Self::Context => Self::Sessions,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Sessions => "Sessions",
            Self::Context => "Context",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderChoice {
    pub name: String,
    pub provider_type: String,
    pub model: String,
    pub base_url: String,
    pub configured: bool,
    pub active: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarDensity {
    Compact,
    Normal,
    Debug,
}

impl StatusBarDensity {
    pub fn next(self) -> Self {
        match self {
            Self::Compact => Self::Normal,
            Self::Normal => Self::Debug,
            Self::Debug => Self::Compact,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Normal => "normal",
            Self::Debug => "debug",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "compact" | "min" | "minimal" => Some(Self::Compact),
            "normal" | "default" => Some(Self::Normal),
            "debug" | "verbose" | "full" => Some(Self::Debug),
            _ => None,
        }
    }
}

/// 交互式 CLI 应用状态
pub struct TuiApp {
    /// 当前模式（兼容字段；正在迁移到 `mode_stack`）
    pub mode: AppMode,
    /// 模式栈，用于 overlay 的进入/返回
    pub mode_stack: Vec<AppMode>,
    /// Leader-key 等待状态
    pub leader_state: Option<LeaderState>,
    /// 当前检测到的 workspace
    pub workspace: Workspace,
    /// 当前 coding agent 产品模式
    pub agent_mode: AgentMode,
    /// 输入状态
    pub input: InputState,
    /// 消息列表
    pub messages: Vec<MessageItem>,
    /// 任务列表
    pub tasks: Vec<TaskItem>,
    /// 是否正在查询中
    pub is_querying: bool,
    /// Streaming start time for t/s calculation
    pub stream_started_at: Option<std::time::Instant>,
    /// Toast notifications (auto-dismiss)
    pub toasts: Vec<Toast>,
    /// Session memory controls (Phase 1)
    pub memory_use: bool,
    pub memory_generate: bool,
    pub memory_recall_mode: String,
    /// 是否处于暂停态（不接受新消息发送）
    pub paused: bool,
    /// 是否启用聚焦模式（仅显示 user/assistant）
    pub focus_mode: bool,
    /// 状态栏信息密度
    pub status_bar_density: StatusBarDensity,
    /// Session run coordinator (Phase 4)
    pub run_coordinator: crate::engine::run_coordinator::SessionRunCoordinator,
    /// 命令注册表
    pub command_registry: CommandRegistry,
    /// 轻量 KV 偏好存储
    pub kv_store: crate::services::kv::KvStore,
    /// 滚动位置
    pub scroll_offset: usize,
    /// Stable timeline item id for manual scroll anchors.
    pub scroll_anchor_id: Option<String>,
    /// 是否自动贴底（用户手动上滚后变为 false，滚到底或新消息时恢复）
    pub pinned_to_bottom: bool,
    /// 应用上下文
    pub context: AppContext,
    /// 最后错误信息
    pub error_message: Option<String>,
    /// 命令历史
    pub history: VecDeque<String>,
    /// 历史索引
    pub history_index: Option<usize>,
    /// Temporarily stashed prompt draft for composer workflows.
    pub prompt_stash: Option<String>,
    /// Selected item in the prompt history/stash picker.
    pub prompt_picker_selected: usize,
    /// 流式查询引擎
    pub streaming_engine: Option<Arc<StreamingQueryEngine>>,
    /// Shared runtime-state snapshot used by status/tool selectors.
    pub runtime_state_snapshot: RuntimeAppState,
    /// Shared product runtime facade snapshot for cross-frontend migration.
    pub runtime_facade_state: RuntimeFacadeState,
    /// TUI-local sync/projection store. Runtime stream events feed this store;
    /// render-compatible fields are refreshed from its snapshot.
    sync_store: Arc<Mutex<TuiSyncStore>>,
    pub sync_snapshot: TuiSyncSnapshot,
    current_tool_anchor_id: Option<String>,
    /// 是否展开工具 transcript 细节
    pub transcript_expanded: bool,
    /// 当前展开的单个工具 id；None 表示全部折叠
    pub expanded_tool_run_id: Option<String>,
    /// 当前展开 reasoning 正文的 assistant message id；None 表示只显示摘要
    pub expanded_reasoning_message_id: Option<String>,
    /// Inline-expanded tool bodies keyed by `tool_call_id`.
    pub expanded_inline_tool_ids: BTreeSet<String>,
    /// Inline-expanded assistant text parts keyed by `TuiMessagePart.id`.
    pub expanded_inline_message_part_ids: BTreeSet<String>,
    stream_usage: Arc<Mutex<Option<StreamUsageSnapshot>>>,
    pub stream_usage_snapshot: Option<StreamUsageSnapshot>,
    /// Cached facade snapshot for synchronous rendering
    pub facade_snapshot: RuntimeStateSnapshot,
    /// 流是否已完成（由后台任务设置）
    stream_done: Arc<AtomicBool>,
    /// 后台流式任务句柄（用于取消）
    stream_handle: Option<tokio::task::JoinHandle<()>>,
    /// 会话管理器
    pub session_manager: crate::tui::session_manager::TuiSessionManager,
    /// 设置状态
    pub settings_state: Option<crate::tui::components::settings::SettingsState>,
    /// 待审批的计划
    pub pending_plan: Option<crate::engine::plan_mode::Plan>,
    /// 计划审批响应发送器
    pub plan_response_tx:
        Option<tokio::sync::oneshot::Sender<crate::engine::plan_mode::PlanApproval>>,
    /// 计划修改输入缓冲
    pub plan_modification_input: String,
    /// 待审批的工具权限请求
    pub pending_permission_request: Option<crate::engine::conversation_loop::ToolApprovalRequest>,
    /// 工具权限审批响应发送器
    pub permission_response_tx: Option<tokio::sync::oneshot::Sender<ToolApprovalResponse>>,
    /// 待回答的用户问题
    pub pending_question: Option<String>,
    /// 用户问题的选项
    pub pending_question_options: Vec<String>,
    /// 用户问题响应发送器
    pub question_response_tx: Option<tokio::sync::oneshot::Sender<String>>,
    /// Diff 查看器内容
    pub diff_content: String,
    /// Diff 查看器标题
    pub diff_title: String,
    /// Diff 查看器滚动偏移
    pub diff_scroll_offset: u16,
    /// 工具输出查看器内容
    pub tool_viewer_content: String,
    /// 工具输出查看器标题
    pub tool_viewer_title: String,
    /// 工具输出查看器滚动偏移
    pub tool_viewer_scroll_offset: u16,
    /// 消息搜索状态
    pub message_search_state: crate::tui::components::message_search::MessageSearchState,
    /// 折叠的消息索引（Vim Normal 模式下 Tab 折叠/展开）
    pub collapsed_indices: std::collections::HashSet<usize>,
    /// 会话侧边栏是否可见
    pub sidebar_visible: bool,
    /// 侧边栏选中索引
    pub sidebar_selected: usize,
    /// 侧边栏搜索筛选文本
    pub sidebar_filter: String,
    /// 已固定的会话 ID 列表（最多 9 个）
    pub pinned_sessions: Vec<String>,
    /// 侧边栏删除确认（二次按 D 才执行）
    pub confirm_delete_session_id: Option<String>,
    /// 侧边栏重命名会话 ID
    pub renaming_session_id: Option<String>,
    /// 侧边栏重命名输入缓冲
    pub rename_buffer: String,
    /// 是否正在侧边栏搜索模式
    pub filtering_sidebar: bool,
    /// 快捷键帮助搜索筛选
    pub shortcut_help_filter: String,
    /// 是否正在快捷键帮助搜索模式
    pub filtering_shortcut_help: bool,
    /// 侧边栏面板类型
    pub sidebar_panel: SidebarPanel,
    /// 打字机效果当前显示位置（字符数）
    pub typewriter_position: usize,
    /// LSP 管理器
    pub lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器
    pub worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    /// CLI app start time for uptime and diagnostics.
    pub app_started_at: std::time::Instant,
    /// Bundled skills
    pub bundled_skills: std::collections::HashMap<String, crate::skills::Skill>,
    /// Goal runner (lazily initialized when engine is available)
    pub goal_runner: Option<crate::engine::goal::runner::GoalRunner>,
    /// Pending goal prompt — set by `/goal <objective>` to trigger first turn
    pub pending_goal_prompt: Option<String>,
    /// Unified skill runtime for bundled, project, and user skills.
    pub skill_runtime: crate::skills::SkillRuntime,
    /// 是否启用 Vim 模式
    pub vim_mode: bool,
    /// 键位映射
    pub keybindings: crate::tui::keybindings::Keybindings,
    /// 当前主题
    pub theme: Arc<crate::tui::theme::Theme>,
    /// 引导状态
    pub onboarding_state: Option<crate::onboarding::OnboardingState>,
    /// Plan Mode 状态标签缓存（用于状态栏显示，避免渲染时异步查询）
    pub plan_mode_label: Option<String>,
    /// Tick 计数器（用于 spinner 等动画）
    pub tick_count: usize,
    /// 被折叠的长粘贴块，发送时还原
    pasted_blocks: Vec<PastedBlock>,
    /// Composer file/context attachments injected into the next user prompt.
    pub composer_attachments: Vec<String>,
    /// File picker state for composer attachments.
    pub file_picker_state: Option<crate::tui::components::file_browser::FileBrowserState>,
    /// Whether file picker keystrokes edit the filter query.
    pub file_picker_filtering: bool,
    /// 命令面板搜索词
    pub command_palette_query: String,
    /// 命令面板选中项
    pub command_palette_selected: usize,
    /// 最近从命令面板执行/选择的命令
    pub recent_palette_commands: VecDeque<String>,
    /// User-scoped temporary permission rules installed by the active skill.
    active_skill_permission_rules: Vec<(String, String)>,
    /// 模型选择器选中项
    pub model_select_selected: usize,
    /// 模型选择器搜索词
    pub model_select_query: String,
    /// 最近一次模型切换提示
    pub model_notice: Option<String>,
    /// Provider 选择器选中项
    pub provider_select_selected: usize,
    /// Provider 选择器搜索词
    pub provider_select_query: String,
    /// 最近一次 provider 切换提示
    pub provider_notice: Option<String>,
    /// Skill invocations waiting for final assistant outcome attribution.
    pending_skill_invocations: Vec<PendingSkillInvocation>,
}

fn parse_on_off(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Some(true),
        "off" | "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn assistant_completion_metadata(
    model_label: String,
    usage: Option<StreamUsageSnapshot>,
    elapsed_ms: Option<u64>,
    provider_phase: crate::engine::runtime_facade::ProviderPhase,
    tool_runs: &[ToolRunView],
) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    metadata.insert("status".to_string(), "complete".to_string());
    metadata.insert("model_label".to_string(), model_label);
    if let Some(elapsed_ms) = elapsed_ms.filter(|value| *value > 0) {
        metadata.insert("elapsed_ms".to_string(), elapsed_ms.to_string());
    }
    if provider_phase != crate::engine::runtime_facade::ProviderPhase::Idle {
        metadata.insert(
            "provider_phase".to_string(),
            provider_phase.label().to_string(),
        );
    }
    if !tool_runs.is_empty() {
        metadata.insert("tool_count".to_string(), tool_runs.len().to_string());
        let failed_tools = tool_runs
            .iter()
            .filter(|run| {
                matches!(
                    run.status,
                    ToolRunStatus::Failed | ToolRunStatus::TimedOut | ToolRunStatus::Cancelled
                )
            })
            .count();
        if failed_tools > 0 {
            metadata.insert("failed_tool_count".to_string(), failed_tools.to_string());
        }

        let validation_runs = tool_runs
            .iter()
            .filter(|run| is_validation_tool_name(&run.name))
            .collect::<Vec<_>>();
        if !validation_runs.is_empty() {
            let failed_validation = validation_runs.iter().any(|run| {
                matches!(
                    run.status,
                    ToolRunStatus::Failed | ToolRunStatus::TimedOut | ToolRunStatus::Cancelled
                )
            });
            metadata.insert(
                "validation_status".to_string(),
                if failed_validation {
                    "failed"
                } else {
                    "passed"
                }
                .to_string(),
            );
        }
    }
    if let Some(usage) = usage {
        metadata.insert("prompt_tokens".to_string(), usage.prompt_tokens.to_string());
        metadata.insert(
            "completion_tokens".to_string(),
            usage.completion_tokens.to_string(),
        );
        metadata.insert("total_tokens".to_string(), usage.total_tokens().to_string());
        if let Some(reasoning_tokens) = usage.reasoning_tokens {
            metadata.insert("reasoning_tokens".to_string(), reasoning_tokens.to_string());
        }
        if let Some(cached_tokens) = usage.cached_tokens {
            metadata.insert("cached_tokens".to_string(), cached_tokens.to_string());
        }
    }
    metadata
}

fn is_validation_tool_name(name: &str) -> bool {
    matches!(name, "run_tests" | "git_status" | "git_diff")
}

#[cfg(test)]
mod completion_metadata_tests {
    use super::*;

    #[test]
    fn assistant_completion_metadata_includes_model_and_usage() {
        let metadata = assistant_completion_metadata(
            "deepseek-v4-flash".to_string(),
            Some(StreamUsageSnapshot {
                prompt_tokens: 100,
                completion_tokens: 25,
                reasoning_tokens: Some(5),
                cached_tokens: Some(90),
            }),
            Some(2_730),
            crate::engine::runtime_facade::ProviderPhase::Completed,
            &[],
        );

        assert_eq!(metadata.get("status").map(String::as_str), Some("complete"));
        assert_eq!(
            metadata.get("model_label").map(String::as_str),
            Some("deepseek-v4-flash")
        );
        assert_eq!(
            metadata.get("completion_tokens").map(String::as_str),
            Some("25")
        );
        assert_eq!(
            metadata.get("total_tokens").map(String::as_str),
            Some("125")
        );
        assert_eq!(
            metadata.get("reasoning_tokens").map(String::as_str),
            Some("5")
        );
        assert_eq!(
            metadata.get("cached_tokens").map(String::as_str),
            Some("90")
        );
        assert_eq!(metadata.get("elapsed_ms").map(String::as_str), Some("2730"));
        assert_eq!(
            metadata.get("provider_phase").map(String::as_str),
            Some("provider done")
        );
    }

    #[test]
    fn assistant_completion_metadata_includes_tool_and_validation_status() {
        let mut validation = ToolRunView::new("tool_1".to_string(), "run_tests".to_string());
        validation.mark_complete("Result: OK\npassed".to_string());
        let mut failed = ToolRunView::new("tool_2".to_string(), "bash".to_string());
        failed.status = ToolRunStatus::Failed;

        let metadata = assistant_completion_metadata(
            "deepseek-v4-flash".to_string(),
            None,
            None,
            crate::engine::runtime_facade::ProviderPhase::Completed,
            &[validation, failed],
        );

        assert_eq!(metadata.get("tool_count").map(String::as_str), Some("2"));
        assert_eq!(
            metadata.get("failed_tool_count").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            metadata.get("validation_status").map(String::as_str),
            Some("passed")
        );
    }
}

impl TuiApp {
    /// Push a new mode onto the mode stack and expose it via `self.mode`.
    pub fn push_mode(&mut self, mode: AppMode) {
        if self.mode != mode {
            self.mode_stack.push(self.mode);
        }
        self.mode = mode;
    }

    /// Pop the current mode off the stack, returning to the previous mode.
    /// Falls back to Chat (or VimNormal if vim mode is active) when the stack is empty.
    pub fn pop_mode(&mut self) -> AppMode {
        self.mode = self.mode_stack.pop().unwrap_or({
            if self.vim_mode {
                AppMode::VimNormal
            } else {
                AppMode::Chat
            }
        });
        self.mode
    }

    /// Replace the current mode without pushing a new stack frame.
    pub fn replace_mode(&mut self, mode: AppMode) {
        self.mode = mode;
    }

    /// Start a leader-key sequence if the leader key was pressed.
    pub fn begin_leader_sequence(&mut self) {
        self.leader_state = Some(LeaderState {
            started_at: std::time::Instant::now(),
        });
    }

    /// Clear an expired or consumed leader-key sequence.
    pub fn clear_leader_sequence(&mut self) {
        self.leader_state = None;
    }

    /// Check whether the leader sequence has expired.
    pub fn leader_expired(&self) -> bool {
        self.leader_state
            .as_ref()
            .map(|s| {
                s.started_at.elapsed().as_millis() as u64 >= self.keybindings.leader_timeout_ms
            })
            .unwrap_or(true)
    }

    pub fn visible_sidebar_sessions(
        &self,
        limit: usize,
    ) -> Vec<crate::session_store::SessionRecord> {
        let sessions = self
            .session_manager
            .list_sessions(limit.min(i64::MAX as usize) as i64)
            .unwrap_or_default();
        let filter = self.sidebar_filter.to_lowercase();
        let mut pinned = Vec::new();
        let mut unpinned = Vec::new();

        for session in sessions {
            let searchable = format!(
                "{} {} {}",
                session.title.to_lowercase(),
                session.id.to_lowercase(),
                session.model.to_lowercase()
            );
            if !filter.is_empty() && !searchable.contains(&filter) {
                continue;
            }
            if self.pinned_sessions.contains(&session.id) {
                pinned.push(session);
            } else {
                unpinned.push(session);
            }
        }

        pinned.extend(unpinned);
        pinned
    }

    pub fn new() -> Self {
        let mut app = Self::create(None, None, None);
        app.kv_store = crate::services::kv::KvStore::in_memory();
        app.status_bar_density = StatusBarDensity::Normal;
        app
    }

    /// 创建带流式引擎的 TuiApp
    pub fn with_engine(
        engine: Option<Arc<StreamingQueryEngine>>,
        lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
        worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    ) -> Self {
        Self::create(engine, lsp_manager, worktree_manager)
    }

    pub fn activate_provider_runtime(&mut self, provider_id: &str) -> Result<String, String> {
        let registry = crate::services::api::provider::ProviderRegistry::from_env();
        let provider = registry
            .get(provider_id)
            .ok_or_else(|| format!("provider '{}' is not configured", provider_id))?;
        let config = registry
            .get_config(provider_id)
            .ok_or_else(|| format!("provider '{}' is missing runtime config", provider_id))?;
        let model = config.default_model.clone();

        if let Some(engine) = &self.streaming_engine {
            engine.set_provider(provider, model.clone());
            return Ok(model);
        }

        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let app_config = crate::services::config::AppConfig::load().unwrap_or_default();
        let engine_config = app_config.engine.clone();
        let task_manager = crate::task_manager::GLOBAL_TASK_MANAGER.clone();
        let tool_registry = Arc::new(crate::tools::ToolRegistry::default_registry());

        let mut query_engine_builder =
            crate::engine::QueryEngine::new(provider.clone(), tool_registry.clone(), &model)
                .with_max_iterations(engine_config.max_iterations)
                .with_task_manager(task_manager.clone());
        if let Some(lsp) = &self.lsp_manager {
            query_engine_builder = query_engine_builder.with_lsp_manager(lsp.clone());
        }
        if let Some(worktree) = &self.worktree_manager {
            query_engine_builder = query_engine_builder.with_worktree_manager(worktree.clone());
        }
        let query_engine = Arc::new(query_engine_builder);

        let llm_memory_extraction = std::env::var("PRIORITY_AGENT_LLM_MEMORY_EXTRACTION")
            .ok()
            .and_then(|value| parse_on_off(value.trim()))
            .unwrap_or(app_config.features.llm_memory_extraction);
        let mut streaming_engine_builder =
            StreamingQueryEngine::new(provider, tool_registry, &model)
                .with_max_iterations(engine_config.max_iterations)
                .with_working_dir(&working_dir)
                .with_task_manager(task_manager)
                .with_llm_memory_extraction(llm_memory_extraction)
                .with_agent_query_engine(query_engine);
        if let Some(lsp) = &self.lsp_manager {
            streaming_engine_builder = streaming_engine_builder.with_lsp_manager(lsp.clone());
        }
        if let Some(worktree) = &self.worktree_manager {
            streaming_engine_builder =
                streaming_engine_builder.with_worktree_manager(worktree.clone());
        }
        let approval_channel =
            Arc::new(crate::engine::conversation_loop::ToolApprovalChannel::new());
        streaming_engine_builder = streaming_engine_builder.with_approval_channel(approval_channel);

        self.streaming_engine = Some(Arc::new(streaming_engine_builder));
        Ok(model)
    }

    fn create(
        engine: Option<Arc<StreamingQueryEngine>>,
        lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
        worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    ) -> Self {
        info!("Creating new TuiApp");

        let context = AppContext::new();

        // 初始化会话管理器。优先复用引擎会话，这样 UI 历史、
        // trace 与 learning events 会写入同一条 conversation。
        let model = engine
            .as_ref()
            .map(|engine| engine.model_name())
            .unwrap_or_else(|| "unknown".to_string());
        let mut session_manager = if let Some((store, session_id)) =
            engine.as_ref().and_then(|engine| engine.session_binding())
        {
            crate::tui::session_manager::TuiSessionManager::from_store(
                store,
                session_id,
                "New Session",
                &model,
            )
            .unwrap_or_else(|e| {
                warn!("Failed to bind TUI session to engine session: {}", e);
                crate::tui::session_manager::TuiSessionManager::new().unwrap_or_else(|e| {
                    warn!("Failed to initialize session manager: {}", e);
                    crate::tui::session_manager::TuiSessionManager::in_memory()
                        .expect("Failed to create in-memory session manager")
                })
            })
        } else {
            crate::tui::session_manager::TuiSessionManager::new().unwrap_or_else(|e| {
                warn!("Failed to initialize session manager: {}", e);
                crate::tui::session_manager::TuiSessionManager::in_memory()
                    .expect("Failed to create in-memory session manager")
            })
        };

        let app_config = crate::services::config::AppConfig::load().unwrap_or_default();
        let pinned_sessions = app_config.ui.pinned_sessions.clone();
        let theme_name = app_config.ui.theme.clone();

        let kv_store = crate::services::kv::KvStore::load().unwrap_or_else(|err| {
            tracing::warn!("Failed to load KV store: {err}");
            crate::services::kv::KvStore::in_memory()
        });

        let status_bar_density = kv_store
            .get_string("ui.status_bar_density")
            .and_then(|v| StatusBarDensity::parse(&v))
            .unwrap_or(StatusBarDensity::Normal);

        let workspace = Workspace::detect(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );

        if session_manager.current_session_id().is_none() {
            if let Ok(id) = session_manager.start_session("New Session", &model) {
                session_manager.tag_session_workspace(&id, &workspace.root.to_string_lossy());
            }
        }

        // Restart safety: mark any active goals as paused so they don't auto-resume

        Self {
            mode: AppMode::Chat,
            mode_stack: Vec::new(),
            leader_state: None,
            workspace,
            agent_mode: AgentMode::Auto,
            input: InputState::new(),
            messages: Vec::new(),
            tasks: Vec::new(),
            is_querying: false,
            stream_started_at: None,
            toasts: Vec::new(),
            memory_use: true,
            memory_generate: true,
            memory_recall_mode: "balanced".to_string(),
            paused: false,
            focus_mode: false,
            status_bar_density,
            run_coordinator: crate::engine::run_coordinator::SessionRunCoordinator::new(),
            command_registry: default_command_registry(),
            kv_store,
            scroll_offset: 0,
            scroll_anchor_id: None,
            pinned_to_bottom: true,
            context,
            error_message: None,
            history: VecDeque::with_capacity(100),
            history_index: None,
            prompt_stash: None,
            prompt_picker_selected: 0,
            streaming_engine: engine,
            runtime_state_snapshot: RuntimeAppState::default(),
            runtime_facade_state: RuntimeFacadeState::default(),
            sync_store: Arc::new(Mutex::new(TuiSyncStore::new())),
            sync_snapshot: TuiSyncSnapshot::default(),
            current_tool_anchor_id: None,
            transcript_expanded: false,
            expanded_tool_run_id: None,
            expanded_reasoning_message_id: None,
            expanded_inline_tool_ids: BTreeSet::new(),
            expanded_inline_message_part_ids: BTreeSet::new(),
            stream_usage: Arc::new(Mutex::new(None)),
            stream_usage_snapshot: None,
            facade_snapshot: RuntimeStateSnapshot::default(),
            stream_done: Arc::new(AtomicBool::new(true)),
            stream_handle: None,
            session_manager,
            settings_state: None,
            pending_plan: None,
            plan_response_tx: None,
            plan_modification_input: String::new(),
            plan_mode_label: None,
            pending_permission_request: None,
            permission_response_tx: None,
            pending_question: None,
            pending_question_options: Vec::new(),
            question_response_tx: None,
            diff_content: String::new(),
            diff_title: String::new(),
            diff_scroll_offset: 0,
            tool_viewer_content: String::new(),
            tool_viewer_title: String::new(),
            tool_viewer_scroll_offset: 0,
            message_search_state: crate::tui::components::message_search::MessageSearchState::new(),
            collapsed_indices: std::collections::HashSet::new(),
            sidebar_visible: false,
            sidebar_selected: 0,
            sidebar_filter: String::new(),
            pinned_sessions,
            confirm_delete_session_id: None,
            renaming_session_id: None,
            rename_buffer: String::new(),
            filtering_sidebar: false,
            shortcut_help_filter: String::new(),
            filtering_shortcut_help: false,
            sidebar_panel: SidebarPanel::Sessions,
            typewriter_position: 0,
            tick_count: 0,
            lsp_manager,
            worktree_manager,
            app_started_at: std::time::Instant::now(),
            bundled_skills: {
                let mut map = std::collections::HashMap::new();
                for skill in crate::skills::loader::load_bundled_skills() {
                    map.insert(skill.meta.name.clone(), skill);
                }
                map
            },
            skill_runtime: crate::skills::SkillRuntime::load(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ),
            vim_mode: false,
            keybindings: crate::tui::keybindings::Keybindings::load(),
            theme: { Arc::new(crate::tui::theme::Theme::from_name(&theme_name)) },
            onboarding_state: None,
            pasted_blocks: Vec::new(),
            composer_attachments: Vec::new(),
            file_picker_state: None,
            file_picker_filtering: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            recent_palette_commands: VecDeque::with_capacity(16),
            active_skill_permission_rules: Vec::new(),
            model_select_selected: 0,
            model_select_query: String::new(),
            model_notice: None,
            provider_select_selected: 0,
            provider_select_query: String::new(),
            provider_notice: None,
            pending_skill_invocations: Vec::new(),
            goal_runner: None,
            pending_goal_prompt: None,
        }
    }

    pub(crate) fn lazy_goal_runner(&mut self) -> Option<&crate::engine::goal::runner::GoalRunner> {
        if self.goal_runner.is_none() {
            if let Some(ref engine) = self.streaming_engine {
                let store = (*self.session_manager.store()).clone();
                let goal_manager = engine.goal_manager();
                self.goal_runner = Some(crate::engine::goal::runner::GoalRunner::new(
                    store,
                    goal_manager,
                ));
            }
        }
        self.goal_runner.as_ref()
    }

    async fn maybe_continue_goal(&mut self) -> bool {
        let runner = match self.lazy_goal_runner() {
            Some(runner) => runner.clone(),
            None => return false,
        };

        let session_id = match self.session_manager.current_session_id() {
            Some(id) => id.to_string(),
            None => return false,
        };

        if !runner.has_active_goal(&session_id).unwrap_or(false) {
            return false;
        }

        let trace = self
            .streaming_engine
            .as_ref()
            .and_then(|engine| engine.trace_store().latest())
            .or_else(|| self.session_manager.latest_trace().ok().flatten());

        let Some(trace) = trace else {
            return false;
        };

        match runner.after_turn(&session_id, &trace) {
            Ok(crate::engine::goal::runner::GoalAfterTurnResult::Continue { prompt, .. }) => {
                self.persist_goal_continuation(&prompt)
            }
            Ok(crate::engine::goal::runner::GoalAfterTurnResult::Terminal {
                decision,
                status: _,
                step,
            }) => {
                self.add_system_message(format!("Goal {:?}: {}", decision, step.summary));
                false
            }
            Err(e) => {
                warn!("Goal continuation error: {}", e);
                false
            }
        }
    }

    async fn drain_next_queued_session_input(&mut self, system_message: &str) -> bool {
        if !self.run_coordinator.wake() {
            return false;
        }

        let Some(next_input) = self.promote_queued_session_input() else {
            self.run_coordinator.accept_wake();
            return false;
        };

        self.run_coordinator.accept_wake();
        self.add_system_message(system_message.to_string());
        self.send_message(next_input).await;
        true
    }

    /// 提交用户消息
    pub async fn submit_message(&mut self) {
        let content = self.expand_paste_placeholders(self.input.value());
        if content.trim().is_empty() {
            return;
        }

        // 清空输入
        self.input.clear();

        // 处理斜杠命令
        if content.starts_with('/') {
            self.handle_slash_command(&content).await;
            if let Some(prompt) = self.pending_goal_prompt.take() {
                self.send_message(prompt).await;
            }
            return;
        }

        let content = self.compose_message_with_attachments(content);
        self.pasted_blocks.clear();
        self.composer_attachments.clear();
        self.send_message(content).await;
    }

    /// 插入粘贴内容；长粘贴折叠为占位符，避免输入区撑满屏幕。
    /// 如果粘贴的是单个存在的文件路径，则作为附件 intake。
    pub fn insert_paste(&mut self, text: String) {
        if text.is_empty() {
            return;
        }

        let trimmed = text.trim();
        if trimmed.lines().count() == 1 && !trimmed.is_empty() {
            if trimmed.starts_with("data:image") {
                return self.insert_image_paste(text);
            }
            let path = std::path::Path::new(trimmed);
            if path.exists() {
                let _ = self.attach_context_path(trimmed);
                return;
            }
        }

        let char_count = text.chars().count();
        let line_count = text.lines().count().max(1);
        if char_count < LONG_PASTE_CHAR_THRESHOLD && line_count < LONG_PASTE_LINE_THRESHOLD {
            self.input.insert_str(&text);
            return;
        }

        let paste_id = self.pasted_blocks.len() + 1;
        let placeholder = format!(
            "[[paste:{} {} lines {} chars]]",
            paste_id, line_count, char_count
        );
        self.pasted_blocks.push(PastedBlock {
            placeholder: placeholder.clone(),
            content: text,
        });
        self.input.insert_str(&placeholder);
    }

    fn insert_image_paste(&mut self, text: String) {
        let paste_id = self.pasted_blocks.len() + 1;
        let char_count = text.chars().count();
        let placeholder = format!("[[image:{} {} chars]]", paste_id, char_count);
        self.pasted_blocks.push(PastedBlock {
            placeholder: placeholder.clone(),
            content: text,
        });
        self.input.insert_str(&placeholder);
    }

    pub fn pasted_block_count(&self) -> usize {
        self.pasted_blocks
            .iter()
            .filter(|block| self.input.value().contains(&block.placeholder))
            .count()
    }

    pub fn pasted_block_summaries(&self) -> Vec<String> {
        self.pasted_blocks
            .iter()
            .filter(|block| self.input.value().contains(&block.placeholder))
            .map(|block| {
                let line_count = block.content.lines().count().max(1);
                let char_count = block.content.chars().count();
                format!("{} lines / {} chars", line_count, char_count)
            })
            .collect()
    }

    pub fn open_paste_viewer(&mut self, index: Option<usize>) -> bool {
        let active_blocks = self
            .pasted_blocks
            .iter()
            .filter(|block| self.input.value().contains(&block.placeholder))
            .collect::<Vec<_>>();
        if active_blocks.is_empty() {
            return false;
        }
        let selected = index.unwrap_or(1).saturating_sub(1);
        let Some(block) = active_blocks.get(selected) else {
            return false;
        };
        let line_count = block.content.lines().count().max(1);
        let char_count = block.content.chars().count();
        self.tool_viewer_title = format!(
            "Paste {} ({} lines / {} chars)",
            selected + 1,
            line_count,
            char_count
        );
        self.tool_viewer_content = block.content.clone();
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub fn attach_context_path(&mut self, raw_path: &str) -> Result<String, String> {
        let raw_path = raw_path.trim();
        if raw_path.is_empty() {
            return Err("Usage: /attach <path>|remove <n>|clear|list".to_string());
        }
        if self.composer_attachments.len() >= 12 {
            return Err("Attachment limit reached for this prompt.".to_string());
        }

        let path = std::path::Path::new(raw_path);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(path)
        };
        let canonical = absolute
            .canonicalize()
            .map_err(|_| format!("Attachment not found: {raw_path}"))?;
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let display = canonical
            .strip_prefix(&cwd)
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|_| canonical.to_string_lossy().to_string());
        if self
            .composer_attachments
            .iter()
            .any(|path| path == &display)
        {
            return Ok(format!("Already attached: {display}"));
        }

        self.composer_attachments.push(display.clone());
        Ok(format!("Attached context: {display}"))
    }

    pub fn remove_composer_attachment(&mut self, one_based_index: usize) -> Option<String> {
        if one_based_index == 0 || one_based_index > self.composer_attachments.len() {
            return None;
        }
        Some(self.composer_attachments.remove(one_based_index - 1))
    }

    pub fn remove_last_composer_attachment(&mut self) -> Option<String> {
        self.composer_attachments.pop()
    }

    pub fn clear_composer_attachments(&mut self) -> usize {
        let count = self.composer_attachments.len();
        self.composer_attachments.clear();
        count
    }

    pub fn composer_attachment_summaries(&self) -> Vec<String> {
        self.composer_attachments
            .iter()
            .enumerate()
            .map(|(idx, path)| format!("[{}] {}", idx + 1, attachment_summary(path, 44)))
            .collect()
    }

    pub fn composer_attachment_count(&self) -> usize {
        self.composer_attachments.len()
    }

    pub fn toggle_pinned_session(&mut self, session_id: &str) -> bool {
        let pinned = toggle_pinned_session_list(&mut self.pinned_sessions, session_id, 9);
        if let Err(err) = self.persist_pinned_sessions() {
            warn!("Failed to persist pinned sessions: {}", err);
            self.add_toast(
                format!("Pinned sessions kept for this run; save failed: {}", err),
                "!",
            );
        }
        pinned
    }

    fn persist_pinned_sessions(&self) -> anyhow::Result<()> {
        let mut config = crate::services::config::AppConfig::load().unwrap_or_default();
        config.ui.pinned_sessions = self.pinned_sessions.clone();
        config.save()?;
        crate::services::config::init_runtime_config(config);
        Ok(())
    }

    pub fn open_attachment_viewer(&mut self, index: Option<usize>) -> bool {
        if self.composer_attachments.is_empty() {
            return false;
        }
        let selected = index.unwrap_or(1).saturating_sub(1);
        let Some(path) = self.composer_attachments.get(selected).cloned() else {
            return false;
        };
        let absolute = resolve_attachment_path(&path);
        let Ok(metadata) = std::fs::metadata(&absolute) else {
            self.tool_viewer_title = format!("Attachment {}: {}", selected + 1, path);
            self.tool_viewer_content = format!("Attachment path is no longer available:\n{path}");
            self.tool_viewer_scroll_offset = 0;
            self.mode = AppMode::ToolViewer;
            return true;
        };

        self.tool_viewer_title = format!("Attachment {}: {}", selected + 1, path);
        self.tool_viewer_content = if metadata.is_dir() {
            attachment_directory_preview(&absolute)
        } else {
            attachment_file_preview(&absolute, metadata.len())
        };
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    fn compose_message_with_attachments(&self, content: String) -> String {
        if self.composer_attachments.is_empty() {
            return content;
        }

        let mut composed = String::from("Attached context:\n");
        for path in &self.composer_attachments {
            composed.push_str("- ");
            composed.push_str(&attachment_summary(path, 96));
            composed.push('\n');
        }
        composed.push_str("\nUser request:\n");
        composed.push_str(&content);
        composed
    }

    fn expand_paste_placeholders(&self, content: &str) -> String {
        let mut expanded = content.to_string();
        for block in &self.pasted_blocks {
            expanded = expanded.replace(&block.placeholder, &block.content);
        }
        expanded
    }

    fn clear_active_skill_rules(&mut self) {
        let Some(engine) = &self.streaming_engine else {
            self.active_skill_permission_rules.clear();
            return;
        };
        for (decision, pattern) in self.active_skill_permission_rules.drain(..) {
            engine.remove_session_permission_rule(&decision, &pattern);
        }
    }

    fn apply_skill_invocation_policy(&mut self, invocation: &crate::skills::SkillInvocation) {
        self.clear_active_skill_rules();
        let Some(engine) = &self.streaming_engine else {
            return;
        };
        for pattern in &invocation.allowed_tools {
            engine.add_session_permission_rule("allow", pattern);
            self.active_skill_permission_rules
                .push(("allow".to_string(), pattern.to_string()));
        }
        for pattern in &invocation.disallowed_tools {
            engine.add_session_permission_rule("deny", pattern);
            self.active_skill_permission_rules
                .push(("deny".to_string(), pattern.to_string()));
        }
    }

    /// 发送消息到 LLM（核心逻辑，可被 skill 调用复用）
    pub(crate) async fn send_message(&mut self, content: String) {
        if content.trim().is_empty() {
            return;
        }
        if self.paused {
            self.add_system_message(
                "Agent is paused. Use `/pause resume` to continue sending messages.".to_string(),
            );
            return;
        }

        debug!("Submitting message: {}", content);

        if self.run_coordinator.is_active() || self.is_querying {
            let queued = self.persist_queued_session_input(&content);
            let message = if queued {
                "A run is already active. Queued your message for the next turn.".to_string()
            } else {
                "A run is already active. Wait for it to complete or press Esc to cancel."
                    .to_string()
            };
            self.add_system_message(message);
            return;
        }

        // 取消之前的流式任务（如果有）
        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
            self.runtime_facade_state.mark_cancelled().await;
        }

        // 添加到历史
        self.history.push_back(content.clone());
        if self.history.len() > 100 {
            self.history.pop_front();
        }

        // 添加用户消息
        let user_msg_id = format!("msg_{}", self.messages.len());
        let user_msg = MessageItem {
            id: user_msg_id.clone(),
            role: MessageRole::User,
            content: content.clone(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(user_msg);

        // 如果流式引擎已经绑定同一条持久化会话，消息由引擎统一写入，
        // 避免 UI 和引擎重复插入 user/assistant 历史。
        if self.should_persist_messages_from_tui() {
            if let Err(e) = self
                .session_manager
                .add_message(MessageRole::User, &content)
            {
                warn!("Failed to save user message: {}", e);
            }
        }

        // 更新会话标题（基于第一条用户消息）
        if self.session_manager.current_session_title() == "New Session" {
            let title = self.session_manager.generate_title(&self.messages);
            if let Err(e) = self.session_manager.update_title(&title) {
                warn!("Failed to update session title: {}", e);
            }
        }

        // 标记正在查询
        if !self.run_coordinator.start_run() {
            self.add_system_message(
                "A run is already active. Wait for it to complete or press Esc to cancel."
                    .to_string(),
            );
            return;
        }
        self.is_querying = true;
        self.stream_started_at = Some(std::time::Instant::now());

        // Only auto-scroll when pinned
        if self.pinned_to_bottom {
            self.scroll_to_bottom();
        }

        // 使用流式引擎发送查询
        if let Some(engine) = self.streaming_engine.clone() {
            {
                let mut usage = self.stream_usage.lock().await;
                *usage = None;
            }
            self.runtime_facade_state.reset().await;
            self.runtime_facade_state.set_querying(true).await;
            self.runtime_facade_state.set_stream_usage(None).await;
            self.current_tool_anchor_id = Some(user_msg_id.clone());
            self.stream_usage_snapshot = None;
            self.runtime_state_snapshot = self.build_runtime_state_snapshot();
            self.sync_context_runtime_state().await;
            // 标记流未完成
            self.stream_done.store(false, Ordering::SeqCst);

            // 创建助手消息占位符
            let assistant_msg = MessageItem {
                id: format!("msg_{}", self.messages.len()),
                role: MessageRole::Assistant,
                content: String::new(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            };
            let assistant_msg_id = assistant_msg.id.clone();
            self.messages.push(assistant_msg);
            self.scroll_to_bottom();
            {
                let mut sync = self.sync_store.lock().await;
                sync.start_turn(user_msg_id.clone(), assistant_msg_id);
                self.sync_snapshot = sync.snapshot();
            }

            // 启动流式查询（在后台任务中）
            let controller = RuntimeController::with_runtime_state(
                engine.clone(),
                Arc::new(self.runtime_facade_state.clone()),
            );
            let sync_store_clone = self.sync_store.clone();
            let usage_clone = self.stream_usage.clone();
            let runtime_facade_state_clone = self.runtime_facade_state.clone();
            let done_flag = self.stream_done.clone();
            let user_msg = content.clone();
            let parent_message_id = user_msg_id.clone();
            let agent_mode = self.agent_mode;

            controller.set_memory_policy(
                self.memory_use,
                self.memory_generate,
                self.memory_recall_mode.clone(),
            );

            let handle = tokio::spawn(async move {
                let mut stream = controller
                    .submit_stream_turn_with_agent_mode_and_parent_message_id(
                        user_msg,
                        agent_mode,
                        parent_message_id.clone(),
                    )
                    .await;
                let mut projection_bus = crate::session_store::SessionProjectionEventBus::new();

                while let Some(event) = stream.next().await {
                    let projection_event =
                        crate::session_store::SessionProjectionEvent::from_stream_event(
                            &event,
                            Some(&parent_message_id),
                            None,
                        );
                    let projection_envelope = projection_bus.publish(projection_event);
                    {
                        let mut sync = sync_store_clone.lock().await;
                        sync.apply_projection_envelope(&projection_envelope);
                    }
                    runtime_facade_state_clone
                        .process_projection_event(&projection_envelope.event)
                        .await;
                    match &projection_envelope.event {
                        crate::session_store::SessionProjectionEvent::Completed => {
                            runtime_facade_state_clone.set_querying(false).await;
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        crate::session_store::SessionProjectionEvent::Usage {
                            prompt_tokens,
                            completion_tokens,
                            reasoning_tokens,
                            cached_tokens,
                        } => {
                            let mut usage = usage_clone.lock().await;
                            *usage = Some(StreamUsageSnapshot {
                                prompt_tokens: *prompt_tokens,
                                completion_tokens: *completion_tokens,
                                reasoning_tokens: *reasoning_tokens,
                                cached_tokens: *cached_tokens,
                            });
                            runtime_facade_state_clone
                                .set_stream_usage(Some(
                                    crate::engine::runtime_facade::StreamUsageSnapshot {
                                        prompt_tokens: *prompt_tokens,
                                        completion_tokens: *completion_tokens,
                                        reasoning_tokens: *reasoning_tokens,
                                        cached_tokens: *cached_tokens,
                                    },
                                ))
                                .await;
                        }
                        crate::session_store::SessionProjectionEvent::Error { .. } => {
                            runtime_facade_state_clone.set_querying(false).await;
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        crate::session_store::SessionProjectionEvent::RuntimeDiagnostic {
                            diagnostic,
                        } => {
                            runtime_facade_state_clone
                                .process_diagnostic(diagnostic)
                                .await;
                        }
                        _ => {}
                    }
                }
                // 确保即使流结束也标记完成
                {
                    let mut sync = sync_store_clone.lock().await;
                    sync.mark_stream_closed();
                }
                runtime_facade_state_clone.set_querying(false).await;
                done_flag.store(true, Ordering::SeqCst);
            });
            self.stream_handle = Some(handle);
        } else {
            // 没有引擎，使用占位响应
            self.add_assistant_response(format!(
                "AI engine not available. Set one provider key: {}.",
                crate::services::api::provider::provider_key_env_hint()
            ))
            .await;
        }
    }

    fn should_persist_messages_from_tui(&self) -> bool {
        let Some(engine) = &self.streaming_engine else {
            return true;
        };
        let Some((_store, session_id)) = engine.session_binding() else {
            return true;
        };
        !self.session_manager.is_current_session(&session_id)
    }

    fn persist_queued_session_input(&self, content: &str) -> bool {
        let Some(engine) = &self.streaming_engine else {
            return false;
        };
        let Some((store, session_id)) = engine.session_binding() else {
            return false;
        };
        let delivery = if self.has_active_goal() {
            crate::engine::run_coordinator::InputDelivery::Steer
        } else {
            crate::engine::run_coordinator::InputDelivery::Queue
        };
        let conn = store.shared_conn();
        let conn = conn.lock().expect("tui app sqlite conn lock poisoned");
        crate::engine::run_coordinator::persist_session_input(&conn, &session_id, content, delivery)
            .is_ok()
    }

    fn persist_goal_continuation(&self, content: &str) -> bool {
        let Some(engine) = &self.streaming_engine else {
            return false;
        };
        let Some((store, session_id)) = engine.session_binding() else {
            return false;
        };
        let conn = store.shared_conn();
        let conn = conn.lock().expect("tui app sqlite conn lock poisoned");
        crate::engine::run_coordinator::persist_session_input(
            &conn,
            &session_id,
            content,
            crate::engine::run_coordinator::InputDelivery::Queue,
        )
        .is_ok()
    }

    fn has_active_goal(&self) -> bool {
        self.goal_runner
            .as_ref()
            .and_then(|runner| {
                let session_id = self.session_manager.current_session_id()?;
                runner.has_active_goal(session_id).ok()
            })
            .unwrap_or(false)
    }

    fn promote_queued_session_input(&self) -> Option<String> {
        let engine = self.streaming_engine.as_ref()?;
        let (store, session_id) = engine.session_binding()?;
        let conn = store.shared_conn();
        let conn = conn.lock().expect("tui app sqlite conn lock poisoned");
        crate::engine::run_coordinator::promote_session_input(&conn, &session_id)
            .ok()
            .flatten()
    }

    /// 刷新当前响应（从缓冲区读取最新的流式内容，带打字机效果）
    pub async fn refresh_response(&mut self) {
        if !self.is_querying {
            return;
        }

        self.sync_snapshot = self.sync_store.lock().await.snapshot();
        // 读取响应长度（最小化锁持有时间，避免克隆整个字符串）
        let total_chars = self.sync_snapshot.assistant_message_content.chars().count();

        // 更新打字机位置
        if self.typewriter_position < total_chars {
            let remaining = total_chars - self.typewriter_position;
            self.typewriter_position += remaining.min(12); // ~48 chars/sec at 4Hz tick
        }

        // 读取需要显示的内容和工具状态
        let display_response: String = self
            .sync_snapshot
            .assistant_message_content
            .chars()
            .take(self.typewriter_position)
            .collect();
        self.stream_usage_snapshot = self.sync_snapshot.usage;
        self.runtime_facade_state.check_slow_warning().await;
        if self.runtime_facade_state.check_timeout().await {
            self.facade_snapshot = self.runtime_facade_state.snapshot().await;
            let reason = self
                .facade_snapshot
                .provider_request
                .message
                .clone()
                .unwrap_or_else(|| "provider request timed out".to_string());
            self.timeout_active_run(&reason).await;
            return;
        }
        self.facade_snapshot = self.runtime_facade_state.snapshot().await;
        self.sync_tool_runs_from_spine_snapshot();
        if self.recover_persisted_final_answer_if_available().await {
            return;
        }
        if let Some(reason) = self.provider_wait_timeout_reason() {
            self.timeout_active_run(&reason).await;
            return;
        }

        // 更新最后一条助手消息
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.content = display_response;
            }
        }

        self.scroll_to_bottom();
    }

    fn sync_tool_runs_from_spine_snapshot(&mut self) {
        let turns = self.facade_snapshot.tool_turns.clone();
        let mut sync_store = TuiSyncStore::from_snapshot(self.sync_snapshot.clone());
        let mut projection_bus = crate::session_store::SessionProjectionEventBus::from_seq(
            self.sync_snapshot.last_projection_seq,
        );
        for turn in turns {
            let Some(parent_message_id) = turn.parent_message_id.clone() else {
                continue;
            };
            let event = crate::session_store::SessionProjectionEvent::ToolPartUpdated {
                message_id: Some(parent_message_id),
                tool_call_id: turn.id.clone(),
                tool_name: turn.name.clone(),
                status: Some(tool_part_status_from_turn_phase(turn.phase).to_string()),
                input_args: turn.arguments_preview.clone(),
                result: turn.result_preview.clone().or_else(|| turn.failure.clone()),
                metadata: Some(serde_json::json!({
                    "replay_source": "runtime_facade_spine",
                    "phase": turn.phase.label(),
                    "success": !matches!(
                        turn.phase,
                        ToolTurnPhase::Failed | ToolTurnPhase::Cancelled | ToolTurnPhase::TimedOut
                    ),
                })),
                result_data: None,
            };
            let envelope = projection_bus.publish(event);
            sync_store.apply_projection_envelope(&envelope);
        }
        self.sync_snapshot = sync_store.snapshot();
    }

    async fn recover_persisted_final_answer_if_available(&mut self) -> bool {
        if std::env::var("PRIORITY_AGENT_TUI_DISABLE_DB_FINAL_RECOVERY")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        {
            return false;
        }
        if !self.is_querying || !self.has_observed_tool_result_in_spine() {
            return false;
        }

        let Some(current_user_content) = self
            .messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::User)
            .map(|message| message.content.trim().to_string())
        else {
            return false;
        };
        let Some(final_answer) = self.recoverable_persisted_final_answer(&current_user_content)
        else {
            return false;
        };

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }
        self.typewriter_position = final_answer.chars().count();
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.content = final_answer.clone();
                let mut sync_store = TuiSyncStore::from_snapshot(self.sync_snapshot.clone());
                let mut projection_bus = crate::session_store::SessionProjectionEventBus::from_seq(
                    self.sync_snapshot.last_projection_seq,
                );
                let envelope = projection_bus.publish(
                    crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                        message_id: Some(last_msg.id.clone()),
                        text: final_answer.clone(),
                        streaming: false,
                    },
                );
                sync_store.apply_projection_envelope(&envelope);
                self.sync_snapshot = sync_store.snapshot();
            }
        }
        self.runtime_facade_state.mark_tool_turns_persisted().await;
        self.runtime_facade_state.set_querying(false).await;
        self.facade_snapshot = self.runtime_facade_state.snapshot().await;
        if let Some(session_id) = self
            .session_manager
            .current_session_id()
            .map(str::to_string)
        {
            if let Err(err) = self.hydrate_persisted_projection_for_session(&session_id) {
                warn!(
                    "Failed to hydrate persisted projection during recovery: {}",
                    err
                );
                self.sync_tool_runs_from_spine_snapshot();
            }
        } else {
            self.sync_tool_runs_from_spine_snapshot();
        }
        self.stream_done.store(true, Ordering::SeqCst);
        self.is_querying = false;
        self.run_coordinator.finish_run();
        self.stream_started_at = None;
        self.current_tool_anchor_id = None;
        self.scroll_to_bottom();
        true
    }

    fn has_observed_tool_result_in_spine(&self) -> bool {
        self.facade_snapshot.tool_turns.iter().any(|turn| {
            matches!(
                turn.phase,
                ToolTurnPhase::ResultObserved
                    | ToolTurnPhase::SentBackToModel
                    | ToolTurnPhase::FinalAnswer
                    | ToolTurnPhase::Persisted
            )
        })
    }

    fn recoverable_persisted_final_answer(&self, current_user_content: &str) -> Option<String> {
        if let Some(messages) = self.load_bound_session_messages_for_recovery() {
            if let Some(answer) = persisted_final_answer_for_user(&messages, current_user_content) {
                return Some(answer);
            }
        }

        let current_prompt = normalize_turn_prompt(current_user_content);
        for session in self.session_manager.list_sessions(8).ok()? {
            if normalize_turn_prompt(&session.title) != current_prompt {
                continue;
            }
            let messages = self.session_manager.load_messages(&session.id).ok()?;
            if let Some(answer) = persisted_final_answer_for_user(&messages, current_user_content) {
                return Some(answer);
            }
        }
        None
    }

    fn load_bound_session_messages_for_recovery(&self) -> Option<Vec<MessageItem>> {
        if let Some(engine) = &self.streaming_engine {
            if let Some((store, session_id)) = engine.session_binding() {
                let records = store.get_messages(&session_id).ok()?;
                return Some(
                    records
                        .into_iter()
                        .map(|record| MessageItem {
                            id: format!("msg_{}", record.id),
                            role: match record.role.as_str() {
                                "user" => MessageRole::User,
                                "assistant" => MessageRole::Assistant,
                                "tool" => MessageRole::Tool,
                                _ => MessageRole::System,
                            },
                            content: record.content,
                            timestamp: std::time::SystemTime::now(),
                            metadata: Default::default(),
                        })
                        .collect(),
                );
            }
        }
        let session_id = self.session_manager.current_session_id()?.to_string();
        self.session_manager.load_messages(&session_id).ok()
    }

    pub(crate) fn provider_wait_timeout_reason(&self) -> Option<String> {
        let provider = &self.facade_snapshot.provider_request;
        if !self.is_querying && self.stream_handle.is_none() {
            return None;
        }
        if self.projected_tool_runs().iter().any(|run| {
            matches!(
                run.status,
                ToolRunStatus::Running | ToolRunStatus::WaitingPermission
            )
        }) {
            return None;
        }
        let runtime_config = crate::services::config::runtime_config();
        let explicit_timeout_ms = runtime_config
            .explicit_llm_request_timeout()
            .map(|timeout| timeout.as_millis() as u64);
        let fallback_timeout_ms = runtime_config.llm_request_timeout().as_millis() as u64;
        let timeout_ms = if provider.timeout_ms > 0 {
            explicit_timeout_ms
                .map(|explicit| explicit.min(provider.timeout_ms))
                .unwrap_or(provider.timeout_ms)
        } else {
            explicit_timeout_ms.unwrap_or(fallback_timeout_ms)
        };
        if timeout_ms == 0 {
            return None;
        }
        if matches!(
            provider.phase,
            crate::engine::runtime_facade::ProviderPhase::Completed
                | crate::engine::runtime_facade::ProviderPhase::Cancelled
        ) {
            if !self.post_tool_turn_wait_has_timed_out(timeout_ms, provider.elapsed_ms) {
                return None;
            }
            return Some(format!(
                "tool turn stalled after result observation for {:.1}s",
                timeout_ms as f64 / 1000.0
            ));
        }
        let local_elapsed_ms = self
            .stream_started_at
            .map(|started| started.elapsed().as_millis() as u64)
            .unwrap_or_default();
        let elapsed_ms = if provider.phase.is_active() {
            provider.elapsed_ms
        } else {
            provider.elapsed_ms.max(local_elapsed_ms)
        };
        if elapsed_ms < timeout_ms {
            return None;
        }
        Some(provider.message.clone().unwrap_or_else(|| {
            format!(
                "provider request timed out after {:.1}s",
                timeout_ms as f64 / 1000.0
            )
        }))
    }

    fn post_tool_turn_wait_has_timed_out(&self, timeout_ms: u64, provider_elapsed_ms: u64) -> bool {
        if timeout_ms == 0 {
            return false;
        }
        let has_waiting_tool_turn = self.facade_snapshot.tool_turns.iter().any(|turn| {
            matches!(
                turn.phase,
                crate::engine::runtime_facade::ToolTurnPhase::ResultObserved
                    | crate::engine::runtime_facade::ToolTurnPhase::SentBackToModel
            )
        });
        if !has_waiting_tool_turn {
            return false;
        }
        provider_elapsed_ms >= timeout_ms
    }

    /// 定时更新 - 处理流式响应刷新和计划审批检查
    pub async fn on_tick(&mut self) {
        self.tick_count += 1;
        // Clean up expired toasts
        self.toasts.retain(|t| t.expires_at_tick > self.tick_count);

        // Auto-clear expired leader-key sequence so the UI does not get stuck.
        if self.leader_expired() {
            self.leader_state = None;
        }

        self.facade_snapshot = self.runtime_facade_state.snapshot().await;
        self.sync_tool_runs_from_spine_snapshot();
        if self.recover_persisted_final_answer_if_available().await {
            return;
        }
        if let Some(reason) = self.provider_wait_timeout_reason() {
            self.timeout_active_run(&reason).await;
            return;
        }

        if self.is_querying {
            self.refresh_response().await;

            // 使用 AtomicBool 检测流是否完成（由后台任务设置）
            if self.stream_done.load(Ordering::SeqCst) {
                // 确保显示完整内容（跳过打字机效果的剩余部分）
                let mut final_response_to_persist = None;
                self.sync_snapshot = self.sync_store.lock().await.snapshot();
                self.stream_usage_snapshot = self.sync_snapshot.usage;
                self.facade_snapshot = self.runtime_facade_state.snapshot().await;
                let elapsed_ms = self
                    .stream_started_at
                    .map(|started| started.elapsed().as_millis() as u64)
                    .or_else(|| {
                        (self.facade_snapshot.provider_request.elapsed_ms > 0)
                            .then_some(self.facade_snapshot.provider_request.elapsed_ms)
                    });
                let model_label = self.current_model_label();
                let provider_phase = self.facade_snapshot.provider_request.phase;
                let response = self.sync_snapshot.assistant_message_content.clone();
                let tool_runs = self.projected_tool_runs();
                let final_metadata = assistant_completion_metadata(
                    model_label,
                    self.stream_usage_snapshot,
                    elapsed_ms,
                    provider_phase,
                    &tool_runs,
                );
                if let Some(last_msg) = self.messages.last_mut() {
                    if last_msg.role == MessageRole::Assistant {
                        last_msg.content = response;
                        last_msg.metadata.extend(final_metadata);
                        final_response_to_persist =
                            Some((last_msg.content.clone(), last_msg.metadata.clone()));
                    }
                }
                self.runtime_state_snapshot = self.build_runtime_state_snapshot();
                self.sync_context_runtime_state().await;
                let final_response_for_outcome = final_response_to_persist
                    .as_ref()
                    .map(|(response, _metadata)| response.clone())
                    .unwrap_or_default();
                if self.should_persist_messages_from_tui() {
                    if let Some((response, metadata)) = final_response_to_persist {
                        if let Err(e) = self.session_manager.add_message_with_metadata(
                            MessageRole::Assistant,
                            &response,
                            &metadata,
                        ) {
                            warn!("Failed to save assistant message: {}", e);
                        }
                    }
                }
                self.record_pending_skill_outcomes(&final_response_for_outcome);
                self.runtime_facade_state.mark_tool_turns_persisted().await;
                self.facade_snapshot = self.runtime_facade_state.snapshot().await;
                self.sync_tool_runs_from_spine_snapshot();
                self.typewriter_position = 0;
                // 流式响应完成，发送终端通知
                crate::tui::notify::send_notification("Priority Agent", "Response ready");
                self.is_querying = false;
                self.run_coordinator.finish_run();
                self.runtime_facade_state.set_querying(false).await;
                self.stream_started_at = None;
                self.current_tool_anchor_id = None;
                let ran_queued_input = self
                    .drain_next_queued_session_input("Running queued message from this session.")
                    .await;
                if !ran_queued_input && self.maybe_continue_goal().await {
                    self.drain_next_queued_session_input(
                        "Goal: continuing with next automatic turn.",
                    )
                    .await;
                }
            }
        }

        // 检查是否有待审批的计划（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_plan.is_none() {
            self.check_pending_plan().await;
        }

        // 检查是否有待审批的工具权限请求（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_permission_request.is_none() {
            self.check_pending_permission_request().await;
        }

        // 检查是否有待回答的用户问题（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_question.is_none() {
            self.check_pending_question().await;
        }

        // 更新 Plan Mode 状态标签缓存
        self.update_plan_mode_label().await;
    }

    /// 异步更新 Plan Mode 状态标签缓存
    async fn update_plan_mode_label(&mut self) {
        let plan_manager = &crate::engine::plan_mode::GLOBAL_PLAN_MANAGER;
        let state = plan_manager.get_state().await;
        self.plan_mode_label = match state {
            crate::engine::plan_mode::PlanModeState::Off => None,
            crate::engine::plan_mode::PlanModeState::Generating => {
                Some("[PLAN: generating]".to_string())
            }
            crate::engine::plan_mode::PlanModeState::Clarifying { ref question } => {
                let q = if question.len() > 20 {
                    format!("{}...", &question[..20])
                } else {
                    question.clone()
                };
                Some(format!("[PLAN: clarifying \"{}\"]", q))
            }
            crate::engine::plan_mode::PlanModeState::WaitingApproval => {
                Some("[PLAN: awaiting approval]".to_string())
            }
            crate::engine::plan_mode::PlanModeState::Executing { current_step } => {
                Some(format!("[PLAN: step {}]", current_step + 1))
            }
            crate::engine::plan_mode::PlanModeState::Completed => Some("[PLAN: done]".to_string()),
            crate::engine::plan_mode::PlanModeState::Rejected => None,
        };
    }

    /// 检查是否有待审批的计划
    async fn check_pending_plan(&mut self) {
        let plan_manager = &crate::engine::plan_mode::GLOBAL_PLAN_MANAGER;
        if !plan_manager.approval_channel().has_pending().await {
            return;
        }

        if let Some((plan, tx)) = plan_manager.approval_channel().take_pending().await {
            info!("CLI received pending plan: {}", plan.title);
            self.pending_plan = Some(plan);
            self.plan_response_tx = Some(tx);
            self.plan_modification_input.clear();
            self.mode = AppMode::PlanApproval;
        }
    }

    /// 响应计划审批
    pub fn respond_to_plan(&mut self, approval: crate::engine::plan_mode::PlanApproval) {
        if let Some(tx) = self.plan_response_tx.take() {
            let _ = tx.send(approval);
        }
        self.pending_plan = None;
        self.plan_modification_input.clear();
        self.mode = AppMode::Chat;
    }

    /// 获取 Plan Mode 状态标签（用于状态栏显示，返回缓存值）
    pub fn plan_mode_status_label(&self) -> Option<String> {
        self.plan_mode_label.clone()
    }

    /// 检查是否有待审批的工具权限请求
    async fn check_pending_permission_request(&mut self) {
        let Some(ref engine) = self.streaming_engine else {
            return;
        };
        let Some(ref channel) = engine.approval_channel() else {
            return;
        };

        if !channel.has_pending().await {
            return;
        }

        if let Some((request, tx)) = channel.take_pending().await {
            info!(
                "CLI received pending permission request: {}",
                request.prompt
            );
            self.pending_permission_request = Some(request);
            self.permission_response_tx = Some(tx);
            self.mode = AppMode::PermissionApproval;
        }
    }

    /// 响应工具权限审批
    pub fn respond_to_permission(&mut self, approved: bool) {
        self.respond_to_permission_with_rule(approved, None, None);
    }

    pub fn respond_to_permission_with_rule(
        &mut self,
        approved: bool,
        decision: Option<&str>,
        scope: Option<RuleSource>,
    ) {
        let mut rule_note = None;
        let mut response = if approved {
            ToolApprovalResponse::approved_once()
        } else {
            ToolApprovalResponse::rejected_once()
        };
        if let Some(ref req) = self.pending_permission_request {
            let pattern = permission_rule_pattern(&req.tool_call.name, &req.tool_call.arguments);
            if let Some(review_decision) =
                permission_review_decision_for_response(approved, decision, scope)
            {
                response =
                    ToolApprovalResponse::with_rule(review_decision, pattern.clone(), None, None);
            }
            if let (Some(decision), Some(scope)) = (decision, scope) {
                match scope {
                    RuleSource::User => {
                        if let Some(engine) = &self.streaming_engine {
                            engine.add_session_permission_rule(decision, &pattern);
                            let note =
                                format!("Session permission rule saved: {} {}", decision, pattern);
                            response.note = Some(note.clone());
                            rule_note = Some(note);
                        }
                    }
                    RuleSource::Project | RuleSource::Global => {
                        let cwd = std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("."));
                        match persist_permission_rule(scope, decision, &pattern, &cwd) {
                            Ok(path) => {
                                response.persisted_path = Some(path.display().to_string());
                                let note = format!(
                                    "Permission rule saved to {}: {} {}",
                                    path.display(),
                                    decision,
                                    pattern
                                );
                                response.note = Some(note.clone());
                                rule_note = Some(note);
                            }
                            Err(err) => {
                                let note = format!("Failed to save permission rule: {}", err);
                                response.note = Some(note.clone());
                                rule_note = Some(note);
                            }
                        }
                    }
                    RuleSource::System => {}
                }
            }
            let log_msg = format!(
                "Permission {} for tool '{}' with arguments: {}",
                if approved { "approved" } else { "denied" },
                req.tool_call.name,
                serde_json::to_string(&req.tool_call.arguments).unwrap_or_default()
            );
            let _ = self
                .session_manager
                .add_message(MessageRole::System, &log_msg);
        }
        if let Some(note) = rule_note {
            self.add_system_message(note);
        }
        if let Some(tx) = self.permission_response_tx.take() {
            let _ = tx.send(response);
        }
        self.pending_permission_request = None;
        self.mode = AppMode::Chat;
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

fn compact_attachment_line(path: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in path.chars().take(max_chars) {
        out.push(ch);
    }
    if path.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn attachment_summary(path: &str, max_path_chars: usize) -> String {
    let compact_path = compact_attachment_line(path, max_path_chars);
    let absolute = resolve_attachment_path(path);
    match std::fs::metadata(&absolute) {
        Ok(metadata) if metadata.is_dir() => {
            format!(
                "{compact_path} (dir, {} items)",
                directory_item_count(&absolute)
            )
        }
        Ok(metadata) => format!(
            "{compact_path} (file, {})",
            format_byte_size(metadata.len())
        ),
        Err(_) => format!("{compact_path} (missing)"),
    }
}

fn format_byte_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    let bytes_f = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes_f < MIB {
        format!("{:.1} KiB", bytes_f / KIB)
    } else {
        format!("{:.1} MiB", bytes_f / MIB)
    }
}

fn directory_item_count(path: &std::path::Path) -> usize {
    std::fs::read_dir(path)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

fn toggle_pinned_session_list(
    pinned_sessions: &mut Vec<String>,
    session_id: &str,
    max: usize,
) -> bool {
    if pinned_sessions.iter().any(|id| id == session_id) {
        pinned_sessions.retain(|id| id != session_id);
        return false;
    }
    if pinned_sessions.len() >= max {
        return false;
    }
    pinned_sessions.push(session_id.to_string());
    true
}

fn tool_part_status_from_turn_phase(phase: ToolTurnPhase) -> &'static str {
    match phase {
        ToolTurnPhase::Requested | ToolTurnPhase::Accepted | ToolTurnPhase::Executing => "running",
        ToolTurnPhase::ResultObserved
        | ToolTurnPhase::SentBackToModel
        | ToolTurnPhase::FinalAnswer
        | ToolTurnPhase::Persisted => "completed",
        ToolTurnPhase::Failed => "failed",
        ToolTurnPhase::Cancelled => "cancelled",
        ToolTurnPhase::TimedOut => "timed_out",
    }
}

fn persisted_final_answer_for_user(
    messages: &[MessageItem],
    current_user_content: &str,
) -> Option<String> {
    let assistant_idx = messages.iter().rposition(|message| {
        message.role == MessageRole::Assistant
            && !message.content.trim().is_empty()
            && !message.content.trim_start().starts_with("[Error:")
            && !message.content.trim_start().starts_with("[Cancelled:")
    })?;
    let prior_user_idx = messages[..assistant_idx]
        .iter()
        .rposition(|message| message.role == MessageRole::User)?;
    let prior_user = &messages[prior_user_idx];
    let latest_user_idx = messages
        .iter()
        .rposition(|message| message.role == MessageRole::User)?;
    (latest_user_idx == prior_user_idx
        && (normalize_turn_prompt(&prior_user.content)
            == normalize_turn_prompt(current_user_content)
            || assistant_idx > latest_user_idx))
        .then(|| messages[assistant_idx].content.clone())
}

fn normalize_turn_prompt(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn resolve_attachment_path(path: &str) -> std::path::PathBuf {
    let candidate = std::path::Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(candidate)
    }
}

fn attachment_file_preview(path: &std::path::Path, byte_len: u64) -> String {
    const MAX_PREVIEW_BYTES: usize = 64 * 1024;
    match std::fs::read(path) {
        Ok(bytes) => {
            let shown_len = bytes.len().min(MAX_PREVIEW_BYTES);
            let mut preview = String::from_utf8_lossy(&bytes[..shown_len]).to_string();
            if bytes.len() > shown_len {
                preview.push_str(&format!(
                    "\n\n... truncated: showing {} of {} bytes",
                    shown_len, byte_len
                ));
            }
            preview
        }
        Err(err) => format!("Failed to read attachment:\n{}", err),
    }
}

fn attachment_directory_preview(path: &std::path::Path) -> String {
    let mut lines = vec![format!("Directory: {}", path.display())];
    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries.into_iter().take(200) {
                let kind = if entry.path().is_dir() {
                    "dir "
                } else {
                    "file"
                };
                lines.push(format!("{}  {}", kind, entry.file_name().to_string_lossy()));
            }
        }
        Err(err) => lines.push(format!("Failed to read directory: {}", err)),
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;

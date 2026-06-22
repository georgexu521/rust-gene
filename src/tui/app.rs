//! 交互式终端 CLI 应用状态管理
//!
//! 对应 Claude Code 中的 AppState 概念

use crate::components::attachment_token::{AttachmentSource, AttachmentToken};
use crate::components::composer::{ComposerPart, ComposerState};
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
pub mod connect_wizard;
mod memory;
mod palette;
mod runtime;
mod runtime_state;
mod slash_commands;
mod status_tools;
pub use connect_wizard::*;
use memory::*;
pub use runtime::StreamUsageSnapshot;
use runtime::*;
pub(crate) use runtime::{
    parse_permission_mode, permission_mode_name, permission_rule_pattern, persist_permission_rule,
};
#[cfg(test)]
use runtime_state::{persisted_final_answer_for_user, toggle_pinned_session_list};

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
pub struct PastedBlock {
    pub placeholder: String,
    pub content: String,
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
    ConnectWizard,
    FilePicker,
    WorkspaceSwitcher,
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

/// Per-session UI state cached in memory so switching sessions preserves scroll position, expanded tools, etc.
#[derive(Debug, Clone, Default)]
pub struct SessionUiState {
    pub scroll_offset: usize,
    pub scroll_anchor_id: Option<String>,
    pub scroll_anchor_row_offset: usize,
    pub pinned_to_bottom: bool,
    pub expanded_tool_run_id: Option<String>,
    pub expanded_reasoning_message_id: Option<String>,
    pub expanded_inline_tool_ids: BTreeSet<String>,
    pub expanded_inline_message_part_ids: BTreeSet<String>,
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
    /// Composer state (text + structured prompt parts).
    pub composer: ComposerState,
    /// 消息列表
    pub messages: Vec<MessageItem>,
    /// 任务列表
    pub tasks: Vec<TaskItem>,
    /// 是否正在查询中
    pub is_querying: bool,
    /// Streaming start time for t/s calculation
    pub stream_started_at: Option<std::time::Instant>,
    /// Local watchdog clock for the post-tool provider round.
    pub post_tool_turn_wait_started_at: Option<std::time::Instant>,
    /// Last persisted session event seq before the active turn started.
    pub current_turn_event_start_seq: Option<i64>,
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
    /// Chat transcript row scroll position.
    pub scroll_offset: usize,
    /// Stable timeline item id for manual scroll anchors.
    pub scroll_anchor_id: Option<String>,
    /// Row offset inside `scroll_anchor_id`, used to survive inserted timeline items.
    pub scroll_anchor_row_offset: usize,
    /// 是否自动贴底（用户手动上滚后变为 false，滚到底或新消息时恢复）
    pub pinned_to_bottom: bool,
    /// Last rendered chat viewport dimensions for row-level scrolling.
    pub chat_viewport_width: u16,
    pub chat_viewport_height: u16,
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
    /// Per-session UI state cache.
    pub session_ui_states: HashMap<String, SessionUiState>,
    /// Recent session navigation stack (most recent at the end).
    pub recent_session_stack: Vec<String>,
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
    /// 侧边栏 workspace 过滤（None 表示全部）。
    pub sidebar_workspace_filter: Option<String>,
    /// Workspace switcher overlay items.
    pub workspace_switcher_items: Vec<String>,
    /// Workspace switcher selected index.
    pub workspace_switcher_selected: usize,
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
    /// Provider connect wizard state.
    pub connect_wizard_state: Option<crate::tui::app::connect_wizard::ConnectWizardState>,
    /// Discovered models for the active provider.
    pub discovered_models: Vec<crate::services::api::model_discovery::DiscoveredModel>,
    /// Whether model discovery is currently fetching live models.
    pub discovering_models: bool,
    /// Model discovery service.
    pub model_discovery: crate::services::api::model_discovery::ModelDiscovery,
    /// Skill invocations waiting for final assistant outcome attribution.
    pending_skill_invocations: Vec<PendingSkillInvocation>,
    /// Discovered plugins and their runtime facts.
    pub plugin_facts: Vec<crate::plugins::PluginRuntimeFacts>,
    /// Static plugin UI slot contributions (sidebar_footer, status_bar).
    pub plugin_ui_contributions: Vec<crate::plugins::PluginUiSlotContent>,
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
        if let Some(cache_write_tokens) = usage.cache_write_tokens {
            metadata.insert(
                "cache_write_tokens".to_string(),
                cache_write_tokens.to_string(),
            );
        }
    }
    metadata
}

fn is_validation_tool_name(name: &str) -> bool {
    matches!(name, "run_tests" | "git_status" | "git_diff")
}

impl TuiApp {
    /// 计算待审批工具的 Diff 预览（前端无关实现位于 `crate::shell::permission_diff`）。
    pub fn compute_permission_diff(&self) -> Option<(String, String)> {
        let req = self.pending_permission_request.as_ref()?;
        crate::shell::permission_diff::compute_permission_diff(req)
    }

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

    pub fn open_workspace_switcher(&mut self) {
        match self.session_manager.list_workspaces() {
            Ok(mut workspaces) => {
                if workspaces.is_empty() {
                    workspaces.push(self.workspace.root.to_string_lossy().to_string());
                }
                self.workspace_switcher_items = workspaces;
                self.workspace_switcher_selected = 0;
                self.push_mode(AppMode::WorkspaceSwitcher);
            }
            Err(e) => {
                self.add_system_message(format!("Failed to list workspaces: {e}"));
            }
        }
    }

    pub fn close_workspace_switcher(&mut self) {
        self.pop_mode();
    }

    pub fn workspace_switcher_next(&mut self) {
        if self.workspace_switcher_items.is_empty() {
            return;
        }
        self.workspace_switcher_selected =
            (self.workspace_switcher_selected + 1) % self.workspace_switcher_items.len();
    }

    pub fn workspace_switcher_prev(&mut self) {
        if self.workspace_switcher_items.is_empty() {
            return;
        }
        if self.workspace_switcher_selected == 0 {
            self.workspace_switcher_selected = self.workspace_switcher_items.len() - 1;
        } else {
            self.workspace_switcher_selected -= 1;
        }
    }

    pub fn accept_workspace_switcher(&mut self) -> String {
        let Some(root) = self
            .workspace_switcher_items
            .get(self.workspace_switcher_selected)
        else {
            return "No workspace selected.".to_string();
        };
        let root = root.clone();
        self.workspace = crate::workspace::Workspace::detect(&root);
        self.sidebar_workspace_filter = Some(root.clone());
        self.close_workspace_switcher();
        if let Err(err) = self.kv_store.set_string("ui.last_workspace_root", &root) {
            tracing::warn!("Failed to persist workspace switch: {err}");
        }
        format!("Switched workspace to {}", self.workspace.display_name)
    }

    /// Save the current UI state for `session_id` into the in-memory cache.
    pub fn save_session_ui_state(&mut self, session_id: &str) {
        self.session_ui_states.insert(
            session_id.to_string(),
            SessionUiState {
                scroll_offset: self.scroll_offset,
                scroll_anchor_id: self.scroll_anchor_id.clone(),
                scroll_anchor_row_offset: self.scroll_anchor_row_offset,
                pinned_to_bottom: self.pinned_to_bottom,
                expanded_tool_run_id: self.expanded_tool_run_id.clone(),
                expanded_reasoning_message_id: self.expanded_reasoning_message_id.clone(),
                expanded_inline_tool_ids: self.expanded_inline_tool_ids.clone(),
                expanded_inline_message_part_ids: self.expanded_inline_message_part_ids.clone(),
            },
        );
    }

    /// Restore the cached UI state for `session_id`, if any.
    pub fn restore_session_ui_state(&mut self, session_id: &str) {
        if let Some(state) = self.session_ui_states.get(session_id).cloned() {
            self.scroll_offset = state.scroll_offset;
            self.scroll_anchor_id = state.scroll_anchor_id;
            self.scroll_anchor_row_offset = state.scroll_anchor_row_offset;
            self.pinned_to_bottom = state.pinned_to_bottom;
            self.expanded_tool_run_id = state.expanded_tool_run_id;
            self.expanded_reasoning_message_id = state.expanded_reasoning_message_id;
            self.expanded_inline_tool_ids = state.expanded_inline_tool_ids;
            self.expanded_inline_message_part_ids = state.expanded_inline_message_part_ids;
        } else {
            // Default state for a freshly restored session: pinned to bottom.
            self.scroll_offset = 0;
            self.scroll_anchor_id = None;
            self.scroll_anchor_row_offset = 0;
            self.pinned_to_bottom = true;
            self.expanded_tool_run_id = None;
            self.expanded_reasoning_message_id = None;
            self.expanded_inline_tool_ids.clear();
            self.expanded_inline_message_part_ids.clear();
        }
    }

    /// Push `session_id` onto the recent session navigation stack.
    pub fn push_recent_session(&mut self, session_id: &str) {
        // Remove existing entry so the most recent instance is at the end.
        self.recent_session_stack.retain(|id| id != session_id);
        self.recent_session_stack.push(session_id.to_string());
        // Keep a bounded history.
        if self.recent_session_stack.len() > 32 {
            self.recent_session_stack.remove(0);
        }
    }

    /// Return the previous session in the recent stack, if any.
    pub fn previous_recent_session(&self) -> Option<&str> {
        // The current session is at the end; the one before it is the previous.
        if self.recent_session_stack.len() >= 2 {
            self.recent_session_stack
                .get(self.recent_session_stack.len() - 2)
                .map(String::as_str)
        } else {
            None
        }
    }

    /// Replace the current session id on top of the recent stack (e.g. after fork).
    pub fn replace_recent_session(&mut self, session_id: &str) {
        if let Some(last) = self.recent_session_stack.last_mut() {
            *last = session_id.to_string();
        } else {
            self.recent_session_stack.push(session_id.to_string());
        }
    }

    /// Replace the current mode without pushing a new stack frame.
    pub fn replace_mode(&mut self, mode: AppMode) {
        self.mode = mode;
    }

    /// Cycle to the next recent session in the stack.
    pub async fn cycle_recent_session_forward(&mut self) {
        let current = self
            .session_manager
            .current_session_id()
            .map(str::to_string);
        if self.recent_session_stack.len() < 2 {
            return;
        }
        // Rotate the stack forward: move oldest to end.
        let oldest = self.recent_session_stack.remove(0);
        self.recent_session_stack.push(oldest.clone());
        // The new end is the target; restore it unless it's already current.
        let target = self.recent_session_stack.last().cloned().unwrap_or(oldest);
        if current.as_deref() != Some(&target) {
            let _ = self.restore_session(&target).await;
        }
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

    pub fn session_list_view_model(
        &self,
        sidebar_width: u16,
    ) -> crate::tui::view_model::session_list::SessionListViewModel {
        use crate::tui::view_model::session_list::{RenameState, SessionListViewModel};
        let sessions = self.visible_sidebar_sessions(50);
        let current_id = self.session_manager.current_session_id();
        let current_workspace = self.workspace.root.to_string_lossy().to_string();
        let rename = self.renaming_session_id.as_ref().map(|id| RenameState {
            session_id: id.clone(),
            buffer: self.rename_buffer.clone(),
        });
        let mut message_counts = std::collections::HashMap::new();
        for session in &sessions {
            if let Ok(count) = self.session_manager.message_count(&session.id) {
                message_counts.insert(session.id.clone(), count);
            }
        }
        SessionListViewModel::build(
            &sessions,
            current_id,
            &current_workspace,
            &self.pinned_sessions,
            self.sidebar_selected,
            self.confirm_delete_session_id.as_deref(),
            rename.as_ref(),
            &self.sidebar_filter,
            self.filtering_sidebar,
            sidebar_width,
            &message_counts,
        )
    }

    pub fn visible_sidebar_sessions(
        &self,
        limit: usize,
    ) -> Vec<crate::session_store::SessionRecord> {
        let sessions = if let Some(ref workspace_root) = self.sidebar_workspace_filter {
            self.session_manager
                .list_sessions_by_workspace(workspace_root, limit.min(i64::MAX as usize) as i64)
                .unwrap_or_default()
        } else {
            self.session_manager
                .list_sessions(limit.min(i64::MAX as usize) as i64)
                .unwrap_or_default()
        };
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
        let task_manager = crate::internal::task_manager::GLOBAL_TASK_MANAGER.clone();
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
                None,
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

        let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let workspace = kv_store
            .get_string("ui.last_workspace_root")
            .and_then(|root| {
                let path = std::path::PathBuf::from(&root);
                if path.is_absolute() && path.exists() {
                    Some(crate::workspace::Workspace::detect(&path))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| Workspace::detect(&current_dir));

        if session_manager.current_session_id().is_none() {
            if let Ok(_id) = session_manager.start_session(
                "New Session",
                &model,
                Some(&workspace.root.to_string_lossy()),
            ) {
                let _ = session_manager.backfill_workspace_root(&workspace.root.to_string_lossy());
            }
        } else {
            let _ = session_manager.backfill_workspace_root(&workspace.root.to_string_lossy());
        }

        // Restart safety: mark any active goals as paused so they don't auto-resume

        let mut app = Self {
            mode: AppMode::Chat,
            mode_stack: Vec::new(),
            leader_state: None,
            workspace,
            agent_mode: AgentMode::Auto,
            composer: ComposerState::new(),
            messages: Vec::new(),
            tasks: Vec::new(),
            is_querying: false,
            stream_started_at: None,
            post_tool_turn_wait_started_at: None,
            current_turn_event_start_seq: None,
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
            scroll_anchor_row_offset: 0,
            pinned_to_bottom: true,
            chat_viewport_width: 80,
            chat_viewport_height: 24,
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
            session_ui_states: HashMap::new(),
            recent_session_stack: Vec::new(),
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
            sidebar_workspace_filter: None,
            workspace_switcher_items: Vec::new(),
            workspace_switcher_selected: 0,
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
            connect_wizard_state: None,
            discovered_models: Vec::new(),
            discovering_models: false,
            model_discovery: crate::services::api::model_discovery::ModelDiscovery::new(),
            pending_skill_invocations: Vec::new(),
            plugin_facts: Vec::new(),
            plugin_ui_contributions: Vec::new(),
            goal_runner: None,
            pending_goal_prompt: None,
        };

        app.refresh_plugin_facts();

        app
    }

    /// Rediscover plugins and refresh runtime facts + static UI contributions.
    pub fn refresh_plugin_facts(&mut self) {
        let roots = crate::plugins::default_plugin_roots(
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        let plugins = crate::plugins::discover_plugins(&roots);
        let trust_mode = crate::plugins::trust::TrustMode::Off;
        self.plugin_facts = crate::plugins::runtime_facts(&plugins, trust_mode);
        let (contributions, warnings) = crate::plugins::load_static_ui_contributions(&plugins);
        self.plugin_ui_contributions = contributions;
        for warning in warnings {
            self.add_toast(
                format!("Plugin {}: {}", warning.plugin_id, warning.message),
                "⚠",
            );
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
        let raw_text = self.composer.text.value().to_string();
        let trimmed = raw_text.trim();
        if trimmed.is_empty() && self.composer.parts.is_empty() {
            return;
        }

        // 处理斜杠命令
        if trimmed.starts_with('/') {
            self.handle_slash_command(trimmed).await;
            if let Some(prompt) = self.pending_goal_prompt.take() {
                self.send_message(prompt).await;
            }
            self.composer.text.clear();
            return;
        }

        let content = self.composer.build_submission();
        if content.trim().is_empty() {
            return;
        }
        self.composer.clear();
        self.send_message(content).await;
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
        self.refresh_post_tool_turn_wait_clock();
        if self.recover_persisted_error_if_available().await {
            return;
        }
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

#[cfg(test)]
mod tests;

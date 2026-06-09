//! 统一对话循环
//!
//! 将 QueryEngine 和 StreamingEngineInner 中重复的工具调用循环合并为一处。
//! 支持流式/非流式两种输出模式，内部逻辑完全一致。
//!
//! 改进（借鉴 hermes-agent）：
//! - 前置压缩（Preflight）：循环前检查总 token，超阈值提前压缩
//! - IterationBudget：迭代预算退还机制（只读工具可退还）

mod action_checkpoint;
mod api_request_controller;
mod approval;
mod assistant_response_retry_controller;
mod closeout_controller;
mod companion_context;
mod context_budget_controller;
#[cfg(test)]
mod direct_task_behavior_tests;
mod first_code_change_controller;
#[cfg(test)]
mod focused_repair_recovery;
mod focused_repair_state_controller;
mod legacy_workflow_gate_controller;
mod memory_snapshot_controller;
mod memory_sync_controller;
#[cfg(test)]
mod patch_recovery;
#[cfg(test)]
mod patch_repair_rules;
#[cfg(test)]
mod patch_synthesis_executor;
#[cfg(test)]
mod patch_synthesis_flow_controller;
mod permission_controller;
mod permission_recovery;
mod post_change_workflow_controller;
mod post_edit_repair_controller;
mod post_edit_verification_controller;
mod preflight_compression_controller;
mod pseudo_tool_text;
mod reflection_gate_controller;
mod repair_controller;
mod request_preparation_controller;
mod request_timeouts;
mod risk_signal_controller;
#[cfg(test)]
mod route_scoped_tools_tests;
mod runtime_diet;
mod session_processor;
mod step_executor;
mod task_guidance_controller;
mod text_sanitizer;
mod tool_batch_result_processor;
mod tool_call_lifecycle;
mod tool_context_helpers;
mod tool_execution;
mod tool_execution_controller;
mod tool_failure_guided_debugging;
mod tool_metadata;
mod tool_orchestrator;
mod tool_result_controller;
mod tool_round_controller;
mod tool_turn_controller;
mod turn_api_failure_controller;
mod turn_assistant_response_controller;
mod turn_completion_controller;
mod turn_context_bootstrap_controller;
mod turn_entry_gate_controller;
#[cfg(test)]
mod turn_focused_repair_action_controller;
mod turn_focused_repair_flow_controller;
mod turn_iteration_closeout_controller;
mod turn_iteration_controller;
mod turn_iteration_loop_controller;
mod turn_iteration_setup_controller;
mod turn_loop_bootstrap_controller;
mod turn_loop_policy;
mod turn_model_step_controller;
mod turn_recording;
mod turn_request_bootstrap_controller;
mod turn_retrieval_context_controller;
mod turn_setup_controller;
mod turn_state;
mod turn_task_context_controller;
mod turn_tool_round_step_controller;
mod validation_runner;
mod workflow_change_tracker;
mod workflow_contract_controller;
mod workflow_prompt_policy;
mod workflow_runtime;
mod workflow_trace;

pub use approval::{ToolApprovalChannel, ToolApprovalRequest, ToolApprovalResponse};
#[cfg(test)]
use patch_recovery::PatchSynthesisAction;
pub(crate) use step_executor::{is_drift_interruption_signal, WorkflowRealStepExecutor};
#[cfg(test)]
use text_sanitizer::strip_hidden_blocks;
#[cfg(test)]
use text_sanitizer::VisibleTextSanitizer;
#[cfg(test)]
use tool_context_helpers::tool_not_allowed_result;
pub(crate) use tool_execution::safe_prefix_by_bytes;
#[cfg(test)]
pub(crate) use tool_execution::safe_suffix_by_bytes;
#[cfg(test)]
use tool_execution::truncate_tool_result;
#[cfg(test)]
use tool_execution_controller::{
    ToolExecutionContext, ToolExecutionController, ToolExecutionRequest,
};
#[cfg(test)]
use tool_metadata::attach_tool_execution_metadata;
#[cfg(test)]
use tool_metadata::tool_execution_start_progress;
use turn_completion_controller::{TurnCompletionContext, TurnCompletionController};
use turn_context_bootstrap_controller::{
    TurnContextBootstrapContext, TurnContextBootstrapController,
};
use turn_entry_gate_controller::{
    TurnEntryGateContext, TurnEntryGateController, TurnEntryGateFlow,
};
use turn_iteration_loop_controller::{TurnIterationLoopContext, TurnIterationLoopController};
use turn_loop_bootstrap_controller::{TurnLoopBootstrapContext, TurnLoopBootstrapController};
use turn_setup_controller::{TurnSetupContext, TurnSetupController};
#[cfg(test)]
use validation_runner::shell_output_with_timeout;
#[cfg(test)]
use validation_runner::verification_source_context;
#[cfg(test)]
use validation_runner::RequiredValidationController;

#[cfg(test)]
use crate::engine::trace::TraceEvent;
use crate::engine::trace::{TraceCollector, TraceStore, TurnStatus};
use crate::engine::workflow::WorkflowPolicy;
use crate::services::api::{LlmProvider, Message, ToolCall};
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::context_compressor::ContextCompressor;
use super::hooks::ToolHookManager;
use super::streaming::StreamEvent;

/// Self-correction: replace the last assistant message when the user steers.
///
/// When the user interrupts with a correction, the previous assistant response
/// (which may contain incorrect tool calls or reasoning) stays in the message
/// history and wastes tokens. This function replaces the last assistant message
/// with a corrected version, so the model sees a clean context.
///
/// Controlled by `PRIORITY_AGENT_SELF_CORRECTION` (default on, set to "0" to disable).
pub fn replace_last_assistant_message(messages: &mut [Message], correction: &str) {
    if std::env::var("PRIORITY_AGENT_SELF_CORRECTION")
        .unwrap_or_else(|_| "1".to_string())
        .trim()
        == "0"
    {
        return;
    }

    // Find the last assistant message (with or without tool_calls).
    if let Some(pos) = messages
        .iter()
        .rposition(|m| matches!(m, Message::Assistant { .. }))
    {
        let replacement = format!(
            "[The user corrected the previous response. The correct approach:]\n{}",
            correction
        );
        messages[pos] = Message::Assistant {
            content: replacement,
            tool_calls: None,
        };
    }
}

fn should_use_nonstreaming_tools(
    provider: &dyn LlmProvider,
    tools: &[crate::services::api::Tool],
) -> bool {
    if tools.is_empty() {
        return false;
    }
    crate::services::api::provider_protocol::ProviderCapabilities::detect(
        provider.base_url(),
        provider.default_model(),
    )
    .requires_nonstreaming_tool_calls
}

/// 统一对话循环
pub struct ConversationLoop {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
    model: String,
    temperature: f32,
    /// 会话 ID（固定，用于追踪 checkpoint、记忆等）
    session_id: String,
    max_iterations: usize,
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    hook_manager: Option<Arc<ToolHookManager>>,
    /// 上下文压缩器
    compressor: Option<Arc<Mutex<ContextCompressor>>>,
    /// 记忆管理器（预取 + 围栏注入 + 同步）
    memory_manager: Option<Arc<Mutex<crate::memory::MemoryManager>>>,
    /// 工具权限模式（由上层引擎注入）
    permission_mode: crate::permissions::PermissionMode,
    /// 当前会话内临时权限规则
    session_permission_rules: crate::permissions::PermissionRules,
    /// 是否启用 LLM 驱动的记忆提取
    llm_memory_extraction: bool,
    /// Whether existing memory may be injected/recalled for model requests.
    memory_use_enabled: bool,
    /// Whether this session may generate future memory proposals/sync output.
    memory_generate_enabled: bool,
    /// Dynamic memory recall mode: balanced, strict, preference-only, or off.
    memory_recall_mode: String,
    /// 工具授权通道（用于 MCP 等工具的交互式授权）
    approval_channel: Option<Arc<ToolApprovalChannel>>,
    /// 工具白名单（用于子 Agent 隔离；None 表示不限制）
    allowed_tools: Option<HashSet<String>>,
    /// Optional per-loop working directory override for isolated workers.
    working_dir_override: Option<PathBuf>,
    /// Optional MCP server allowlist for scoped sub-agent runs.
    allowed_mcp_servers: Option<Vec<String>>,
    /// 本轮是否已触发过 Workflow（每轮最多一次）
    workflow_triggered_this_turn: std::sync::atomic::AtomicBool,
    /// Workflow 策略（默认从环境变量读取，可覆盖）
    workflow_policy: WorkflowPolicy,
    /// Product-level coding agent mode selected by the user.
    agent_mode: crate::engine::agent_mode::AgentMode,
    /// 拒绝追踪器
    denial_tracker: Option<Arc<crate::security::DenialTracker>>,
    /// 安全审计日志
    audit_log: Option<Arc<crate::security::SecurityAuditLog>>,
    /// Runtime trace store for recent turn timelines.
    trace_store: Option<Arc<TraceStore>>,
    /// Runtime session goal manager.
    goal_manager: Option<Arc<crate::engine::session_goal::SessionGoalManager>>,
    /// Optional persistent store for completed traces.
    session_store: Option<Arc<crate::session_store::SessionStore>>,
    /// Monotonic turn counter used for trace display.
    turn_counter: std::sync::atomic::AtomicU64,
    /// Read-before-edit guard — shared with tool contexts so file_read
    /// results are visible to file_edit/file_write guards.
    read_tracker: Option<Arc<crate::engine::read_tracker::ReadTracker>>,
}

/// 对话循环结果
pub struct LoopResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_calls_made: bool,
    pub iterations: usize,
    /// 流式预执行的只读工具结果（tool_index → result）
    /// execute_tools_parallel 应跳过已有结果的只读工具
    pub pre_executed_results: std::collections::HashMap<usize, ToolResult>,
}

impl ConversationLoop {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
        model: String,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            cost_tracker,
            model,
            temperature: 0.2,
            max_iterations: 50, // Match Reasonix DEFAULT_MAX_ITER_PER_TURN
            agent_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            hook_manager: ToolHookManager::from_env().map(Arc::new),
            compressor: None,
            memory_manager: None,
            permission_mode: crate::permissions::PermissionMode::AutoAll,
            session_permission_rules: crate::permissions::PermissionRules::new(),
            llm_memory_extraction: false,
            memory_use_enabled: true,
            memory_generate_enabled: true,
            memory_recall_mode: "balanced".to_string(),
            approval_channel: None,
            allowed_tools: None,
            working_dir_override: None,
            allowed_mcp_servers: None,
            workflow_triggered_this_turn: std::sync::atomic::AtomicBool::new(false),
            workflow_policy: WorkflowPolicy::from_env(),
            agent_mode: crate::engine::agent_mode::AgentMode::Auto,
            session_id: format!("session-{}", uuid::Uuid::new_v4()),
            denial_tracker: None,
            audit_log: None,
            trace_store: None,
            goal_manager: None,
            session_store: None,
            turn_counter: std::sync::atomic::AtomicU64::new(0),
            read_tracker: None,
        }
    }

    /// 启用记忆管理器（预取 + 围栏注入 + 同步）
    pub fn with_memory_manager(
        mut self,
        manager: Arc<Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        self.memory_manager = Some(manager);
        self
    }

    /// 启用上下文压缩（设置最大上下文 token 数）
    pub fn with_compression(mut self, max_context_tokens: u64) -> Self {
        self.compressor = Some(Arc::new(Mutex::new(
            ContextCompressor::new(max_context_tokens)
                .with_llm_provider(self.provider.clone(), &self.model),
        )));
        self
    }

    pub fn with_compressor(mut self, compressor: Arc<Mutex<ContextCompressor>>) -> Self {
        self.compressor = Some(compressor);
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature.clamp(0.0, 2.0);
        self
    }

    pub fn with_agent_manager(mut self, manager: Arc<crate::agent::AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    pub fn with_mcp_manager(mut self, manager: Arc<crate::engine::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub fn with_lsp_manager(mut self, manager: Arc<crate::engine::lsp::LspManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    pub fn with_worktree_manager(
        mut self,
        manager: Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    pub fn with_hook_manager(mut self, manager: Arc<ToolHookManager>) -> Self {
        self.hook_manager = Some(manager);
        self
    }

    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    pub fn with_session_permission_rules(
        mut self,
        rules: crate::permissions::PermissionRules,
    ) -> Self {
        self.session_permission_rules = rules;
        self
    }

    pub fn with_llm_memory_extraction(mut self, enabled: bool) -> Self {
        self.llm_memory_extraction = enabled;
        self
    }

    pub fn with_memory_use(mut self, enabled: bool) -> Self {
        self.memory_use_enabled = enabled;
        self
    }

    pub fn with_memory_generate(mut self, enabled: bool) -> Self {
        self.memory_generate_enabled = enabled;
        self
    }

    pub fn with_memory_recall_mode(mut self, mode: impl Into<String>) -> Self {
        self.memory_recall_mode = mode.into();
        self
    }

    pub(super) fn memory_manager_for_static_memory(
        &self,
    ) -> Option<&Arc<Mutex<crate::memory::MemoryManager>>> {
        self.memory_use_enabled
            .then_some(self.memory_manager.as_ref())
            .flatten()
    }

    pub(super) fn memory_manager_for_dynamic_recall(
        &self,
    ) -> Option<&Arc<Mutex<crate::memory::MemoryManager>>> {
        let recall_enabled = !self.memory_recall_mode.eq_ignore_ascii_case("off");
        (self.memory_use_enabled && recall_enabled)
            .then_some(self.memory_manager.as_ref())
            .flatten()
    }

    pub(super) fn memory_manager_for_generate(
        &self,
    ) -> Option<&Arc<Mutex<crate::memory::MemoryManager>>> {
        self.memory_generate_enabled
            .then_some(self.memory_manager.as_ref())
            .flatten()
    }

    pub fn with_approval_channel(mut self, channel: Arc<ToolApprovalChannel>) -> Self {
        self.approval_channel = Some(channel);
        self
    }

    pub fn with_allowed_tools(mut self, tools: HashSet<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    pub fn with_working_dir(mut self, working_dir: impl Into<PathBuf>) -> Self {
        self.working_dir_override = Some(working_dir.into());
        self
    }

    /// Attach the ReadTracker so file_read results gate file_edit/file_write.
    pub fn with_read_tracker(
        mut self,
        tracker: Arc<crate::engine::read_tracker::ReadTracker>,
    ) -> Self {
        self.read_tracker = Some(tracker);
        self
    }

    pub fn with_allowed_mcp_servers(mut self, servers: Vec<String>) -> Self {
        let servers = servers
            .into_iter()
            .map(|server| server.trim().to_string())
            .filter(|server| !server.is_empty())
            .collect::<Vec<_>>();
        if !servers.is_empty() {
            self.allowed_mcp_servers = Some(servers);
        }
        self
    }

    pub fn with_workflow_policy(mut self, policy: WorkflowPolicy) -> Self {
        self.workflow_policy = policy;
        self
    }

    pub fn with_agent_mode(mut self, mode: crate::engine::agent_mode::AgentMode) -> Self {
        self.agent_mode = mode;
        self
    }

    pub fn with_trace_store(mut self, store: Arc<TraceStore>) -> Self {
        self.trace_store = Some(store);
        self
    }

    pub fn with_session_goal_manager(
        mut self,
        manager: Arc<crate::engine::session_goal::SessionGoalManager>,
    ) -> Self {
        self.goal_manager = Some(manager);
        self
    }

    pub fn with_session_store(
        mut self,
        store: Arc<crate::session_store::SessionStore>,
        session_id: impl Into<String>,
    ) -> Self {
        self.session_store = Some(store);
        self.session_id = session_id.into();
        self
    }

    /// 创建工具执行上下文
    fn create_tool_context(&self) -> ToolContext {
        let working_dir = self
            .working_dir_override
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let mut ctx = ToolContext::new(working_dir, self.session_id.clone());
        if let Some(ref manager) = self.agent_manager {
            ctx = ctx.with_agent_manager(manager.clone());
        }
        if let Some(ref store) = self.session_store {
            ctx = ctx.with_session_store(store.clone());
        }
        if let Some(ref mcp) = self.mcp_manager {
            ctx = ctx.with_mcp_manager(mcp.clone());
        }
        if let Some(ref lsp) = self.lsp_manager {
            ctx = ctx.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            ctx = ctx.with_worktree_manager(wt.clone());
        }
        if let Some(ref memory) = self.memory_manager {
            ctx = ctx.with_memory_manager(memory.clone());
        }
        if let Some(ref tracker) = self.read_tracker {
            ctx = ctx.with_read_tracker(tracker.clone());
        }
        if let Some(servers) = self.allowed_mcp_servers.as_ref() {
            ctx.metadata
                .insert("allowed_mcp_servers".to_string(), servers.join(","));
        }
        ctx = ctx.with_llm_provider(self.provider.clone());
        ctx = ctx.with_model(&self.model);
        ctx = ctx.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        // 权限模式由上层引擎注入（默认 AutoAll，保留高风险确认）
        ctx.permission_context.mode = self.permission_mode;
        ctx.permission_context
            .rules
            .always_allow
            .extend(self.session_permission_rules.always_allow.clone());
        ctx.permission_context
            .rules
            .always_deny
            .extend(self.session_permission_rules.always_deny.clone());
        ctx.permission_context
            .rules
            .always_ask
            .extend(self.session_permission_rules.always_ask.clone());
        ctx
    }

    fn create_tool_context_with_trace(&self, trace: &TraceCollector) -> ToolContext {
        self.create_tool_context()
            .with_trace_collector(trace.clone())
    }

    /// Durable settlement recovery: scan session_parts for tools that are still
    /// running/pending from a previous turn (e.g. after a crash or provider
    /// interruption) and write `tool_failed` events so the projection never
    /// leaves them dangling.
    fn recover_unsettled_tools(&self, trace: &TraceCollector) {
        let Some(ref store) = self.session_store else {
            return;
        };
        let parts = match store.get_session_parts(&self.session_id) {
            Ok(parts) => parts,
            Err(err) => {
                tracing::warn!("Failed to read session_parts for recovery: {}", err);
                return;
            }
        };
        let unsettled: Vec<_> = parts
            .into_iter()
            .filter(|part| {
                (part.kind == "tool" || part.kind == "shell")
                    && matches!(part.status.as_deref(), Some("running" | "pending"))
            })
            .collect();
        if unsettled.is_empty() {
            return;
        }
        let writer =
            crate::session_store::SessionEventWriter::new(store.shared_conn(), &self.session_id);
        let unsettled_count = unsettled.len();
        for part in &unsettled {
            let tool_call_id = part
                .tool_call_id
                .clone()
                .unwrap_or_else(|| part.part_id.clone());
            let tool_name = part
                .tool_name
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let error = format!(
                "Tool execution interrupted before settlement ({}:{})",
                tool_name, tool_call_id
            );
            if let Err(err) = writer.tool_failed(&tool_call_id, &error) {
                tracing::warn!("Failed to write recovery tool_failed event: {}", err);
            }
        }
        trace.record(crate::engine::trace::TraceEvent::WorkflowFallback {
            error: format!(
                "durable_recovery: settled {} interrupted tool(s) from previous turn",
                unsettled_count
            ),
        });
    }

    /// 运行对话循环（非流式）
    pub async fn run(&self, messages: Vec<Message>) -> Result<LoopResult> {
        self.run_inner(messages, None::<&mpsc::Sender<StreamEvent>>)
            .await
    }

    /// 运行对话循环（流式）
    pub async fn run_streaming(
        &self,
        messages: Vec<Message>,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<LoopResult> {
        self.run_inner(messages, Some(tx)).await
    }

    /// 核心循环实现
    async fn run_inner(
        &self,
        mut messages: Vec<Message>,
        tx: Option<&mpsc::Sender<StreamEvent>>,
    ) -> Result<LoopResult> {
        // Self-correction: if the user message looks like a correction/drift
        // signal, replace the previous (likely incorrect) assistant response.
        let drift_text: Option<String> = messages.iter().rev().find_map(|m| match m {
            Message::User { content } if is_drift_interruption_signal(content) => {
                Some(content.clone())
            }
            _ => None,
        });
        if let Some(correction) = drift_text {
            replace_last_assistant_message(&mut messages, &correction);
        }

        let setup = TurnSetupController::prepare(TurnSetupContext {
            conversation: self,
            messages: &messages,
        });
        let last_user_preview = setup.last_user_preview;
        let required_validation_commands = setup.required_validation_commands;
        let no_diff_audit_closeout_allowed = setup.no_diff_audit_closeout_allowed;
        let code_write_tools_forbidden = setup.code_write_tools_forbidden;
        let trace = setup.trace;
        let learning_events = setup.learning_events;
        let route = setup.route;
        let resource_policy = setup.resource_policy;
        let working_dir = setup.working_dir;
        let destructive_scope = setup.destructive_scope;
        let main_loop_profile =
            turn_loop_policy::MainLoopProfile::from_turn(&route, &required_validation_commands);
        let turn_context_bootstrap =
            TurnContextBootstrapController::run(TurnContextBootstrapContext {
                conversation: self,
                last_user_preview: &last_user_preview,
                route: &route,
                profile: main_loop_profile,
                resource_policy: &resource_policy,
                working_dir: &working_dir,
                required_validation_commands: &required_validation_commands,
                trace: &trace,
            })
            .await;
        let turn_retrieval_context = turn_context_bootstrap.retrieval_context;
        let retained_context = turn_context_bootstrap.retained_context;
        let mut task_bundle = turn_context_bootstrap.task_bundle;
        let mut code_workflow = turn_context_bootstrap.code_workflow;
        let mut turn_state = turn_context_bootstrap.turn_state;

        // Durable settlement recovery: before starting a new turn, ensure any
        // tools left running from a previous turn are marked as failed.
        self.recover_unsettled_tools(&trace);

        match TurnEntryGateController::run(TurnEntryGateContext {
            conversation: self,
            last_user_preview: last_user_preview.as_str(),
            route: &route,
            working_dir: &working_dir,
            learning_events: &learning_events,
            retrieval_context: turn_retrieval_context.as_ref(),
            task_bundle: &mut task_bundle,
            code_workflow: &mut code_workflow,
            required_validation_commands: &required_validation_commands,
            messages: &mut messages,
            trace: &trace,
            tx,
        })
        .await
        {
            TurnEntryGateFlow::Continue => {}
            TurnEntryGateFlow::Stop { content, status } => {
                self.finish_trace(trace.clone(), status).await;
                return Ok(LoopResult {
                    content,
                    tool_calls: Vec::new(),
                    tool_calls_made: false,
                    iterations: 0,
                    pre_executed_results: std::collections::HashMap::new(),
                });
            }
        }

        let turn_loop_bootstrap = TurnLoopBootstrapController::run(TurnLoopBootstrapContext {
            conversation: self,
            route: &route,
            profile: main_loop_profile,
            retrieval_context: turn_retrieval_context.as_ref(),
            working_dir: &working_dir,
            turn_state: &mut turn_state,
            messages: &mut messages,
            trace: &trace,
            tx,
        })
        .await;
        let base_tools = turn_loop_bootstrap.base_tools;
        let available_tools = turn_loop_bootstrap.available_tools;
        let mut loop_state = turn_loop_bootstrap.loop_state;

        TurnIterationLoopController::run(TurnIterationLoopContext {
            conversation: self,
            route: &route,
            profile: main_loop_profile,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            turn_retrieval_context: turn_retrieval_context.as_ref(),
            retained_context: &retained_context,
            base_tools: &base_tools,
            available_tools: &available_tools,
            loop_state: &mut loop_state,
            turn_state: &mut turn_state,
            no_diff_audit_closeout_allowed,
            code_write_tools_forbidden,
            resource_policy: &resource_policy,
            working_dir: &working_dir,
            last_user_preview: last_user_preview.as_str(),
            required_validation_commands: &required_validation_commands,
            destructive_scope: &destructive_scope,
            messages: &mut messages,
            trace: &trace,
            tx,
        })
        .await?;

        let settlement_gaps = turn_state.tool_lifecycle.unsettled_summaries();
        let result = TurnCompletionController::complete(TurnCompletionContext {
            trace: &trace,
            route: &route,
            code_workflow: &code_workflow,
            task_bundle: &task_bundle,
            required_validation_commands: &required_validation_commands,
            runtime_diet: &mut turn_state.runtime_diet,
            final_content: &mut loop_state.final_content,
            final_tool_calls: &loop_state.final_tool_calls,
            iterations_used: turn_state.iterations_used,
            max_iterations: self.max_iterations,
            tool_calls_made: loop_state.tool_calls_made,
            evidence_ledger: &turn_state.evidence_ledger,
            settlement_gaps: &settlement_gaps,
            memory_generate_enabled: self.memory_generate_enabled,
            tx,
        })
        .await;

        self.finish_trace(trace, TurnStatus::Completed).await;

        Ok(result)
    }
}

#[cfg(test)]
mod tests;

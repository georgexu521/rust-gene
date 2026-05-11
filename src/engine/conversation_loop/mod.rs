//! 统一对话循环
//!
//! 将 QueryEngine 和 StreamingEngineInner 中重复的工具调用循环合并为一处。
//! 支持流式/非流式两种输出模式，内部逻辑完全一致。
//!
//! 改进（借鉴 hermes-agent）：
//! - 前置压缩（Preflight）：循环前检查总 token，超阈值提前压缩
//! - IterationBudget：迭代预算退还机制（只读工具可退还）

mod action_checkpoint;
mod approval;
mod closeout_controller;
mod companion_context;
mod patch_recovery;
mod patch_repair_rules;
mod pseudo_tool_text;
mod repair_controller;
mod runtime_diet;
mod runtime_timeouts;
mod session_processor;
mod step_executor;
mod text_sanitizer;
mod tool_execution;
mod tool_execution_controller;
mod tool_metadata;
mod tool_orchestrator;
mod tool_result_controller;
mod turn_recording;
mod validation_runner;
mod workflow_trace;

pub use approval::{ToolApprovalChannel, ToolApprovalRequest};
use closeout_controller::FinalCloseoutContext;
use patch_recovery::PatchSynthesisAction;
use repair_controller::{
    AcceptanceRepairContext, GuidedValidationDebuggingContext, VerificationRepairContext,
};
use runtime_diet::{trace_runtime_diet_report, RuntimeDietSnapshot};
pub(crate) use step_executor::{is_drift_interruption_signal, WorkflowRealStepExecutor};
use text_sanitizer::strip_think_blocks;
#[cfg(test)]
use text_sanitizer::VisibleTextSanitizer;
pub(crate) use tool_execution::{
    safe_prefix_by_bytes, safe_suffix_by_bytes, truncate_tool_result, READ_ONLY_TOOLS,
};
use tool_metadata::attach_tool_execution_metadata;
#[cfg(test)]
use tool_metadata::tool_execution_start_progress;
use tool_result_controller::append_provider_tool_result;
use turn_recording::record_recovery_plan;
#[cfg(test)]
use validation_runner::shell_output_with_timeout;
use validation_runner::{should_run_default_auto_tests, verification_source_context};
use workflow_trace::{
    apply_workflow_feedback_and_trace, trace_adaptive_workflow_trigger, trace_stage_validation,
};

use crate::engine::evidence_ledger::{changed_files_diff_evidence, EvidenceLedger};
use crate::engine::intent_router::{IntentKind, IntentRoute, IntentRouter, WorkflowKind};
use crate::engine::trace::{TraceCollector, TraceEvent, TraceStore, TurnStatus, TurnTrace};
use crate::engine::workflow::{Gate, WorkflowEngine, WorkflowPolicy};
use crate::services::api::{ChatRequest, LlmProvider, Message, ToolCall};
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};

use super::context_compressor::{
    estimate_messages_tokens, estimate_tool_schemas_tokens, ContextCompressor,
};
use super::hooks::ToolHookManager;
use super::streaming::StreamEvent;

fn tool_result_dialog_content(result: &ToolResult) -> String {
    if !result.content.is_empty() {
        result.content.clone()
    } else {
        result.error.clone().unwrap_or_default()
    }
}

fn tool_call_fingerprint(tc: &ToolCall) -> String {
    let args = serde_json::to_string(&tc.arguments).unwrap_or_else(|_| "null".to_string());
    format!("{}|{}", tc.name, args)
}

fn tool_allowed_by_context(allowed_tools: &Option<HashSet<String>>, tool_name: &str) -> bool {
    allowed_tools
        .as_ref()
        .map(|allowed| allowed.contains(tool_name))
        .unwrap_or(true)
}

fn tool_not_allowed_result(tool_call: &ToolCall) -> ToolResult {
    let mut result = ToolResult::error(format!(
        "Tool '{}' is not allowed in this agent context",
        tool_call.name
    ));
    attach_tool_execution_metadata(tool_call, &mut result);
    result
}

async fn build_project_retrieval_context(
    query: &str,
    working_dir: &std::path::Path,
    policy: crate::engine::intent_router::RetrievalPolicy,
) -> Option<crate::engine::retrieval_context::RetrievalContext> {
    if !policy.allows_project_context() {
        return None;
    }
    let root = working_dir.to_path_buf();
    let query = query.to_string();
    tokio::task::spawn_blocking(move || {
        let mut scanner = crate::tools::project_tool::ProjectScanner::new();
        scanner.scan(&root);
        crate::engine::retrieval_context::RetrievalContext::from_project_summary(
            &query,
            scanner.tree_summary(),
            &root,
            policy,
        )
    })
    .await
    .ok()
    .flatten()
}

async fn build_session_retrieval_context(
    query: &str,
    store: Option<Arc<crate::session_store::SessionStore>>,
    policy: crate::engine::intent_router::RetrievalPolicy,
) -> Option<crate::engine::retrieval_context::RetrievalContext> {
    if !policy.allows_memory_context() {
        return None;
    }
    let store = store?;
    let query = fts_phrase_query(query);
    if query.trim().is_empty() {
        return None;
    }
    tokio::task::spawn_blocking(move || {
        store.search_messages(&query, 4).ok().and_then(|messages| {
            crate::engine::retrieval_context::RetrievalContext::from_session_messages(
                &query, &messages, policy,
            )
        })
    })
    .await
    .ok()
    .flatten()
}

fn fts_phrase_query(query: &str) -> String {
    let compact = query
        .chars()
        .filter(|ch| !ch.is_control())
        .take(160)
        .collect::<String>()
        .replace('"', "\"\"");
    if compact.trim().is_empty() {
        String::new()
    } else {
        format!("\"{}\"", compact)
    }
}

fn workflow_contract_enabled(provider: &dyn LlmProvider) -> bool {
    if provider.base_url().starts_with("mock://") {
        return false;
    }

    std::env::var("PRIORITY_AGENT_WORKFLOW_CONTRACT")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "0" | "false" | "off" | "no")
        })
        .unwrap_or(true)
}

fn should_use_nonstreaming_tools(
    provider: &dyn LlmProvider,
    tools: &[crate::services::api::Tool],
) -> bool {
    if tools.is_empty() {
        return false;
    }
    let base_url = provider.base_url().to_ascii_lowercase();
    let model = provider.default_model().to_ascii_lowercase();
    base_url.contains("minimax") || model.contains("minimax")
}

fn persist_workflow_learning_event(
    store: Option<&Arc<crate::session_store::SessionStore>>,
    session_id: &str,
    kind: &str,
    summary: String,
    confidence: f64,
    payload: serde_json::Value,
) {
    let Some(store) = store else {
        return;
    };
    if let Err(e) = store.add_learning_event(
        session_id,
        kind,
        "conversation_loop",
        &summary,
        confidence,
        &payload,
    ) {
        warn!("Failed to persist workflow learning event: {}", e);
    }
}

fn is_high_risk_workflow(
    route: &crate::engine::intent_router::IntentRoute,
    judgment: Option<&crate::engine::workflow_contract::ProgrammingWorkflowJudgment>,
) -> bool {
    matches!(route.risk, crate::engine::intent_router::RiskLevel::High)
        || judgment
            .map(|judgment| matches!(judgment.risk, crate::engine::intent_router::RiskLevel::High))
            .unwrap_or(false)
}

fn is_local_filesystem_inspection_route(route: &IntentRoute) -> bool {
    matches!(route.intent, IntentKind::DirectAnswer)
        && matches!(route.workflow, WorkflowKind::Direct)
        && route
            .recommended_tools
            .iter()
            .any(|tool| matches!(tool.as_str(), "file_read" | "glob"))
        && !route
            .recommended_tools
            .iter()
            .any(|tool| tool.as_str() == "bash")
}

/// 统一对话循环
pub struct ConversationLoop {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
    model: String,
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
    /// 工具授权通道（用于 MCP 等工具的交互式授权）
    approval_channel: Option<Arc<ToolApprovalChannel>>,
    /// 工具白名单（用于子 Agent 隔离；None 表示不限制）
    allowed_tools: Option<HashSet<String>>,
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
            max_iterations: 10,
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
            approval_channel: None,
            allowed_tools: None,
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

    pub fn with_approval_channel(mut self, channel: Arc<ToolApprovalChannel>) -> Self {
        self.approval_channel = Some(channel);
        self
    }

    pub fn with_allowed_tools(mut self, tools: HashSet<String>) -> Self {
        self.allowed_tools = Some(tools);
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
        let mut ctx = ToolContext::new(".", self.session_id.clone());
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

    fn create_tool_context_with_optional_trace(
        &self,
        trace: &Option<TraceCollector>,
    ) -> ToolContext {
        match trace {
            Some(trace) => self.create_tool_context_with_trace(trace),
            None => self.create_tool_context(),
        }
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
        let last_user_preview = messages
            .iter()
            .rposition(|m| matches!(m, Message::User { .. }))
            .and_then(|i| match &messages[i] {
                Message::User { content } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or("")
            .to_string();
        let required_validation_commands =
            Self::extract_required_validation_commands(&last_user_preview);
        let no_diff_audit_closeout_allowed =
            Self::allows_no_diff_audit_closeout(&last_user_preview);
        let turn_index = self
            .trace_store
            .as_ref()
            .and_then(|store| store.latest().map(|trace| trace.turn_index + 1))
            .unwrap_or_else(|| {
                self.turn_counter
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1
            });
        let trace = TraceCollector::new(TurnTrace::new(
            self.session_id.clone(),
            turn_index,
            &last_user_preview,
        ));
        let learning_events = self
            .session_store
            .as_ref()
            .and_then(|store| store.recent_learning_events(&self.session_id, 20).ok())
            .unwrap_or_default();
        let mut route =
            IntentRouter::new().route_with_learning(&last_user_preview, &learning_events);
        self.agent_mode.apply_to_route(&mut route);
        trace.record(TraceEvent::IntentRouted {
            agent_mode: Some(self.agent_mode.label().to_string()),
            intent: format!("{:?}", route.intent),
            workflow: format!("{:?}", route.workflow),
            retrieval: format!("{:?}", route.retrieval),
            confidence: route.confidence,
            risk: format!("{:?}", route.risk),
            reason: route.reason.clone(),
        });
        let resource_policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
        trace.record(TraceEvent::ResourcePolicySelected {
            latency: format!("{:?}", resource_policy.latency),
            target_ms: resource_policy.latency.target_ms(),
            cost_ceiling_usd: resource_policy.cost_ceiling_usd,
            reasoning: format!("{:?}", resource_policy.reasoning),
            parallelism_limit: resource_policy.parallelism_limit,
            max_tool_calls: resource_policy.max_tool_calls,
            context_budget_tokens: resource_policy.context_budget_tokens,
            reason: resource_policy.reason.clone(),
        });
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let destructive_scope =
            crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
                &last_user_preview,
                &working_dir,
            );
        let mut turn_retrieval_context =
            build_project_retrieval_context(&last_user_preview, &working_dir, route.retrieval)
                .await;
        if let Some(session_ctx) = build_session_retrieval_context(
            &last_user_preview,
            self.session_store.clone(),
            route.retrieval,
        )
        .await
        {
            if let Some(ref mut ctx) = turn_retrieval_context {
                ctx.extend(session_ctx);
            } else {
                turn_retrieval_context = Some(session_ctx);
            }
        }
        if route.retrieval.allows_memory_context() {
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                mem.reset_turn();
                if let Some(memory_ctx) = mem
                    .prefetch_retrieval_context_with_llm_rerank(
                        &last_user_preview,
                        self.provider.as_ref(),
                        &self.model,
                        route.retrieval,
                    )
                    .await
                {
                    trace.record(TraceEvent::MemoryPrefetch {
                        chars: memory_ctx
                            .items
                            .iter()
                            .map(|item| item.content_preview.chars().count())
                            .sum(),
                    });
                    if let Some(ref mut ctx) = turn_retrieval_context {
                        ctx.extend(memory_ctx);
                    } else {
                        turn_retrieval_context = Some(memory_ctx);
                    }
                }
            }
        }
        if let Some(ref ctx) = turn_retrieval_context {
            trace.record(TraceEvent::RetrievalContextBuilt {
                policy: format!("{:?}", ctx.policy),
                sources: ctx
                    .items
                    .iter()
                    .map(|item| format!("{:?}", item.source))
                    .collect(),
                items: ctx.items.len(),
                estimated_tokens: ctx.token_estimate,
                provenance: ctx.provenance_summaries(),
                conflicts: ctx.conflict_count(),
            });
        }
        let mut task_bundle = crate::engine::task_context::TaskContextBundle::new(
            &last_user_preview,
            &working_dir,
            route.clone(),
            self.goal_manager
                .as_ref()
                .and_then(|manager| manager.current()),
        );
        if let Some(ref ctx) = turn_retrieval_context {
            task_bundle = task_bundle.with_retrieval(ctx.clone());
        }
        task_bundle.add_constraint(format!(
            "resource_policy={}",
            resource_policy.compact_label()
        ));
        if matches!(
            route.workflow,
            crate::engine::intent_router::WorkflowKind::CodeChange
                | crate::engine::intent_router::WorkflowKind::BugFix
        ) {
            task_bundle.add_risk("code-change tasks require explicit verification");
        }
        let mut code_workflow =
            crate::engine::code_change_workflow::CodeChangeWorkflowRunner::new(&task_bundle);
        let mut evidence_ledger = EvidenceLedger::new();
        if !required_validation_commands.is_empty()
            && code_workflow.activate_trigger(
                crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::RequiredValidation,
            )
        {
            trace_adaptive_workflow_trigger(
                &trace,
                crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::RequiredValidation,
                &code_workflow,
            );
            trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "adaptive workflow trigger activated: required_validation commands={}",
                    required_validation_commands.len()
                ),
            });
        }
        let workflow_contract_prompt =
            crate::engine::workflow_contract::WorkflowContractPrompt::new(
                last_user_preview.as_str(),
                route.clone(),
                working_dir.display().to_string(),
            );
        if code_workflow.should_request_workflow_judgment()
            && workflow_contract_prompt.should_ask_model()
            && workflow_contract_enabled(self.provider.as_ref())
        {
            let analyzer = crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
                self.provider.as_ref(),
                self.model.clone(),
            );
            match analyzer.analyze(workflow_contract_prompt).await {
                Ok(mut judgment) => {
                    let learning_audit =
                        crate::engine::learning_planning::apply_learning_to_workflow_judgment(
                            &mut judgment,
                            &learning_events,
                            turn_retrieval_context.as_ref(),
                        );
                    let context_note = judgment.to_turn_context();
                    trace.record(TraceEvent::WorkflowJudgmentCompleted {
                        task_type: judgment.task_type.clone(),
                        complexity: format!("{:?}", judgment.complexity),
                        risk: format!("{:?}", judgment.risk),
                        plan_steps: judgment.plan.len(),
                        acceptance_checks: judgment.acceptance.criteria.len(),
                        questions: judgment.questions.len(),
                        guided_reasoning: judgment.guided_reasoning_required,
                    });
                    let top_step = judgment.top_plan_step();
                    trace.record(TraceEvent::WorkflowPlanProgress {
                        total_steps: judgment.plan.len(),
                        completed_steps: 0,
                        active_step: top_step.as_ref().map(|step| step.description.clone()),
                        top_priority: top_step.as_ref().map(|step| format!("{:?}", step.priority)),
                        top_importance_score: top_step
                            .as_ref()
                            .map(|step| step.normalized_weight()),
                        top_weight_share: top_step
                            .as_ref()
                            .map(|step| step.computed_weight_share()),
                        weight_source: top_step
                            .as_ref()
                            .and_then(|step| step.weight_source())
                            .map(|source| format!("{:?}", source)),
                        reweighted: learning_audit.applied,
                    });
                    if learning_audit.applied {
                        trace.record(TraceEvent::WorkflowLearningAdjusted {
                            adjustments: learning_audit.adjustments.len(),
                            before_top_step: learning_audit.before_top_step.clone(),
                            after_top_step: learning_audit.after_top_step.clone(),
                            reason: learning_audit.explanation.clone(),
                        });
                        persist_workflow_learning_event(
                            self.session_store.as_ref(),
                            &self.session_id,
                            "planning_adjustment",
                            format!(
                                "Learning adjusted workflow plan with {} change(s)",
                                learning_audit.adjustments.len()
                            ),
                            0.85,
                            serde_json::to_value(&learning_audit)
                                .unwrap_or_else(|_| serde_json::json!({})),
                        );
                    }
                    persist_workflow_learning_event(
                        self.session_store.as_ref(),
                        &self.session_id,
                        "workflow_judgment",
                        format!(
                            "Workflow judgment task_type={} risk={:?} questions={} guided={}",
                            judgment.task_type,
                            judgment.risk,
                            judgment.questions.len(),
                            judgment.guided_reasoning_required
                        ),
                        0.8,
                        serde_json::json!({
                            "task_type": judgment.task_type.clone(),
                            "complexity": format!("{:?}", judgment.complexity),
                            "risk": format!("{:?}", judgment.risk),
                            "requirement_complete_enough": judgment.requirement_complete_enough,
                            "needs_user_questions": judgment.needs_user_questions,
                            "question_reason": judgment.question_reason.clone(),
                            "questions": judgment.questions.clone(),
                            "assumptions": judgment.assumptions.clone(),
                            "guided_reasoning_required": judgment.guided_reasoning_required,
                            "guided_reasoning_triggers": judgment.guided_reasoning_triggers.iter().map(|trigger| format!("{:?}", trigger)).collect::<Vec<_>>(),
                            "plan_steps": judgment.plan.len(),
                            "weighted_plan": judgment.weighted_plan_summary(),
                            "acceptance_checks": judgment.acceptance.criteria.len(),
                        }),
                    );
                    task_bundle.apply_workflow_judgment(judgment);
                    code_workflow.refresh_policy(&task_bundle);
                    let insert_at = messages
                        .iter()
                        .take_while(|message| matches!(message, Message::System { .. }))
                        .count();
                    messages.insert(insert_at, Message::system(context_note));
                }
                Err(err) => {
                    if crate::engine::workflow_contract::is_recoverable_workflow_judgment_parse_error(&err) {
                        debug!(
                            "Workflow judgment skipped after non-JSON model response: {}",
                            err
                        );
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "workflow judgment skipped after non-JSON model response"
                                .to_string(),
                        });
                    } else {
                        warn!("Workflow judgment analysis failed: {}", err);
                        trace.record(TraceEvent::WorkflowFallback {
                            error: format!("workflow judgment analysis failed: {}", err),
                        });
                    }
                }
            }
        }
        trace.record(TraceEvent::TaskContextBuilt {
            task_id: task_bundle.task_id.clone(),
            workflow: format!("{:?}", task_bundle.route.workflow),
            files: task_bundle.relevant_files.len(),
            constraints: task_bundle.constraints.len(),
            risks: task_bundle.risks.len(),
            acceptance_checks: task_bundle.acceptance_checks.len(),
        });
        if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
            trace.record(TraceEvent::ImplementationIntentRecorded {
                task_id: task_bundle.task_id.clone(),
                workflow: format!("{:?}", task_bundle.route.workflow),
                target_files: task_bundle.relevant_files.len(),
                validation_commands: required_validation_commands.clone(),
                risks: task_bundle.risks.len(),
                reason: "code-change workflow must identify target scope and validation before first edit".to_string(),
            });
        }
        let reflection_pass =
            crate::engine::reflection_pass::ReflectionPass::from_task_bundle(&task_bundle);
        trace.record(TraceEvent::ReflectionPassCompleted {
            pass_id: reflection_pass.pass_id.clone(),
            task_id: reflection_pass.task_id.clone(),
            status: format!("{:?}", reflection_pass.status),
            findings: reflection_pass.findings.len(),
            unresolved: reflection_pass.unresolved_count(),
        });
        if reflection_pass.status == crate::engine::reflection_pass::ReflectionStatus::NeedsWork
            && code_workflow.should_block_on_reflection()
        {
            let review_prompt = format!(
                "Reflection pass '{}' found {} unresolved issue(s) before executing a {:?} workflow. Allow the turn to continue?",
                reflection_pass.pass_id,
                reflection_pass.unresolved_count(),
                route.workflow
            );
            let review_call = ToolCall {
                id: format!(
                    "reflection-{}",
                    &reflection_pass.pass_id[..8.min(reflection_pass.pass_id.len())]
                ),
                name: "reflection_review".to_string(),
                arguments: serde_json::json!({
                    "task_id": reflection_pass.task_id.clone(),
                    "pass_id": reflection_pass.pass_id.clone(),
                    "status": format!("{:?}", reflection_pass.status),
                    "unresolved": reflection_pass.unresolved_count(),
                    "workflow": format!("{:?}", route.workflow),
                }),
            };
            let mut approved = false;
            if let (Some(channel), Some(tx)) = (&self.approval_channel, tx) {
                let _ = tx
                    .send(StreamEvent::PermissionRequest {
                        id: review_call.id.clone(),
                        tool_name: review_call.name.clone(),
                        arguments: review_call.arguments.clone(),
                        prompt: review_prompt.clone(),
                    })
                    .await;
                trace.record(TraceEvent::PermissionRequested {
                    tool: review_call.name.clone(),
                    call_id: review_call.id.clone(),
                    prompt: review_prompt.clone(),
                });
                match channel
                    .submit(ToolApprovalRequest {
                        tool_call: review_call.clone(),
                        prompt: review_prompt.clone(),
                        review: Some(
                            crate::engine::human_review::HumanReviewRequest::reflection_gate(
                                reflection_pass.pass_id.clone(),
                                reflection_pass.unresolved_count(),
                                format!("{:?}", route.workflow),
                            ),
                        ),
                    })
                    .await
                {
                    Ok(is_approved) => approved = is_approved,
                    Err(e) => warn!("Reflection approval error: {}", e),
                }
                trace.record(TraceEvent::PermissionResolved {
                    tool: review_call.name,
                    call_id: review_call.id,
                    approved,
                });
            } else {
                approved = true;
            }
            if !approved {
                let content = "Stopped before code-change execution because reflection found unresolved acceptance gaps.".to_string();
                trace.record(TraceEvent::AssistantResponded {
                    chars: content.chars().count(),
                    iterations: 0,
                });
                self.finish_trace(trace.clone(), TurnStatus::Failed);
                return Ok(LoopResult {
                    content,
                    tool_calls: Vec::new(),
                    tool_calls_made: false,
                    iterations: 0,
                    pre_executed_results: std::collections::HashMap::new(),
                });
            }
        }
        if let Some(manager) = &self.goal_manager {
            if let Some(goal) = manager.update_from_user_message(&last_user_preview, Some(&route)) {
                trace.record(TraceEvent::SessionGoalUpdated {
                    goal_id: goal.id,
                    title: goal.title,
                    status: format!("{:?}", goal.status),
                    reason: "user turn routed to trackable workflow".to_string(),
                });
            }
        }

        // ── Workflow 闸门检查 ──────────────────────────
        let already_triggered = self
            .workflow_triggered_this_turn
            .swap(true, std::sync::atomic::Ordering::SeqCst);
        if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
            trace.record(TraceEvent::WorkflowRouted {
                decision: "direct".to_string(),
                reason:
                    "code-change contract uses the tool loop; legacy workflow step executor skipped"
                        .to_string(),
            });
        } else if !already_triggered {
            if let Some(last_user_msg) = messages
                .iter()
                .rposition(|m| matches!(m, Message::User { .. }))
                .and_then(|i| match &messages[i] {
                    Message::User { content } => Some(content.as_str()),
                    _ => None,
                })
            {
                let workflow_policy = self.workflow_policy.clone();
                let gate = Gate::new().with_policy(workflow_policy.gate.clone());
                if is_drift_interruption_signal(last_user_msg) {
                    crate::engine::workflow::metrics::record_drift_interruption();
                }
                let decision = if workflow_policy.gate.llm_classifier_enabled {
                    gate.decide_with_llm(last_user_msg, self.provider.as_ref(), &self.model)
                        .await
                } else {
                    gate.decide(last_user_msg)
                };
                trace.record(TraceEvent::WorkflowRouted {
                    decision: if decision.is_workflow() {
                        "workflow".to_string()
                    } else {
                        "direct".to_string()
                    },
                    reason: decision.reason().to_string(),
                });
                if decision.is_workflow() {
                    crate::engine::workflow::metrics::record_workflow_run();
                    if let Some(ref mem_mgr) = self.memory_manager {
                        let mut mem = mem_mgr.lock().await;
                        mem.save_workflow_decision(
                            "gate",
                            last_user_msg,
                            "Workflow",
                            decision.reason(),
                        );
                    }
                    debug!("Workflow mode activated: {}", decision.reason());
                    let workflow_executor = WorkflowRealStepExecutor {
                        tool_registry: self.tool_registry.clone(),
                        llm_provider: self.provider.clone(),
                        model: self.model.clone(),
                        base_context: self.create_tool_context_with_trace(&trace),
                    };
                    let workflow_engine =
                        WorkflowEngine::new(self.provider.clone()).with_policy(workflow_policy);
                    match workflow_engine
                        .run(last_user_msg, last_user_msg, &workflow_executor)
                        .await
                    {
                        Ok(result) => {
                            trace.record(TraceEvent::WorkflowCompleted {
                                steps: result.plan.steps.len(),
                            });
                            let workflow_report = strip_think_blocks(&result.final_report);
                            if let Some(ref mem_mgr) = self.memory_manager {
                                let mut mem = mem_mgr.lock().await;
                                mem.save_workflow_decision(
                                    "execution",
                                    last_user_msg,
                                    "Success",
                                    &format!(
                                        "workflow completed with {} steps",
                                        result.plan.steps.len()
                                    ),
                                );
                            }
                            if let Some(tx) = tx {
                                if !workflow_report.trim().is_empty() {
                                    let _ = tx
                                        .send(StreamEvent::TextChunk(workflow_report.clone()))
                                        .await;
                                }
                                let _ = tx.send(StreamEvent::Complete).await;
                            }
                            trace.record(TraceEvent::AssistantResponded {
                                chars: workflow_report.chars().count(),
                                iterations: 0,
                            });
                            self.finish_trace(trace.clone(), TurnStatus::Completed);
                            return Ok(LoopResult {
                                content: workflow_report,
                                tool_calls: Vec::new(),
                                tool_calls_made: false,
                                iterations: 0,
                                pre_executed_results: std::collections::HashMap::new(),
                            });
                        }
                        Err(e) => {
                            trace.record(TraceEvent::WorkflowFallback { error: e.clone() });
                            if let Some(ref mem_mgr) = self.memory_manager {
                                let mut mem = mem_mgr.lock().await;
                                mem.save_workflow_decision(
                                    "fallback",
                                    last_user_msg,
                                    "DirectMode",
                                    &e,
                                );
                            }
                            warn!(
                                "Workflow execution failed: {}, falling back to direct mode",
                                e
                            );
                        }
                    }
                }
            }
        }

        let base_tools = self.get_tools_for_route(&route);
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut tool_calls_made = false;
        let mut iterations_used = 0;
        let mut no_code_progress_rounds = 0usize;
        let mut action_checkpoint_active = false;
        let mut action_checkpoint_lookup_count = 0usize;
        let mut patch_synthesis_recovery_used = false;
        let mut action_checkpoint_reopen_used = false;
        let mut no_diff_audit_validation_checkpoint_sent = false;
        let mut file_edit_failure_retry_used = false;
        let mut pseudo_tool_retry_used = false;
        let mut filesystem_grounding_retry_used = false;
        let mut companion_context_keys: HashSet<String> = HashSet::new();
        let mut failed_tool_fingerprints: HashMap<String, usize> = HashMap::new();
        let mut failed_tool_names: HashMap<String, usize> = HashMap::new();
        let mut successful_required_validation_commands: HashSet<String> = HashSet::new();
        let mut runtime_diet = RuntimeDietSnapshot::new(Self::route_scoped_tools_enabled());
        if let Some(ref ctx) = turn_retrieval_context {
            runtime_diet.observe_retrieval_context(ctx);
        }
        if base_tools.iter().any(|tool| tool.name == "skills_list") {
            let skill_summary =
                crate::skills::SkillRuntime::load(&working_dir).discovery_summary("", 30);
            runtime_diet.observe_skill_list_summary(&skill_summary);
        }

        // ── 记忆围栏注入：先注入，再让 preflight 统计真实请求大小 ──
        if route.retrieval.allows_memory_context() {
            if let Some(ref mem_mutex) = self.memory_manager {
                let mem = mem_mutex.lock().await;
                let snapshot = mem.get_snapshot();
                if !snapshot.is_empty()
                    && !messages.iter().any(|m| {
                        matches!(m, Message::System { content } if content.contains("<memory-context>"))
                    })
                {
                    runtime_diet.observe_memory_snapshot(&snapshot);
                    trace.record(TraceEvent::MemorySnapshotInjected {
                        chars: snapshot.chars().count(),
                    });
                    let insert_pos = messages
                        .iter()
                        .position(|m| !matches!(m, Message::System { .. }))
                        .unwrap_or(messages.len());
                    messages.insert(insert_pos, Message::system(&snapshot));
                    debug!("Injected memory context fence at position {}", insert_pos);
                }
            }
        }

        // ── 前置压缩（Preflight）─────────────────────────
        if let Some(ref compressor_mutex) = self.compressor {
            let mut no_gain_passes = 0u8;
            for pass in 0..3 {
                let compressor = compressor_mutex.lock().await;
                let tool_tokens = estimate_tool_schemas_tokens(&base_tools);
                let msg_tokens = estimate_messages_tokens(&messages);
                // `messages` already includes the system prompt at this point,
                // so only add tool schema tokens as external request overhead.
                if !compressor.preflight_check(&messages, 0, tool_tokens) {
                    break;
                }
                debug!(
                    "Preflight compression pass {}/3 ({} msg + {} tool tokens)",
                    pass + 1,
                    msg_tokens,
                    tool_tokens
                );
                drop(compressor);
                let before_tokens = estimate_messages_tokens(&messages);
                messages = compressor_mutex
                    .lock()
                    .await
                    .compress_async(&messages)
                    .await;
                let after_tokens = estimate_messages_tokens(&messages);
                trace.record(TraceEvent::ContextCompacted {
                    before_tokens: before_tokens as usize,
                    after_tokens: after_tokens as usize,
                    strategy: "preflight".to_string(),
                });
                if after_tokens >= before_tokens {
                    no_gain_passes += 1;
                    if no_gain_passes >= 2 {
                        warn!(
                            "Preflight compression made no progress for 2 consecutive passes ({} -> {}). Stop retrying this turn.",
                            before_tokens, after_tokens
                        );
                        break;
                    }
                } else {
                    no_gain_passes = 0;
                }
            }
        }

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Start).await;
        }

        if let Some(ref ctx) = turn_retrieval_context {
            let block = ctx.format_for_prompt();
            if !block.is_empty()
                && !messages.iter().any(|m| {
                    matches!(m, Message::System { content } if content.contains("project.index:"))
                })
            {
                messages.push(Message::system(block));
            }
        }

        // ── 迭代预算 ─────────────────────────────────────
        let mut effective_iterations: usize = 0;
        let mut acceptance_repair_attempts: usize = 0;
        let mut reserved_repair_rounds: usize = 0;
        let max_loop_iterations = self.max_iterations + code_workflow.max_repair_attempts().max(3);
        let baseline_git_status_files = Self::git_status_files();
        let mut action_checkpoint_no_change_rounds = 0usize;
        let mut action_checkpoint_requires_patch_before_validation = false;

        for iteration in 0..max_loop_iterations {
            debug!(
                "Conversation loop iteration {} (effective: {}/{})",
                iteration, effective_iterations, self.max_iterations
            );
            iterations_used = iteration + 1;

            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                mem.reset_turn();
            }

            if effective_iterations >= self.max_iterations {
                if reserved_repair_rounds > 0 {
                    reserved_repair_rounds -= 1;
                    trace.record(TraceEvent::WorkflowFallback {
                        error: format!(
                            "using reserved repair round after validation failure (remaining={})",
                            reserved_repair_rounds
                        ),
                    });
                } else {
                    warn!(
                        "Effective iteration budget exhausted ({}/{})",
                        effective_iterations, self.max_iterations
                    );
                    break;
                }
            }

            let has_changes_before_request =
                crate::engine::code_change_workflow::is_programming_workflow(route.workflow)
                    && !Self::git_status_files_since(&baseline_git_status_files).is_empty();
            let validation_allowed_before_request =
                has_changes_before_request && !action_checkpoint_requires_patch_before_validation;
            let tools = if action_checkpoint_active {
                let action_tools = Self::code_action_tools(
                    &base_tools,
                    validation_allowed_before_request,
                    action_checkpoint_lookup_count < Self::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET,
                );
                if action_tools.is_empty() {
                    base_tools.clone()
                } else {
                    action_tools
                }
            } else {
                base_tools.clone()
            };
            let exposed_tool_names = tools
                .iter()
                .map(|tool| tool.name.clone())
                .collect::<HashSet<_>>();

            let mut request_messages = messages.clone();
            if action_checkpoint_active {
                let mut exposed_names = exposed_tool_names.iter().cloned().collect::<Vec<_>>();
                exposed_names.sort();
                request_messages.push(Message::system(Self::focused_repair_mode_prompt(
                    &exposed_names,
                    action_checkpoint_lookup_count,
                )));
            }
            let memory_already_in_turn_context = turn_retrieval_context
                .as_ref()
                .map(|ctx| {
                    ctx.item_count_by_source(
                        crate::engine::retrieval_context::RetrievalSource::Memory,
                    ) > 0
                })
                .unwrap_or(false);
            if !memory_already_in_turn_context && route.retrieval.allows_memory_context() {
                if let Some(ref mem_mutex) = self.memory_manager {
                    let mut mem = mem_mutex.lock().await;
                    if let Some(last_user_idx) = request_messages
                        .iter()
                        .rposition(|m| matches!(m, Message::User { .. }))
                    {
                        if let Message::User { content } = &request_messages[last_user_idx] {
                            let retrieval_context = mem
                                .prefetch_retrieval_context_with_llm_rerank(
                                    content,
                                    self.provider.as_ref(),
                                    &self.model,
                                    route.retrieval,
                                )
                                .await;
                            if let Some(ref ctx) = retrieval_context {
                                runtime_diet.observe_retrieval_context(ctx);
                                trace.record(TraceEvent::MemoryPrefetch {
                                    chars: ctx
                                        .items
                                        .iter()
                                        .map(|item| item.content_preview.chars().count())
                                        .sum(),
                                });
                                trace.record(TraceEvent::RetrievalContextBuilt {
                                    policy: format!("{:?}", ctx.policy),
                                    sources: ctx
                                        .items
                                        .iter()
                                        .map(|item| format!("{:?}", item.source))
                                        .collect(),
                                    items: ctx.items.len(),
                                    estimated_tokens: ctx.token_estimate,
                                    provenance: ctx.provenance_summaries(),
                                    conflicts: ctx.conflict_count(),
                                });
                                let retrieval_block = ctx.format_for_prompt();
                                let enhanced = format!("{}\n{}", content, retrieval_block);
                                request_messages[last_user_idx] = Message::user(&enhanced);
                                debug!("Prefetched memory context injected into user message");
                            }
                        }
                    }
                }
            }

            runtime_diet.prompt_tokens = runtime_diet
                .prompt_tokens
                .max(estimate_messages_tokens(&request_messages));
            runtime_diet.tool_schema_tokens = runtime_diet
                .tool_schema_tokens
                .max(estimate_tool_schemas_tokens(&tools));
            runtime_diet.exposed_tools = runtime_diet.exposed_tools.max(tools.len());

            let mut request = ChatRequest::new(&self.model)
                .with_messages(request_messages)
                .with_tools(tools.clone())
                .with_temperature(0.2);

            // ── 响应式压缩循环 ─────────────────────────────
            let mut compressed_this_turn = false;
            let mut api_result: Result<(
                String,
                Vec<ToolCall>,
                std::collections::HashMap<usize, ToolResult>,
            )> = Err(anyhow::anyhow!("initial"));
            for compress_retry in 0..3 {
                trace.record(TraceEvent::ApiRequestStarted {
                    iteration: iteration + 1,
                    model: self.model.clone(),
                    tools: tools.len(),
                });
                let nonstreaming_tool_request =
                    tx.is_some() && should_use_nonstreaming_tools(self.provider.as_ref(), &tools);
                api_result = if let Some(tx) = tx {
                    if nonstreaming_tool_request {
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "provider stream is incompatible with tool/usage chunks; using non-streaming tool request".to_string(),
                        });
                        self.call_api(request.clone()).await
                    } else {
                        self.call_api_streaming(request.clone(), tx, &trace, &exposed_tool_names)
                            .await
                    }
                } else {
                    self.call_api(request.clone()).await
                };

                match &api_result {
                    Ok(_) => break,
                    Err(e) => {
                        let err_str = e.to_string().to_lowercase();
                        let needs_compress = err_str.contains("payload too large")
                            || err_str.contains("413")
                            || err_str.contains("context")
                            || err_str.contains("too many tokens")
                            || err_str.contains("maximum context length");
                        if needs_compress && compress_retry < 2 {
                            let classified =
                                crate::engine::error_classifier::ErrorClassifier::from_anyhow(e);
                            let plan = crate::engine::recovery_plan::RecoveryPlan::from_classified(
                                "api_reactive_compress",
                                &classified,
                            )
                            .with_status(crate::engine::recovery_plan::RecoveryStatus::Applied);
                            record_recovery_plan(&trace, &plan);
                            warn!(
                                "API error (attempt {}/3): {}. Compressing context and retrying...",
                                compress_retry + 1,
                                e
                            );
                            if let Some(ref comp) = self.compressor {
                                let msgs_for_comp = if compress_retry == 0 {
                                    messages.clone()
                                } else {
                                    let mut comp = comp.lock().await;
                                    comp.micro_compress(&messages)
                                };
                                let compressed =
                                    comp.lock().await.compress_async(&msgs_for_comp).await;
                                trace.record(TraceEvent::ContextCompacted {
                                    before_tokens: estimate_messages_tokens(&msgs_for_comp)
                                        as usize,
                                    after_tokens: estimate_messages_tokens(&compressed) as usize,
                                    strategy: "reactive".to_string(),
                                });
                                request = ChatRequest::new(&self.model)
                                    .with_messages(compressed)
                                    .with_tools(tools.clone())
                                    .with_temperature(0.2);
                                compressed_this_turn = true;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }

            let (content, tool_calls, pre_executed) = match api_result {
                Ok(value) => value,
                Err(e) => {
                    trace.record(TraceEvent::Error {
                        message: e.to_string(),
                    });
                    runtime_diet.validation_evidence = "api_error".to_string();
                    trace_runtime_diet_report(&trace, &route, &code_workflow, &runtime_diet);
                    self.finish_trace(trace.clone(), TurnStatus::Failed);
                    return Err(e);
                }
            };
            trace.record(TraceEvent::ApiRequestCompleted {
                iteration: iteration + 1,
                tool_calls: tool_calls.len(),
                content_chars: content.chars().count(),
            });

            if compressed_this_turn {
                debug!("Context compressed due to size limits");
            }

            final_content = content.clone();
            final_tool_calls = tool_calls.clone();
            if !tool_calls.is_empty() {
                tool_calls_made = true;
            }

            if tool_calls.is_empty() {
                let needs_bash_tool_retry = pseudo_tool_text::contains_unexecuted_tool_command(
                    &content,
                    &exposed_tool_names,
                )
                    || pseudo_tool_text::contains_false_bash_unavailable_claim(
                        &content,
                        &exposed_tool_names,
                    );
                let needs_filesystem_tool_retry = !tool_calls_made
                    && is_local_filesystem_inspection_route(&route)
                    && pseudo_tool_text::contains_local_filesystem_claim_without_tool(
                        &content,
                        &exposed_tool_names,
                    );
                let filesystem_grounding_gaps = if is_local_filesystem_inspection_route(&route) {
                    evidence_ledger.unsupported_filesystem_claims(&content)
                } else {
                    Vec::new()
                };
                let needs_filesystem_grounding_retry = !filesystem_grounding_gaps.is_empty();
                if (!pseudo_tool_retry_used
                    && (needs_bash_tool_retry || needs_filesystem_tool_retry))
                    || (!filesystem_grounding_retry_used && needs_filesystem_grounding_retry)
                {
                    if needs_filesystem_grounding_retry {
                        filesystem_grounding_retry_used = true;
                    } else {
                        pseudo_tool_retry_used = true;
                    }
                    let fallback_error = if needs_filesystem_grounding_retry {
                        format!(
                            "assistant included unsupported filesystem claim(s): {}; retrying with evidence-grounded correction",
                            filesystem_grounding_gaps.join(", ")
                        )
                    } else if needs_filesystem_tool_retry {
                        "assistant answered local filesystem state without a tool; retrying with explicit filesystem tool-use correction"
                            .to_string()
                    } else {
                        "assistant emitted an unexecuted or false-unavailable shell response; retrying with explicit bash tool-use correction"
                            .to_string()
                    };
                    trace.record(TraceEvent::WorkflowFallback {
                        error: fallback_error,
                    });
                    messages.push(Message::assistant(safe_prefix_by_bytes(&content, 1200)));
                    let correction = if needs_filesystem_grounding_retry {
                        "Your previous answer added filesystem metadata that was not explicitly supported by tool output. \
Re-answer from the evidence already gathered. Do not state size, item count, creation time, or exact contents unless the tool output directly contains that fact. \
If the user did not ask for those metadata fields, omit them."
                    } else if needs_filesystem_tool_retry {
                        "file_read and glob are currently exposed to you as callable tools. \
The user asked for current local filesystem state, so do not answer from memory or inference. \
Inspect the requested path with file_read or glob now, then answer only from that tool output. \
Do not invent size, item count, creation time, or contents that are not present in tool output."
                    } else {
                        "Bash is currently exposed to you as a callable tool. \
The user asked for current local/runtime state, so do not answer from an unexecuted command and do not claim bash is unavailable. \
If a command appears in a code block or your answer asks the user to run a shell command manually, execute it with the bash tool now. \
Only report a tool as unavailable when it is not exposed in the current tool list."
                    };
                    messages.push(Message::system(correction));
                    continue;
                }
                if let Some(tx) = tx {
                    if should_use_nonstreaming_tools(self.provider.as_ref(), &tools)
                        && !content.is_empty()
                    {
                        let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                    }
                }
                break;
            }

            messages.push(Message::assistant_with_tools(&content, tool_calls.clone()));

            let has_changes_before_tools =
                crate::engine::code_change_workflow::is_programming_workflow(route.workflow)
                    && !Self::git_status_files_since(&baseline_git_status_files).is_empty();
            let mut results = self
                .execute_tools_parallel(
                    &tool_calls,
                    tx,
                    pre_executed,
                    Some(trace.clone()),
                    &resource_policy,
                    &exposed_tool_names,
                    action_checkpoint_active,
                    action_checkpoint_lookup_count,
                    has_changes_before_tools,
                    &destructive_scope,
                )
                .await;

            // ── 迭代预算退还 ──────────────────────────────
            let all_read_only = tool_calls
                .iter()
                .all(|tc| READ_ONLY_TOOLS.iter().any(|&name| tc.name == name));

            if all_read_only {
                debug!("All tools read-only, refunding iteration budget");
            } else {
                effective_iterations += 1;
            }

            let mut tool_results_text = String::new();
            let mut changed_files = Vec::new();
            let used_write_tool = tool_calls
                .iter()
                .any(|tc| Self::is_code_write_tool_name(&tc.name));
            let mut successful_write_tool = false;
            let used_action_checkpoint_lookup = action_checkpoint_active
                && tool_calls
                    .iter()
                    .any(|tc| matches!(tc.name.as_str(), "file_read" | "grep"));
            let mut any_tool_success = false;
            let mut repeated_failed_tools = Vec::new();
            let mut failed_tool_names_this_round = Vec::new();
            let mut failed_tool_evidence = Vec::new();
            let mut file_edit_failure_correction_added = false;
            let mut successful_validation_commands = Vec::new();
            let mut should_closeout_after_verified_change = false;
            if used_write_tool && !required_validation_commands.is_empty() {
                successful_required_validation_commands.clear();
            }
            for (tc, result) in results.iter_mut() {
                truncate_tool_result(result, &tc.name, &tc.id).await;
                append_provider_tool_result(
                    tc,
                    result,
                    &mut evidence_ledger,
                    &mut tool_results_text,
                    &mut messages,
                );

                if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
                    if let Some(note) = companion_context::companion_context_note(
                        &working_dir,
                        &last_user_preview,
                        tc,
                        result,
                    ) {
                        if companion_context_keys.insert(note.key) {
                            tool_results_text.push('\n');
                            tool_results_text.push_str(&note.text);
                            tool_results_text.push('\n');
                            messages.push(Message::system(note.text));
                        }
                    }
                }

                let fp = tool_call_fingerprint(tc);
                if result.success {
                    any_tool_success = true;
                    failed_tool_fingerprints.remove(&fp);
                    failed_tool_names.remove(&tc.name);
                } else {
                    let count = failed_tool_fingerprints.entry(fp).or_insert(0);
                    *count += 1;
                    if *count >= 2 {
                        repeated_failed_tools.push(tc.name.clone());
                    }
                    let name_count = failed_tool_names.entry(tc.name.clone()).or_insert(0);
                    *name_count += 1;
                    failed_tool_names_this_round.push(tc.name.clone());
                    failed_tool_evidence.push(format!(
                        "{} {} failed:\n{}",
                        tc.name,
                        tc.id,
                        tool_result_dialog_content(result)
                    ));
                }

                if result.success && (tc.name == "file_edit" || tc.name == "file_write") {
                    successful_write_tool = true;
                    action_checkpoint_requires_patch_before_validation = false;
                    if let Some(path) = tc.arguments["path"].as_str() {
                        changed_files.push(std::path::PathBuf::from(path));
                    }
                }
                if result.success && Self::is_validation_tool_call(tc) {
                    if let Some(command) = tc.arguments["command"].as_str() {
                        let command = command.trim().to_string();
                        let normalized_command =
                            Self::normalize_validation_command_for_match(&command);
                        if required_validation_commands.iter().any(|required| {
                            Self::normalize_validation_command_for_match(required)
                                == normalized_command
                        }) {
                            successful_required_validation_commands.insert(command.clone());
                        }
                        successful_validation_commands.push(command);
                    }
                }
            }
            if let Some(guard) = destructive_scope.completion_guard_for_results(
                results.iter().map(|(tc, result)| (tc, result.success)),
                &working_dir,
            ) {
                trace.record(TraceEvent::DestructiveScopeChecked {
                    tool: "assistant_response".to_string(),
                    call_id: "post_action_guard".to_string(),
                    operation: "post_action_guard".to_string(),
                    target: None,
                    allowed: false,
                    reason: guard.clone(),
                });
                messages.push(Message::system(guard.clone()));
                tool_results_text.push('\n');
                tool_results_text.push_str(&guard);
                tool_results_text.push('\n');
            }
            if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
                if let Some(correction) =
                    Self::file_edit_failure_repair_correction(&failed_tool_evidence)
                {
                    trace.record(TraceEvent::WorkflowFallback {
                        error: "file_edit failure converted to line-range repair correction"
                            .to_string(),
                    });
                    file_edit_failure_correction_added = true;
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&correction);
                    messages.push(Message::system(correction));
                }
            }
            if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
                for path in Self::git_status_files_since(&baseline_git_status_files) {
                    if !changed_files.iter().any(|existing| existing == &path) {
                        changed_files.push(path);
                    }
                }
            }
            let has_worktree_changes = !changed_files.is_empty();

            if Self::should_retry_after_file_edit_failure_correction(
                action_checkpoint_active,
                file_edit_failure_correction_added,
                file_edit_failure_retry_used,
                successful_write_tool,
            ) {
                file_edit_failure_retry_used = true;
                action_checkpoint_no_change_rounds = 0;
                trace.record(TraceEvent::WorkflowFallback {
                    error: "file_edit repair correction returned to model before patch synthesis"
                        .to_string(),
                });
                continue;
            }

            let mut force_patch_synthesis_after_no_change = false;
            let mut force_patch_synthesis_reason: Option<&'static str> = None;
            if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
                let mut activated_checkpoint_this_round = false;
                if successful_write_tool {
                    no_code_progress_rounds = 0;
                    action_checkpoint_no_change_rounds = 0;
                    action_checkpoint_active = false;
                    action_checkpoint_lookup_count = 0;
                    file_edit_failure_retry_used = false;
                } else if used_write_tool {
                    action_checkpoint_requires_patch_before_validation = true;
                } else if any_tool_success && !used_write_tool {
                    if (no_diff_audit_closeout_allowed || has_worktree_changes)
                        && !successful_validation_commands.is_empty()
                    {
                        no_code_progress_rounds = 0;
                        action_checkpoint_active = false;
                        action_checkpoint_no_change_rounds = 0;
                        action_checkpoint_lookup_count = 0;
                    } else {
                        no_code_progress_rounds += 1;
                    }
                    if no_diff_audit_closeout_allowed && !has_worktree_changes {
                        if no_code_progress_rounds >= 2
                            && !action_checkpoint_active
                            && !no_diff_audit_validation_checkpoint_sent
                        {
                            let checkpoint = "Audit/regression checkpoint: this task allows a no-diff closeout when the requested behavior is already present. Do not force an arbitrary edit. Run the required validation commands now; if they pass, provide a Closeout with direct evidence and changed files as none. If a concrete missing behavior is proven, then make the smallest focused edit."
                                .to_string();
                            trace.record(TraceEvent::WorkflowFallback {
                                error: "audit/regression task should validate before forcing edits"
                                    .to_string(),
                            });
                            messages.push(Message::system(checkpoint.clone()));
                            tool_results_text.push('\n');
                            tool_results_text.push_str(&checkpoint);
                            no_diff_audit_validation_checkpoint_sent = true;
                            no_code_progress_rounds = 0;
                        }
                    } else if has_worktree_changes
                        && successful_validation_commands.is_empty()
                        && no_code_progress_rounds >= 2
                        && !action_checkpoint_active
                    {
                        if code_workflow.activate_trigger(
                            crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::RepeatedNoCodeProgress,
                        ) {
                            trace_adaptive_workflow_trigger(
                                &trace,
                                crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::RepeatedNoCodeProgress,
                                &code_workflow,
                            );
                            trace.record(TraceEvent::WorkflowFallback {
                                error:
                                    "adaptive workflow trigger activated: repeated_no_code_progress"
                                        .to_string(),
                            });
                        }
                        let checkpoint = format!(
                            "Workflow acceptance repair checkpoint: this {:?} task already has code changes, but {} consecutive successful tool rounds made no additional edit. Use the evidence already gathered to synthesize the smallest remaining file_edit/file_write patch now. If multiple independent acceptance-critical bypasses are visible, fix them together; otherwise stop with a Closeout status of not_verified and name the blocker.",
                            route.workflow, no_code_progress_rounds
                        );
                        trace.record(TraceEvent::WorkflowFallback {
                            error:
                                "existing diff still needs repair; entering patch synthesis after repeated read-only rounds"
                                    .to_string(),
                        });
                        messages.push(Message::system(checkpoint.clone()));
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&checkpoint);
                        action_checkpoint_active = true;
                        action_checkpoint_lookup_count =
                            Self::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET;
                        file_edit_failure_retry_used = false;
                        action_checkpoint_no_change_rounds = 2;
                        force_patch_synthesis_after_no_change = true;
                        force_patch_synthesis_reason = Some(
                            "existing diff still needs repair after repeated read-only rounds",
                        );
                        activated_checkpoint_this_round = true;
                    } else if no_code_progress_rounds == 2 && !action_checkpoint_active {
                        let lookup_rule = Self::targeted_lookup_budget_rule(0);
                        let checkpoint = format!(
                            "Workflow progress checkpoint: this is a {:?} task and {} consecutive successful tool rounds produced no code change. Keep investigation focused: on the next response either make the smallest safe file_edit/file_write patch, or use the focused lookup budget if a required symbol, test, or call site is still missing. {} If a scorer/decision object already returns final status, use that status directly instead of reimplementing acceptance gates.",
                            route.workflow, no_code_progress_rounds, lookup_rule
                        );
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "code-change task needs an edit after repeated inspection"
                                .to_string(),
                        });
                        messages.push(Message::system(checkpoint.clone()));
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&checkpoint);
                    } else if no_code_progress_rounds >= 3 && !action_checkpoint_active {
                        if code_workflow.activate_trigger(
                            crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::RepeatedNoCodeProgress,
                        ) {
                            trace_adaptive_workflow_trigger(
                                &trace,
                                crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::RepeatedNoCodeProgress,
                                &code_workflow,
                            );
                            trace.record(TraceEvent::WorkflowFallback {
                                error:
                                    "adaptive workflow trigger activated: repeated_no_code_progress"
                                        .to_string(),
                            });
                        }
                        let lookup_rule = Self::targeted_lookup_budget_rule(0);
                        let checkpoint = format!(
                            "Workflow action checkpoint: this is a {:?} task and {} consecutive successful tool rounds produced no code change. On the next response, use file_edit or file_write to apply the smallest safe patch, then run validation after the file changes. If prior grep/file_read results include line numbers, prefer file_edit with line_start/line_end or exact old_string copied from that current source context. Do not call glob/project_list or repeat broad inspection. If a specific symbol, test, or call site is still missing, use the focused lookup budget, then patch. {} If a scorer/decision object already returns final status, use that status directly instead of reimplementing acceptance gates. If you cannot patch safely from the evidence already gathered, stop with a Closeout status of not_verified and a concrete blocker.",
                            route.workflow, no_code_progress_rounds, lookup_rule
                        );
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "code-change task made no edit after repeated inspection"
                                .to_string(),
                        });
                        messages.push(Message::system(checkpoint.clone()));
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&checkpoint);
                        action_checkpoint_active = true;
                        action_checkpoint_lookup_count = 0;
                        file_edit_failure_retry_used = false;
                        action_checkpoint_no_change_rounds = 2;
                        force_patch_synthesis_after_no_change = false;
                        force_patch_synthesis_reason = None;
                        activated_checkpoint_this_round = true;
                    } else if action_checkpoint_active && used_action_checkpoint_lookup {
                        action_checkpoint_lookup_count = (action_checkpoint_lookup_count + 1)
                            .min(Self::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET);
                        action_checkpoint_no_change_rounds = 0;
                        activated_checkpoint_this_round = true;
                        let lookup_notice = if action_checkpoint_lookup_count
                            >= Self::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET
                        {
                            "focused repair targeted lookup budget used; next checkpoint request will expose patch tools only"
                        } else {
                            "focused repair targeted lookup used; one targeted lookup remains before patch-only mode"
                        };
                        trace.record(TraceEvent::WorkflowFallback {
                            error: lookup_notice.to_string(),
                        });
                        if action_checkpoint_lookup_count
                            >= Self::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET
                        {
                            action_checkpoint_no_change_rounds = 1;
                            force_patch_synthesis_after_no_change = true;
                            force_patch_synthesis_reason =
                                Some("focused repair lookup budget exhausted");
                        }
                    }
                    if action_checkpoint_active && !activated_checkpoint_this_round {
                        action_checkpoint_no_change_rounds += 1;
                        if action_checkpoint_no_change_rounds >= 3 {
                            trace.record(TraceEvent::WorkflowFallback {
                                error: "action checkpoint entered patch synthesis after repeated focused repair reads"
                                    .to_string(),
                            });
                            force_patch_synthesis_after_no_change = true;
                            force_patch_synthesis_reason =
                                Some("focused repair lookup did not produce a patch");
                        }
                    }
                }
            }

            if action_checkpoint_active
                && ((!any_tool_success && !failed_tool_evidence.is_empty())
                    || force_patch_synthesis_after_no_change)
            {
                action_checkpoint_no_change_rounds += 1;
                let lookup_rule = Self::targeted_lookup_budget_rule(action_checkpoint_lookup_count);
                let reminder = format!(
                    "Focused repair correction: the last tool call did not execute. The current request only permits these tools: {}. Use file_edit/file_write for exact replacements or line_start/line_end replacements from earlier line-numbered output. If a specific symbol or call site is missing, use the focused lookup budget, then patch. {}",
                    exposed_tool_names.iter().cloned().collect::<Vec<_>>().join(", "),
                    lookup_rule
                );
                if action_checkpoint_no_change_rounds >= 2 {
                    trace.record(TraceEvent::WorkflowFallback {
                        error: if force_patch_synthesis_after_no_change {
                            format!(
                                "action checkpoint entered patch synthesis: {}",
                                force_patch_synthesis_reason
                                    .unwrap_or("repeated no-change checkpoint")
                            )
                        } else {
                            "action checkpoint entered patch synthesis after repeated invalid tools"
                                .to_string()
                        },
                    });
                    if !Self::patch_synthesis_enabled() {
                        let deterministic_calls = if Self::deterministic_patch_synthesis_enabled() {
                            let evidence = Self::patch_synthesis_evidence(&messages);
                            let deterministic_seed = if last_user_preview.trim().is_empty() {
                                evidence.clone()
                            } else if evidence.trim().is_empty() {
                                format!("TASK:\n{}", last_user_preview.as_str())
                            } else {
                                format!(
                                    "TASK:\n{}\n\nEVIDENCE:\n{}",
                                    last_user_preview.as_str(),
                                    evidence
                                )
                            };
                            let cwd = std::env::current_dir()
                                .unwrap_or_else(|_| std::path::PathBuf::from("."));
                            self.deterministic_patch_tool_calls(&deterministic_seed, &cwd)
                        } else {
                            Vec::new()
                        };
                        if !deterministic_calls.is_empty() {
                            trace.record(TraceEvent::WorkflowFallback {
                                error: format!(
                                    "deterministic patch synthesis produced {} file_edit action(s)",
                                    deterministic_calls.len()
                                ),
                            });
                            messages.push(Message::assistant_with_tools(
                                "Applying deterministic patch from prior evidence.",
                                deterministic_calls.clone(),
                            ));
                            let exposed_synth_tools =
                                HashSet::from(["file_edit".to_string(), "file_write".to_string()]);
                            let mut synthesized_results = self
                                .execute_tools_parallel(
                                    &deterministic_calls,
                                    tx,
                                    std::collections::HashMap::new(),
                                    Some(trace.clone()),
                                    &resource_policy,
                                    &exposed_synth_tools,
                                    false,
                                    0,
                                    false,
                                    &destructive_scope,
                                )
                                .await;
                            for (tc, result) in synthesized_results.iter_mut() {
                                truncate_tool_result(result, &tc.name, &tc.id).await;
                                append_provider_tool_result(
                                    tc,
                                    result,
                                    &mut evidence_ledger,
                                    &mut tool_results_text,
                                    &mut messages,
                                );
                                if result.success && Self::is_code_write_tool_name(&tc.name) {
                                    action_checkpoint_requires_patch_before_validation = false;
                                    if let Some(path) = tc.arguments["path"].as_str() {
                                        changed_files.push(std::path::PathBuf::from(path));
                                    }
                                }
                            }
                            final_tool_calls.extend(deterministic_calls);
                            if crate::engine::code_change_workflow::is_programming_workflow(
                                route.workflow,
                            ) {
                                for path in Self::git_status_files_since(&baseline_git_status_files)
                                {
                                    if !changed_files.iter().any(|existing| existing == &path) {
                                        changed_files.push(path);
                                    }
                                }
                            }
                            if !changed_files.is_empty() {
                                action_checkpoint_active = false;
                                action_checkpoint_lookup_count = 0;
                                action_checkpoint_no_change_rounds = 0;
                                no_code_progress_rounds = 0;
                                continue;
                            }
                        }
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "patch synthesis disabled by default; returning control to model-led repair"
                                .to_string(),
                        });
                        if !patch_synthesis_recovery_used {
                            patch_synthesis_recovery_used = true;
                            action_checkpoint_no_change_rounds = 0;
                            let lookup_rule =
                                Self::targeted_lookup_budget_rule(action_checkpoint_lookup_count);
                            let recovery = format!(
                                "Patch synthesis is disabled by default. Use only the exposed tools ({}) to make the smallest safe patch from the evidence already gathered. Prefer file_edit/file_write; bash is allowed only for a mutating patch command here. If file_read or grep is still exposed, use the remaining focused lookup budget before patching; otherwise patch from the evidence already gathered. {} Do not call tools that are not exposed.",
                                exposed_tool_names.iter().cloned().collect::<Vec<_>>().join(", "),
                                lookup_rule
                            );
                            messages.push(Message::system(recovery.clone()));
                            tool_results_text.push('\n');
                            tool_results_text.push_str(&recovery);
                            continue;
                        }
                        if !action_checkpoint_reopen_used {
                            action_checkpoint_reopen_used = true;
                            action_checkpoint_active = false;
                            action_checkpoint_lookup_count = 0;
                            action_checkpoint_no_change_rounds = 0;
                            no_code_progress_rounds = 1;
                            trace.record(TraceEvent::WorkflowFallback {
                                error: "focused repair did not produce a patch; reopening normal code-change tools once"
                                    .to_string(),
                            });
                            let recovery = "Focused repair did not produce a file change. Return to normal coding tools for one final recovery pass: inspect only the exact function or call site needed, then make a real file_edit/file_write or mutating bash patch before running validation. Do not close out until a file change succeeds or a concrete blocker is proven."
                                .to_string();
                            messages.push(Message::system(recovery.clone()));
                            tool_results_text.push('\n');
                            tool_results_text.push_str(&recovery);
                            continue;
                        }
                        let stop_msg =
                            "[Stopped action checkpoint without patch synthesis; no model-led file change was produced]";
                        debug!("{}", stop_msg);
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                                .await;
                        }
                        if final_content.trim().is_empty() {
                            final_content = stop_msg.to_string();
                        } else {
                            final_content.push('\n');
                            final_content.push_str(stop_msg);
                        }
                        break;
                    }
                    match self
                        .synthesize_patch_tool_calls(&messages, last_user_preview.as_str())
                        .await
                    {
                        Ok(synthesized_calls) => {
                            trace.record(TraceEvent::WorkflowFallback {
                                error: format!(
                                    "patch synthesis produced {} file_edit action(s)",
                                    synthesized_calls.len()
                                ),
                            });
                            messages.push(Message::assistant_with_tools(
                                "Applying synthesized patch from prior evidence.",
                                synthesized_calls.clone(),
                            ));
                            let exposed_synth_tools =
                                HashSet::from(["file_edit".to_string(), "file_write".to_string()]);
                            let mut synthesized_results = self
                                .execute_tools_parallel(
                                    &synthesized_calls,
                                    tx,
                                    std::collections::HashMap::new(),
                                    Some(trace.clone()),
                                    &resource_policy,
                                    &exposed_synth_tools,
                                    // Synthesized edits have already passed
                                    // validate_patch_synthesis_action(). Avoid
                                    // applying the direct action-checkpoint
                                    // guard again, or safe recovered patches can
                                    // be rejected without giving the model a way
                                    // to inspect and repair the arguments.
                                    false,
                                    0,
                                    false,
                                    &destructive_scope,
                                )
                                .await;
                            for (tc, result) in synthesized_results.iter_mut() {
                                truncate_tool_result(result, &tc.name, &tc.id).await;
                                append_provider_tool_result(
                                    tc,
                                    result,
                                    &mut evidence_ledger,
                                    &mut tool_results_text,
                                    &mut messages,
                                );
                                if result.success {
                                    any_tool_success = true;
                                }
                                if result.success && Self::is_code_write_tool_name(&tc.name) {
                                    if let Some(path) = tc.arguments["path"].as_str() {
                                        changed_files.push(std::path::PathBuf::from(path));
                                    }
                                }
                            }
                            final_tool_calls.extend(synthesized_calls);
                            if crate::engine::code_change_workflow::is_programming_workflow(
                                route.workflow,
                            ) {
                                for path in Self::git_status_files_since(&baseline_git_status_files)
                                {
                                    if !changed_files.iter().any(|existing| existing == &path) {
                                        changed_files.push(path);
                                    }
                                }
                            }
                            if !changed_files.is_empty() {
                                action_checkpoint_active = false;
                                action_checkpoint_lookup_count = 0;
                                action_checkpoint_no_change_rounds = 0;
                                no_code_progress_rounds = 0;
                            } else {
                                let stop_msg =
                                    "[Patch synthesis did not produce a file change; stopped action checkpoint]";
                                debug!("{}", stop_msg);
                                if let Some(tx) = tx {
                                    let _ = tx
                                        .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                                        .await;
                                }
                                if final_content.trim().is_empty() {
                                    final_content = stop_msg.to_string();
                                } else {
                                    final_content.push('\n');
                                    final_content.push_str(stop_msg);
                                }
                                break;
                            }
                        }
                        Err(err) => {
                            trace.record(TraceEvent::WorkflowFallback {
                                error: format!("patch synthesis failed: {}", err),
                            });
                            let err_text = err.to_string();
                            let lower_err = err_text.to_lowercase();
                            if !patch_synthesis_recovery_used
                                && (lower_err.contains("declined")
                                    || lower_err.contains("inspect more")
                                    || lower_err.contains("need to inspect")
                                    || lower_err.contains("not enough evidence"))
                            {
                                patch_synthesis_recovery_used = true;
                                action_checkpoint_active = false;
                                action_checkpoint_lookup_count = 0;
                                action_checkpoint_no_change_rounds = 0;
                                no_code_progress_rounds = 1;
                                file_edit_failure_retry_used = false;
                                let lookup_rule = Self::targeted_lookup_budget_rule(0);
                                let recovery = format!(
                                    "Patch synthesis declined because evidence was insufficient: {}. Use a targeted read/search for the missing symbol, call site, or test, then make the smallest safe edit. {}",
                                    safe_prefix_by_bytes(&err_text, 500),
                                    lookup_rule
                                );
                                messages.push(Message::system(recovery.clone()));
                                tool_results_text.push('\n');
                                tool_results_text.push_str(&recovery);
                                continue;
                            }
                            if !action_checkpoint_reopen_used {
                                action_checkpoint_reopen_used = true;
                                action_checkpoint_active = false;
                                action_checkpoint_lookup_count = 0;
                                action_checkpoint_no_change_rounds = 0;
                                no_code_progress_rounds = 1;
                                trace.record(TraceEvent::WorkflowFallback {
                                    error:
                                        "patch synthesis failed; reopening normal code-change tools once"
                                            .to_string(),
                                });
                                let recovery = format!(
                                    "Patch synthesis could not produce an executable edit: {}. Return to normal coding tools for one final recovery pass: inspect only the exact function or call site needed, then make a real file_edit/file_write or mutating bash patch before validation.",
                                    safe_prefix_by_bytes(&err_text, 500)
                                );
                                messages.push(Message::system(recovery.clone()));
                                tool_results_text.push('\n');
                                tool_results_text.push_str(&recovery);
                                continue;
                            }
                            let stop_msg =
                                "[Stopped action checkpoint after repeated invalid tool requests]";
                            debug!("{}", stop_msg);
                            if let Some(tx) = tx {
                                let _ = tx
                                    .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                                    .await;
                            }
                            if final_content.trim().is_empty() {
                                final_content = stop_msg.to_string();
                            } else {
                                final_content.push('\n');
                                final_content.push_str(stop_msg);
                            }
                            break;
                        }
                    }
                } else {
                    messages.push(Message::system(reminder.clone()));
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&reminder);
                    continue;
                }
            }

            if !any_tool_success
                && !failed_tool_evidence.is_empty()
                && workflow_contract_enabled(self.provider.as_ref())
            {
                let analyzer = crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
                    self.provider.as_ref(),
                    self.model.clone(),
                );
                let prompt = crate::engine::workflow_contract::GuidedDebuggingPrompt::new(
                    last_user_preview.as_str(),
                    task_bundle
                        .workflow_judgment
                        .as_ref()
                        .map(|judgment| judgment.to_turn_context()),
                    failed_tool_names_this_round.clone(),
                    failed_tool_evidence.clone(),
                );
                match analyzer.analyze_debugging(prompt).await {
                    Ok(debugging) => {
                        trace.record(TraceEvent::GuidedDebuggingCompleted {
                            blocker: debugging.blocker,
                            next_action: format!("{:?}", debugging.next_action),
                            causes: debugging.likely_causes.len(),
                            evidence_items: debugging.evidence_to_collect.len(),
                            ask_user: debugging.ask_user,
                        });
                        persist_workflow_learning_event(
                            self.session_store.as_ref(),
                            &self.session_id,
                            "guided_debugging",
                            format!(
                                "Guided debugging selected {:?}: {}",
                                debugging.next_action, debugging.symptom
                            ),
                            if debugging.blocker { 0.85 } else { 0.7 },
                            serde_json::json!({
                                "blocker": debugging.blocker,
                                "symptom": debugging.symptom.clone(),
                                "likely_causes": debugging.likely_causes.clone(),
                                "evidence_to_collect": debugging.evidence_to_collect.clone(),
                                "smallest_safe_action": debugging.smallest_safe_action.clone(),
                                "ask_user": debugging.ask_user,
                                "questions": debugging.questions.clone(),
                                "next_action": format!("{:?}", debugging.next_action),
                                "failed_tools": failed_tool_names_this_round.clone(),
                            }),
                        );
                        let debugging_text = debugging.format_for_prompt();
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&debugging_text);
                        messages.push(Message::system(debugging_text));
                        apply_workflow_feedback_and_trace(
                            &mut task_bundle,
                            &trace,
                            crate::engine::workflow_contract::WeightFeedbackEvent {
                                kind: crate::engine::workflow_contract::WeightFeedbackKind::ToolFailure,
                                severity: if debugging.blocker {
                                    crate::engine::workflow_contract::WeightFeedbackSeverity::High
                                } else {
                                    crate::engine::workflow_contract::WeightFeedbackSeverity::Medium
                                },
                                confidence: 0.85,
                                reason: Some(debugging.symptom.clone()),
                            },
                        );
                    }
                    Err(err) => {
                        warn!("Guided debugging analysis failed: {}", err);
                        trace.record(TraceEvent::WorkflowFallback {
                            error: format!("guided debugging analysis failed: {}", err),
                        });
                    }
                }
            }

            if !any_tool_success && !repeated_failed_tools.is_empty() {
                repeated_failed_tools.sort();
                repeated_failed_tools.dedup();
                let stop_msg = format!(
                    "[Stopped repeated failed tool attempts: {}]",
                    repeated_failed_tools.join(", ")
                );
                debug!("{}", stop_msg);
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                        .await;
                }
                if final_content.trim().is_empty() {
                    final_content = stop_msg;
                } else {
                    final_content.push('\n');
                    final_content.push_str(&stop_msg);
                }
                break;
            }

            if !any_tool_success {
                let mut noisy_by_name = Vec::new();
                for (name, count) in &failed_tool_names {
                    if *count >= 2 && !READ_ONLY_TOOLS.contains(&name.as_str()) {
                        noisy_by_name.push(name.clone());
                    }
                }
                if !noisy_by_name.is_empty() {
                    noisy_by_name.sort();
                    noisy_by_name.dedup();
                    let stop_msg = format!(
                        "[Stopped noisy retries after repeated failures: {}]",
                        noisy_by_name.join(", ")
                    );
                    debug!("{}", stop_msg);
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                            .await;
                    }
                    if final_content.trim().is_empty() {
                        final_content = stop_msg;
                    } else {
                        final_content.push('\n');
                        final_content.push_str(&stop_msg);
                    }
                    break;
                }
            }

            // ── 自动验证闭环 ──────────────────────────────
            if !changed_files.is_empty() {
                evidence_ledger.record_changed_files(&changed_files);
                if code_workflow.activate_trigger(
                    crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::FirstCodeChange,
                ) {
                    trace_adaptive_workflow_trigger(
                        &trace,
                        crate::engine::code_change_workflow::AdaptiveWorkflowTrigger::FirstCodeChange,
                        &code_workflow,
                    );
                    trace.record(TraceEvent::WorkflowFallback {
                        error: format!(
                            "adaptive workflow trigger activated: first_code_change files={}",
                            changed_files.len()
                        ),
                    });
                }
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let mut post_edit_evidence = Vec::new();
                let mut acceptance_evidence = Vec::new();
                let mut failed_commands = Vec::new();
                let verify_results =
                    super::auto_verify::verify_file_changes(&working_dir, &changed_files).await;
                let check_passed = verify_results.iter().all(|r| r.success);
                if !check_passed {
                    if let Some(source_context) =
                        verification_source_context(&working_dir, &verify_results)
                    {
                        post_edit_evidence.push(source_context.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&source_context);
                        messages.push(Message::system(source_context));
                    }
                }
                for result in verify_results {
                    let verify_text = result.to_dialog_text();
                    acceptance_evidence.push(verify_text.clone());
                    evidence_ledger.record_validation_result(
                        "auto_verify",
                        Some(&result.command),
                        result.success,
                        &verify_text,
                    );
                    if !result.success {
                        failed_commands.push(result.command.clone());
                        post_edit_evidence.push(verify_text.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&verify_text);
                        messages.push(Message::system(verify_text));
                    } else {
                        debug!("{}", verify_text);
                    }
                }

                // ── LSP 诊断补充 ───────────────────────────
                if let Some(ref lsp_mgr) = self.lsp_manager {
                    let mut lsp_issues = Vec::new();
                    for path in &changed_files {
                        let uri = super::lsp::path_to_uri(path);
                        for name in lsp_mgr.server_names() {
                            if let Some(client) = lsp_mgr.get_client(&name) {
                                let diagnostics = client.get_diagnostics(&uri).await;
                                for d in diagnostics {
                                    let sev = match d.severity {
                                        Some(1) => "error",
                                        Some(2) => "warning",
                                        Some(3) => "info",
                                        Some(4) => "hint",
                                        _ => "diagnostic",
                                    };
                                    lsp_issues.push(format!(
                                        "  [{}] {}:{}: {}",
                                        sev,
                                        path.display(),
                                        d.range.start.line + 1,
                                        d.message.replace('\n', " ")
                                    ));
                                }
                            }
                        }
                    }
                    if !lsp_issues.is_empty() {
                        let lsp_text = format!(
                            "[LSP diagnostics for modified files]:\n{}",
                            lsp_issues.join("\n")
                        );
                        post_edit_evidence.push(lsp_text.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&lsp_text);
                        messages.push(Message::system(lsp_text));
                    }
                }

                // ── Required validation first ───────────────────
                //
                // Live tasks can define domain-specific required commands. Run
                // those before generic auto-test discovery so a cold full
                // `cargo test` probe cannot spend the turn budget before the
                // deterministic closeout path sees the required evidence.
                let mut required_validation_passed = true;
                if !required_validation_commands.is_empty() {
                    let already_ran = successful_validation_commands
                        .iter()
                        .map(|cmd| Self::normalize_validation_command_for_match(cmd))
                        .chain(
                            successful_required_validation_commands
                                .iter()
                                .map(|cmd| Self::normalize_validation_command_for_match(cmd)),
                        )
                        .collect::<HashSet<_>>();
                    let required_to_run = required_validation_commands
                        .iter()
                        .filter(|cmd| {
                            !already_ran
                                .contains(&Self::normalize_validation_command_for_match(cmd))
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if !required_to_run.is_empty() {
                        let required_results =
                            Self::run_required_validation_commands(&working_dir, &required_to_run)
                                .await;
                        for result in required_results {
                            let text = result.to_dialog_text();
                            acceptance_evidence.push(text.clone());
                            evidence_ledger.record_validation_result(
                                "required_validation",
                                Some(&result.command),
                                result.success,
                                &text,
                            );
                            if result.success {
                                successful_required_validation_commands
                                    .insert(result.command.trim().to_string());
                                debug!("{}", text);
                            } else {
                                required_validation_passed = false;
                                failed_commands.push(result.command.clone());
                                post_edit_evidence.push(text.clone());
                                tool_results_text.push('\n');
                                tool_results_text.push_str(&text);
                                messages.push(Message::system(text));
                            }
                        }
                    }
                }
                let required_validation_covers_tests =
                    !required_validation_commands.is_empty() && required_validation_passed;

                // ── 自动测试闭环 ──────────────────────────────
                let manual_validation_after_changes = !successful_validation_commands.is_empty();
                let test_results = if should_run_default_auto_tests(&required_validation_commands) {
                    super::auto_verify::run_tests(&working_dir, &changed_files, check_passed).await
                } else {
                    Vec::new()
                };
                let tests_passed = required_validation_covers_tests
                    || test_results.iter().all(|r| r.success)
                    || (manual_validation_after_changes && check_passed);
                if !tests_passed {
                    if let Some(source_context) =
                        verification_source_context(&working_dir, &test_results)
                    {
                        post_edit_evidence.push(source_context.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&source_context);
                        messages.push(Message::system(source_context));
                    }
                }
                for result in test_results {
                    let test_text = result.to_dialog_text();
                    acceptance_evidence.push(test_text.clone());
                    evidence_ledger.record_validation_result(
                        "auto_test",
                        Some(&result.command),
                        result.success,
                        &test_text,
                    );
                    if !result.success {
                        if manual_validation_after_changes || required_validation_covers_tests {
                            debug!(
                                "Ignoring stale automatic test failure after successful required/manual validation command: {}",
                                result.command
                            );
                        } else {
                            failed_commands.push(result.command.clone());
                            post_edit_evidence.push(test_text.clone());
                            tool_results_text.push('\n');
                            tool_results_text.push_str(&test_text);
                            messages.push(Message::system(test_text));
                        }
                    } else {
                        debug!("{}", test_text);
                    }
                }
                if manual_validation_after_changes {
                    let manual_text = format!(
                        "[Manual validation passed after code changes]\n{}",
                        successful_validation_commands
                            .iter()
                            .map(|cmd| format!("  $ {}", cmd))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                    acceptance_evidence.push(manual_text.clone());
                    post_edit_evidence.push(manual_text.clone());
                    for command in &successful_validation_commands {
                        evidence_ledger.record_validation_result(
                            "manual_validation",
                            Some(command),
                            true,
                            &manual_text,
                        );
                    }
                    debug!("{}", manual_text);
                }

                if let Some(diff_text) =
                    changed_files_diff_evidence(&working_dir, &changed_files).await
                {
                    acceptance_evidence.push(diff_text.clone());
                    post_edit_evidence.push(diff_text.clone());
                    evidence_ledger.record_validation_result("diff", None, true, &diff_text);
                    debug!("{}", diff_text);
                }

                // ── 代码自审查 ────────────────────────────────
                let review_result =
                    super::code_review::review_changed_files(&working_dir, &changed_files);
                let review_dialog = review_result.to_dialog_text();
                evidence_ledger.record_validation_result(
                    "code_review",
                    None,
                    review_result.success,
                    &review_dialog,
                );
                acceptance_evidence.push(review_dialog);
                if !review_result.success {
                    let review_text = review_result.to_dialog_text();
                    post_edit_evidence.push(review_text.clone());
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&review_text);
                    messages.push(Message::system(review_text));
                }

                // ── 编程质量可观测性 ───────────────────────
                // When all required commands pass, they are stronger evidence
                // than the repository's default auto-test for that changed
                // area.
                let effective_check_passed = check_passed || required_validation_covers_tests;
                let effective_tests_passed = tests_passed || required_validation_covers_tests;
                let verify_passed = effective_check_passed
                    && effective_tests_passed
                    && required_validation_passed
                    && review_result.success;
                should_closeout_after_verified_change = verify_passed;
                trace.record(TraceEvent::VerificationCompleted {
                    changed_files: changed_files.len(),
                    passed: verify_passed,
                    check_passed: effective_check_passed,
                    tests_passed: effective_tests_passed,
                    review_passed: review_result.success,
                    failed_commands: failed_commands.clone(),
                });
                let post_edit_reflection =
                    Self::record_verification_repair_context(VerificationRepairContext {
                        trace: &trace,
                        code_workflow: &mut code_workflow,
                        task_id: task_bundle.task_id.clone(),
                        changed_files: &changed_files,
                        verify_passed,
                        post_edit_evidence: &post_edit_evidence,
                        failed_commands: &failed_commands,
                        acceptance_repair_attempts,
                        tool_results_text: &mut tool_results_text,
                        messages: &mut messages,
                    });
                trace.record(TraceEvent::ReflectionPassCompleted {
                    pass_id: post_edit_reflection.pass_id.clone(),
                    task_id: post_edit_reflection.task_id.clone(),
                    status: format!("{:?}", post_edit_reflection.status),
                    findings: post_edit_reflection.findings.len(),
                    unresolved: post_edit_reflection.unresolved_count(),
                });
                let stage_record = code_workflow.record_stage_validation(
                    &task_bundle,
                    &changed_files,
                    verify_passed,
                    &acceptance_evidence,
                );
                trace_stage_validation(&trace, &stage_record);
                if let Some(feedback) = stage_record.feedback.clone() {
                    apply_workflow_feedback_and_trace(&mut task_bundle, &trace, feedback);
                }
                if !verify_passed {
                    self.run_guided_validation_debugging(GuidedValidationDebuggingContext {
                        trace: &trace,
                        last_user_preview: last_user_preview.as_str(),
                        task_bundle: &task_bundle,
                        post_edit_evidence: &post_edit_evidence,
                        tool_results_text: &mut tool_results_text,
                        messages: &mut messages,
                    })
                    .await;
                }
                let acceptance_repair_outcome = self
                    .run_acceptance_repair_review(AcceptanceRepairContext {
                        trace: &trace,
                        route: &route,
                        code_workflow: &mut code_workflow,
                        task_bundle: &mut task_bundle,
                        changed_files: &changed_files,
                        verify_passed,
                        review_success: review_result.success,
                        required_validation_commands: &required_validation_commands,
                        failed_commands: &failed_commands,
                        post_edit_evidence: &post_edit_evidence,
                        acceptance_evidence: &acceptance_evidence,
                        required_validation_passed,
                        check_passed,
                        acceptance_repair_attempts: &mut acceptance_repair_attempts,
                        reserved_repair_rounds: &mut reserved_repair_rounds,
                        action_checkpoint_no_change_rounds: &mut action_checkpoint_no_change_rounds,
                        action_checkpoint_active: &mut action_checkpoint_active,
                        action_checkpoint_lookup_count: &mut action_checkpoint_lookup_count,
                        file_edit_failure_retry_used: &mut file_edit_failure_retry_used,
                        action_checkpoint_requires_patch_before_validation:
                            &mut action_checkpoint_requires_patch_before_validation,
                        should_closeout_after_verified_change,
                        tool_results_text: &mut tool_results_text,
                        messages: &mut messages,
                    })
                    .await;
                should_closeout_after_verified_change =
                    acceptance_repair_outcome.should_closeout_after_verified_change;
                if let Some(content) = acceptance_repair_outcome.final_content {
                    final_content = content;
                }
                if acceptance_repair_outcome.break_loop {
                    break;
                }
                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_coding_round(verify_passed);
                }
                if post_edit_reflection.status
                    != crate::engine::reflection_pass::ReflectionStatus::Passed
                {
                    should_closeout_after_verified_change = false;
                    action_checkpoint_requires_patch_before_validation = true;
                    let repair_instruction = format!(
                        "{}\nPost-edit reflection found unresolved quality gaps. Fix the changed files before giving a final answer.",
                        post_edit_reflection.format_for_prompt()
                    );
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&repair_instruction);
                    messages.push(Message::system(repair_instruction));
                    if effective_iterations >= self.max_iterations {
                        reserved_repair_rounds = reserved_repair_rounds.max(1);
                        trace.record(TraceEvent::WorkflowFallback {
                            error:
                                "reserved repair round granted after post-edit reflection failure"
                                    .to_string(),
                        });
                    }
                }
            }

            // ── 记忆同步 ──────────────────────────────────
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                let user_msg = messages
                    .iter()
                    .rposition(|m| matches!(m, Message::User { .. }))
                    .and_then(|i| match &messages[i] {
                        Message::User { content } => Some(content.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                if !user_msg.is_empty() {
                    let assistant_text = format!("{} {}", final_content, tool_results_text);
                    if self.llm_memory_extraction {
                        if mem.should_extract_with_llm() {
                            let provider: Option<&dyn LlmProvider> = Some(self.provider.as_ref());
                            mem.sync_turn_llm(user_msg, &assistant_text, provider, &self.model)
                                .await;
                            mem.mark_main_agent_wrote();
                            trace.record(TraceEvent::MemorySynced {
                                mode: "llm".to_string(),
                            });
                        }
                    } else {
                        mem.sync_turn(user_msg, &assistant_text);
                        mem.mark_main_agent_wrote();
                        trace.record(TraceEvent::MemorySynced {
                            mode: "heuristic".to_string(),
                        });
                    }
                }
                mem.increment_turn();
            }

            if should_closeout_after_verified_change {
                trace.record(TraceEvent::WorkflowFallback {
                    error:
                        "verified code change passed validation; preparing deterministic closeout"
                            .to_string(),
                });
                break;
            }
        }

        Self::apply_final_closeout(FinalCloseoutContext {
            trace: &trace,
            code_workflow: &code_workflow,
            task_bundle: &task_bundle,
            runtime_diet: &mut runtime_diet,
            final_content: &mut final_content,
            final_tool_calls: &final_tool_calls,
            iterations_used,
            max_iterations: self.max_iterations,
            evidence_ledger: &evidence_ledger,
            tx,
        })
        .await;

        trace_runtime_diet_report(&trace, &route, &code_workflow, &runtime_diet);

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Complete).await;
        }

        trace.record(TraceEvent::AssistantResponded {
            chars: final_content.chars().count(),
            iterations: iterations_used,
        });
        self.finish_trace(trace, TurnStatus::Completed);

        Ok(LoopResult {
            content: final_content,
            tool_calls: Vec::new(),
            tool_calls_made,
            iterations: iterations_used,
            pre_executed_results: std::collections::HashMap::new(),
        })
    }

    /// 非流式 API 调用
    fn git_status_files() -> HashSet<std::path::PathBuf> {
        let output = std::process::Command::new("git")
            .args(["status", "--short", "--untracked-files=all"])
            .output();
        let Ok(output) = output else {
            return HashSet::new();
        };
        if !output.status.success() {
            return HashSet::new();
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .filter_map(Self::parse_git_status_path)
            .collect()
    }

    fn git_status_files_since(baseline: &HashSet<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
        let mut changed: Vec<_> = Self::git_status_files()
            .into_iter()
            .filter(|path| !baseline.contains(path))
            .collect();
        changed.sort();
        changed
    }

    fn parse_git_status_path(line: &str) -> Option<std::path::PathBuf> {
        let path = line.get(3..)?.trim();
        if path.is_empty() {
            return None;
        }
        let path = path.rsplit_once(" -> ").map(|(_, new)| new).unwrap_or(path);
        Some(std::path::PathBuf::from(path.trim_matches('"')))
    }

    fn allows_no_diff_audit_closeout(prompt: &str) -> bool {
        let lower = prompt.to_ascii_lowercase();
        lower.contains("eval intent: `audit_or_regression_check`")
            || lower.contains("eval intent: audit_or_regression_check")
            || lower.contains("eval intent: `stale_or_already_satisfied`")
            || lower.contains("eval intent: stale_or_already_satisfied")
            || lower.contains("if the requested behavior is already present")
            || lower.contains("do not force an arbitrary edit")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::{ChatResponse, Tool, ToolCall, Usage};
    use crate::test_utils::env_guard::EnvVarGuard;
    use crate::tools::{BashTool, FileEditTool, FileReadTool, FileWriteTool, GitTool};
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::{HashSet, VecDeque};
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_truncate_tool_result_handles_utf8_boundaries() {
        let mut result = ToolResult::success("中".repeat(20_000));
        truncate_tool_result(&mut result, "grep", "call_utf8").await;
        assert!(result.content.contains("Output truncated"));
    }

    #[tokio::test]
    async fn test_required_validation_shell_strips_agent_runtime_env() {
        let mut env = EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_TEST", "check_then_test");
        env.set(
            "PRIORITY_AGENT_EVAL_EVENTS",
            "/tmp/priority-agent-events.jsonl",
        );

        let tmp = tempdir().expect("create temp dir");
        let output = shell_output_with_timeout(
            "printf '%s:%s' \"${PRIORITY_AGENT_AUTO_TEST:-unset}\" \"${PRIORITY_AGENT_EVAL_EVENTS:-unset}\"",
            tmp.path(),
            std::time::Duration::from_secs(5),
        )
        .await
        .expect("run shell command");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "unset:unset");
    }

    #[test]
    fn test_extract_required_validation_commands_keeps_live_eval_script_checks() {
        let prompt = r#"
## Acceptance checks
- `bash -n scripts/run_live_eval.sh`
- `scripts/run_live_eval.sh --list`
- `scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke`
- `cargo test -q -- --test-threads=1`
"#;

        let commands = ConversationLoop::extract_required_validation_commands(prompt);
        assert_eq!(
            commands,
            vec![
                "bash -n scripts/run_live_eval.sh",
                "scripts/run_live_eval.sh --list",
                "scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke",
                "cargo test -q -- --test-threads=1",
            ]
        );
    }

    #[test]
    fn test_audit_eval_allows_no_diff_closeout() {
        let audit_prompt = r#"
# Live coding regression task: memory recall should demote only relevant conflicts
- Eval intent: `audit_or_regression_check`
## Closeout requirements
- This is an audit/regression evaluation. If the requested behavior is already present, prove it with direct evidence and required commands instead of forcing an arbitrary edit.
"#;

        assert!(ConversationLoop::allows_no_diff_audit_closeout(
            audit_prompt
        ));
        assert!(!ConversationLoop::allows_no_diff_audit_closeout(
            "- Eval intent: `seeded_code_change`\n- This is a real code-change evaluation."
        ));
    }

    #[test]
    fn test_allowed_tool_context_enforces_subagent_tool_scope() {
        assert!(tool_allowed_by_context(&None, "bash"));

        let allowed = Some(HashSet::from(["file_read".to_string(), "grep".to_string()]));
        assert!(tool_allowed_by_context(&allowed, "file_read"));
        assert!(tool_allowed_by_context(&allowed, "grep"));
        assert!(!tool_allowed_by_context(&allowed, "bash"));
    }

    fn fake_tools(names: &[&str]) -> Vec<Tool> {
        names
            .iter()
            .map(|name| Tool::new(*name, format!("{} tool", name)))
            .collect()
    }

    fn exposed_names(tools: &[Tool]) -> HashSet<String> {
        tools.iter().map(|tool| tool.name.clone()).collect()
    }

    fn sorted_tool_names(tools: &[Tool]) -> Vec<String> {
        let mut names = tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn runtime_diet_tool_universe() -> Vec<Tool> {
        fake_tools(&[
            "agent",
            "ask_user",
            "bash",
            "calculate",
            "datetime",
            "diff",
            "enter_plan_mode",
            "exit_plan_mode",
            "file_edit",
            "file_read",
            "file_write",
            "format",
            "git",
            "glob",
            "grep",
            "json_query",
            "list_mcp_resources",
            "lsp",
            "mcp",
            "mcp_auth",
            "mcp_tool",
            "memory_load",
            "memory_save",
            "plan",
            "project_list",
            "read_mcp_resource",
            "refactor",
            "repl",
            "skill_manage",
            "skills_list",
            "skill_view",
            "swarm",
            "symbol_query",
            "task_create",
            "task_get",
            "task_list",
            "task_output",
            "task_stop",
            "task_update",
            "todo_write",
            "web_fetch",
            "web_search",
            "workbench",
            "worktree",
        ])
    }

    #[test]
    fn route_scoped_tools_for_file_delete_keep_destructive_scope_small() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let route = IntentRouter::new().route("帮我把这个文件删了吧");
        let tools = fake_tools(&[
            "file_read",
            "file_write",
            "file_edit",
            "glob",
            "bash",
            "web_search",
            "memory_save",
            "mcp",
            "agent",
        ]);

        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("file_read"));
        assert!(exposed.contains("glob"));
        assert!(exposed.contains("bash"));
        assert!(!exposed.contains("file_write"));
        assert!(!exposed.contains("file_edit"));
        assert!(!exposed.contains("web_search"));
        assert!(!exposed.contains("memory_save"));
        assert!(!exposed.contains("mcp"));
        assert!(!exposed.contains("agent"));
    }

    #[test]
    fn route_scoped_tools_for_local_inspection_prefer_structured_read_tools() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let route = IntentRouter::new().route("请帮我看看桌面有没有 gex 文件夹");
        let tools = fake_tools(&[
            "file_read",
            "file_write",
            "file_edit",
            "glob",
            "bash",
            "web_search",
            "memory_save",
            "mcp",
            "agent",
        ]);

        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("file_read"));
        assert!(exposed.contains("glob"));
        assert!(!exposed.contains("bash"));
        assert!(!exposed.contains("file_write"));
        assert!(!exposed.contains("file_edit"));
        assert!(!exposed.contains("web_search"));
        assert!(!exposed.contains("memory_save"));
        assert!(!exposed.contains("mcp"));
        assert!(!exposed.contains("agent"));
    }

    #[test]
    fn local_filesystem_inspection_route_is_distinct_from_terminal_route() {
        let local_route = IntentRouter::new().route("请帮我看看桌面有没有 gex 文件夹");
        let terminal_route =
            IntentRouter::new().route("帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧");

        assert!(is_local_filesystem_inspection_route(&local_route));
        assert!(!is_local_filesystem_inspection_route(&terminal_route));
    }

    #[test]
    fn route_scoped_tools_for_terminal_operation_include_bash_without_write_tools() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let route =
            IntentRouter::new().route("帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧");
        let tools = fake_tools(&[
            "file_read",
            "file_write",
            "file_edit",
            "glob",
            "bash",
            "web_search",
            "memory_save",
            "mcp",
            "agent",
        ]);

        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("bash"));
        assert!(exposed.contains("file_read"));
        assert!(exposed.contains("glob"));
        assert!(!exposed.contains("file_write"));
        assert!(!exposed.contains("file_edit"));
        assert!(!exposed.contains("web_search"));
        assert!(!exposed.contains("memory_save"));
        assert!(!exposed.contains("mcp"));
        assert!(!exposed.contains("agent"));
    }

    #[test]
    fn route_scoped_tools_for_python_creation_include_write_and_validation() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
        let tools = fake_tools(&[
            "project_list",
            "grep",
            "file_read",
            "file_write",
            "file_edit",
            "bash",
            "diff",
            "web_search",
            "memory_save",
            "mcp",
        ]);

        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("project_list"));
        assert!(exposed.contains("grep"));
        assert!(exposed.contains("file_read"));
        assert!(exposed.contains("file_write"));
        assert!(exposed.contains("file_edit"));
        assert!(exposed.contains("bash"));
        assert!(exposed.contains("diff"));
        assert!(!exposed.contains("web_search"));
        assert!(!exposed.contains("memory_save"));
        assert!(!exposed.contains("mcp"));
    }

    #[test]
    fn route_scoped_tools_for_debugging_include_search_read_shell_and_edit() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let route = IntentRouter::new().route("cargo test 报错了，帮我修一下");
        let tools = fake_tools(&[
            "project_list",
            "grep",
            "file_read",
            "file_write",
            "file_edit",
            "bash",
            "lsp",
            "symbol_query",
            "web_search",
            "memory_load",
        ]);

        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("grep"));
        assert!(exposed.contains("file_read"));
        assert!(exposed.contains("file_write"));
        assert!(exposed.contains("file_edit"));
        assert!(exposed.contains("bash"));
        assert!(exposed.contains("lsp"));
        assert!(exposed.contains("symbol_query"));
        assert!(!exposed.contains("web_search"));
        assert!(!exposed.contains("memory_load"));
    }

    #[test]
    fn route_scoped_tools_hide_skill_tools_without_skill_relevance() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
        let tools = fake_tools(&[
            "project_list",
            "grep",
            "file_read",
            "file_write",
            "file_edit",
            "bash",
            "skills_list",
            "skill_view",
            "skill_manage",
        ]);

        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("file_write"));
        assert!(exposed.contains("file_edit"));
        assert!(!exposed.contains("skills_list"));
        assert!(!exposed.contains("skill_view"));
        assert!(!exposed.contains("skill_manage"));
    }

    #[test]
    fn runtime_diet_sample_prompts_stay_within_route_tool_budgets() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.remove("PRIORITY_AGENT_TOOL_PROFILE");

        struct Sample {
            label: &'static str,
            prompt: &'static str,
            intent: IntentKind,
            workflow: WorkflowKind,
            max_tools: usize,
        }

        let samples = [
            Sample {
                label: "direct answer",
                prompt: "简单回答：2+2 等于几？",
                intent: IntentKind::DirectAnswer,
                workflow: WorkflowKind::Direct,
                max_tools: 0,
            },
            Sample {
                label: "scoped file delete",
                prompt: "帮我把这个文件删了吧",
                intent: IntentKind::DirectAnswer,
                workflow: WorkflowKind::Direct,
                max_tools: 4,
            },
            Sample {
                label: "local inspection",
                prompt: "请帮我看看桌面有没有 gex 文件夹",
                intent: IntentKind::DirectAnswer,
                workflow: WorkflowKind::Direct,
                max_tools: 4,
            },
            Sample {
                label: "terminal operation",
                prompt: "帮我看看默认 python 有没有安装 pygame，帮我安装一下吧",
                intent: IntentKind::DirectAnswer,
                workflow: WorkflowKind::Direct,
                max_tools: 4,
            },
            Sample {
                label: "python code creation",
                prompt: "帮我做一个贪吃蛇游戏吧，用 python 做吧",
                intent: IntentKind::CodeChange,
                workflow: WorkflowKind::CodeChange,
                max_tools: 12,
            },
            Sample {
                label: "running issue debug",
                prompt: "我在运行中发现了一个问题，你帮我看看是怎么回事吧",
                intent: IntentKind::Debugging,
                workflow: WorkflowKind::BugFix,
                max_tools: 12,
            },
            Sample {
                label: "reference comparison",
                prompt: "帮我对比 claude 和 opencode 的 agent 指令设计",
                intent: IntentKind::Research,
                workflow: WorkflowKind::Research,
                max_tools: 6,
            },
        ];

        let router = IntentRouter::new();
        let tools = runtime_diet_tool_universe();
        for sample in samples {
            let route = router.route(sample.prompt);
            assert_eq!(
                route.intent, sample.intent,
                "runtime diet sample '{}' routed to unexpected intent: {:?}; reason={}",
                sample.label, route.intent, route.reason
            );
            assert_eq!(
                route.workflow, sample.workflow,
                "runtime diet sample '{}' routed to unexpected workflow: {:?}; reason={}",
                sample.label, route.workflow, route.reason
            );

            let exposed = sorted_tool_names(&ConversationLoop::route_scoped_tools(&tools, &route));
            assert!(
                exposed.len() <= sample.max_tools,
                "runtime diet sample '{}' exposed {} tools, budget {}; route={}; reason={}; exposed={:?}",
                sample.label,
                exposed.len(),
                sample.max_tools,
                route.compact_label(),
                route.reason,
                exposed
            );
        }
    }

    #[test]
    fn route_scoped_tools_can_be_disabled_for_full_or_debug_exposure() {
        let route = IntentRouter::new().route("帮我做一个贪吃蛇游戏吧，用 python 做吧");
        let tools = fake_tools(&[
            "file_read",
            "file_write",
            "bash",
            "web_search",
            "memory_save",
        ]);

        let mut env = EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_TOOL_PROFILE", "full");
        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("web_search"));
        assert!(exposed.contains("memory_save"));

        env.remove("PRIORITY_AGENT_TOOL_PROFILE");
        env.set("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE", "1");
        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("web_search"));
        assert!(exposed.contains("memory_save"));

        env.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        env.set("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS", "0");
        let exposed = exposed_names(&ConversationLoop::route_scoped_tools(&tools, &route));
        assert!(exposed.contains("web_search"));
        assert!(exposed.contains("memory_save"));
    }

    #[test]
    fn test_not_allowed_tool_result_has_recovery_metadata() {
        let tool_call = ToolCall {
            id: "call_denied".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "echo hi"}),
        };
        let result = tool_not_allowed_result(&tool_call);
        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("not allowed"));
        let data = result.data.expect("tool summary data");
        assert_eq!(data["tool_summary"]["tool"], "bash");
        assert_eq!(data["tool_summary"]["call_id"], "call_denied");
    }

    #[test]
    fn test_tool_recovery_metadata_attached_to_failure() {
        let mut result = ToolResult::error("command timed out");
        let tool_call = ToolCall {
            id: "call_bash".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cargo test -q"
            }),
        };
        attach_tool_execution_metadata(&tool_call, &mut result);
        assert_eq!(result.content, "command timed out");
        let summary = result
            .data
            .as_ref()
            .and_then(|data| data.get("tool_summary"))
            .expect("tool summary metadata");
        assert_eq!(summary["tool"], "bash");
        assert_eq!(summary["command_kind"], "validation");
        assert_eq!(summary["validation_family"], "cargo_test");
        assert_eq!(summary["safe_for_closeout"], true);
        let recovery = result
            .data
            .as_ref()
            .and_then(|data| data.get("recovery"))
            .expect("recovery metadata");
        assert_eq!(recovery["recoverable"], true);
        assert_eq!(recovery["safe_retry"], true);
        assert_eq!(recovery["suggested_command"], "/retry");
    }

    #[test]
    fn test_tool_summary_metadata_attached_to_success() {
        let mut result = ToolResult::success_with_data(
            "File edited successfully",
            serde_json::json!({
                "path": "src/lib.rs",
                "replacements": 1
            }),
        );
        let tool_call = ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "old",
                "new_string": "new"
            }),
        };
        attach_tool_execution_metadata(&tool_call, &mut result);
        let summary = result
            .data
            .as_ref()
            .and_then(|data| data.get("tool_summary"))
            .expect("tool summary metadata");
        assert_eq!(summary["tool"], "file_edit");
        assert_eq!(summary["path"], "src/lib.rs");
        assert_eq!(summary["replacements"], 1);
        assert!(result
            .data
            .as_ref()
            .and_then(|data| data.get("recovery"))
            .is_none());
    }

    #[test]
    fn test_tool_execution_start_progress_uses_validation_labels() {
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "cargo test -q -- --test-threads=1"})
            ),
            "Running Rust tests: cargo test -q -- --test-threads=1"
        );
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "env PRIORITY_AGENT=1 cargo check -q"})
            ),
            "Running cargo check: env PRIORITY_AGENT=1 cargo check -q"
        );
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "cargo clippy -q -- -D warnings"})
            ),
            "Running cargo clippy: cargo clippy -q -- -D warnings"
        );
    }

    #[test]
    fn test_tool_execution_start_progress_handles_generic_shell_and_tools() {
        assert_eq!(
            tool_execution_start_progress("bash", &serde_json::json!({"command": "ls src"})),
            "Inspecting with shell: ls src"
        );
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "python scripts/update.py"})
            ),
            "Executing shell command: python scripts/update.py"
        );
        assert_eq!(
            tool_execution_start_progress("grep", &serde_json::json!({"pattern": "Closeout"})),
            "Executing grep..."
        );
    }

    #[test]
    fn test_strip_think_blocks_removes_internal_reasoning() {
        let input = "你好<think>内部推理</think>世界";
        assert_eq!(strip_think_blocks(input), "你好世界");
    }

    #[test]
    fn test_visible_text_sanitizer_handles_split_think_tags() {
        let mut sanitizer = VisibleTextSanitizer::default();
        let mut out = String::new();
        out.push_str(&sanitizer.push_chunk("你好<th"));
        out.push_str(&sanitizer.push_chunk("ink>不该显示</th"));
        out.push_str(&sanitizer.push_chunk("ink>世界"));
        out.push_str(&sanitizer.finish());
        assert_eq!(out, "你好世界");
    }

    #[test]
    fn test_visible_text_sanitizer_preserves_utf8_chunks_without_panicking() {
        let mut sanitizer = VisibleTextSanitizer::default();
        let mut out = String::new();
        out.push_str(&sanitizer.push_chunk("你"));
        out.push_str(&sanitizer.push_chunk("好"));
        out.push_str(&sanitizer.finish());
        assert_eq!(out, "你好");
    }

    #[tokio::test]
    async fn test_truncate_tool_result_keeps_small_output_unchanged() {
        let original = "short output".to_string();
        let mut result = ToolResult::success(original.clone());
        truncate_tool_result(&mut result, "grep", "call_small").await;
        assert_eq!(result.content, original);
    }

    #[tokio::test]
    async fn test_truncate_tool_result_includes_head_and_tail_markers() {
        let mut result = ToolResult::success(format!(
            "{}\n{}\n{}",
            "A".repeat(40_000),
            "中".repeat(8_000),
            "Z".repeat(40_000)
        ));
        truncate_tool_result(&mut result, "grep", "call_markers").await;
        assert!(result.content.contains("--- First"));
        assert!(result.content.contains("--- Last"));
        assert!(result.content.contains("Output truncated"));
    }

    #[test]
    fn test_normalize_params_fills_missing_required_fields() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "运行 cargo test 验证修复",
            Some("bash".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "timeout": { "type": "integer" }
            },
            "required": ["command", "timeout"]
        });

        let out = WorkflowRealStepExecutor::normalize_params(serde_json::json!({}), &schema, &step)
            .expect("normalize should succeed");
        assert_eq!(out["command"], "cargo test");
        assert!(out["timeout"].is_number());
    }

    #[test]
    fn test_normalize_params_coerces_required_field_types() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "在 src/main.rs 中搜索 TODO",
            Some("grep".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "path": { "type": "string" },
                "limit": { "type": "integer" },
                "recursive": { "type": "boolean" }
            },
            "required": ["pattern", "path", "limit", "recursive"]
        });

        let out = WorkflowRealStepExecutor::normalize_params(
            serde_json::json!({
                "pattern": 123,
                "path": true,
                "limit": "20",
                "recursive": "yes"
            }),
            &schema,
            &step,
        )
        .expect("normalize should succeed");

        assert_eq!(out["pattern"], "123");
        assert_eq!(out["path"], "true");
        assert_eq!(out["limit"], 20);
        assert_eq!(out["recursive"], true);
    }

    #[test]
    fn test_normalize_params_rejects_non_object_payload() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "读取 README.md",
            Some("file_read".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        });
        let err = WorkflowRealStepExecutor::normalize_params(
            serde_json::json!(["not", "object"]),
            &schema,
            &step,
        )
        .expect_err("non-object params should be rejected");
        assert!(err.contains("JSON object"));
    }

    #[test]
    fn test_get_tools_filters_denied_tools_before_model_request() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(BashTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        )
        .with_session_permission_rules(crate::permissions::PermissionRules::new().deny("bash"));

        let names = loop_instance
            .get_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"file_read".to_string()));
        assert!(!names.contains(&"bash".to_string()));
    }

    #[test]
    fn test_get_tools_hides_write_tools_in_read_only_mode() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(FileWriteTool);
        registry.register(BashTool);
        registry.register(GitTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        )
        .with_permission_mode(crate::permissions::PermissionMode::ReadOnly);

        let names = loop_instance
            .get_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"file_read".to_string()));
        assert!(!names.contains(&"file_write".to_string()));
        assert!(!names.contains(&"bash".to_string()));
        assert!(!names.contains(&"git".to_string()));
    }

    #[test]
    fn test_action_checkpoint_allows_patch_bash_but_blocks_read_only_bash() {
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "python3 - <<'PY'\nfrom pathlib import Path\nPath('x').write_text('y')\nPY"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "sed -n '1,20p' src/main.rs"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cargo test -q"}),
            false,
        ));
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cargo test -q"}),
            true,
        ));
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke"}),
            true,
        ));
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "bash -n scripts/run_live_eval.sh"}),
            true,
        ));
    }

    #[test]
    fn test_code_action_tools_keep_bash_visible_but_guarded() {
        let tools = vec![
            crate::services::api::Tool {
                name: "file_edit".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            crate::services::api::Tool {
                name: "file_read".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            crate::services::api::Tool {
                name: "grep".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            crate::services::api::Tool {
                name: "bash".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
        ];

        let before_change = ConversationLoop::code_action_tools(&tools, false, true)
            .into_iter()
            .map(|tool| tool.name)
            .collect::<HashSet<_>>();
        assert!(before_change.contains("file_edit"));
        assert!(before_change.contains("file_read"));
        assert!(before_change.contains("grep"));
        assert!(before_change.contains("bash"));

        let after_change = ConversationLoop::code_action_tools(&tools, true, true)
            .into_iter()
            .map(|tool| tool.name)
            .collect::<HashSet<_>>();
        assert!(after_change.contains("bash"));

        let after_lookup = ConversationLoop::code_action_tools(&tools, false, false)
            .into_iter()
            .map(|tool| tool.name)
            .collect::<HashSet<_>>();
        assert!(after_lookup.contains("file_edit"));
        assert!(after_lookup.contains("bash"));
        assert!(!after_lookup.contains("file_read"));
        assert!(!after_lookup.contains("grep"));
    }

    #[test]
    fn focused_repair_prompt_allows_one_targeted_read_without_broad_tools() {
        let exposed = vec![
            "file_edit".to_string(),
            "file_read".to_string(),
            "grep".to_string(),
        ];

        let prompt = ConversationLoop::focused_repair_mode_prompt(&exposed, 0);

        assert!(prompt.contains("Up to 2 targeted file_read/grep lookups remain"));
        assert!(prompt.contains("Do not call glob/project_list"));
        assert!(prompt.contains("bash only for a mutating patch command"));
        assert!(!prompt.contains("Do not call grep/glob/file_read/project_list"));

        let prompt_after_one_lookup = ConversationLoop::focused_repair_mode_prompt(&exposed, 1);
        assert!(prompt_after_one_lookup.contains("One targeted file_read/grep lookup remains"));

        let prompt_after_budget = ConversationLoop::focused_repair_mode_prompt(&exposed, 2);
        assert!(prompt_after_budget.contains("targeted lookup budget has already been used"));
        assert!(prompt_after_budget.contains("do not call file_read/grep again"));
    }

    #[test]
    fn file_edit_failure_correction_prefers_line_range_retry() {
        let correction = ConversationLoop::file_edit_failure_repair_correction(&[r#"
file_edit call_1 failed:
Expected 1 occurrence(s) of old_string, but found 1487.
  ... showing first 12 of 1487 matches. The old_string is too broad.
"#
        .to_string()])
        .expect("ambiguous file_edit should produce a correction");

        assert!(correction.contains("line_start, line_end"));
        assert!(correction.contains("Do not retry the same broad old_string"));
        assert!(correction.contains("not close out"));
    }

    #[test]
    fn file_edit_failure_correction_gets_one_model_retry_before_synthesis() {
        assert!(
            ConversationLoop::should_retry_after_file_edit_failure_correction(
                true, true, false, false,
            )
        );
        assert!(
            !ConversationLoop::should_retry_after_file_edit_failure_correction(
                true, true, true, false,
            )
        );
        assert!(
            !ConversationLoop::should_retry_after_file_edit_failure_correction(
                true, true, false, true,
            )
        );
        assert!(
            !ConversationLoop::should_retry_after_file_edit_failure_correction(
                false, true, false, false,
            )
        );
    }

    #[test]
    fn action_checkpoint_unexposed_tool_message_lists_allowed_tools() {
        let exposed = HashSet::from([
            "file_edit".to_string(),
            "file_read".to_string(),
            "grep".to_string(),
        ]);

        let message =
            ConversationLoop::action_checkpoint_unexposed_tool_message("project_list", &exposed, 0);

        assert!(message.contains("project_list"));
        assert!(message.contains("Exposed tools: file_edit, file_read, grep"));
        assert!(message.contains("Use file_edit/file_write or a mutating bash command"));
        assert!(message.contains("lookup budget still has room"));
        assert!(message.contains("Up to 2 targeted file_read/grep lookups remain"));

        let exhausted =
            ConversationLoop::action_checkpoint_unexposed_tool_message("file_read", &exposed, 2);
        assert!(exhausted.contains("targeted lookup budget has already been used"));
    }

    #[test]
    fn test_patch_synthesis_is_default_on_with_opt_out() {
        let mut guard = EnvVarGuard::acquire_blocking();
        guard.remove("PRIORITY_AGENT_PATCH_SYNTHESIS");
        assert!(ConversationLoop::patch_synthesis_enabled());

        guard.set("PRIORITY_AGENT_PATCH_SYNTHESIS", "0");
        assert!(!ConversationLoop::patch_synthesis_enabled());
    }

    #[test]
    fn test_verification_source_context_includes_current_error_line() {
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "fn demo() {\n    let score = 1;\n    let status = missing_value;\n}\n",
        )
        .expect("write source");
        let results = vec![super::super::auto_verify::VerificationResult {
            language: "rust".to_string(),
            command: "cargo check".to_string(),
            success: false,
            issues: vec![super::super::auto_verify::VerificationIssue {
                severity: "error".to_string(),
                file: Some("src/lib.rs".to_string()),
                line: Some(3),
                message: "cannot find value `missing_value` in this scope".to_string(),
            }],
            raw_output: String::new(),
            summary: String::new(),
        }];

        let context = verification_source_context(tmp.path(), &results)
            .expect("verification context should be generated");

        assert!(context.contains("src/lib.rs:3"));
        assert!(context.contains(">    3 |     let status = missing_value;"));
        assert!(context.contains("repair compile/validation errors"));
    }

    #[test]
    fn test_parse_patch_synthesis_plan_from_fenced_json() {
        let content = r#"```json
{"can_patch":true,"reason":"safe","actions":[{"tool":"file_edit","path":"src/lib.rs","old_string":"a","new_string":"b","expected_replacements":1}]}
```"#;
        let plan = ConversationLoop::parse_patch_synthesis_plan(content)
            .expect("fenced JSON should parse");
        assert!(plan.can_patch);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].path, "src/lib.rs");
    }

    #[test]
    fn test_patch_synthesis_validation_rejects_parent_traversal() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "../outside.rs".to_string(),
            old_string: Some("a".to_string()),
            new_string: "b".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };
        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("parent traversal must be rejected");
        assert!(err.to_string().contains("parent traversal"));
    }

    #[test]
    fn test_patch_synthesis_line_range_ignores_extra_old_string_for_shell_script() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
        )
        .expect("write script");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: Some("summary_task() {".to_string()),
            new_string: "summary_task() {\n  echo \"# Live Eval Summary: ${RUN_ID}\" >\"$summary\"\n  return 0\n}\n".to_string(),
            line_start: Some(1),
            line_end: Some(4),
            expected_replacements: Some(1),
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("line-range shell patch should be accepted");

        assert_eq!(call.arguments["path"], "scripts/run_live_eval.sh");
        assert_eq!(call.arguments["line_start"], 1);
        assert_eq!(call.arguments["line_end"], 4);
        assert!(call.arguments["old_string"].is_null());
    }

    #[test]
    fn test_patch_synthesis_accepts_function_sized_shell_line_range() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        let source = (0..70)
            .map(|idx| format!("  echo line-{idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            format!("summary_task() {{\n{source}\n}}\n"),
        )
        .expect("write script");
        let replacement = (0..70)
            .map(|idx| format!("  printf '%s\\n' item-{idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: None,
            new_string: format!("summary_task() {{\n{replacement}\n}}\n"),
            line_start: Some(1),
            line_end: Some(72),
            expected_replacements: None,
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("function-sized shell replacement should be accepted");

        assert_eq!(call.arguments["line_start"], 1);
        assert_eq!(call.arguments["line_end"], 72);
    }

    #[test]
    fn test_patch_synthesis_rejects_shell_line_range_crossing_next_function() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            "summary_task() {\n  echo stub\n}\n\nrun_one() {\n  echo next\n}\n",
        )
        .expect("write script");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: None,
            new_string: "summary_task() {\n  echo ok\n}\n".to_string(),
            line_start: Some(1),
            line_end: Some(6),
            expected_replacements: None,
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("cross-function shell replacement should be rejected");

        assert!(err.to_string().contains("crosses function boundary"));
    }

    #[test]
    fn test_patch_synthesis_recovers_shell_function_anchor_from_highlighted_old_string() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n\nrun_one() {\n  echo next\n}\n",
        )
        .expect("write script");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: Some(
                "1359: **summary_task**() {\n  echo \"summary mode is not implemented yet\"\n}"
                    .to_string(),
            ),
            new_string: "summary_task() {\n  echo ok\n}\n".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("highlighted shell function anchor should recover safely");

        assert!(call.arguments["old_string"]
            .as_str()
            .unwrap_or_default()
            .contains("summary mode is not implemented yet"));
        assert!(!call.arguments["old_string"]
            .as_str()
            .unwrap_or_default()
            .contains("run_one()"));
    }

    #[test]
    fn test_patch_synthesis_rejects_bare_live_eval_parser_import_in_shell_heredoc() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
        )
        .expect("write script");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: None,
            new_string: r#"summary_task() {
python3 - <<'PY'
import pathlib
import sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from live_eval_report_parser import report_rows
PY
}
"#
            .to_string(),
            line_start: Some(1),
            line_end: Some(4),
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("bare live_eval_report_parser import should be rejected");

        assert!(err
            .to_string()
            .contains("Python heredocs execute from stdin"));
    }

    #[test]
    fn test_patch_synthesis_rejects_markdown_highlight_in_shell_patch() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
        )
        .expect("write script");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: None,
            new_string: "**summary_task()** {\n  echo ok\n}\n".to_string(),
            line_start: Some(1),
            line_end: Some(4),
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("markdown highlighting should be rejected");

        assert!(err.to_string().contains("Markdown emphasis markers"));
    }

    #[test]
    fn test_patch_synthesis_accepts_scripts_package_import_in_shell_heredoc() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            "summary_task() {\n  echo \"summary mode is not implemented yet\" >&2\n  return 2\n}\n",
        )
        .expect("write script");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: None,
            new_string: r#"summary_task() {
python3 - <<'PY'
from scripts.live_eval_report_parser import report_rows
PY
}
"#
            .to_string(),
            line_start: Some(1),
            line_end: Some(4),
            expected_replacements: Some(1),
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("package import should be accepted");

        assert_eq!(call.arguments["path"], "scripts/run_live_eval.sh");
    }

    #[test]
    fn test_patch_synthesis_path_resolves_root_relative_src_path() {
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
        std::fs::write(tmp.path().join("src/lib.rs"), "fn main() {}\n").expect("write file");

        let (canonical, tool_path) = ConversationLoop::resolve_synthesized_patch_path(
            std::path::Path::new("/src/lib.rs"),
            tmp.path(),
        )
        .expect("root-relative src path should resolve inside cwd");

        assert_eq!(
            canonical,
            tmp.path().join("src/lib.rs").canonicalize().unwrap()
        );
        assert_eq!(tool_path, "src/lib.rs");
    }

    #[test]
    fn test_patch_synthesis_recovers_wrong_path_from_unique_old_string() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/assessment.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("unique old_string should recover the real file path");

        assert_eq!(call.arguments["path"], "src/memory/quality.rs");
    }

    #[test]
    fn test_patch_synthesis_keeps_failed_compiler_evidence() {
        let messages = vec![Message::tool(
            "cargo_check",
            "Result: ERROR\nerror[E0596]: cannot borrow `self.memory_manager.0` as mutable\n[exit status: 101]",
        )];

        let evidence = ConversationLoop::patch_synthesis_evidence(&messages);

        assert!(evidence.contains("FAILED TOOL RESULT"));
        assert!(evidence.contains("error[E0596]"));
    }

    #[test]
    fn test_patch_synthesis_large_file_evidence_keeps_relevant_late_function() {
        let mut content = String::from("Result: OK\n");
        for idx in 0..600 {
            content.push_str(&format!("{idx:4} | echo filler_{idx}\n"));
        }
        content.push_str(
            "1359 | summary_task() {\n1360 |   echo \"summary mode is not implemented yet\" >&2\n1361 |   return 2\n1362 | }\n",
        );
        for idx in 601..900 {
            content.push_str(&format!("{idx:4} | echo tail_{idx}\n"));
        }
        let messages = vec![Message::tool("file_read", content)];

        let evidence = ConversationLoop::patch_synthesis_evidence(&messages);

        assert!(evidence.contains("summary_task()"));
        assert!(evidence.contains("summary mode is not implemented yet"));
        assert!(evidence.contains("[relevant excerpt]"));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_ref_mut_e0596() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "if let Some(ref mut mem_mutex) = self.memory_manager {\n    let mut mem = mem_mutex.lock().await;\n}\n",
        )
        .expect("write module file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "error[E0596]: cannot borrow `self.memory_manager.0` as mutable, as it is behind a `&` reference",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].arguments["old_string"],
            "if let Some(ref mut mem_mutex) = self.memory_manager {"
        );
        assert_eq!(
            calls[0].arguments["new_string"],
            "if let Some(ref mem_mutex) = self.memory_manager {"
        );
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_persistent_memory_marker() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n        if let Some(ref ctx) = turn_retrieval_context {\n",
        )
        .expect("write module file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "the regression marker identifies the missing planning prefetch block",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("prefetch_retrieval_context_with_llm_rerank"));
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("if let Some(ref mem_mutex) = self.memory_manager"));
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains(".lock().await"));
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("&self.model"));
        assert!(!calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("futures::executor::block_on"));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_live_eval_summary_stub() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("scripts")).expect("create scripts dir");
        std::fs::write(
            tmp.path().join("scripts/run_live_eval.sh"),
            r###"summary_task() {
  local run_report_dir="$REPORT_DIR/live-$RUN_ID"
  local summary="$run_report_dir/summary.md"
  mkdir -p "$run_report_dir"
  echo "summary mode is not implemented yet" >&2
  echo "# Live Eval Summary: $RUN_ID" >"$summary"
  echo "" >>"$summary"
  echo "- status: not_implemented" >>"$summary"
  return 2
}

run_one() {
  echo next
}
"###,
        )
        .expect("write live eval script");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "TASK: live-eval-dashboard-summary requires summary_task to generate plan_quality, tool_boundary, and verification_status",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].arguments["path"], "scripts/run_live_eval.sh");
        assert_eq!(calls[0].arguments["line_start"], 1);
        assert_eq!(calls[0].arguments["line_end"], 10);
        let replacement = calls[0].arguments["new_string"].as_str().unwrap();
        assert!(replacement.contains("from scripts.live_eval_report_parser import report_rows"));
        assert!(replacement.contains("plan_quality"));
        assert!(replacement.contains("tool_boundary"));
        assert!(replacement.contains("verification_status"));
        assert!(!replacement.contains("summary mode is not implemented yet"));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_record_repair_action_arity() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        let damaged_call = concat!(
            r#"fn repair() {
                if !verify_passed {
                    let verification_command = failed_commands
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "post-edit verification".to_string());
                    post_edit_reflection.record_repair_action(
                  acceptance_repair_attempts + 1,
                  &format!("retry: {"#,
            r#"}", verification_command),
                  changed_files.first().map(|path| path.display().to_string()),
              );
                }
}
"#
        );
        std::fs::write(
            tmp.path()
                .join("src/engine/conversation_loop/repair_controller.rs"),
            damaged_call,
        )
        .expect("write repair controller file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "error[E0061]: this method takes 4 arguments but 3 arguments were supplied\nargument #4 is missing\nrecord_repair_action",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].arguments["path"],
            "src/engine/conversation_loop/repair_controller.rs"
        );
        assert_eq!(calls[0].arguments["line_start"], 7);
        assert_eq!(calls[0].arguments["line_end"], 11);
        let replacement = calls[0].arguments["new_string"].as_str().unwrap();
        assert!(replacement.contains("context.acceptance_repair_attempts + 1"));
        assert!(replacement.contains("\"repair failed verification before closeout\""));
        assert!(replacement.contains("verification_command,"));
        assert!(!replacement.contains(ConversationLoop::retry_format_marker().as_str()));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_skill_promotion_gate_apply_path() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/tui/slash_handler"))
            .expect("create slash handler dir");
        std::fs::write(
            tmp.path().join("src/tui/slash_handler/learning.rs"),
            r#"fn validate_skill_promotion_for_apply() {}
fn skill_fitness_from_bound_eval() {}
fn estimate_skill_semantic_drift() {}

fn handle_apply() {
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                        );
                    }
                }
            }
}
"#,
        )
        .expect("write fixture file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "skill-promotion-gate required command failed because validate_skill_promotion_for_apply is not called before write_active_skill and EvolutionController cooldown is missing",
            tmp.path(),
        );

        assert_eq!(calls.len(), 2);
        let first = calls[0].arguments["new_string"].as_str().unwrap();
        assert!(first.contains(
            "validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())"
        ));
        assert!(first.contains("Skill proposal {} was not applied by promotion gate"));
        let second = calls[1].arguments["new_string"].as_str().unwrap();
        assert!(second.contains("record_evolution_update("));
        assert!(second.contains("EvolutionTarget::Skill"));
    }

    #[test]
    fn test_deterministic_patch_synthesis_uses_skill_task_preview_without_failed_evidence() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/tui/slash_handler"))
            .expect("create slash handler dir");
        std::fs::write(
            tmp.path().join("src/tui/slash_handler/learning.rs"),
            r#"fn validate_skill_promotion_for_apply() {}
fn skill_fitness_from_bound_eval() {}
fn estimate_skill_semantic_drift() {}

fn handle_apply() {
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                        );
                    }
                }
            }
}
"#,
        )
        .expect("write fixture file");

        let task_seed =
            "TASK:\n修复 /skill-proposals apply 没有强制使用 fitness promotion gate 的问题。";
        let calls = loop_instance.deterministic_patch_tool_calls(task_seed, tmp.path());

        assert_eq!(calls.len(), 2);
        assert!(calls[0].arguments["new_string"].as_str().unwrap().contains(
            "validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())"
        ));
        assert!(calls[1].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("record_evolution_update("));
    }

    #[test]
    fn test_deterministic_patch_synthesis_ignores_unrelated_memory_tool_mentions() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/tools/memory_tool"))
            .expect("create memory tool dir");
        std::fs::write(
            tmp.path().join("src/tools/memory_tool/mod.rs"),
            "let assessment = assess_memory_candidate(content, category, &existing, true);\n",
        )
        .expect("write fixture file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "resume-session-picker inspected /resume and saw memory_save while checking whether \
             restore_session flushes current memory before switching sessions",
            tmp.path(),
        );

        assert!(
            calls.is_empty(),
            "memory quality repair must not fire for unrelated resume tasks"
        );
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_memory_quality_gate() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/tools/memory_tool"))
            .expect("create memory tool dir");
        std::fs::write(
            tmp.path().join("src/tools/memory_tool/mod.rs"),
            "let assessment = assess_memory_candidate(content, category, &existing, true);\n",
        )
        .expect("write fixture file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "memory-save-quality-gate found that explicit memory_save bypasses the quality gate",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].arguments["path"], "src/tools/memory_tool/mod.rs");
        assert_eq!(
            calls[0].arguments["new_string"],
            "assess_memory_candidate(content, category, &existing, false)"
        );
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_memory_recall_conflict_precision() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine")).expect("create engine dir");
        std::fs::write(
            tmp.path().join("src/engine/retrieval_context.rs"),
            r#"fn memory_conflict_matches_item(
    conflict: &str,
    item: &crate::memory::manager::MemoryMatch,
) -> bool {
    let conflict = conflict.to_lowercase();
    let snippet = item.snippet.to_lowercase();
    if let Some((key, values)) = parse_memory_conflict(&conflict) {
        return snippet.contains(&key) && values.iter().any(|value| snippet.contains(value));
    }

    let tokens = conflict
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| {
            part.len() >= 4
                && !matches!(
                    *part,
                    "memory" | "project" | "user" | "value" | "values" | "conflicting"
                )
        })
        .collect::<Vec<_>>();
    tokens.len() >= 2
        && tokens
            .iter()
            .filter(|part| snippet.contains(**part))
            .count()
            >= 2
}

fn parse_memory_conflict(conflict: &str) -> Option<(String, Vec<String>)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_conflict_matching_uses_structured_key_and_value() {
        let conflict = "- key 'language' has conflicting values: chinese | english";
        let unrelated = crate::memory::manager::MemoryMatch {
            source: "memory/cli.md".to_string(),
            score: 30,
            rerank_score: Some(0.90),
            snippet: "The project memory mentions conflicting work before.".to_string(),
        };
        let related = crate::memory::manager::MemoryMatch {
            source: "memory/cli.md".to_string(),
            score: 30,
            rerank_score: Some(0.90),
            snippet: "language: Chinese\nUse compact CLI status bars.".to_string(),
        };

        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn items_are_sorted_by_score() {}
}
"#,
        )
        .expect("write fixture file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "TASK:\n强化记忆检索中的冲突匹配精度。memory-recall-conflict-precision",
            tmp.path(),
        );

        assert_eq!(calls.len(), 3);
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("is_generic_conflict_token(&key)"));
        assert!(calls[1].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("fn is_generic_conflict_token("));
        assert!(calls[2].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("memory_conflict_matching_ignores_generic_key_conflicts"));
    }

    #[test]
    fn test_patch_synthesis_rejects_bad_persistent_memory_async_shape() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(
                "        // Regression fixture: persistent memory prefetch was missing before workflow judgment."
                    .to_string(),
            ),
            new_string: r#"        if let Some(memory_ctx) = self
            .memory_manager
            .as_mut()
            .and_then(|m| {
                futures::executor::block_on(m.prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref(),
                    self.provider.as_ref().and_then(|p| p.preferred_model()).unwrap_or("default"),
                    route.retrieval,
                ))
            })
        {
            turn_retrieval_context = Some(memory_ctx);
        }"#
            .to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("bad async memory block should be rejected")
            .to_string();

        assert!(err.contains("block_on"));
    }

    #[test]
    fn test_patch_synthesis_rejects_provider_option_style_in_memory_prefetch() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(
                "        // Regression fixture: persistent memory prefetch was missing before workflow judgment."
                    .to_string(),
            ),
            new_string: r#"        if let Some(ref mem_mutex) = self.memory_manager {
            let mut mem = mem_mutex.lock().await;
            if let Some(mem_ctx) = mem
                .prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref().map(|p| p.as_ref()).unwrap(),
                    &self.model,
                    route.retrieval,
                )
                .await
            {
                turn_retrieval_context = Some(mem_ctx);
            }
        }"#
            .to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("provider option-style call should be rejected")
            .to_string();

        assert!(err.contains("Option"));
    }

    #[test]
    fn test_validation_tool_call_detects_success_gate_commands() {
        let cargo_test = ToolCall {
            id: "test".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cargo test -q -- --test-threads=1"
            }),
        };
        let ls = ToolCall {
            id: "ls".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "ls -la"
            }),
        };
        let file_read = ToolCall {
            id: "read".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({
                "path": "src/main.rs"
            }),
        };
        let python_assertion = ToolCall {
            id: "python".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "python3 -c \"assert True\""
            }),
        };
        let node_test = ToolCall {
            id: "node".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "node fixtures/live_frontend/book_notes/test-book-notes.cjs"
            }),
        };
        let python_unittest = ToolCall {
            id: "unittest".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py"
            }),
        };
        let rg_assertion = ToolCall {
            id: "rg".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "! rg 'bad_pattern' src/lib.rs"
            }),
        };
        let rg_assertion_with_ampersand_pattern = ToolCall {
            id: "rg_amp".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "! rg '&format!\\(\"retry: \\{\\}\", verification_command\\)' src/engine/conversation_loop/mod.rs"
            }),
        };
        let env_prefixed_cargo_test = ToolCall {
            id: "env_test".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
            }),
        };
        let shell_wrapped_cargo_test = ToolCall {
            id: "wrapped_test".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
            }),
        };

        assert!(ConversationLoop::is_validation_tool_call(&cargo_test));
        assert!(ConversationLoop::is_validation_tool_call(&python_assertion));
        assert!(ConversationLoop::is_validation_tool_call(&node_test));
        assert!(ConversationLoop::is_validation_tool_call(&python_unittest));
        assert!(ConversationLoop::is_validation_tool_call(&rg_assertion));
        assert!(ConversationLoop::is_validation_tool_call(
            &rg_assertion_with_ampersand_pattern
        ));
        assert!(ConversationLoop::is_validation_tool_call(
            &env_prefixed_cargo_test
        ));
        assert!(ConversationLoop::is_validation_tool_call(
            &shell_wrapped_cargo_test
        ));
        assert!(!ConversationLoop::is_validation_tool_call(&ls));
        assert!(!ConversationLoop::is_validation_tool_call(&file_read));
    }

    #[test]
    fn test_validation_command_match_normalizes_shell_lc_wrappers() {
        assert_eq!(
            ConversationLoop::normalize_validation_command_for_match(
                "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
            ),
            "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
        );
        assert_eq!(
            ConversationLoop::normalize_validation_command_for_match(
                "  env   PRIORITY_AGENT_WORKFLOW_ENABLED=1   cargo test --quiet -- --test-threads=1  "
            ),
            "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
        );
    }

    #[test]
    fn test_extract_required_validation_commands_from_live_eval_prompt() {
        let prompt = r#"
## Acceptance checks
- `env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1`
- `cargo test -q learning_planning -- --test-threads=1`
- `node fixtures/live_frontend/book_notes/test-book-notes.cjs`
- `python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py`
- `python3 -c "p='src/lib.rs'; assert True"`
- `! rg 'bad_pattern' src/lib.rs`
- `! rg '&format!\("retry: \{\}", verification_command\)' src/engine/conversation_loop/mod.rs`
- `rg 'good_pattern' src/lib.rs`
- `rm -rf /tmp/nope`
- `(none)`
"#;

        let commands = ConversationLoop::extract_required_validation_commands(prompt);

        assert_eq!(
            commands,
            vec![
                "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1".to_string(),
                "cargo test -q learning_planning -- --test-threads=1".to_string(),
                "node fixtures/live_frontend/book_notes/test-book-notes.cjs".to_string(),
                "python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py".to_string(),
                "python3 -c \"p='src/lib.rs'; assert True\"".to_string(),
                "! rg 'bad_pattern' src/lib.rs".to_string(),
                "! rg '&format!\\(\"retry: \\{\\}\", verification_command\\)' src/engine/conversation_loop/mod.rs".to_string(),
                "rg 'good_pattern' src/lib.rs".to_string()
            ]
        );
    }

    #[test]
    fn test_required_validation_disables_default_auto_tests() {
        assert!(should_run_default_auto_tests(&[]));
        assert!(!should_run_default_auto_tests(&[
            "cargo test -q -- --test-threads=1".to_string()
        ]));
    }

    #[test]
    fn test_patch_synthesis_recovers_assignment_anchor_when_old_string_is_inexact() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "fn assess() {\n    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n}\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some(
                "let status = if explicit { MemoryStatus::Accepted } else { write_decision.status };"
                    .to_string(),
            ),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("unique assignment anchor should recover exact old_string");

        assert_eq!(
            call.arguments["old_string"],
            "    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };"
        );
        assert_eq!(
            call.arguments["new_string"],
            "    let status = write_decision.status;"
        );
    }

    #[test]
    fn test_patch_synthesis_rejects_inexact_multiline_replacement() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "fn assess() {\n    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n}\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 {\n    MemoryStatus::Accepted\n} else {\n    write_decision.status\n};".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("inexact multiline replacement should be rejected");
        assert!(err.to_string().contains("inexact multi-line replacement"));
    }

    #[test]
    fn test_patch_synthesis_rejects_unbalanced_replacement() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 {".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("unbalanced replacement should be rejected");
        assert!(err.to_string().contains("unbalanced delimiters"));
    }

    #[test]
    fn test_patch_synthesis_rejects_score_based_memory_status_promotion() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("score-only accepted promotion should be rejected");
        assert!(err
            .to_string()
            .contains("preserve score_memory_write hard gates"));
    }

    #[test]
    fn test_patch_synthesis_rejects_unknown_enum_variant() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
        std::fs::write(
            tmp.path().join("src/types.rs"),
            "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n}\n",
        )
        .expect("write types");
        std::fs::write(
            tmp.path().join("src/quality.rs"),
            "let status = MemoryStatus::Accepted;\n",
        )
        .expect("write quality");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/quality.rs".to_string(),
            old_string: Some("let status = MemoryStatus::Accepted;".to_string()),
            new_string: "let status = MemoryStatus::Blocked;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("unknown enum variant should be rejected before editing");

        assert!(err.to_string().contains("MemoryStatus::Blocked"));
        assert!(err.to_string().contains("Accepted"));
    }

    #[test]
    fn test_patch_synthesis_rejects_memory_status_duplicate_extension() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        let old_enum = "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n}\n";
        std::fs::write(tmp.path().join("src/memory/types.rs"), old_enum).expect("write types");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/types.rs".to_string(),
            old_string: Some(old_enum.to_string()),
            new_string: "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n    Duplicate,\n    Demoted,\n}\n".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("duplicate/demote should use MemoryWriteOutcomeStatus");

        assert!(err.to_string().contains("MemoryWriteOutcomeStatus"));
    }

    #[tokio::test]
    async fn test_tool_specific_confirmation_blocks_git_push_without_approval() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(GitTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let route = crate::engine::intent_router::IntentRouter::new().route("push the branch");
        let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
        let destructive_scope =
            crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
                "push the branch",
                &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            );
        let tool_calls = vec![ToolCall {
            id: "git_push".to_string(),
            name: "git".to_string(),
            arguments: serde_json::json!({"action": "push"}),
        }];
        let exposed_tool_names = HashSet::from(["git".to_string()]);

        let results = loop_instance
            .execute_tools_parallel(
                &tool_calls,
                None,
                Default::default(),
                None,
                &policy,
                &exposed_tool_names,
                false,
                0,
                false,
                &destructive_scope,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].1.success);
        assert!(results[0]
            .1
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("requires user confirmation"));
    }

    #[tokio::test]
    async fn test_unexposed_tool_call_is_denied_before_execution() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(GitTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let route = crate::engine::intent_router::IntentRouter::new().route("push the branch");
        let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
        let destructive_scope =
            crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
                "push the branch",
                &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            );
        let tool_calls = vec![ToolCall {
            id: "git_push".to_string(),
            name: "git".to_string(),
            arguments: serde_json::json!({"action": "push"}),
        }];
        let exposed_tool_names = HashSet::from(["file_edit".to_string()]);

        let results = loop_instance
            .execute_tools_parallel(
                &tool_calls,
                None,
                Default::default(),
                None,
                &policy,
                &exposed_tool_names,
                false,
                0,
                false,
                &destructive_scope,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].1.success);
        assert!(results[0]
            .1
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("was not exposed"));
    }

    #[tokio::test]
    async fn destructive_scope_blocks_parent_delete_before_bash_execution() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(BashTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let route = crate::engine::intent_router::IntentRouter::new().route("删除 abc.txt");
        let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
        let destructive_scope =
            crate::engine::destructive_scope::DestructiveScopeContract::from_user_request(
                "删除 abc.txt",
                &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            );
        let tool_calls = vec![ToolCall {
            id: "rm_parent".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "rm -rf /tmp/gex"}),
        }];
        let exposed_tool_names = HashSet::from(["bash".to_string()]);

        let results = loop_instance
            .execute_tools_parallel(
                &tool_calls,
                None,
                Default::default(),
                None,
                &policy,
                &exposed_tool_names,
                false,
                0,
                false,
                &destructive_scope,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].1.success);
        assert!(results[0]
            .1
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Destructive scope blocked"));
    }

    #[test]
    fn test_action_checkpoint_rejects_multi_replacement_file_edit() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet status = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status",
                "new_string": "let checked_status",
                "expected_replacements": 2
            }),
            tmp.path(),
        )
        .expect("multi replacement edit should be rejected");

        assert!(rejection.contains("only permits one replacement"));
    }

    #[test]
    fn test_action_checkpoint_rejects_non_unique_anchor() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet status = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status",
                "new_string": "let checked_status"
            }),
            tmp.path(),
        )
        .expect("non-unique anchor should be rejected");

        assert!(rejection.contains("unique edit anchor"));
    }

    #[test]
    fn test_action_checkpoint_rejects_multi_line_range_edit() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let write_decision = score();\nlet score = write_decision.score;\nlet status = write_decision.status;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "line_start": 1,
                "line_end": 3,
                "new_string": "let status = write_decision.status;"
            }),
            tmp.path(),
        )
        .expect("multi-line action checkpoint edit should be rejected");

        assert!(rejection.contains("exactly one line"));
    }

    #[test]
    fn test_action_checkpoint_accepts_unique_anchor() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet other = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status = true;",
                "new_string": "let status = false;"
            }),
            tmp.path(),
        );

        assert!(rejection.is_none(), "{rejection:?}");
    }

    struct MockLlmProvider {
        responses: StdMutex<VecDeque<ChatResponse>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let mut guard = self.responses.lock().unwrap();
            guard
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("no mock response left"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    #[tokio::test]
    async fn runtime_diet_report_is_recorded_for_real_loop_turn() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::from(vec![ChatResponse {
                content: "hello".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 12,
                    completion_tokens: 3,
                    total_tokens: 15,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            }])),
        });
        let tool_registry = Arc::new(ToolRegistry::new());
        let cost_tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));
        let trace_store = Arc::new(TraceStore::default());
        let loop_instance =
            ConversationLoop::new(provider, tool_registry, cost_tracker, "test".into())
                .with_trace_store(trace_store.clone())
                .with_max_iterations(1);

        let result = loop_instance
            .run(vec![Message::user("请简单回复一句 hello")])
            .await
            .expect("loop should complete");

        assert_eq!(result.content, "hello");
        let trace = trace_store.latest().expect("trace should be recorded");
        let diet = trace.events.iter().find_map(|event| {
            if let TraceEvent::RuntimeDietReport {
                prompt_tokens,
                tool_schema_tokens,
                exposed_tools,
                memory_snapshot_tokens,
                retrieval_items,
                skill_list_tokens,
                workflow_context,
                validation_evidence,
                ..
            } = event
            {
                Some((
                    *prompt_tokens,
                    *tool_schema_tokens,
                    *exposed_tools,
                    *memory_snapshot_tokens,
                    *retrieval_items,
                    *skill_list_tokens,
                    workflow_context.as_str(),
                    validation_evidence.as_str(),
                ))
            } else {
                None
            }
        });
        let (
            prompt_tokens,
            tool_schema_tokens,
            exposed_tools,
            memory_snapshot_tokens,
            retrieval_items,
            skill_list_tokens,
            workflow_context,
            validation,
        ) = diet.expect("runtime diet event should be recorded");
        assert!(prompt_tokens > 0);
        assert_eq!(tool_schema_tokens, 0);
        assert_eq!(exposed_tools, 0);
        assert_eq!(memory_snapshot_tokens, 0);
        assert_eq!(retrieval_items, 0);
        assert_eq!(skill_list_tokens, 0);
        assert_eq!(workflow_context, "none");
        assert_eq!(validation, "none");
        assert!(crate::engine::trace::format_trace_summary(&trace, 80).contains("Runtime Diet:"));
    }

    #[tokio::test]
    async fn test_coding_quality_tracks_fail_then_repair_cycle() {
        let mut env = EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_REVIEW", "1");
        let tmp = tempdir().expect("create temp dir");
        let target_file = tmp.path().join("sample.rs");
        let target_path = target_file.to_string_lossy().to_string();

        let failing_code = "fn main() { let x = Some(1).unwrap(); let _ = x; }";
        let fixed_code = "fn main() { let x = Some(1); if let Some(v) = x { let _ = v; } }";

        let responses = VecDeque::from(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": failing_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_2".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": fixed_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
            ChatResponse {
                content: "repaired".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(FileWriteTool);
        let tool_registry = Arc::new(registry);
        let cost_tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_instance =
            ConversationLoop::new(provider, tool_registry, cost_tracker, "test".into())
                .with_max_iterations(5);

        let messages = vec![Message::user("write code and fix issues")];
        let result = loop_instance
            .run(messages)
            .await
            .expect("loop should succeed");

        assert!(
            result.iterations >= 2,
            "should iterate at least twice for write+fix"
        );
    }
}

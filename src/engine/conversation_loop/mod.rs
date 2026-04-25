//! 统一对话循环
//!
//! 将 QueryEngine 和 StreamingEngineInner 中重复的工具调用循环合并为一处。
//! 支持流式/非流式两种输出模式，内部逻辑完全一致。
//!
//! 改进（借鉴 hermes-agent）：
//! - 前置压缩（Preflight）：循环前检查总 token，超阈值提前压缩
//! - IterationBudget：迭代预算退还机制（只读工具可退还）

mod approval;
mod step_executor;
mod tool_execution;

pub use approval::{ToolApprovalChannel, ToolApprovalRequest};
pub(crate) use step_executor::{is_drift_interruption_signal, WorkflowRealStepExecutor};
pub(crate) use tool_execution::{
    is_read_only, read_only_tool_concurrency, safe_prefix_by_bytes, truncate_tool_result,
    READ_ONLY_TOOLS,
};

use crate::engine::intent_router::IntentRouter;
use crate::engine::trace::{TraceCollector, TraceEvent, TraceStore, TurnStatus, TurnTrace};
use crate::engine::workflow::{Gate, WorkflowEngine, WorkflowPolicy};
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Message, ToolCall};
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use anyhow::Result;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, warn};

use super::context_compressor::{
    estimate_messages_tokens, estimate_tool_schemas_tokens, ContextCompressor,
};
use super::hooks::{HookDecision, ToolHookManager};
use super::streaming::StreamEvent;

const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Default)]
struct VisibleTextSanitizer {
    buffer: String,
    in_think_block: bool,
}

impl VisibleTextSanitizer {
    fn push_chunk(&mut self, chunk: &str) -> String {
        self.buffer.push_str(chunk);
        self.drain_visible(false)
    }

    fn finish(&mut self) -> String {
        self.drain_visible(true)
    }

    fn drain_visible(&mut self, flush_all: bool) -> String {
        let mut out = String::new();
        loop {
            if self.in_think_block {
                if let Some(end_idx) = self.buffer.find(THINK_CLOSE_TAG) {
                    let drain_len = end_idx + THINK_CLOSE_TAG.len();
                    self.buffer.drain(..drain_len);
                    self.in_think_block = false;
                    continue;
                }

                if flush_all {
                    self.buffer.clear();
                } else {
                    let keep = THINK_CLOSE_TAG.len().saturating_sub(1);
                    if self.buffer.len() > keep {
                        let drain_len = floor_char_boundary(&self.buffer, self.buffer.len() - keep);
                        self.buffer.drain(..drain_len);
                    }
                }
                break;
            }

            if let Some(start_idx) = self.buffer.find(THINK_OPEN_TAG) {
                out.push_str(&self.buffer[..start_idx]);
                let drain_len = start_idx + THINK_OPEN_TAG.len();
                self.buffer.drain(..drain_len);
                self.in_think_block = true;
                continue;
            }

            if flush_all {
                out.push_str(&self.buffer);
                self.buffer.clear();
            } else {
                let keep = THINK_OPEN_TAG.len().saturating_sub(1);
                if self.buffer.len() > keep {
                    let emit_len = floor_char_boundary(&self.buffer, self.buffer.len() - keep);
                    out.push_str(&self.buffer[..emit_len]);
                    self.buffer.drain(..emit_len);
                }
            }
            break;
        }

        out
    }
}

fn strip_think_blocks(text: &str) -> String {
    let mut sanitizer = VisibleTextSanitizer::default();
    let mut visible = sanitizer.push_chunk(text);
    visible.push_str(&sanitizer.finish());
    visible
}

async fn emit_usage_event(response: &ChatResponse, tx: &mpsc::Sender<StreamEvent>) {
    if let Some(usage) = &response.usage {
        let _ = tx
            .send(StreamEvent::Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                cached_tokens: usage.cached_tokens,
            })
            .await;
    }
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn tool_call_fingerprint(tc: &ToolCall) -> String {
    let args = serde_json::to_string(&tc.arguments).unwrap_or_else(|_| "null".to_string());
    format!("{}|{}", tc.name, args)
}

fn persist_turn_learning_event(
    store: &crate::session_store::SessionStore,
    trace: &crate::engine::trace::TurnTrace,
) -> rusqlite::Result<i64> {
    let intent = trace.events.iter().find_map(|event| match event {
        TraceEvent::IntentRouted { intent, .. } => Some(intent.as_str()),
        _ => None,
    });
    let goal = trace.events.iter().find_map(|event| match event {
        TraceEvent::SessionGoalUpdated { title, .. } => Some(title.as_str()),
        _ => None,
    });
    let tool_count = trace
        .events
        .iter()
        .filter(|event| matches!(event, TraceEvent::ToolCompleted { .. }))
        .count();
    let summary = match (goal, intent) {
        (Some(goal), Some(intent)) => format!("Turn {:?}: {} ({})", trace.status, goal, intent),
        (Some(goal), None) => format!("Turn {:?}: {}", trace.status, goal),
        (None, Some(intent)) => format!("Turn {:?}: intent {}", trace.status, intent),
        (None, None) => format!("Turn {:?}: no routed intent", trace.status),
    };
    let payload = serde_json::json!({
        "trace_id": trace.trace_id,
        "turn_index": trace.turn_index,
        "status": format!("{:?}", trace.status),
        "intent": intent,
        "goal": goal,
        "tool_count": tool_count,
        "event_count": trace.events.len(),
        "duration_ms": trace.duration_ms(),
    });
    let confidence = if trace.status == TurnStatus::Completed {
        1.0
    } else {
        0.45
    };
    store.add_learning_event(
        &trace.session_id,
        "turn_outcome",
        "conversation_loop",
        &summary,
        confidence,
        &payload,
    )
}

fn record_recovery_plan(trace: &TraceCollector, plan: &crate::engine::recovery_plan::RecoveryPlan) {
    trace.record(TraceEvent::RecoveryPlan {
        plan_id: plan.id.clone(),
        source: plan.source.clone(),
        category: plan.category.clone(),
        action: plan.action.clone(),
        retryable: plan.retryable,
        safe_retry: plan.safe_retry,
        suggested_command: plan.suggested_command.clone(),
        status: format!("{:?}", plan.status),
    });
    trace.record(TraceEvent::RecoveryApplied {
        error: plan.primary_error.clone(),
        action: plan.trace_action(),
    });
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
            permission_mode: crate::permissions::PermissionMode::AutoLowRisk,
            session_permission_rules: crate::permissions::PermissionRules::new(),
            llm_memory_extraction: false,
            approval_channel: None,
            allowed_tools: None,
            workflow_triggered_this_turn: std::sync::atomic::AtomicBool::new(false),
            workflow_policy: WorkflowPolicy::from_env(),
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
        // 权限模式由上层引擎注入（默认 AutoLowRisk）
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
            .unwrap_or("");
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
            last_user_preview,
        ));
        let route = IntentRouter::new().route(last_user_preview);
        trace.record(TraceEvent::IntentRouted {
            intent: format!("{:?}", route.intent),
            workflow: format!("{:?}", route.workflow),
            retrieval: format!("{:?}", route.retrieval),
            confidence: route.confidence,
            risk: format!("{:?}", route.risk),
            reason: route.reason.clone(),
        });
        if let Some(manager) = &self.goal_manager {
            if let Some(goal) = manager.update_from_user_message(last_user_preview, Some(&route)) {
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
        if !already_triggered {
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
                        base_context: self.create_tool_context(),
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

        let tools = self.get_tools();
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut iterations_used = 0;
        let mut failed_tool_fingerprints: HashMap<String, usize> = HashMap::new();
        let mut failed_tool_names: HashMap<String, usize> = HashMap::new();

        // ── 记忆围栏注入：先注入，再让 preflight 统计真实请求大小 ──
        if let Some(ref mem_mutex) = self.memory_manager {
            let mem = mem_mutex.lock().await;
            let snapshot = mem.get_snapshot();
            if !snapshot.is_empty() && !messages.iter().any(|m| {
                matches!(m, Message::System { content } if content.contains("<memory-context>"))
            }) {
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

        // ── 前置压缩（Preflight）─────────────────────────
        if let Some(ref compressor_mutex) = self.compressor {
            let mut no_gain_passes = 0u8;
            for pass in 0..3 {
                let compressor = compressor_mutex.lock().await;
                let tool_tokens = estimate_tool_schemas_tokens(&tools);
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

        // ── 迭代预算 ─────────────────────────────────────
        let mut effective_iterations: usize = 0;

        for iteration in 0..self.max_iterations {
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
                warn!(
                    "Effective iteration budget exhausted ({}/{})",
                    effective_iterations, self.max_iterations
                );
                break;
            }

            let mut request_messages = messages.clone();
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                if let Some(last_user_idx) = request_messages
                    .iter()
                    .rposition(|m| matches!(m, Message::User { .. }))
                {
                    if let Message::User { content } = &request_messages[last_user_idx] {
                        let prefetch = mem
                            .prefetch_with_llm_rerank(content, self.provider.as_ref(), &self.model)
                            .await;
                        if !prefetch.is_empty() {
                            trace.record(TraceEvent::MemoryPrefetch {
                                chars: prefetch.chars().count(),
                            });
                            let enhanced = format!(
                                "{}\n<relevant-memory>\n{}\n</relevant-memory>",
                                content, prefetch
                            );
                            request_messages[last_user_idx] = Message::user(&enhanced);
                            debug!("Prefetched memory context injected into user message");
                        }
                    }
                }
            }

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
                api_result = if let Some(tx) = tx {
                    self.call_api_streaming(request.clone(), tx).await
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

            if tool_calls.is_empty() {
                break;
            }

            messages.push(Message::assistant_with_tools(&content, tool_calls.clone()));

            let mut results = self
                .execute_tools_parallel(&tool_calls, tx, pre_executed, Some(trace.clone()))
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
            let mut any_tool_success = false;
            let mut repeated_failed_tools = Vec::new();
            for (tc, result) in results.iter_mut() {
                truncate_tool_result(result, &tc.name, &tc.id).await;
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                tool_results_text.push_str(&result_content);
                tool_results_text.push('\n');
                messages.push(Message::tool(tc.id.clone(), result_content));

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
                }

                if result.success && (tc.name == "file_edit" || tc.name == "file_write") {
                    if let Some(path) = tc.arguments["path"].as_str() {
                        changed_files.push(std::path::PathBuf::from(path));
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
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let verify_results =
                    super::auto_verify::verify_file_changes(&working_dir, &changed_files).await;
                let check_passed = verify_results.iter().all(|r| r.success);
                for result in verify_results {
                    let verify_text = result.to_dialog_text();
                    if !result.success {
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
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&lsp_text);
                        messages.push(Message::system(lsp_text));
                    }
                }

                // ── 自动测试闭环 ──────────────────────────────
                let test_results =
                    super::auto_verify::run_tests(&working_dir, &changed_files, check_passed).await;
                let tests_passed = test_results.iter().all(|r| r.success);
                for result in test_results {
                    let test_text = result.to_dialog_text();
                    if !result.success {
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&test_text);
                        messages.push(Message::system(test_text));
                    } else {
                        debug!("{}", test_text);
                    }
                }

                // ── 代码自审查 ────────────────────────────────
                let review_result =
                    super::code_review::review_changed_files(&working_dir, &changed_files);
                if !review_result.success {
                    let review_text = review_result.to_dialog_text();
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&review_text);
                    messages.push(Message::system(review_text));
                }

                // ── 编程质量可观测性 ───────────────────────
                let verify_passed = check_passed && tests_passed && review_result.success;
                trace.record(TraceEvent::VerificationCompleted {
                    changed_files: changed_files.len(),
                    passed: verify_passed,
                });
                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_coding_round(verify_passed);
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
        }

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
            tool_calls: final_tool_calls,
            iterations: iterations_used,
            pre_executed_results: std::collections::HashMap::new(),
        })
    }

    /// 非流式 API 调用
    async fn call_api(
        &self,
        request: ChatRequest,
    ) -> Result<(
        String,
        Vec<ToolCall>,
        std::collections::HashMap<usize, ToolResult>,
    )> {
        let response = self.provider.chat(request).await?;
        self.record_cost(&response).await;

        let content = strip_think_blocks(&response.content);
        let tool_calls = response.tool_calls.unwrap_or_default();

        Ok((content, tool_calls, std::collections::HashMap::new()))
    }

    /// 流式 API 调用
    async fn call_api_streaming(
        &self,
        request: ChatRequest,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<(
        String,
        Vec<ToolCall>,
        std::collections::HashMap<usize, ToolResult>,
    )> {
        let fallback_messages = request.messages.clone();
        let fallback_tools = request.tools.clone();

        match self.provider.chat_stream(request).await {
            Ok(mut stream) => {
                let mut raw_content = String::new();
                let mut full_content = String::new();
                let mut collected_tool_calls: Vec<ToolCall> = Vec::new();
                let mut raw_args_accum: Vec<String> = Vec::new();
                let mut stream_failed: Option<String> = None;
                let mut visible_sanitizer = VisibleTextSanitizer::default();

                let _ = tx.send(StreamEvent::ThinkingStart).await;

                let mut read_only_tasks: std::collections::HashMap<
                    usize,
                    tokio::task::JoinHandle<ToolResult>,
                > = std::collections::HashMap::new();
                let read_only_concurrency = read_only_tool_concurrency();
                let tool_registry = self.tool_registry.clone();
                let tool_context = self.create_tool_context();
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(chunk) => {
                            if let Some(usage) = &chunk.usage {
                                let _ = tx
                                    .send(StreamEvent::Usage {
                                        prompt_tokens: usage.prompt_tokens,
                                        completion_tokens: usage.completion_tokens,
                                        reasoning_tokens: usage
                                            .completion_tokens_details
                                            .as_ref()
                                            .and_then(|d| d.reasoning_tokens),
                                        cached_tokens: usage
                                            .prompt_tokens_details
                                            .as_ref()
                                            .and_then(|d| d.cached_tokens),
                                    })
                                    .await;
                            }
                            if let Some(choice) = chunk.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    if !content.is_empty() {
                                        raw_content.push_str(content);
                                        let visible_chunk = visible_sanitizer.push_chunk(content);
                                        if !visible_chunk.is_empty() {
                                            full_content.push_str(&visible_chunk);
                                            let _ = tx
                                                .send(StreamEvent::TextChunk(visible_chunk))
                                                .await;
                                        }
                                    }
                                }

                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                    for tc_delta in tool_calls {
                                        let idx = tc_delta.index as usize;
                                        while collected_tool_calls.len() <= idx {
                                            collected_tool_calls.push(ToolCall {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: serde_json::Value::Null,
                                            });
                                            raw_args_accum.push(String::new());
                                        }

                                        let mut tool_name_for_spawn: Option<String> = None;
                                        let mut tool_id_for_spawn: Option<String> = None;
                                        let mut args_for_spawn: Option<String> = None;

                                        let tc = &mut collected_tool_calls[idx];
                                        if let Some(id) = &tc_delta.id {
                                            tc.id = id.clone();
                                            let _ = tx
                                                .send(StreamEvent::ToolCallStart {
                                                    id: id.clone(),
                                                    name: tc.name.clone(),
                                                })
                                                .await;
                                        }
                                        if let Some(function) = &tc_delta.function {
                                            if let Some(name) = &function.name {
                                                tc.name = name.clone();
                                            }
                                            if let Some(args) = &function.arguments {
                                                raw_args_accum[idx].push_str(args);

                                                tool_name_for_spawn = Some(tc.name.clone());
                                                tool_id_for_spawn = Some(tc.id.clone());
                                                args_for_spawn = Some(raw_args_accum[idx].clone());

                                                let _ = tx
                                                    .send(StreamEvent::ToolCallArgs {
                                                        id: tc.id.clone(),
                                                        args_delta: args.clone(),
                                                    })
                                                    .await;
                                            }
                                        }

                                        if let (Some(tool_name), Some(tid), Some(current_args)) =
                                            (tool_name_for_spawn, tool_id_for_spawn, args_for_spawn)
                                        {
                                            if !tool_name.is_empty()
                                                && is_read_only(&tool_name)
                                                && !read_only_tasks.contains_key(&idx)
                                                && read_only_tasks.len() < read_only_concurrency
                                            {
                                                let registry = tool_registry.clone();
                                                let context = tool_context.clone();
                                                let ct = cost_tracker.clone();
                                                let hooks = hook_manager.clone();
                                                let tid2 = tid.clone();
                                                let tool_n = tool_name.clone();
                                                let tool_n2 = tool_name.clone();

                                                read_only_tasks.insert(
                                                    idx,
                                                    tokio::spawn(async move {
                                                        let started_at =
                                                            std::time::Instant::now();
                                                        let pre_decision = if let Some(ref h)
                                                            = hooks
                                                        {
                                                            let t = ToolCall {
                                                                id: tid.clone(),
                                                                name: tool_n.clone(),
                                                                arguments:
                                                                    serde_json::from_str(
                                                                        &current_args,
                                                                    )
                                                                    .unwrap_or(serde_json::Value::Null),
                                                            };
                                                            h.run_pre_tool(&t, &context).await
                                                        } else {
                                                            HookDecision {
                                                                allow: true,
                                                                reason: None,
                                                            }
                                                        };

                                                        let ctx_clone = context.clone();
                                                        let mut result = if !pre_decision.allow {
                                                            ToolResult::error(
                                                                pre_decision.reason.unwrap_or_else(
                                                                    || format!(
                                                                        "blocked by pre-tool hook: {}",
                                                                        tool_n
                                                                    ),
                                                                ),
                                                            )
                                                        } else if let Some(tool) =
                                                            registry.get(&tool_n)
                                                        {
                                                            let parsed_args =
                                                                serde_json::from_str(
                                                                    &current_args,
                                                                )
                                                                .unwrap_or(serde_json::Value::Null);
                                                            tool.execute(parsed_args, context)
                                                                .await
                                                        } else {
                                                            ToolResult::error(format!(
                                                                "Tool '{}' not found",
                                                                tool_n
                                                            ))
                                                        };

                                                        let duration_ms =
                                                            started_at.elapsed().as_millis()
                                                                as u64;
                                                        if result.duration_ms.is_none() {
                                                            result.duration_ms =
                                                                Some(duration_ms);
                                                        }
                                                        if let Some(ref h) = hooks {
                                                            let tc_for_hook = ToolCall {
                                                                id: tid2.clone(),
                                                                name: tool_n2.clone(),
                                                                arguments:
                                                                    serde_json::from_str(
                                                                        &current_args,
                                                                    )
                                                                    .unwrap_or(serde_json::Value::Null),
                                                            };
                                                            h.run_post_tool(&tc_for_hook, &result, &ctx_clone)
                                                                .await;
                                                        }
                                                        {
                                                            let mut tracker = ct.lock().await;
                                                            tracker.record_tool_execution(
                                                                &tool_n,
                                                                result.success,
                                                                duration_ms,
                                                                result.error.as_deref(),
                                                            );
                                                        }
                                                        result
                                                    }),
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            let truncated = chunk.choices.iter().any(|c| {
                                c.finish_reason
                                    .as_ref()
                                    .is_some_and(|fr| format!("{:?}", fr).contains("Length"))
                            });
                            if truncated {
                                let _ = tx.send(StreamEvent::OutputTruncated).await;
                            }
                            if chunk.choices.iter().any(|c| c.finish_reason.is_some()) {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {}", e);
                            stream_failed = Some(e.to_string());
                            break;
                        }
                    }
                }

                let _ = tx.send(StreamEvent::ThinkingComplete).await;
                let visible_tail = visible_sanitizer.finish();
                if !visible_tail.is_empty() {
                    full_content.push_str(&visible_tail);
                    let _ = tx.send(StreamEvent::TextChunk(visible_tail)).await;
                }

                for (i, tc) in collected_tool_calls.iter_mut().enumerate() {
                    if i < raw_args_accum.len() && !raw_args_accum[i].is_empty() {
                        tc.arguments =
                            serde_json::from_str(&raw_args_accum[i]).unwrap_or_else(|e| {
                                warn!("Failed to parse tool args: {}", e);
                                serde_json::Value::Null
                            });
                        let _ = tx
                            .send(StreamEvent::ToolCallComplete { id: tc.id.clone() })
                            .await;
                    }
                }

                let mut pre_executed: std::collections::HashMap<usize, ToolResult> =
                    std::collections::HashMap::new();
                for (idx, handle) in read_only_tasks {
                    if let Ok(result) = handle.await {
                        debug!(
                            "Read-only tool at index {} pre-executed with result: {}",
                            idx,
                            if result.success { "OK" } else { "ERROR" }
                        );
                        pre_executed.insert(idx, result);
                    }
                }

                // If streaming failed before receiving any usable content/tool calls,
                // transparently fall back to non-streaming to improve provider compatibility.
                if let Some(stream_err) = stream_failed {
                    if raw_content.trim().is_empty() && collected_tool_calls.is_empty() {
                        let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                            "stream_empty",
                            &stream_err,
                        );
                        warn!("{}", plan.user_note);
                        warn!(
                            "Streaming yielded no content (error: {}), falling back to non-streaming",
                            stream_err
                        );
                        let base_request = ChatRequest::new(&self.model)
                            .with_messages(fallback_messages.clone())
                            .with_temperature(0.2);
                        let response = if let Some(tools) = fallback_tools.clone() {
                            match self
                                .provider
                                .chat(base_request.clone().with_tools(tools))
                                .await
                            {
                                Ok(r) => r,
                                Err(with_tools_err) => {
                                    warn!(
                                        "Non-streaming fallback with tools failed: {}. Retrying without tools.",
                                        with_tools_err
                                    );
                                    self.provider.chat(base_request).await?
                                }
                            }
                        } else {
                            self.provider.chat(base_request).await?
                        };
                        self.record_cost(&response).await;
                        emit_usage_event(&response, tx).await;

                        let content = strip_think_blocks(&response.content);
                        if !content.is_empty() {
                            let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                        }
                        let tool_calls = response.tool_calls.unwrap_or_default();
                        return Ok((content, tool_calls, std::collections::HashMap::new()));
                    } else {
                        let _ = tx
                            .send(StreamEvent::Error(format!(
                                "Stream interrupted: {}",
                                stream_err
                            )))
                            .await;
                    }
                }

                Ok((full_content, collected_tool_calls, pre_executed))
            }
            Err(e) => {
                let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                    "stream_open",
                    &e.to_string(),
                );
                warn!("{}", plan.user_note);
                warn!("Streaming failed, falling back to non-streaming: {}", e);
                let base_request = ChatRequest::new(&self.model)
                    .with_messages(fallback_messages.clone())
                    .with_temperature(0.2);
                let response = if let Some(tools) = fallback_tools.clone() {
                    match self
                        .provider
                        .chat(base_request.clone().with_tools(tools))
                        .await
                    {
                        Ok(r) => r,
                        Err(with_tools_err) => {
                            warn!(
                                "Non-streaming fallback with tools failed: {}. Retrying without tools.",
                                with_tools_err
                            );
                            self.provider.chat(base_request).await?
                        }
                    }
                } else {
                    self.provider.chat(base_request).await?
                };
                self.record_cost(&response).await;
                emit_usage_event(&response, tx).await;

                let content = strip_think_blocks(&response.content);
                if !content.is_empty() {
                    let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                }
                let tool_calls = response.tool_calls.unwrap_or_default();
                Ok((content, tool_calls, std::collections::HashMap::new()))
            }
        }
    }

    /// 记录 API 调用成本
    async fn record_cost(&self, response: &ChatResponse) {
        if let Some(ref usage) = response.usage {
            let mut tracker = self.cost_tracker.lock().await;
            tracker.record_api_call(
                &self.model,
                usage.prompt_tokens as u64,
                usage.completion_tokens as u64,
                usage.cached_tokens.map(|t| t as u64),
            );
        }
    }

    fn finish_trace(&self, trace: TraceCollector, status: TurnStatus) {
        let trace = trace.finish(status);
        if let Some(store) = &self.trace_store {
            store.push(trace.clone());
        }
        if let Some(store) = &self.session_store {
            if let Err(e) = store.add_turn_trace(&trace) {
                warn!("Failed to persist turn trace: {}", e);
            }
            if let Err(e) = persist_turn_learning_event(store, &trace) {
                warn!("Failed to persist learning event: {}", e);
            }
        }
    }

    /// 获取工具定义列表
    fn get_tools(&self) -> Vec<crate::services::api::Tool> {
        self.tool_registry
            .iter_tools()
            .filter(|t| {
                if let Some(ref allowed) = self.allowed_tools {
                    allowed.contains(t.name())
                } else {
                    true
                }
            })
            .map(|t| crate::services::api::Tool {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }

    /// 并行执行工具调用
    async fn execute_tools_parallel(
        &self,
        tool_calls: &[ToolCall],
        tx: Option<&mpsc::Sender<StreamEvent>>,
        pre_executed: std::collections::HashMap<usize, ToolResult>,
        trace: Option<TraceCollector>,
    ) -> Vec<(ToolCall, ToolResult)> {
        let mut read_only_jobs = Vec::new();
        let mut read_write_calls = Vec::new();
        let mut denied_results = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();

        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }
            if let Some(ref allowed) = self.allowed_tools {
                if !allowed.contains(&tc.name) {
                    denied_results.push((
                        tc.clone(),
                        ToolResult::error(format!(
                            "Tool '{}' is not allowed in this agent context",
                            tc.name
                        )),
                    ));
                    continue;
                }
            }

            if let Some(pre_result) = pre_executed.get(&i) {
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: true,
                        pre_executed: true,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: pre_result.success,
                        duration_ms: pre_result.duration_ms,
                        output_chars: pre_result.content.chars().count(),
                    });
                }
                debug!(
                    "Skipping pre-executed read-only tool at index {}: {}",
                    i, tc.name
                );
                results.push((tc.clone(), pre_result.clone()));
                if let Some(tx) = tx {
                    let result_content = format!(
                        "Result: {}\n{}",
                        if pre_result.success { "OK" } else { "ERROR" },
                        pre_result.content
                    );
                    let _ = tx
                        .send(StreamEvent::ToolExecutionComplete {
                            id: tc.id.clone(),
                            result: result_content,
                        })
                        .await;
                }
                continue;
            }

            if is_read_only(&tc.name) {
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                        })
                        .await;
                }
                let registry = self.tool_registry.clone();
                let context = self.create_tool_context();
                let tc_clone = tc.clone();
                let tool_name = tc.name.clone();
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();
                let trace = trace.clone();
                read_only_jobs.push(async move {
                    let started_at = std::time::Instant::now();
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolStarted {
                            tool: tool_name.clone(),
                            call_id: tc_clone.id.clone(),
                            parallel: true,
                            pre_executed: false,
                        });
                    }
                    let pre_decision = if let Some(ref hooks) = hook_manager {
                        hooks.run_pre_tool(&tc_clone, &context).await
                    } else {
                        HookDecision {
                            allow: true,
                            reason: None,
                        }
                    };

                    let mut result =
                        if !pre_decision.allow {
                            ToolResult::error(pre_decision.reason.unwrap_or_else(|| {
                                format!("blocked by pre-tool hook: {}", tool_name)
                            }))
                        } else if let Some(tool) = registry.get(&tool_name) {
                            tool.execute(tc_clone.arguments.clone(), context.clone())
                                .await
                        } else {
                            ToolResult::error(format!("Tool '{}' not found", tool_name))
                        };
                    let duration_ms = started_at.elapsed().as_millis() as u64;
                    if result.duration_ms.is_none() {
                        result.duration_ms = Some(duration_ms);
                    }

                    if let Some(ref hooks) = hook_manager {
                        hooks.run_post_tool(&tc_clone, &result, &context).await;
                    };
                    {
                        let mut tracker = cost_tracker.lock().await;
                        tracker.record_tool_execution(
                            &tool_name,
                            result.success,
                            duration_ms,
                            result.error.as_deref(),
                        );
                    }
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolCompleted {
                            tool: tool_name,
                            call_id: tc_clone.id.clone(),
                            success: result.success,
                            duration_ms: result.duration_ms,
                            output_chars: result.content.chars().count(),
                        });
                    }
                    (tc_clone, result)
                });
            } else {
                read_write_calls.push(tc.clone());
            }
        }

        results.append(&mut denied_results);

        let concurrency = read_only_tool_concurrency();
        let mut readonly_stream =
            futures::stream::iter(read_only_jobs).buffer_unordered(concurrency);

        while let Some((tc, result)) = readonly_stream.next().await {
            if let Some(tx) = tx {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tc.id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            results.push((tc, result));
        }

        for tc in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if let Some(ref allowed) = self.allowed_tools {
                if !allowed.contains(&tool_name) {
                    results.push((
                        tc,
                        ToolResult::error(format!(
                            "Tool '{}' is not allowed in this agent context",
                            tool_name
                        )),
                    ));
                    continue;
                }
            }

            if let Some(tx) = tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                    })
                    .await;
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolStarted {
                    tool: tool_name.clone(),
                    call_id: tool_id.clone(),
                    parallel: false,
                    pre_executed: false,
                });
            }

            let (result, hook_context) = if let Some(tool) = self.tool_registry.get(&tool_name) {
                let mut context = self.create_tool_context();
                let pre_decision = if let Some(ref hooks) = self.hook_manager {
                    hooks.run_pre_tool(&tc, &context).await
                } else {
                    HookDecision {
                        allow: true,
                        reason: None,
                    }
                };

                let started_at = std::time::Instant::now();
                let mut result = if !pre_decision.allow {
                    ToolResult::error(
                        pre_decision
                            .reason
                            .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                    )
                } else if context
                    .permission_context
                    .requires_confirmation(&tool_name, &tc.arguments)
                {
                    let mut approved = false;
                    if let (Some(ref channel), Some(tx)) = (&self.approval_channel, tx) {
                        let prompt = if tool_name == "mcp_tool" {
                            let server = tc.arguments["server_name"].as_str().unwrap_or("");
                            let t = tc.arguments["tool_name"].as_str().unwrap_or("");
                            format!(
                                "MCP tool '{}' on server '{}' requires approval. Allow?",
                                t, server
                            )
                        } else {
                            format!("Tool '{}' requires approval. Allow?", tool_name)
                        };
                        let _ = tx
                            .send(StreamEvent::PermissionRequest {
                                id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                arguments: tc.arguments.clone(),
                                prompt: prompt.clone(),
                            })
                            .await;
                        if let Some(ref trace) = trace {
                            trace.record(TraceEvent::PermissionRequested {
                                tool: tool_name.clone(),
                                call_id: tool_id.clone(),
                                prompt: prompt.clone(),
                            });
                        }
                        let request = ToolApprovalRequest {
                            tool_call: tc.clone(),
                            prompt,
                        };
                        match channel.submit(request).await {
                            Ok(is_approved) => approved = is_approved,
                            Err(e) => {
                                warn!("Tool approval error: {}", e);
                            }
                        }
                        if let Some(ref trace) = trace {
                            trace.record(TraceEvent::PermissionResolved {
                                tool: tool_name.clone(),
                                call_id: tool_id.clone(),
                                approved,
                            });
                        }
                    }
                    if approved {
                        if context.permission_context.mode
                            == crate::permissions::PermissionMode::Once
                        {
                            context.permission_context.grant_once(&tool_name);
                        }
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamEvent::ToolExecutionProgress {
                                    id: tool_id.clone(),
                                    progress: format!("Executing {}...", tool_name),
                                })
                                .await;
                        }
                        tool.execute(tc.arguments.clone(), context.clone()).await
                    } else {
                        ToolResult::error(format!(
                            "Permission denied: '{}' requires user confirmation.",
                            tool_name
                        ))
                    }
                } else {
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::ToolExecutionProgress {
                                id: tool_id.clone(),
                                progress: format!("Executing {}...", tool_name),
                            })
                            .await;
                    }
                    tool.execute(tc.arguments.clone(), context.clone()).await
                };
                let duration_ms = started_at.elapsed().as_millis() as u64;
                if result.duration_ms.is_none() {
                    result.duration_ms = Some(duration_ms);
                }

                // ── Security Audit & Denial Tracking ──────────────────────
                let params_summary = if let Some(tool) = self.tool_registry.get(&tool_name) {
                    tool.to_classifier_input(&tc.arguments)
                } else {
                    tool_name.clone()
                };

                if let Some(ref log) = self.audit_log {
                    let decision = if result.success {
                        "EXECUTED"
                    } else if result
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .contains("Permission denied")
                    {
                        "DENIED"
                    } else {
                        "FAILED"
                    };
                    log.log_execution(&tool_name, &params_summary, result.success, decision)
                        .await;
                }

                if let Some(ref tracker) = self.denial_tracker {
                    if result.success {
                        tracker.record_success().await;
                    } else if result
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .contains("Permission denied")
                        || result
                            .error
                            .as_deref()
                            .unwrap_or("")
                            .contains("Dangerous command")
                    {
                        tracker
                            .record_denial(
                                &tool_name,
                                &params_summary,
                                result.error.as_deref().unwrap_or("security block"),
                            )
                            .await;
                    }
                }
                // ─────────────────────────────────────────────────────────

                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_tool_execution(
                        &tool_name,
                        result.success,
                        duration_ms,
                        result.error.as_deref(),
                    );
                }

                (result, Some(context))
            } else {
                (
                    ToolResult::error(format!("Tool '{}' not found", tool_name)),
                    None,
                )
            };

            if let (Some(hooks), Some(context)) = (&self.hook_manager, &hook_context) {
                hooks.run_post_tool(&tc, &result, context).await;
            }

            if let Some(tx) = tx {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolCompleted {
                    tool: tool_name,
                    call_id: tool_id,
                    success: result.success,
                    duration_ms: result.duration_ms,
                    output_chars: result.content.chars().count(),
                });
            }
            results.push((tc, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::{ChatResponse, ToolCall, Usage};
    use crate::test_utils::env_guard::EnvVarGuard;
    use crate::tools::{FileReadTool, FileWriteTool};
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_truncate_tool_result_handles_utf8_boundaries() {
        let mut result = ToolResult::success("中".repeat(20_000));
        truncate_tool_result(&mut result, "grep", "call_utf8").await;
        assert!(result.content.contains("Output truncated"));
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

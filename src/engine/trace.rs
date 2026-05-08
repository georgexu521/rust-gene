//! Runtime turn tracing.
//!
//! The trace spine records high-level events for a user turn without storing
//! full sensitive tool outputs. It is designed to back `/trace` and future
//! eval assertions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

const DEFAULT_MAX_TRACES: usize = 100;
const PREVIEW_CHARS: usize = 120;
pub const RUNTIME_DIET_PROMPT_TOKEN_BUDGET: u64 = 4_000;
pub const RUNTIME_DIET_TOOL_COUNT_BUDGET: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTrace {
    pub trace_id: String,
    pub session_id: String,
    pub turn_index: u64,
    pub user_message_preview: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: TurnStatus,
    pub events: Vec<TraceEvent>,
}

impl TurnTrace {
    pub fn new(session_id: impl Into<String>, turn_index: u64, user_message: &str) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            turn_index,
            user_message_preview: preview(user_message),
            started_at: Utc::now(),
            finished_at: None,
            status: TurnStatus::Running,
            events: vec![TraceEvent::UserPromptSubmitted {
                chars: user_message.chars().count(),
            }],
        }
    }

    pub fn finish(&mut self, status: TurnStatus) {
        self.status = status;
        self.finished_at = Some(Utc::now());
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at
            .map(|end| (end - self.started_at).num_milliseconds())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    UserPromptSubmitted {
        chars: usize,
    },
    IntentRouted {
        intent: String,
        workflow: String,
        retrieval: String,
        confidence: f32,
        risk: String,
        reason: String,
    },
    ResourcePolicySelected {
        latency: String,
        target_ms: u64,
        cost_ceiling_usd: f64,
        reasoning: String,
        parallelism_limit: usize,
        max_tool_calls: usize,
        context_budget_tokens: usize,
        reason: String,
    },
    TaskContextBuilt {
        task_id: String,
        workflow: String,
        files: usize,
        constraints: usize,
        risks: usize,
        acceptance_checks: usize,
    },
    ImplementationIntentRecorded {
        task_id: String,
        workflow: String,
        target_files: usize,
        validation_commands: Vec<String>,
        risks: usize,
        reason: String,
    },
    WorkflowJudgmentCompleted {
        task_type: String,
        complexity: String,
        risk: String,
        plan_steps: usize,
        acceptance_checks: usize,
        questions: usize,
        guided_reasoning: bool,
    },
    WorkflowPlanProgress {
        total_steps: usize,
        completed_steps: usize,
        active_step: Option<String>,
        top_priority: Option<String>,
        #[serde(default)]
        top_importance_score: Option<f32>,
        #[serde(default)]
        top_weight_share: Option<f32>,
        #[serde(default)]
        weight_source: Option<String>,
        reweighted: bool,
    },
    WorkflowLearningAdjusted {
        adjustments: usize,
        before_top_step: Option<String>,
        after_top_step: Option<String>,
        reason: String,
    },
    StageValidationCompleted {
        step: Option<String>,
        status: String,
        changed_files: usize,
        evidence_items: usize,
    },
    ReflectionPassCompleted {
        pass_id: String,
        task_id: String,
        status: String,
        findings: usize,
        unresolved: usize,
    },
    SessionGoalUpdated {
        goal_id: String,
        title: String,
        status: String,
        reason: String,
    },
    GoalDriftDetected {
        goal_id: String,
        tool: String,
        call_id: String,
        level: String,
        reason: String,
        suggested_action: Option<String>,
    },
    DestructiveScopeChecked {
        tool: String,
        call_id: String,
        operation: String,
        target: Option<String>,
        allowed: bool,
        reason: String,
    },
    WorkflowRouted {
        decision: String,
        reason: String,
    },
    WorkflowCompleted {
        steps: usize,
    },
    WorkflowFallback {
        error: String,
    },
    AdaptiveWorkflowTriggered {
        trigger: String,
        depth: String,
        require_workflow_judgment: bool,
        require_stage_validation: bool,
        max_repair_attempts: usize,
        reason: String,
    },
    MemorySnapshotInjected {
        chars: usize,
    },
    MemoryPrefetch {
        chars: usize,
    },
    RetrievalContextBuilt {
        policy: String,
        sources: Vec<String>,
        items: usize,
        estimated_tokens: usize,
        #[serde(default)]
        provenance: Vec<String>,
        #[serde(default)]
        conflicts: usize,
    },
    MemorySynced {
        mode: String,
    },
    ContextCompacted {
        before_tokens: usize,
        after_tokens: usize,
        strategy: String,
    },
    RuntimeDietReport {
        prompt_tokens: u64,
        tool_schema_tokens: u64,
        exposed_tools: usize,
        memory_snapshot_chars: usize,
        memory_snapshot_tokens: u64,
        retrieval_items: usize,
        retrieval_tokens: u64,
        skill_list_chars: usize,
        skill_list_tokens: u64,
        route_scoped_tools: bool,
        workflow_context: String,
        closeout_visibility: String,
        validation_evidence: String,
    },
    ApiRequestStarted {
        iteration: usize,
        model: String,
        tools: usize,
    },
    ApiRequestCompleted {
        iteration: usize,
        tool_calls: usize,
        content_chars: usize,
    },
    ToolStarted {
        tool: String,
        call_id: String,
        parallel: bool,
        pre_executed: bool,
    },
    PermissionRequested {
        tool: String,
        call_id: String,
        prompt: String,
    },
    PermissionResolved {
        tool: String,
        call_id: String,
        approved: bool,
    },
    ToolCompleted {
        tool: String,
        call_id: String,
        success: bool,
        duration_ms: Option<u64>,
        output_chars: usize,
    },
    HookCompleted {
        event: String,
        hook_name: String,
        call_id: String,
        tool: Option<String>,
        success: bool,
        blocked: bool,
        duration_ms: u64,
        error: Option<String>,
        output_preview: Option<String>,
    },
    SubagentStarted {
        agent_id: String,
        profile: Option<String>,
        role: String,
        description: String,
        timeout_secs: u64,
        allowed_tools: usize,
    },
    SubagentCompleted {
        agent_id: String,
        status: String,
        duration_ms: u64,
        output_chars: usize,
        tools_used: usize,
    },
    VerificationCompleted {
        changed_files: usize,
        passed: bool,
        #[serde(default)]
        check_passed: bool,
        #[serde(default)]
        tests_passed: bool,
        #[serde(default)]
        review_passed: bool,
        #[serde(default)]
        failed_commands: Vec<String>,
    },
    AcceptanceReviewCompleted {
        accepted: bool,
        confidence: String,
        criteria: usize,
        unresolved: usize,
        next_action: String,
    },
    GuidedDebuggingCompleted {
        blocker: bool,
        next_action: String,
        causes: usize,
        evidence_items: usize,
        ask_user: bool,
    },
    RecoveryApplied {
        error: String,
        action: String,
    },
    RecoveryPlan {
        plan_id: String,
        source: String,
        category: String,
        action: String,
        retryable: bool,
        safe_retry: bool,
        suggested_command: Option<String>,
        status: String,
    },
    McpResourceAccessed {
        server: String,
        uri: String,
        action: String,
        success: bool,
        content_chars: usize,
    },
    AssistantResponded {
        chars: usize,
        iterations: usize,
    },
    FinalCloseoutPrepared {
        status: String,
        changed_files: usize,
        validation_items: usize,
        acceptance_items: usize,
        residual_risks: usize,
    },
    Error {
        message: String,
    },
}

impl TraceEvent {
    pub fn label(&self) -> &'static str {
        match self {
            TraceEvent::UserPromptSubmitted { .. } => "prompt",
            TraceEvent::IntentRouted { .. } => "intent",
            TraceEvent::ResourcePolicySelected { .. } => "resource.policy",
            TraceEvent::TaskContextBuilt { .. } => "task.context",
            TraceEvent::ImplementationIntentRecorded { .. } => "implementation.intent",
            TraceEvent::WorkflowJudgmentCompleted { .. } => "workflow.judgment",
            TraceEvent::WorkflowPlanProgress { .. } => "workflow.plan",
            TraceEvent::WorkflowLearningAdjusted { .. } => "workflow.learning",
            TraceEvent::StageValidationCompleted { .. } => "stage.validation",
            TraceEvent::ReflectionPassCompleted { .. } => "reflection.pass",
            TraceEvent::SessionGoalUpdated { .. } => "goal",
            TraceEvent::GoalDriftDetected { .. } => "goal.drift",
            TraceEvent::DestructiveScopeChecked { .. } => "destructive.scope",
            TraceEvent::WorkflowRouted { .. } => "workflow.route",
            TraceEvent::WorkflowCompleted { .. } => "workflow.done",
            TraceEvent::WorkflowFallback { .. } => "workflow.fallback",
            TraceEvent::AdaptiveWorkflowTriggered { .. } => "workflow.trigger",
            TraceEvent::MemorySnapshotInjected { .. } => "memory.snapshot",
            TraceEvent::MemoryPrefetch { .. } => "memory.prefetch",
            TraceEvent::RetrievalContextBuilt { .. } => "retrieval.context",
            TraceEvent::MemorySynced { .. } => "memory.sync",
            TraceEvent::ContextCompacted { .. } => "context.compact",
            TraceEvent::RuntimeDietReport { .. } => "runtime.diet",
            TraceEvent::ApiRequestStarted { .. } => "api.start",
            TraceEvent::ApiRequestCompleted { .. } => "api.done",
            TraceEvent::ToolStarted { .. } => "tool.start",
            TraceEvent::PermissionRequested { .. } => "permission.request",
            TraceEvent::PermissionResolved { .. } => "permission.resolve",
            TraceEvent::ToolCompleted { .. } => "tool.done",
            TraceEvent::HookCompleted { .. } => "hook.done",
            TraceEvent::SubagentStarted { .. } => "subagent.start",
            TraceEvent::SubagentCompleted { .. } => "subagent.done",
            TraceEvent::VerificationCompleted { .. } => "verify.done",
            TraceEvent::AcceptanceReviewCompleted { .. } => "acceptance.review",
            TraceEvent::GuidedDebuggingCompleted { .. } => "guided.debug",
            TraceEvent::RecoveryApplied { .. } => "recovery",
            TraceEvent::RecoveryPlan { .. } => "recovery.plan",
            TraceEvent::McpResourceAccessed { .. } => "mcp.resource",
            TraceEvent::AssistantResponded { .. } => "assistant",
            TraceEvent::FinalCloseoutPrepared { .. } => "closeout",
            TraceEvent::Error { .. } => "error",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            TraceEvent::UserPromptSubmitted { chars } => format!("user prompt: {} chars", chars),
            TraceEvent::IntentRouted {
                intent,
                workflow,
                retrieval,
                confidence,
                risk,
                reason,
            } => format!(
                "intent={} workflow={} retrieval={} risk={} confidence={:.2}: {}",
                intent,
                workflow,
                retrieval,
                risk,
                confidence,
                preview(reason)
            ),
            TraceEvent::ResourcePolicySelected {
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                reason,
            } => format!(
                "resource policy: latency={} target={}ms cost<=${:.2} reasoning={} parallel={} tools={} ctx={} ({})",
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                preview(reason)
            ),
            TraceEvent::TaskContextBuilt {
                task_id,
                workflow,
                files,
                constraints,
                risks,
                acceptance_checks,
            } => format!(
                "task context {} workflow={} files={} constraints={} risks={} checks={}",
                short_id(task_id),
                workflow,
                files,
                constraints,
                risks,
                acceptance_checks
            ),
            TraceEvent::ImplementationIntentRecorded {
                task_id,
                workflow,
                target_files,
                validation_commands,
                risks,
                reason,
            } => format!(
                "implementation intent {} workflow={} targets={} validations={} risks={} reason={}",
                short_id(task_id),
                workflow,
                target_files,
                validation_commands.len(),
                risks,
                preview(reason)
            ),
            TraceEvent::WorkflowJudgmentCompleted {
                task_type,
                complexity,
                risk,
                plan_steps,
                acceptance_checks,
                questions,
                guided_reasoning,
            } => format!(
                "workflow judgment type={} complexity={} risk={} steps={} checks={} questions={} guided={}",
                preview(task_type),
                complexity,
                risk,
                plan_steps,
                acceptance_checks,
                questions,
                guided_reasoning
            ),
            TraceEvent::WorkflowPlanProgress {
                total_steps,
                completed_steps,
                active_step,
                top_priority,
                top_importance_score,
                top_weight_share,
                weight_source,
                reweighted,
            } => format!(
                "workflow plan {}/{} active={} priority={} importance={} share={} source={} reweighted={}",
                completed_steps,
                total_steps,
                active_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                top_priority.as_deref().unwrap_or("none"),
                top_importance_score
                    .map(|score| format!("{:.2}", score))
                    .unwrap_or_else(|| "none".to_string()),
                top_weight_share
                    .map(|share| format!("{:.2}", share))
                    .unwrap_or_else(|| "none".to_string()),
                weight_source.as_deref().unwrap_or("none"),
                reweighted
            ),
            TraceEvent::WorkflowLearningAdjusted {
                adjustments,
                before_top_step,
                after_top_step,
                reason,
            } => format!(
                "workflow learning adjusted count={} before={} after={} reason={}",
                adjustments,
                before_top_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                after_top_step
                    .as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                preview(reason)
            ),
            TraceEvent::StageValidationCompleted {
                step,
                status,
                changed_files,
                evidence_items,
            } => format!(
                "stage validation step={} status={} files={} evidence={}",
                step.as_deref()
                    .map(preview)
                    .unwrap_or_else(|| "none".to_string()),
                status,
                changed_files,
                evidence_items
            ),
            TraceEvent::ReflectionPassCompleted {
                pass_id,
                task_id,
                status,
                findings,
                unresolved,
            } => format!(
                "reflection {} task={} status={} findings={} unresolved={}",
                short_id(pass_id),
                short_id(task_id),
                status,
                findings,
                unresolved
            ),
            TraceEvent::SessionGoalUpdated {
                goal_id,
                title,
                status,
                reason,
            } => format!(
                "goal {} {}: {} ({})",
                short_id(goal_id),
                status,
                preview(title),
                preview(reason)
            ),
            TraceEvent::GoalDriftDetected {
                goal_id,
                tool,
                call_id,
                level,
                reason,
                suggested_action,
            } => format!(
                "{} {} drift={} goal={} reason={} suggested={}",
                tool,
                short_id(call_id),
                level,
                short_id(goal_id),
                preview(reason),
                suggested_action.as_deref().unwrap_or("none")
            ),
            TraceEvent::DestructiveScopeChecked {
                tool,
                call_id,
                operation,
                target,
                allowed,
                reason,
            } => format!(
                "{} {} destructive_scope op={} target={} allowed={} reason={}",
                tool,
                short_id(call_id),
                operation,
                target.as_deref().map(preview).unwrap_or_else(|| "none".to_string()),
                allowed,
                preview(reason)
            ),
            TraceEvent::WorkflowRouted { decision, reason } => {
                format!("workflow decision: {} ({})", decision, preview(reason))
            }
            TraceEvent::WorkflowCompleted { steps } => {
                format!("workflow completed: {} steps", steps)
            }
            TraceEvent::WorkflowFallback { error } => {
                format!("workflow fallback: {}", preview(error))
            }
            TraceEvent::AdaptiveWorkflowTriggered {
                trigger,
                depth,
                require_workflow_judgment,
                require_stage_validation,
                max_repair_attempts,
                reason,
            } => format!(
                "workflow trigger={} depth={} judgment={} stage_validation={} repairs={} reason={}",
                trigger,
                depth,
                require_workflow_judgment,
                require_stage_validation,
                max_repair_attempts,
                preview(reason)
            ),
            TraceEvent::MemorySnapshotInjected { chars } => {
                format!("memory snapshot injected: {} chars", chars)
            }
            TraceEvent::MemoryPrefetch { chars } => format!("memory prefetch: {} chars", chars),
            TraceEvent::RetrievalContextBuilt {
                policy,
                sources,
                items,
                estimated_tokens,
                provenance,
                conflicts,
            } => {
                let provenance = if provenance.is_empty() {
                    "none".to_string()
                } else {
                    provenance
                        .iter()
                        .take(3)
                        .map(|item| preview(item))
                        .collect::<Vec<_>>()
                        .join(" | ")
                };
                format!(
                    "retrieval context: policy={} sources={} items={} tokens~{} conflicts={} provenance={}",
                    policy,
                    sources.join(","),
                    items,
                    estimated_tokens,
                    conflicts,
                    provenance
                )
            }
            TraceEvent::MemorySynced { mode } => format!("memory synced: {}", mode),
            TraceEvent::ContextCompacted {
                before_tokens,
                after_tokens,
                strategy,
            } => format!(
                "context compacted: {} -> {} tokens ({})",
                before_tokens, after_tokens, strategy
            ),
            TraceEvent::RuntimeDietReport {
                prompt_tokens,
                tool_schema_tokens,
                exposed_tools,
                memory_snapshot_chars,
                memory_snapshot_tokens,
                retrieval_items,
                retrieval_tokens,
                skill_list_chars,
                skill_list_tokens,
                route_scoped_tools,
                workflow_context,
                closeout_visibility,
                validation_evidence,
            } => {
                let total = prompt_tokens.saturating_add(*tool_schema_tokens);
                let level = runtime_diet_level(*prompt_tokens, *exposed_tools);
                format!(
                    "{} prompt={} tool_schema={} total={} tools={} memory={}ch/~{}t retrieval={}items/~{}t skills={}ch/~{}t route_scoped={} workflow={} closeout={} validation={}",
                    level,
                    prompt_tokens,
                    tool_schema_tokens,
                    total,
                    exposed_tools,
                    memory_snapshot_chars,
                    memory_snapshot_tokens,
                    retrieval_items,
                    retrieval_tokens,
                    skill_list_chars,
                    skill_list_tokens,
                    route_scoped_tools,
                    workflow_context,
                    closeout_visibility,
                    validation_evidence
                )
            }
            TraceEvent::ApiRequestStarted {
                iteration,
                model,
                tools,
            } => format!(
                "api request #{}: model={}, tools={}",
                iteration, model, tools
            ),
            TraceEvent::ApiRequestCompleted {
                iteration,
                tool_calls,
                content_chars,
            } => format!(
                "api response #{}: {} tool calls, {} chars",
                iteration, tool_calls, content_chars
            ),
            TraceEvent::ToolStarted {
                tool,
                call_id,
                parallel,
                pre_executed,
            } => format!(
                "{} {} started{}{}",
                tool,
                short_id(call_id),
                if *parallel { " in parallel" } else { "" },
                if *pre_executed { " (pre-executed)" } else { "" }
            ),
            TraceEvent::PermissionRequested {
                tool,
                call_id,
                prompt,
            } => format!(
                "{} {} requested permission: {}",
                tool,
                short_id(call_id),
                preview(prompt)
            ),
            TraceEvent::PermissionResolved {
                tool,
                call_id,
                approved,
            } => format!(
                "{} {} permission {}",
                tool,
                short_id(call_id),
                if *approved { "approved" } else { "denied" }
            ),
            TraceEvent::ToolCompleted {
                tool,
                call_id,
                success,
                duration_ms,
                output_chars,
            } => format!(
                "{} {} {} in {}ms ({} chars)",
                tool,
                short_id(call_id),
                if *success { "ok" } else { "failed" },
                duration_ms.unwrap_or_default(),
                output_chars
            ),
            TraceEvent::HookCompleted {
                event,
                hook_name,
                call_id,
                tool,
                success,
                blocked,
                duration_ms,
                error,
                output_preview,
            } => {
                let detail = error
                    .as_deref()
                    .or(output_preview.as_deref())
                    .map(preview)
                    .unwrap_or_else(|| "no output".to_string());
                format!(
                    "{} hook '{}' for {} {}{} in {}ms: {}",
                    event,
                    hook_name,
                    tool.as_deref().unwrap_or(call_id),
                    if *success { "ok" } else { "failed" },
                    if *blocked { " blocked" } else { "" },
                    duration_ms,
                    detail
                )
            }
            TraceEvent::SubagentStarted {
                agent_id,
                profile,
                role,
                description,
                timeout_secs,
                allowed_tools,
            } => format!(
                "subagent {} started role={} profile={} timeout={}s tools={} task={}",
                agent_id,
                role,
                profile.as_deref().unwrap_or("none"),
                timeout_secs,
                allowed_tools,
                preview(description)
            ),
            TraceEvent::SubagentCompleted {
                agent_id,
                status,
                duration_ms,
                output_chars,
                tools_used,
            } => format!(
                "subagent {} {} in {}ms ({} chars, {} tools)",
                agent_id, status, duration_ms, output_chars, tools_used
            ),
            TraceEvent::VerificationCompleted {
                changed_files,
                passed,
                check_passed,
                tests_passed,
                review_passed,
                failed_commands,
            } => format!(
                "verification {} for {} changed files (check={} tests={} review={} failed={})",
                if *passed { "passed" } else { "failed" },
                changed_files,
                check_passed,
                tests_passed,
                review_passed,
                failed_commands.len()
            ),
            TraceEvent::AcceptanceReviewCompleted {
                accepted,
                confidence,
                criteria,
                unresolved,
                next_action,
            } => format!(
                "acceptance accepted={} confidence={} criteria={} unresolved={} next={}",
                accepted, confidence, criteria, unresolved, next_action
            ),
            TraceEvent::GuidedDebuggingCompleted {
                blocker,
                next_action,
                causes,
                evidence_items,
                ask_user,
            } => format!(
                "guided debug blocker={} next={} causes={} evidence={} ask_user={}",
                blocker, next_action, causes, evidence_items, ask_user
            ),
            TraceEvent::RecoveryApplied { error, action } => {
                format!("recovery: {} -> {}", preview(error), action)
            }
            TraceEvent::RecoveryPlan {
                plan_id,
                source,
                category,
                action,
                retryable,
                safe_retry,
                suggested_command,
                status,
            } => format!(
                "{} {} {} action={} retryable={} safe_retry={} suggested={} status={}",
                source,
                short_id(plan_id),
                category,
                preview(action),
                retryable,
                safe_retry,
                suggested_command.as_deref().unwrap_or("none"),
                status
            ),
            TraceEvent::McpResourceAccessed {
                server,
                uri,
                action,
                success,
                content_chars,
            } => format!(
                "{} resource {} on {} success={} ({} chars)",
                action,
                preview(uri),
                server,
                success,
                content_chars
            ),
            TraceEvent::AssistantResponded { chars, iterations } => {
                format!(
                    "assistant responded: {} chars, {} iterations",
                    chars, iterations
                )
            }
            TraceEvent::FinalCloseoutPrepared {
                status,
                changed_files,
                validation_items,
                acceptance_items,
                residual_risks,
            } => format!(
                "final closeout status={} files={} validation={} acceptance={} risks={}",
                status, changed_files, validation_items, acceptance_items, residual_risks
            ),
            TraceEvent::Error { message } => format!("error: {}", preview(message)),
        }
    }
}

#[derive(Clone)]
pub struct TraceCollector {
    inner: Arc<Mutex<TurnTrace>>,
}

impl TraceCollector {
    pub fn new(trace: TurnTrace) -> Self {
        Self {
            inner: Arc::new(Mutex::new(trace)),
        }
    }

    pub fn record(&self, event: TraceEvent) {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.events.push(event);
    }

    pub fn finish(&self, status: TurnStatus) -> TurnTrace {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.finish(status);
        trace.clone()
    }
}

#[derive(Debug)]
pub struct TraceStore {
    max_traces: usize,
    traces: RwLock<VecDeque<TurnTrace>>,
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_TRACES)
    }
}

impl TraceStore {
    pub fn new(max_traces: usize) -> Self {
        Self {
            max_traces: max_traces.max(1),
            traces: RwLock::new(VecDeque::new()),
        }
    }

    pub fn push(&self, trace: TurnTrace) {
        let mut traces = self.traces.write().unwrap_or_else(|e| e.into_inner());
        traces.push_back(trace);
        while traces.len() > self.max_traces {
            traces.pop_front();
        }
    }

    pub fn latest(&self) -> Option<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .back()
            .cloned()
    }

    pub fn recent(&self, limit: usize) -> Vec<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.traces.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub fn format_trace_summary(trace: &TurnTrace, max_events: usize) -> String {
    let duration = trace
        .duration_ms()
        .map(|ms| format!("{}ms", ms))
        .unwrap_or_else(|| "running".to_string());
    let mut lines = vec![format!(
        "Trace {}\nSession: {}\nTurn: {}\nStatus: {:?}\nDuration: {}\nPrompt: {}",
        short_id(&trace.trace_id),
        trace.session_id,
        trace.turn_index,
        trace.status,
        duration,
        trace.user_message_preview
    )];
    if let Some(diet) = latest_runtime_diet_summary(trace) {
        lines.push(format!("\nRuntime Diet: {}", diet));
    }

    lines.push("\nEvents:".to_string());
    for (idx, event) in trace.events.iter().take(max_events).enumerate() {
        lines.push(format!(
            "{:>2}. {:<20} {}",
            idx + 1,
            event.label(),
            event.summary()
        ));
    }
    if trace.events.len() > max_events {
        lines.push(format!(
            "... {} more events",
            trace.events.len().saturating_sub(max_events)
        ));
    }

    lines.join("\n")
}

pub fn latest_runtime_diet_summary(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if matches!(event, TraceEvent::RuntimeDietReport { .. }) {
            Some(event.summary())
        } else {
            None
        }
    })
}

fn runtime_diet_level(prompt_tokens: u64, exposed_tools: usize) -> &'static str {
    if prompt_tokens > RUNTIME_DIET_PROMPT_TOKEN_BUDGET
        || exposed_tools > RUNTIME_DIET_TOOL_COUNT_BUDGET
    {
        "heavy"
    } else {
        "light"
    }
}

fn preview(text: &str) -> String {
    let mut out: String = text.chars().take(PREVIEW_CHARS).collect();
    if text.chars().count() > PREVIEW_CHARS {
        out.push_str("...");
    }
    out.replace('\n', " ")
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_store_retains_latest_entries() {
        let store = TraceStore::new(2);
        store.push(TurnTrace::new("s1", 1, "one"));
        store.push(TurnTrace::new("s1", 2, "two"));
        store.push(TurnTrace::new("s1", 3, "three"));

        assert_eq!(store.len(), 2);
        assert_eq!(store.latest().unwrap().turn_index, 3);
        assert_eq!(store.recent(2)[1].turn_index, 2);
    }

    #[test]
    fn trace_summary_includes_events() {
        let collector = TraceCollector::new(TurnTrace::new("s1", 1, "hello"));
        collector.record(TraceEvent::ToolStarted {
            tool: "bash".to_string(),
            call_id: "abcdef123".to_string(),
            parallel: false,
            pre_executed: false,
        });
        let trace = collector.finish(TurnStatus::Completed);
        let summary = format_trace_summary(&trace, 10);
        assert!(summary.contains("tool.start"));
        assert!(summary.contains("bash"));
    }

    #[test]
    fn trace_summary_includes_mcp_resource_access() {
        let collector = TraceCollector::new(TurnTrace::new("s1", 1, "read mcp resource"));
        collector.record(TraceEvent::McpResourceAccessed {
            server: "filesystem".to_string(),
            uri: "file:///tmp/a.txt".to_string(),
            action: "read".to_string(),
            success: true,
            content_chars: 12,
        });

        let trace = collector.finish(TurnStatus::Completed);
        let summary = format_trace_summary(&trace, 10);
        assert!(summary.contains("mcp.resource"));
        assert!(summary.contains("filesystem"));
        assert!(summary.contains("file:///tmp/a.txt"));
    }

    #[test]
    fn trace_summary_includes_runtime_diet_report() {
        let collector = TraceCollector::new(TurnTrace::new("s1", 1, "make a small edit"));
        collector.record(TraceEvent::RuntimeDietReport {
            prompt_tokens: 1_200,
            tool_schema_tokens: 320,
            exposed_tools: 6,
            memory_snapshot_chars: 180,
            memory_snapshot_tokens: 45,
            retrieval_items: 2,
            retrieval_tokens: 80,
            skill_list_chars: 120,
            skill_list_tokens: 30,
            route_scoped_tools: true,
            workflow_context: "minimal".to_string(),
            closeout_visibility: "concise".to_string(),
            validation_evidence: "passed".to_string(),
        });

        let trace = collector.finish(TurnStatus::Completed);
        let summary = format_trace_summary(&trace, 10);
        assert!(summary.contains("Runtime Diet: light"));
        assert!(summary.contains("prompt=1200"));
        assert!(summary.contains("tools=6"));
        assert!(summary.contains("memory=180ch/~45t"));
        assert!(summary.contains("retrieval=2items/~80t"));
        assert!(summary.contains("skills=120ch/~30t"));
        assert!(summary.contains("workflow=minimal"));
    }

    #[test]
    fn runtime_diet_report_flags_budget_bloat() {
        let event = TraceEvent::RuntimeDietReport {
            prompt_tokens: RUNTIME_DIET_PROMPT_TOKEN_BUDGET + 1,
            tool_schema_tokens: 0,
            exposed_tools: 1,
            memory_snapshot_chars: 0,
            memory_snapshot_tokens: 0,
            retrieval_items: 0,
            retrieval_tokens: 0,
            skill_list_chars: 0,
            skill_list_tokens: 0,
            route_scoped_tools: true,
            workflow_context: "minimal".to_string(),
            closeout_visibility: "none".to_string(),
            validation_evidence: "none".to_string(),
        };

        assert!(event.summary().starts_with("heavy "));
    }
}

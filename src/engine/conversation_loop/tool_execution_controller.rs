use super::permission_controller::{PermissionController, PermissionRequestRuntime};
use super::tool_call_lifecycle::{ToolCallLifecycle, ToolCallLifecycleRecord, ToolCallStatus};
use super::tool_context_helpers::{tool_allowed_by_context, tool_not_allowed_result};
use super::tool_execution::{read_only_tool_concurrency, tool_call_is_concurrency_safe};
use super::tool_metadata::{
    attach_tool_contract_metadata, attach_tool_execution_metadata, merge_tool_result_metadata,
    persist_tool_outcome_learning_event, tool_execution_start_progress,
};
use super::tool_result_controller::{invalid_tool_params_result, ToolResultNormalizer};
use super::turn_recording::{
    record_goal_drift_if_needed, record_hook_traces, record_mcp_resource_trace,
    record_permission_denial_recovery_trace, record_remote_bridge_trace,
    record_web_retrieval_trace,
};
use super::ConversationLoop;
use crate::engine::action_decision::{
    ActionDecision, ActionDecisionInput, ActionScoreModifier, ActionScoreModifierSource,
};
use crate::engine::action_review::{ActionReview, ActionReviewInput};
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::hooks::HookDecision;
use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::{AgentTaskStage, AgentTaskState};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use crate::tools::{
    ToolContext, ToolContextRetainedContext, ToolContextRetentionItem, ToolRegistry, ToolResult,
};
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::debug;

type ToolExecutionResults = Vec<(ToolCall, ToolResult)>;
type ToolLifecycleRecords = Vec<(String, ToolCallLifecycleRecord)>;

#[derive(Debug, Clone, Default)]
pub(super) struct ToolExecutionBatch {
    results: ToolExecutionResults,
    lifecycle: ToolLifecycleRecords,
}

impl ToolExecutionBatch {
    pub(super) fn new(results: ToolExecutionResults, lifecycle: ToolLifecycleRecords) -> Self {
        let (results, lifecycle) = complete_provider_result_pairs(results, lifecycle);
        Self { results, lifecycle }
    }

    #[cfg(test)]
    pub(super) fn results(&self) -> &[(ToolCall, ToolResult)] {
        &self.results
    }

    pub(super) fn results_mut(&mut self) -> &mut [(ToolCall, ToolResult)] {
        &mut self.results
    }

    pub(super) fn any_success(&self) -> bool {
        self.results.iter().any(|(_, result)| result.success)
    }

    pub(super) fn unsuccessful_count(&self) -> usize {
        self.results
            .iter()
            .filter(|(_, result)| !result.success)
            .count()
    }

    pub(super) fn result_successes(&self) -> impl Iterator<Item = (&ToolCall, bool)> {
        self.results
            .iter()
            .map(|(tool_call, result)| (tool_call, result.success))
    }

    pub(super) fn denied_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.status == ToolCallStatus::Denied)
            .count()
    }

    pub(super) fn failed_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.status == ToolCallStatus::Failed)
            .count()
    }

    pub(super) fn pre_executed_count(&self) -> usize {
        self.lifecycle
            .iter()
            .filter(|(_, record)| record.pre_executed)
            .count()
    }
}

fn complete_provider_result_pairs(
    mut results: ToolExecutionResults,
    mut lifecycle: ToolLifecycleRecords,
) -> (ToolExecutionResults, ToolLifecycleRecords) {
    let result_ids = results
        .iter()
        .map(|(tool_call, _)| tool_call.id.clone())
        .collect::<HashSet<_>>();

    for (call_id, record) in &mut lifecycle {
        if result_ids.contains(call_id) {
            continue;
        }

        let status = record.status;
        let mut result = ToolResult::error(format!(
            "Tool '{}' ended with lifecycle status {:?} but no terminal result was recorded. Treating it as interrupted.",
            record.tool_name, status
        ));
        merge_tool_result_metadata(
            &mut result,
            "tool_lifecycle_recovery",
            serde_json::json!({
                "schema": "tool_lifecycle_recovery.v1",
                "call_id": call_id,
                "tool": record.tool_name.clone(),
                "previous_status": format!("{:?}", status),
                "terminal_result": "interrupted",
                "synthesized": true,
            }),
        );
        results.push((
            ToolCall {
                id: call_id.clone(),
                name: record.tool_name.clone(),
                arguments: serde_json::json!({}),
            },
            result,
        ));
        record.status = ToolCallStatus::Failed;
    }

    (results, lifecycle)
}

pub(super) struct ToolExecutionRequest<'a> {
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) parent_assistant_content: &'a str,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) pre_executed: HashMap<usize, ToolResult>,
    pub(super) trace: Option<TraceCollector>,
    pub(super) route: Option<&'a IntentRoute>,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) retained_context: &'a ToolContextRetainedContext,
    pub(super) task_stage: AgentTaskStage,
    pub(super) task_state: Option<&'a AgentTaskState>,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) no_progress_rounds: usize,
    pub(super) has_changes_before_tools: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) lifecycle: &'a mut ToolCallLifecycle,
}

struct ReadWriteExecutionContext<'a> {
    tx: Option<&'a mpsc::Sender<StreamEvent>>,
    trace: &'a Option<TraceCollector>,
    runtime_context: &'a ToolRuntimeContext,
    retained_context: &'a ToolContextRetainedContext,
    task_state: Option<&'a AgentTaskState>,
    parent_tool_calls: &'a [ToolCall],
    parent_assistant_content: &'a str,
}

pub(super) struct ToolExecutionContext {
    tool_registry: Arc<ToolRegistry>,
    cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
    session_id: String,
    session_store: Option<Arc<crate::session_store::SessionStore>>,
    hook_manager: Option<Arc<crate::engine::hooks::ToolHookManager>>,
    approval_channel: Option<Arc<super::ToolApprovalChannel>>,
    allowed_tools: Option<HashSet<String>>,
    denial_tracker: Option<Arc<crate::security::DenialTracker>>,
    audit_log: Option<Arc<crate::security::SecurityAuditLog>>,
    active_goal: Option<crate::engine::session_goal::SessionGoal>,
    base_tool_context: ToolContext,
}

impl ToolExecutionContext {
    pub(super) fn from_conversation(conversation: &ConversationLoop) -> Self {
        Self {
            tool_registry: conversation.tool_registry.clone(),
            cost_tracker: conversation.cost_tracker.clone(),
            session_id: conversation.session_id.clone(),
            session_store: conversation.session_store.clone(),
            hook_manager: conversation.hook_manager.clone(),
            approval_channel: conversation.approval_channel.clone(),
            allowed_tools: conversation.allowed_tools.clone(),
            denial_tracker: conversation.denial_tracker.clone(),
            audit_log: conversation.audit_log.clone(),
            active_goal: conversation
                .goal_manager
                .as_ref()
                .and_then(|manager| manager.current()),
            base_tool_context: conversation.create_tool_context(),
        }
    }

    fn tool_context(
        &self,
        trace: &Option<TraceCollector>,
        retained_context: &ToolContextRetainedContext,
    ) -> ToolContext {
        let context = match trace {
            Some(trace) => self
                .base_tool_context
                .clone()
                .with_trace_collector(trace.clone()),
            None => self.base_tool_context.clone(),
        };
        context.with_retained_context(retained_context.clone())
    }
}

enum ToolExecutionGateOutcome {
    Allow(Box<ActionReview>),
    Deny(ToolResult),
}

struct ReadOnlyJobInput<'a> {
    trace: &'a Option<TraceCollector>,
    runtime_context: &'a ToolRuntimeContext,
    retained_context: &'a ToolContextRetainedContext,
    tool_call: &'a ToolCall,
    action_review: ActionReview,
    parent_tool_calls: Vec<ToolCall>,
    parent_assistant_content: String,
}

#[derive(Debug, Clone)]
struct ToolRuntimeContext {
    has_route: bool,
    route_intent: String,
    route_workflow: String,
    route_retrieval: String,
    route_reasoning: String,
    route_risk: String,
    policy_latency: String,
    policy_parallelism_limit: usize,
    policy_max_tool_calls: usize,
    policy_context_budget_tokens: usize,
    policy_allow_fallback_model: bool,
    policy_cost_ceiling_usd: String,
    action_checkpoint_active: bool,
    has_changes_before_tools: bool,
    task_stage: AgentTaskStage,
    route_workflow_kind: Option<WorkflowKind>,
    route_risk_level: Option<RiskLevel>,
    no_progress_rounds: usize,
    exposed_tools_count: usize,
    retained_retrieval_items: usize,
    retained_skill_triggers: usize,
    retained_context_tokens: usize,
    retained_context_provenance: Vec<String>,
    retained_context_items: Vec<ToolContextRetentionItem>,
    observer_signal: Option<ObserverActionSignal>,
}

struct ToolRuntimeContextInput<'a> {
    route: Option<&'a IntentRoute>,
    policy: &'a ResourcePolicy,
    task_stage: AgentTaskStage,
    action_checkpoint_active: bool,
    no_progress_rounds: usize,
    has_changes_before_tools: bool,
    exposed_tools_count: usize,
    retained_context: &'a ToolContextRetainedContext,
    task_state: Option<&'a AgentTaskState>,
}

#[derive(Debug, Clone, Default)]
struct ObserverActionSignal {
    uncertainty_not_reduced_steps: usize,
    candidate_focus: Vec<String>,
    key_findings: usize,
    consecutive_validation_failures: usize,
    validation_verified: bool,
    risks: usize,
    last_progress_signal: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ToolRuntimeTiming {
    started_at_unix_ms: Option<u64>,
    finished_at_unix_ms: Option<u64>,
}

impl ToolRuntimeTiming {
    fn instant() -> Self {
        let now = unix_time_millis();
        Self {
            started_at_unix_ms: now,
            finished_at_unix_ms: now,
        }
    }

    fn finished(started_at_unix_ms: Option<u64>) -> Self {
        Self {
            started_at_unix_ms,
            finished_at_unix_ms: unix_time_millis(),
        }
    }
}

impl ToolRuntimeContext {
    fn new(input: ToolRuntimeContextInput<'_>) -> Self {
        let ToolRuntimeContextInput {
            route,
            policy,
            task_stage,
            action_checkpoint_active,
            no_progress_rounds,
            has_changes_before_tools,
            exposed_tools_count,
            retained_context,
            task_state,
        } = input;

        Self {
            has_route: route.is_some(),
            route_intent: route
                .map(|route| serde_label(&route.intent))
                .unwrap_or_default(),
            route_workflow: route
                .map(|route| serde_label(&route.workflow))
                .unwrap_or_default(),
            route_retrieval: route
                .map(|route| serde_label(&route.retrieval))
                .unwrap_or_default(),
            route_reasoning: route
                .map(|route| serde_label(&route.reasoning))
                .unwrap_or_default(),
            route_risk: route
                .map(|route| serde_label(&route.risk))
                .unwrap_or_default(),
            policy_latency: serde_label(&policy.latency),
            policy_parallelism_limit: policy.parallelism_limit,
            policy_max_tool_calls: policy.max_tool_calls,
            policy_context_budget_tokens: policy.context_budget_tokens,
            policy_allow_fallback_model: policy.allow_fallback_model,
            policy_cost_ceiling_usd: format!("{:.4}", policy.cost_ceiling_usd),
            action_checkpoint_active,
            has_changes_before_tools,
            task_stage,
            route_workflow_kind: route.map(|route| route.workflow),
            route_risk_level: route.map(|route| route.risk),
            no_progress_rounds,
            exposed_tools_count,
            retained_retrieval_items: retained_context.retrieval_items.len(),
            retained_skill_triggers: retained_context.skill_triggers.len(),
            retained_context_tokens: retained_context.token_estimate,
            retained_context_provenance: retained_context.provenance.clone(),
            retained_context_items: retained_context.retrieval_items.clone(),
            observer_signal: task_state.map(ObserverActionSignal::from_task_state),
        }
    }

    fn attach(
        &self,
        result: &mut ToolResult,
        parallel: bool,
        pre_executed: bool,
        timing: Option<ToolRuntimeTiming>,
    ) {
        let route = if self.has_route {
            serde_json::json!({
                "intent": self.route_intent,
                "workflow": self.route_workflow,
                "retrieval": self.route_retrieval,
                "reasoning": self.route_reasoning,
                "risk": self.route_risk,
            })
        } else {
            serde_json::Value::Null
        };
        merge_tool_result_metadata(
            result,
            "tool_runtime",
            serde_json::json!({
                "route": route,
                "policy": {
                    "latency": self.policy_latency,
                    "parallelism_limit": self.policy_parallelism_limit,
                    "max_tool_calls": self.policy_max_tool_calls,
                    "context_budget_tokens": self.policy_context_budget_tokens,
                    "allow_fallback_model": self.policy_allow_fallback_model,
                    "cost_ceiling_usd": self.policy_cost_ceiling_usd,
                },
                "execution": {
                    "parallel": parallel,
                    "pre_executed": pre_executed,
                    "task_stage": format!("{:?}", self.task_stage),
                    "action_checkpoint_active": self.action_checkpoint_active,
                    "no_progress_rounds": self.no_progress_rounds,
                    "has_changes_before_tools": self.has_changes_before_tools,
                    "exposed_tools_count": self.exposed_tools_count,
                    "started_at_unix_ms": timing.and_then(|timing| timing.started_at_unix_ms),
                    "finished_at_unix_ms": timing.and_then(|timing| timing.finished_at_unix_ms),
                },
                "retained_context": {
                    "retrieval_items": self.retained_retrieval_items,
                    "skill_triggers": self.retained_skill_triggers,
                    "token_estimate": self.retained_context_tokens,
                    "provenance": self.retained_context_provenance,
                },
            }),
        );
    }

    fn action_decision(&self, tool_call: &ToolCall) -> ActionDecision {
        let mut decision = ActionDecision::for_tool_call(
            tool_call,
            ActionDecisionInput {
                task_stage: self.task_stage,
                route_workflow: self.route_workflow_kind,
                route_risk: self.route_risk_level,
                action_checkpoint_active: self.action_checkpoint_active,
                has_changes_before_tools: self.has_changes_before_tools,
                no_progress_rounds: self.no_progress_rounds,
            },
        );
        apply_memory_action_signal(&mut decision, tool_call, &self.retained_context_items);
        if let Some(observer_signal) = &self.observer_signal {
            apply_observer_action_signal(&mut decision, tool_call, observer_signal);
        }
        decision
    }

    fn attach_action_decision(&self, tool_call: &ToolCall, result: &mut ToolResult) {
        let decision = self.action_decision(tool_call);
        if let Ok(value) = serde_json::to_value(&decision) {
            merge_tool_result_metadata(result, "action_decision", value);
        }
        ToolResultNormalizer::attach_observation_metadata(tool_call, result);
    }
}

fn unix_time_millis() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn serde_label<T>(value: &T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{value:?}"))
}

impl ObserverActionSignal {
    fn from_task_state(task_state: &AgentTaskState) -> Self {
        Self {
            uncertainty_not_reduced_steps: task_state.uncertainty_not_reduced_steps,
            candidate_focus: task_state
                .candidate_focus
                .iter()
                .rev()
                .take(5)
                .map(|focus| focus.target.clone())
                .collect(),
            key_findings: task_state.key_findings.len(),
            consecutive_validation_failures: task_state.consecutive_validation_failures,
            validation_verified: task_state.verification_plan.status
                == crate::engine::task_context::VerificationStatus::Verified,
            risks: task_state.risks.len(),
            last_progress_signal: task_state.last_progress_signal.clone(),
        }
    }
}

fn apply_observer_action_signal(
    decision: &mut ActionDecision,
    tool_call: &ToolCall,
    signal: &ObserverActionSignal,
) {
    let tool_name = tool_call.name.as_str();
    let text = format!(
        "{} {}",
        tool_name,
        serde_json::to_string(&tool_call.arguments).unwrap_or_default()
    )
    .to_lowercase();

    if signal.uncertainty_not_reduced_steps >= 2
        && matches!(tool_name, "file_read" | "grep" | "glob" | "bash")
    {
        decision.apply_score_modifier(
            ActionScoreModifier::new(
                ActionScoreModifierSource::Observer,
                "uncertainty_not_reduced",
                "recent observations did not reduce uncertainty",
            )
            .uncertainty_reduction(-2)
            .cost(1),
        );
    }

    if !signal.candidate_focus.is_empty()
        && signal
            .candidate_focus
            .iter()
            .any(|focus| text.contains(&focus.to_lowercase()))
    {
        decision.apply_score_modifier(
            ActionScoreModifier::new(
                ActionScoreModifierSource::Observer,
                "candidate_focus_match",
                "action targets recent observer candidate focus",
            )
            .scope_fit(2)
            .uncertainty_reduction(1),
        );
    }

    if signal.consecutive_validation_failures > 0
        && matches!(
            tool_name,
            "file_read" | "grep" | "run_tests" | "bash" | "file_edit" | "file_patch"
        )
    {
        decision.apply_score_modifier(
            ActionScoreModifier::new(
                ActionScoreModifierSource::Observer,
                "validation_failure_focus",
                "recent validation failure raises value of focused repair evidence",
            )
            .value(1)
            .uncertainty_reduction(1),
        );
    }

    if signal.validation_verified
        && matches!(
            tool_name,
            "file_edit" | "file_write" | "file_patch" | "format" | "bash"
        )
    {
        decision.apply_score_modifier(
            ActionScoreModifier::new(
                ActionScoreModifierSource::Observer,
                "validated_mutation_penalty",
                "validated work should prefer closeout over more mutation",
            )
            .value(-2)
            .risk(1)
            .scope_fit(-2),
        );
    }

    if signal.risks > 0 && decision.action.mutates_workspace {
        decision.apply_score_modifier(
            ActionScoreModifier::new(
                ActionScoreModifierSource::Observer,
                "active_risk_mutation",
                "active task risks increase mutation caution",
            )
            .risk(1),
        );
    }

    if signal.key_findings > 0
        && !signal.candidate_focus.is_empty()
        && matches!(tool_name, "file_edit" | "file_patch" | "run_tests" | "bash")
    {
        decision.apply_score_modifier(
            ActionScoreModifier::new(
                ActionScoreModifierSource::Observer,
                "finding_to_action",
                "recent key findings support targeted edit or validation",
            )
            .value(1)
            .scope_fit(1),
        );
    }

    if let Some(progress) = &signal.last_progress_signal {
        if !progress.trim().is_empty() && matches!(tool_name, "diff" | "git_diff" | "run_tests") {
            decision.apply_score_modifier(
                ActionScoreModifier::new(
                    ActionScoreModifierSource::Observer,
                    "progress_validation",
                    "recent progress makes verification evidence more valuable",
                )
                .value(1)
                .scope_fit(1),
            );
        }
    }

    if decision
        .score_computation
        .modifiers
        .iter()
        .any(|modifier| modifier.source == ActionScoreModifierSource::Observer)
    {
        decision.trace_recommended = true;
        decision.reason_summary = format!("{}; observer modifier applied", decision.reason_summary);
    }
}

fn apply_memory_action_signal(
    decision: &mut ActionDecision,
    tool_call: &ToolCall,
    retained_items: &[ToolContextRetentionItem],
) {
    let signal = memory_action_signal(tool_call, retained_items);
    if signal.is_empty() {
        return;
    }
    decision.apply_score_modifier(
        ActionScoreModifier::new(
            ActionScoreModifierSource::Memory,
            signal.kind,
            signal.reasons.join("; "),
        )
        .value(signal.value_delta)
        .risk(signal.risk_delta)
        .uncertainty_reduction(signal.uncertainty_reduction_delta)
        .scope_fit(signal.scope_fit_delta),
    );
    decision.trace_recommended = true;
    decision.reason_summary = format!(
        "{}; memory modifier value_delta={} risk_delta={} uncertainty_delta={} scope_delta={} ({})",
        decision.reason_summary,
        signal.value_delta,
        signal.risk_delta,
        signal.uncertainty_reduction_delta,
        signal.scope_fit_delta,
        signal.reasons.join("; ")
    );
}

#[derive(Debug, Clone, Default)]
struct MemoryActionSignal {
    kind: String,
    value_delta: i8,
    risk_delta: i8,
    uncertainty_reduction_delta: i8,
    scope_fit_delta: i8,
    reasons: Vec<String>,
}

impl MemoryActionSignal {
    fn is_empty(&self) -> bool {
        self.value_delta == 0
            && self.risk_delta == 0
            && self.uncertainty_reduction_delta == 0
            && self.scope_fit_delta == 0
    }
}

fn memory_action_signal(
    tool_call: &ToolCall,
    retained_items: &[ToolContextRetentionItem],
) -> MemoryActionSignal {
    let mut signal = MemoryActionSignal {
        kind: "memory_context".to_string(),
        ..Default::default()
    };
    let tool_name = tool_call.name.as_str();
    let validation_or_inspection = matches!(
        tool_name,
        "file_read" | "grep" | "glob" | "bash" | "run_tests" | "diff" | "git_status" | "git_diff"
    );

    for item in retained_items {
        if item.source != "Memory" {
            continue;
        }
        let text = format!("{} {} {}", item.title, item.provenance, item.reason).to_lowercase();
        let trusted = matches!(item.trust.as_str(), "High" | "Verified" | "Trusted");
        let memory_id = item
            .provenance
            .split_whitespace()
            .find(|part| part.contains("memory_record/"))
            .unwrap_or(item.provenance.as_str());

        if item.conflict {
            signal.risk_delta = signal.risk_delta.saturating_add(1).min(2);
            signal.uncertainty_reduction_delta =
                signal.uncertainty_reduction_delta.saturating_add(1).min(2);
            signal.kind = "memory_conflict_uncertainty".to_string();
            signal
                .reasons
                .push(format!("{} is conflicting memory context", memory_id));
        }
        if contains_any_local(&text, &["stale", "outdated", "过期", "旧"]) {
            signal.value_delta = signal.value_delta.saturating_sub(1).max(-2);
            signal.scope_fit_delta = signal.scope_fit_delta.saturating_sub(1).max(-2);
            signal.kind = "memory_stale_penalty".to_string();
            signal.reasons.push(format!("{} appears stale", memory_id));
        }
        if contains_any_local(
            &text,
            &[
                "failure",
                "failed",
                "risk",
                "rollback",
                "strategy-failures",
                "never",
                "avoid",
                "失败",
                "回滚",
                "禁止",
            ],
        ) && (tool_name == "file_edit"
            || tool_name == "file_write"
            || tool_name == "file_patch"
            || tool_name == "format"
            || tool_name == "bash")
        {
            let delta = if trusted { 2 } else { 1 };
            signal.risk_delta = signal.risk_delta.saturating_add(delta).min(3);
            signal.kind = "memory_failure_risk".to_string();
            signal
                .reasons
                .push(format!("{} warns about prior failure", memory_id));
        }
        if contains_any_local(
            &text,
            &[
                "successful",
                "strategy",
                "diagnostic",
                "validate",
                "targeted",
                "fix",
                "成功",
                "策略",
                "验证",
            ],
        ) && validation_or_inspection
        {
            let delta = if trusted { 2 } else { 1 };
            signal.value_delta = signal.value_delta.saturating_add(delta).min(3);
            signal.scope_fit_delta = signal.scope_fit_delta.saturating_add(1).min(2);
            signal.kind = "memory_success_value".to_string();
            signal.reasons.push(format!(
                "{} supports diagnostic/validation action",
                memory_id
            ));
        }
        if contains_any_local(&text, &["project", "repo", "workspace", "项目", "仓库"]) {
            signal.scope_fit_delta = signal.scope_fit_delta.saturating_add(1).min(2);
            if signal.kind == "memory_context" {
                signal.kind = "memory_project_fit".to_string();
            }
            signal
                .reasons
                .push(format!("{} is project-scoped memory", memory_id));
        }
    }
    signal.reasons.sort();
    signal.reasons.dedup();
    signal
}

fn contains_any_local(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn record_action_decision_if_needed(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    decision: &ActionDecision,
) {
    if !decision.trace_recommended && !mva_runtime_profile_enabled() {
        return;
    }
    if let Some(trace) = trace {
        trace.record(TraceEvent::ActionDecisionEvaluated {
            tool: tool_call.name.clone(),
            call_id: tool_call.id.clone(),
            stage: format!("{:?}", decision.action.stage),
            value: decision.scores.value,
            risk: decision.scores.risk,
            uncertainty_reduction: decision.scores.uncertainty_reduction,
            cost: decision.scores.cost,
            reversibility: decision.scores.reversibility,
            scope_fit: decision.scores.scope_fit,
            action_score: decision.scores.action_score,
            formula_stage: decision
                .score_computation
                .formula_stage
                .as_str()
                .to_string(),
            formula_version: decision.score_computation.formula_version.clone(),
            phase_aligned: decision.action.phase_aligned,
            mutates_workspace: decision.action.mutates_workspace,
            broad_shell: decision.action.broad_shell,
            modifiers: decision
                .score_computation
                .modifiers
                .iter()
                .filter_map(|modifier| serde_json::to_value(modifier).ok())
                .collect(),
            requires_confirmation: decision.requires_confirmation,
            reason: decision.reason_summary.clone(),
        });
    }
}

fn mva_runtime_profile_enabled() -> bool {
    matches!(
        std::env::var("PRIORITY_AGENT_RUNTIME_PROFILE")
            .ok()
            .as_deref(),
        Some("minimum_viable_agent" | "mva")
    )
}

fn attach_action_review_metadata(result: &mut ToolResult, review: &ActionReview) {
    let observed_checkpoint_id = result
        .data
        .as_ref()
        .and_then(|data| data.get("checkpoint"))
        .and_then(|checkpoint| checkpoint.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut metadata = review.metadata();
    if let Some(checkpoint_id) = observed_checkpoint_id {
        if let Some(checkpoint) = metadata
            .get_mut("checkpoint")
            .and_then(serde_json::Value::as_object_mut)
        {
            checkpoint.insert(
                "status".to_string(),
                serde_json::Value::String("required_and_present".to_string()),
            );
            checkpoint.insert(
                "checkpoint_id".to_string(),
                serde_json::Value::String(checkpoint_id),
            );
            checkpoint.insert(
                "observed_result_checkpoint".to_string(),
                serde_json::Value::Bool(true),
            );
        }
    }
    merge_tool_result_metadata(result, "action_review", metadata);
}

fn record_action_review(trace: &Option<TraceCollector>, review: &ActionReview) {
    if let Some(trace) = trace {
        let network = serde_json::to_value(review.side_effects.network.class)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{:?}", review.side_effects.network.class));
        let external_effect = serde_json::to_value(review.side_effects.external_side_effect)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{:?}", review.side_effects.external_side_effect));
        trace.record(TraceEvent::ActionReviewed {
            tool: review.tool.clone(),
            call_id: review.call_id.clone(),
            decision: review.decision.as_str().to_string(),
            reason: review.primary_reason.as_str().to_string(),
            permission: review.permission.decision.clone(),
            scope_allowed: review.scope.allowed,
            budget_allowed: review.budget.allowed,
            checkpoint: review.checkpoint.status.clone(),
            network,
            external_effect,
            recovery: review.model_recovery.clone(),
        });
    }
}

fn record_tool_observation(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(trace) = trace else {
        return;
    };
    let Some(observation) = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_observation"))
    else {
        return;
    };
    let files_read = observation
        .get("files_read")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let files_changed = observation
        .get("files_changed")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let key_findings = observation
        .get("key_findings")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let evidence_items = observation
        .get("evidence")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let quality_warning_labels = observation
        .get("quality_warnings")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let quality_warnings = quality_warning_labels.len();
    let context_policy = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_result_context_policy"));
    trace.record(TraceEvent::ToolObservationRecorded {
        tool: tool_call.name.clone(),
        call_id: tool_call.id.clone(),
        status: observation
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(if result.success { "success" } else { "failed" })
            .to_string(),
        result_kind: observation
            .get("result_kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("generic")
            .to_string(),
        model_visibility: context_policy
            .and_then(|policy| policy.get("model_visibility"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        include_in_next_context: observation
            .get("include_in_next_context")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        store_in_state: observation
            .get("store_in_state")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        key_findings,
        evidence_items,
        failure_type: observation
            .get("failure_type")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        recovery_plan_id: observation
            .get("recovery_plan_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        recovery_kind: observation
            .get("recovery_kind")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        raw_result_ref: observation
            .get("raw_result_ref")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        quality_warnings,
        quality_warning_labels,
        files_read,
        files_changed,
        checkpoint_id: observation
            .get("checkpoint_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        summary: observation
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
    });
}

struct ToolExecutionGate<'a> {
    tool_registry: &'a ToolRegistry,
    active_goal: Option<&'a crate::engine::session_goal::SessionGoal>,
    task_state: Option<&'a AgentTaskState>,
    allowed_tools: &'a Option<HashSet<String>>,
    resource_policy: &'a ResourcePolicy,
    exposed_tool_names: &'a HashSet<String>,
    action_checkpoint_active: bool,
    action_checkpoint_lookup_count: usize,
    has_changes_before_tools: bool,
    destructive_scope: &'a DestructiveScopeContract,
    working_dir: &'a Path,
    trace: &'a Option<TraceCollector>,
    runtime_context: &'a ToolRuntimeContext,
    permission_context: &'a crate::permissions::PermissionContext,
}

impl<'a> ToolExecutionGate<'a> {
    fn evaluate(&self, tool_call: &ToolCall, scheduled_count: usize) -> ToolExecutionGateOutcome {
        let decision = self.runtime_context.action_decision(tool_call);
        record_action_decision_if_needed(self.trace, tool_call, &decision);
        let tool = self.tool_registry.get(&tool_call.name);
        let context_allows_tool = tool_allowed_by_context(self.allowed_tools, &tool_call.name);
        let destructive_check = self
            .destructive_scope
            .check_tool_call(tool_call, self.working_dir);
        let action_checkpoint_rejection = self.action_checkpoint_rejection(tool_call);
        let review = ActionReview::build(ActionReviewInput {
            tool_call,
            tool,
            exposed_tool_names: self.exposed_tool_names,
            scheduled_count,
            max_tool_calls: self.resource_policy.max_tool_calls,
            action_decision: decision,
            permission_context: Some(self.permission_context),
            task_state: self.task_state,
            working_dir: Some(self.working_dir),
            tool_allowed_by_context: context_allows_tool,
            destructive_scope_check: Some(destructive_check.clone()),
            action_checkpoint_rejection: action_checkpoint_rejection.clone(),
        });
        record_action_review(self.trace, &review);

        if !review.tool_contract.available {
            let result = ToolResult::error(format!("Tool '{}' not found", tool_call.name));
            return self.deny_with_trace(tool_call, result, &review);
        }

        if !review.tool_contract.exposed {
            let error = if self.action_checkpoint_active {
                ConversationLoop::action_checkpoint_unexposed_tool_message(
                    &tool_call.name,
                    self.exposed_tool_names,
                    self.action_checkpoint_lookup_count,
                )
            } else {
                format!(
                    "Tool '{}' was not exposed in the current request and cannot be executed.",
                    tool_call.name
                )
            };
            return self.deny_with_trace(tool_call, ToolResult::error(error), &review);
        }

        if let Some(error) = review.tool_contract.validation_error.clone() {
            return self.deny_with_trace(
                tool_call,
                invalid_tool_params_result(tool_call, error),
                &review,
            );
        }

        if !review.budget.allowed {
            let result = ToolResult::error(format!(
                "Resource policy blocked tool '{}': max tool calls ({}) reached.",
                tool_call.name, self.resource_policy.max_tool_calls
            ));
            return self.deny_with_trace(tool_call, result, &review);
        }

        record_goal_drift_if_needed(self.trace, self.active_goal, self.task_state, tool_call);

        if !context_allows_tool {
            return self.deny_with_trace(tool_call, tool_not_allowed_result(tool_call), &review);
        }

        if destructive_check.applies {
            if let Some(ref trace) = self.trace {
                trace.record(TraceEvent::DestructiveScopeChecked {
                    tool: tool_call.name.clone(),
                    call_id: tool_call.id.clone(),
                    operation: destructive_check.operation.clone(),
                    target: destructive_check.target.clone(),
                    allowed: destructive_check.allowed,
                    reason: destructive_check.reason.clone(),
                });
            }
            if !destructive_check.allowed {
                let result = ToolResult::error(format!(
                    "Destructive scope blocked: {}",
                    destructive_check.reason
                ));
                return self.deny_with_trace(tool_call, result, &review);
            }
        }

        if let Some(reason) = action_checkpoint_rejection {
            let result = if tool_call.name == "file_edit" {
                ToolResult::error(format!("Action checkpoint file_edit rejected: {reason}"))
            } else {
                ToolResult::error(reason)
            };
            return self.deny_with_trace(tool_call, result, &review);
        }

        if review.decision.blocks_execution() {
            let result = ToolResult::error(format!(
                "{}\nRecovery: {}",
                review.user_reason, review.model_recovery
            ));
            return self.deny_with_trace(tool_call, result, &review);
        }

        ToolExecutionGateOutcome::Allow(Box::new(review))
    }

    fn action_checkpoint_rejection(&self, tool_call: &ToolCall) -> Option<String> {
        if !self.action_checkpoint_active {
            return None;
        }

        if tool_call.name == "bash"
            && !ConversationLoop::bash_allowed_at_action_checkpoint(
                &tool_call.arguments,
                self.has_changes_before_tools,
            )
        {
            return Some(
                "Bash is restricted during the action checkpoint: use file_edit/file_write/file_patch for patches so permission, stale-read, diff, and rollback checks stay active. Bash is allowed only for validation after files have changed."
                    .to_string(),
            );
        }

        if tool_call.name == "file_edit" {
            return ConversationLoop::action_checkpoint_file_edit_rejection(
                &tool_call.arguments,
                &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            );
        }

        None
    }

    fn deny_with_trace(
        &self,
        tool_call: &ToolCall,
        mut result: ToolResult,
        review: &ActionReview,
    ) -> ToolExecutionGateOutcome {
        attach_tool_execution_metadata(tool_call, &mut result);
        if let Some(tool) = self.tool_registry.get(&tool_call.name) {
            attach_tool_contract_metadata(tool, tool_call, &mut result);
        }
        attach_action_review_metadata(&mut result, review);
        self.runtime_context.attach(
            &mut result,
            false,
            false,
            Some(ToolRuntimeTiming::instant()),
        );
        self.runtime_context
            .attach_action_decision(tool_call, &mut result);
        record_tool_observation(self.trace, tool_call, &result);
        if let Some(ref trace) = self.trace {
            trace.record(TraceEvent::ToolStarted {
                tool: tool_call.name.clone(),
                call_id: tool_call.id.clone(),
                parallel: false,
                pre_executed: false,
            });
            trace.record(TraceEvent::ToolCompleted {
                tool: tool_call.name.clone(),
                call_id: tool_call.id.clone(),
                success: false,
                duration_ms: Some(0),
                output_chars: result.content.chars().count(),
            });
            let (reason, terminal_status, action) = match review.decision {
                crate::engine::action_review::ActionReviewDecision::Deny => {
                    ("action_denied", "blocked", "stop")
                }
                crate::engine::action_review::ActionReviewDecision::Revise => {
                    ("action_needs_revision", "blocked", "replan")
                }
                crate::engine::action_review::ActionReviewDecision::AskUser => {
                    ("high_risk_needs_user", "needs_user", "ask_user")
                }
                crate::engine::action_review::ActionReviewDecision::Allow => {
                    ("no_issue", "missing", "continue")
                }
            };
            trace.record(TraceEvent::StopCheckEvaluated {
                status: "stop".to_string(),
                reason: reason.to_string(),
                stage: "PreAction".to_string(),
                terminal_status: Some(terminal_status.to_string()),
                action: action.to_string(),
                no_code_progress_rounds: self.runtime_context.no_progress_rounds,
                action_checkpoint_active: self.action_checkpoint_active,
                summary: review.user_reason.clone(),
                evidence_items: review.reasons.len().max(1),
                failure_type: Some(review.primary_reason.as_str().to_string()),
                recovery_plan_id: None,
                rollback_recommended: false,
                next_action: Some(review.model_recovery.clone()),
            });
        }
        ToolExecutionGateOutcome::Deny(result)
    }
}

pub(super) struct ToolExecutionController {
    context: ToolExecutionContext,
}

impl ToolExecutionController {
    pub(super) fn new(context: ToolExecutionContext) -> Self {
        Self { context }
    }

    fn read_only_job(
        &self,
        input: ReadOnlyJobInput<'_>,
    ) -> impl Future<Output = (ToolCall, ToolResult)> + 'static {
        let execution = &self.context;
        let registry = execution.tool_registry.clone();
        let tc_clone = input.tool_call.clone();
        let tool_name = input.tool_call.name.clone();
        let context = execution
            .tool_context(input.trace, input.retained_context)
            .with_tool_call_metadata(tool_name.clone(), tc_clone.id.clone())
            .with_parent_assistant_tool_calls(
                input.parent_tool_calls,
                input.parent_assistant_content,
            );
        let cost_tracker = execution.cost_tracker.clone();
        let hook_manager = execution.hook_manager.clone();
        let trace = input.trace.clone();
        let runtime_context = input.runtime_context.clone();
        let action_review = input.action_review;
        async move {
            let started_at = std::time::Instant::now();
            let started_at_unix_ms = unix_time_millis();
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolStarted {
                    tool: tool_name.clone(),
                    call_id: tc_clone.id.clone(),
                    parallel: true,
                    pre_executed: false,
                });
            }
            let pre_decision = if let Some(ref hooks) = hook_manager {
                let hook_start = hooks.current_record_sequence();
                let decision = hooks.run_pre_tool(&tc_clone, &context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc_clone.id);
                record_hook_traces(&trace, &hook_records);
                decision
            } else {
                HookDecision {
                    allow: true,
                    reason: None,
                }
            };

            let mut result = if !pre_decision.allow {
                ToolResult::error(
                    pre_decision
                        .reason
                        .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                )
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
                let hook_start = hooks.current_record_sequence();
                hooks.run_post_tool(&tc_clone, &result, &context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc_clone.id);
                record_hook_traces(&trace, &hook_records);
            };
            attach_tool_execution_metadata(&tc_clone, &mut result);
            if let Some(tool) = registry.get(&tc_clone.name) {
                attach_tool_contract_metadata(tool, &tc_clone, &mut result);
            }
            attach_action_review_metadata(&mut result, &action_review);
            runtime_context.attach(
                &mut result,
                true,
                false,
                Some(ToolRuntimeTiming::finished(started_at_unix_ms)),
            );
            runtime_context.attach_action_decision(&tc_clone, &mut result);
            record_tool_observation(&trace, &tc_clone, &result);
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
                let trace_ref = Some(trace.clone());
                record_mcp_resource_trace(&trace_ref, &tc_clone, &result);
                record_remote_bridge_trace(&trace_ref, &tc_clone, &result);
                record_permission_denial_recovery_trace(&trace_ref, &tc_clone, &result);
                record_web_retrieval_trace(&trace_ref, &tc_clone, &result);
            }
            (tc_clone, result)
        }
    }

    async fn collect_read_only_results<F>(
        &self,
        read_only_jobs: Vec<(usize, F)>,
        concurrency: usize,
        tx: Option<&mpsc::Sender<StreamEvent>>,
        lifecycle: &mut ToolCallLifecycle,
    ) -> Vec<(ToolCall, ToolResult)>
    where
        F: Future<Output = (ToolCall, ToolResult)>,
    {
        let execution = &self.context;
        let mut completed = Vec::new();
        let mut readonly_stream =
            futures::stream::iter(read_only_jobs.into_iter().map(|(order, job)| async move {
                let result = job.await;
                (order, result)
            }))
            .buffer_unordered(concurrency);

        while let Some((order, (tc, result))) = readonly_stream.next().await {
            lifecycle.completed(&tc, &result);
            persist_tool_outcome_learning_event(
                execution.session_store.as_ref(),
                &execution.session_id,
                &tc,
                &result,
            );
            if let Some(tx) = tx {
                let result_content = ToolResultNormalizer::normalize(&tc, &result).ui_content;
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tc.id.clone(),
                        result: result_content,
                        metadata: tool_completion_metadata(&result),
                    })
                    .await;
            }
            completed.push((order, (tc, result)));
        }

        completed.sort_by_key(|(order, _)| *order);
        completed.into_iter().map(|(_, result)| result).collect()
    }

    async fn execute_read_write_calls(
        &self,
        read_write_calls: Vec<(ToolCall, ActionReview)>,
        exec_context: ReadWriteExecutionContext<'_>,
        lifecycle: &mut ToolCallLifecycle,
    ) -> Vec<(ToolCall, ToolResult)> {
        let execution = &self.context;
        let mut results = Vec::new();

        for (tc, action_review) in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if !tool_allowed_by_context(&execution.allowed_tools, &tool_name) {
                let mut result = tool_not_allowed_result(&tc);
                attach_action_review_metadata(&mut result, &action_review);
                exec_context.runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::instant()),
                );
                exec_context
                    .runtime_context
                    .attach_action_decision(&tc, &mut result);
                record_tool_observation(exec_context.trace, &tc, &result);
                persist_tool_outcome_learning_event(
                    execution.session_store.as_ref(),
                    &execution.session_id,
                    &tc,
                    &result,
                );
                lifecycle.denied(&tc);
                results.push((tc, result));
                continue;
            }
            lifecycle.running(&tc, false, false);

            if let Some(tx) = exec_context.tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                        metadata: tool_execution_start_metadata(&tool_name, &tc.arguments),
                    })
                    .await;
            }
            if let Some(ref trace) = exec_context.trace {
                trace.record(TraceEvent::ToolStarted {
                    tool: tool_name.clone(),
                    call_id: tool_id.clone(),
                    parallel: false,
                    pre_executed: false,
                });
            }

            let (result, hook_context) = if let Some(tool) = execution.tool_registry.get(&tool_name)
            {
                let mut context = execution
                    .tool_context(exec_context.trace, exec_context.retained_context)
                    .with_tool_call_metadata(tool_name.clone(), tool_id.clone())
                    .with_parent_assistant_tool_calls(
                        exec_context.parent_tool_calls.to_vec(),
                        exec_context.parent_assistant_content.to_string(),
                    );
                let drift_check = crate::engine::goal_drift::GoalDriftDetector::new()
                    .check_with_context(
                        crate::engine::goal_drift::GoalDriftContext {
                            goal: execution.active_goal.as_ref(),
                            task_state: exec_context.task_state,
                        },
                        &tc,
                    );
                let pre_decision = if let Some(ref hooks) = execution.hook_manager {
                    let hook_start = hooks.current_record_sequence();
                    let decision = hooks.run_pre_tool(&tc, &context).await;
                    let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                    record_hook_traces(exec_context.trace, &hook_records);
                    decision
                } else {
                    HookDecision {
                        allow: true,
                        reason: None,
                    }
                };

                let started_at = std::time::Instant::now();
                let started_at_unix_ms = unix_time_millis();
                let permission_evaluation = PermissionController::evaluate_tool_permission(
                    &execution.session_id,
                    &tc,
                    tool,
                    &context,
                    &drift_check,
                );
                let mut result = if !pre_decision.allow {
                    ToolResult::error(
                        pre_decision
                            .reason
                            .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                    )
                } else if permission_evaluation.requires_approval {
                    let permission_outcome = PermissionController::request_user_permission(
                        &tc,
                        &permission_evaluation,
                        PermissionRequestRuntime {
                            approval_channel: execution.approval_channel.as_ref(),
                            tx: exec_context.tx,
                            trace: exec_context.trace,
                            hook_manager: execution.hook_manager.as_ref(),
                            context: &context,
                            action_review: Some(&action_review),
                        },
                    )
                    .await;
                    if permission_outcome.approved {
                        PermissionController::record_approved_session_rule(
                            &mut context,
                            &tool_name,
                        );
                        if let Some(tx) = exec_context.tx {
                            let _ = tx
                                .send(StreamEvent::ToolExecutionProgress {
                                    id: tool_id.clone(),
                                    progress: tool_execution_start_progress(
                                        &tool_name,
                                        &tc.arguments,
                                    ),
                                })
                                .await;
                        }
                        let mut result = tool.execute(tc.arguments.clone(), context.clone()).await;
                        if let Some(record) = permission_evaluation.record.as_ref() {
                            merge_tool_result_metadata(
                                &mut result,
                                "permission_request",
                                record.to_json_with_approval_source(
                                    true,
                                    Some(&permission_outcome.source),
                                ),
                            );
                        }
                        result
                    } else {
                        PermissionController::denied_result(
                            &tool_name,
                            permission_evaluation.record.as_ref(),
                            Some(&permission_outcome.source),
                        )
                    }
                } else {
                    if let Some(tx) = exec_context.tx {
                        let _ = tx
                            .send(StreamEvent::ToolExecutionProgress {
                                id: tool_id.clone(),
                                progress: tool_execution_start_progress(&tool_name, &tc.arguments),
                            })
                            .await;
                    }
                    tool.execute(tc.arguments.clone(), context.clone()).await
                };
                let duration_ms = started_at.elapsed().as_millis() as u64;
                if result.duration_ms.is_none() {
                    result.duration_ms = Some(duration_ms);
                }
                attach_tool_execution_metadata(&tc, &mut result);
                attach_tool_contract_metadata(tool, &tc, &mut result);
                attach_action_review_metadata(&mut result, &action_review);
                exec_context.runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::finished(started_at_unix_ms)),
                );
                exec_context
                    .runtime_context
                    .attach_action_decision(&tc, &mut result);
                record_tool_observation(exec_context.trace, &tc, &result);

                // ── Security Audit & Denial Tracking ──────────────────────
                let params_summary = if let Some(tool) = execution.tool_registry.get(&tool_name) {
                    tool.to_classifier_input(&tc.arguments)
                } else {
                    tool_name.clone()
                };

                if let Some(ref log) = execution.audit_log {
                    let decision = if result.success {
                        "EXECUTED"
                    } else if PermissionController::is_permission_denied(&result) {
                        "DENIED"
                    } else {
                        "FAILED"
                    };
                    log.log_execution(&tool_name, &params_summary, result.success, decision)
                        .await;
                }

                if let Some(ref tracker) = execution.denial_tracker {
                    if result.success {
                        tracker.record_success().await;
                    } else if PermissionController::is_permission_denied(&result)
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
                    let mut tracker = execution.cost_tracker.lock().await;
                    tracker.record_tool_execution(
                        &tool_name,
                        result.success,
                        duration_ms,
                        result.error.as_deref(),
                    );
                }

                (result, Some(context))
            } else {
                let mut result = ToolResult::error(format!("Tool '{}' not found", tool_name));
                attach_tool_execution_metadata(&tc, &mut result);
                if let Some(tool) = execution.tool_registry.get(&tc.name) {
                    attach_tool_contract_metadata(tool, &tc, &mut result);
                }
                attach_action_review_metadata(&mut result, &action_review);
                exec_context.runtime_context.attach(
                    &mut result,
                    false,
                    false,
                    Some(ToolRuntimeTiming::instant()),
                );
                exec_context
                    .runtime_context
                    .attach_action_decision(&tc, &mut result);
                record_tool_observation(exec_context.trace, &tc, &result);
                (result, None)
            };

            if let (Some(hooks), Some(context)) = (&execution.hook_manager, &hook_context) {
                let hook_start = hooks.current_record_sequence();
                hooks.run_post_tool(&tc, &result, context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                record_hook_traces(exec_context.trace, &hook_records);
            }

            if let Some(tx) = exec_context.tx {
                let result_content = ToolResultNormalizer::normalize(&tc, &result).ui_content;
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                        metadata: tool_completion_metadata(&result),
                    })
                    .await;
            }
            if let Some(ref trace) = exec_context.trace {
                trace.record(TraceEvent::ToolCompleted {
                    tool: tool_name,
                    call_id: tool_id,
                    success: result.success,
                    duration_ms: result.duration_ms,
                    output_chars: result.content.chars().count(),
                });
                let trace_ref = Some(trace.clone());
                record_mcp_resource_trace(&trace_ref, &tc, &result);
                record_remote_bridge_trace(&trace_ref, &tc, &result);
                record_permission_denial_recovery_trace(&trace_ref, &tc, &result);
                record_web_retrieval_trace(&trace_ref, &tc, &result);
            }
            if PermissionController::is_permission_denied(&result) {
                lifecycle.denied(&tc);
            } else {
                lifecycle.completed(&tc, &result);
            }
            persist_tool_outcome_learning_event(
                execution.session_store.as_ref(),
                &execution.session_id,
                &tc,
                &result,
            );
            results.push((tc, result));
        }

        results
    }

    pub(super) async fn execute_tools_parallel(
        &self,
        request: ToolExecutionRequest<'_>,
    ) -> ToolExecutionBatch {
        let ToolExecutionRequest {
            tool_calls,
            parent_assistant_content,
            tx,
            pre_executed,
            trace,
            route,
            resource_policy,
            exposed_tool_names,
            retained_context,
            task_stage,
            task_state,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            no_progress_rounds,
            has_changes_before_tools,
            destructive_scope,
            lifecycle,
        } = request;
        let execution = &self.context;
        let mut read_only_jobs = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();
        let mut scheduled_count = 0usize;
        let mut serial_boundary_seen = false;
        lifecycle.pending_batch(tool_calls);
        let runtime_context = ToolRuntimeContext::new(ToolRuntimeContextInput {
            route,
            policy: resource_policy,
            task_stage,
            action_checkpoint_active,
            no_progress_rounds,
            has_changes_before_tools,
            exposed_tools_count: exposed_tool_names.len(),
            retained_context,
            task_state,
        });
        let gate = ToolExecutionGate {
            tool_registry: execution.tool_registry.as_ref(),
            active_goal: execution.active_goal.as_ref(),
            task_state,
            allowed_tools: &execution.allowed_tools,
            resource_policy,
            exposed_tool_names,
            action_checkpoint_active,
            action_checkpoint_lookup_count,
            has_changes_before_tools,
            destructive_scope,
            working_dir: execution.base_tool_context.working_dir.as_path(),
            trace: &trace,
            runtime_context: &runtime_context,
            permission_context: &execution.base_tool_context.permission_context,
        };
        let concurrency =
            read_only_tool_concurrency().min(resource_policy.parallelism_limit.max(1));

        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }

            let concurrency_safe = tool_call_is_concurrency_safe(
                execution.tool_registry.as_ref(),
                &tc.name,
                &tc.arguments,
            );
            if !concurrency_safe && !read_only_jobs.is_empty() {
                let read_only_results = self
                    .collect_read_only_results(
                        std::mem::take(&mut read_only_jobs),
                        concurrency,
                        tx,
                        lifecycle,
                    )
                    .await;
                results.extend(read_only_results);
            }

            let action_review = match gate.evaluate(tc, scheduled_count) {
                ToolExecutionGateOutcome::Allow(review) => *review,
                ToolExecutionGateOutcome::Deny(result) => {
                    if !read_only_jobs.is_empty() {
                        let read_only_results = self
                            .collect_read_only_results(
                                std::mem::take(&mut read_only_jobs),
                                concurrency,
                                tx,
                                lifecycle,
                            )
                            .await;
                        results.extend(read_only_results);
                    }
                    persist_tool_outcome_learning_event(
                        execution.session_store.as_ref(),
                        &execution.session_id,
                        tc,
                        &result,
                    );
                    lifecycle.denied(tc);
                    results.push((tc.clone(), result));
                    scheduled_count += 1;
                    serial_boundary_seen = true;
                    continue;
                }
            };

            if let Some(pre_result) = pre_executed.get(&i).filter(|_| !serial_boundary_seen) {
                if !read_only_jobs.is_empty() {
                    let read_only_results = self
                        .collect_read_only_results(
                            std::mem::take(&mut read_only_jobs),
                            concurrency,
                            tx,
                            lifecycle,
                        )
                        .await;
                    results.extend(read_only_results);
                }
                let mut pre_result = pre_result.clone();
                attach_tool_execution_metadata(tc, &mut pre_result);
                if let Some(tool) = execution.tool_registry.get(&tc.name) {
                    attach_tool_contract_metadata(tool, tc, &mut pre_result);
                }
                attach_action_review_metadata(&mut pre_result, &action_review);
                runtime_context.attach(&mut pre_result, true, true, None);
                runtime_context.attach_action_decision(tc, &mut pre_result);
                record_tool_observation(&trace, tc, &pre_result);
                persist_tool_outcome_learning_event(
                    execution.session_store.as_ref(),
                    &execution.session_id,
                    tc,
                    &pre_result,
                );
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
                    let trace_ref = Some(trace.clone());
                    record_mcp_resource_trace(&trace_ref, tc, &pre_result);
                    record_remote_bridge_trace(&trace_ref, tc, &pre_result);
                    record_permission_denial_recovery_trace(&trace_ref, tc, &pre_result);
                    record_web_retrieval_trace(&trace_ref, tc, &pre_result);
                }
                lifecycle.provider_executed(tc, &pre_result);
                debug!(
                    "Skipping pre-executed read-only tool at index {}: {}",
                    i, tc.name
                );
                results.push((tc.clone(), pre_result.clone()));
                if let Some(tx) = tx {
                    let result_content =
                        ToolResultNormalizer::normalize(tc, &pre_result).ui_content;
                    let _ = tx
                        .send(StreamEvent::ToolExecutionComplete {
                            id: tc.id.clone(),
                            result: result_content,
                            metadata: tool_completion_metadata(&pre_result),
                        })
                        .await;
                }
                scheduled_count += 1;
                continue;
            }

            if concurrency_safe {
                lifecycle.running(tc, true, false);
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            metadata: tool_execution_start_metadata(&tc.name, &tc.arguments),
                        })
                        .await;
                }
                read_only_jobs.push((
                    i,
                    self.read_only_job(ReadOnlyJobInput {
                        trace: &trace,
                        runtime_context: &runtime_context,
                        retained_context,
                        tool_call: tc,
                        action_review,
                        parent_tool_calls: tool_calls.to_vec(),
                        parent_assistant_content: parent_assistant_content.to_string(),
                    }),
                ));
                scheduled_count += 1;
            } else {
                let read_write_results = self
                    .execute_read_write_calls(
                        vec![(tc.clone(), action_review)],
                        ReadWriteExecutionContext {
                            tx,
                            trace: &trace,
                            runtime_context: &runtime_context,
                            retained_context,
                            task_state,
                            parent_tool_calls: tool_calls,
                            parent_assistant_content,
                        },
                        lifecycle,
                    )
                    .await;
                results.extend(read_write_results);
                scheduled_count += 1;
                serial_boundary_seen = true;
            }
        }

        if !read_only_jobs.is_empty() {
            let read_only_results = self
                .collect_read_only_results(read_only_jobs, concurrency, tx, lifecycle)
                .await;
            results.extend(read_only_results);
        }

        let lifecycle_snapshot = lifecycle.snapshot();
        let lifecycle_summary = lifecycle_snapshot
            .iter()
            .map(|(id, record)| {
                format!(
                    "{}:{}:{:?}:parallel={}:pre_executed={}",
                    id, record.tool_name, record.status, record.parallel, record.pre_executed
                )
            })
            .collect::<Vec<_>>();
        let batch = ToolExecutionBatch::new(results, lifecycle_snapshot);
        if !lifecycle_summary.is_empty() {
            debug!(
                "Tool lifecycle batch: {} (denied={}, failed={}, pre_executed={})",
                lifecycle_summary.join(", "),
                batch.denied_count(),
                batch.failed_count(),
                batch.pre_executed_count()
            );
        }

        batch
    }
}

fn tool_completion_metadata(result: &ToolResult) -> Option<serde_json::Value> {
    let data = result.data.as_ref()?;
    let mut metadata = data
        .get("tool_summary")
        .cloned()
        .or_else(|| data.get("tool_observation").cloned())?;
    if let (Some(object), Some(observation)) =
        (metadata.as_object_mut(), data.get("tool_observation"))
    {
        object.insert("tool_observation".to_string(), observation.clone());
    }
    Some(metadata)
}

fn tool_execution_start_metadata(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<serde_json::Value> {
    let mut object = serde_json::Map::new();
    object.insert(
        "tool".to_string(),
        serde_json::Value::String(tool_name.to_string()),
    );
    for key in ["path", "command", "pattern", "query"] {
        if let Some(value) = arguments.get(key).and_then(serde_json::Value::as_str) {
            object.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    (object.len() > 1).then_some(serde_json::Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::resource_policy::ResourcePolicy;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::{Tool, ToolContext};
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: serde_json::json!({}),
        }
    }

    #[test]
    fn memory_action_signal_raises_risk_for_prior_failure_memory() {
        let call = tool_call("call_1", "file_edit");
        let mut decision = ActionDecision::for_tool_call(
            &call,
            ActionDecisionInput {
                task_stage: AgentTaskStage::Edit,
                route_workflow: None,
                route_risk: None,
                action_checkpoint_active: false,
                has_changes_before_tools: false,
                no_progress_rounds: 0,
            },
        );
        let before = decision.scores.risk;
        let items = vec![ToolContextRetentionItem {
            source: "Memory".to_string(),
            title: "memory_record/mem1:memory/strategy-failures.md".to_string(),
            provenance: "memory.match:memory_record/mem1:memory/strategy-failures.md".to_string(),
            reason: "failure pattern warns about broad edit".to_string(),
            trust: "High".to_string(),
            conflict: false,
            token_estimate: 12,
        }];

        apply_memory_action_signal(&mut decision, &call, &items);

        assert!(decision.scores.risk > before);
        assert!(decision.reason_summary.contains("memory modifier"));
    }

    struct NoopProvider;

    #[async_trait]
    impl LlmProvider for NoopProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse {
                content: String::new(),
                tool_calls: None,
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("unused test provider stream"))
        }

        fn base_url(&self) -> &str {
            "test://noop"
        }

        fn default_model(&self) -> &str {
            "test"
        }
    }

    struct ProbeReadTool {
        writes: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for ProbeReadTool {
        fn name(&self) -> &str {
            "probe_read"
        }

        fn description(&self) -> &str {
            "Read the probe write counter"
        }

        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }

        async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
            ToolResult::success(format!(
                "writes_seen={}",
                self.writes.load(Ordering::SeqCst)
            ))
        }

        fn is_read_only(&self, _params: &Value) -> bool {
            true
        }

        fn is_concurrency_safe(&self, _params: &Value) -> bool {
            true
        }
    }

    struct ProbeWriteTool {
        writes: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for ProbeWriteTool {
        fn name(&self) -> &str {
            "probe_write"
        }

        fn description(&self) -> &str {
            "Increment the probe write counter"
        }

        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }

        async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
            let previous = self.writes.fetch_add(1, Ordering::SeqCst);
            ToolResult::success(format!("writes_before={previous}"))
        }
    }

    struct PrematureEditProbeTool {
        executions: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for PrematureEditProbeTool {
        fn name(&self) -> &str {
            "file_edit"
        }

        fn description(&self) -> &str {
            "Probe premature file edits"
        }

        fn parameters(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "new_string": { "type": "string" }
                },
                "required": ["path", "new_string"]
            })
        }

        async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
            self.executions.fetch_add(1, Ordering::SeqCst);
            ToolResult::success("edit executed")
        }
    }

    fn probe_loop(writes: Arc<AtomicUsize>) -> ConversationLoop {
        let mut registry = ToolRegistry::new();
        registry.register(ProbeReadTool {
            writes: writes.clone(),
        });
        registry.register(ProbeWriteTool { writes });
        ConversationLoop::new(
            Arc::new(NoopProvider),
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        )
    }

    async fn execute_probe_tools(
        loop_instance: &ConversationLoop,
        tool_calls: &[ToolCall],
        pre_executed: HashMap<usize, ToolResult>,
    ) -> ToolExecutionBatch {
        execute_probe_tools_with_trace(loop_instance, tool_calls, pre_executed, None).await
    }

    async fn execute_probe_tools_with_trace(
        loop_instance: &ConversationLoop,
        tool_calls: &[ToolCall],
        pre_executed: HashMap<usize, ToolResult>,
        trace: Option<TraceCollector>,
    ) -> ToolExecutionBatch {
        let route = IntentRouter::new().route("probe ordered tools");
        let mut policy = ResourcePolicy::from_route(&route);
        policy.max_tool_calls = 20;
        policy.parallelism_limit = 4;
        let destructive_scope = DestructiveScopeContract::from_user_request(
            "probe ordered tools",
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        let exposed_tool_names =
            HashSet::from(["probe_read".to_string(), "probe_write".to_string()]);
        let mut lifecycle = ToolCallLifecycle::default();

        ToolExecutionController::new(ToolExecutionContext::from_conversation(loop_instance))
            .execute_tools_parallel(ToolExecutionRequest {
                tool_calls,
                parent_assistant_content: "",
                tx: None,
                pre_executed,
                trace,
                route: Some(&route),
                resource_policy: &policy,
                exposed_tool_names: &exposed_tool_names,
                retained_context: &crate::tools::ToolContextRetainedContext::default(),
                task_stage: AgentTaskStage::Understand,
                task_state: None,
                action_checkpoint_active: false,
                action_checkpoint_lookup_count: 0,
                no_progress_rounds: 0,
                has_changes_before_tools: false,
                destructive_scope: &destructive_scope,
                lifecycle: &mut lifecycle,
            })
            .await
    }

    #[test]
    fn batch_summarizes_results_and_lifecycle_statuses() {
        let mut lifecycle = ToolCallLifecycle::default();
        let denied = tool_call("call_1", "file_write");
        let failed = tool_call("call_2", "bash");
        let pre_executed = tool_call("call_3", "file_read");

        lifecycle.denied(&denied);
        lifecycle.completed(&failed, &ToolResult::error("nope"));
        lifecycle.provider_executed(&pre_executed, &ToolResult::success("ok"));

        let batch = ToolExecutionBatch::new(
            vec![
                (denied.clone(), ToolResult::error("denied")),
                (failed.clone(), ToolResult::error("nope")),
                (pre_executed.clone(), ToolResult::success("ok")),
            ],
            lifecycle.snapshot(),
        );

        assert!(batch.any_success());
        assert_eq!(batch.unsuccessful_count(), 2);
        let result_successes = batch
            .result_successes()
            .map(|(tool_call, success)| (tool_call.id.as_str(), success))
            .collect::<Vec<_>>();
        assert_eq!(
            result_successes,
            vec![("call_1", false), ("call_2", false), ("call_3", true)]
        );
        assert_eq!(batch.denied_count(), 1);
        assert_eq!(batch.failed_count(), 1);
        assert_eq!(batch.pre_executed_count(), 1);
    }

    #[test]
    fn batch_synthesizes_terminal_result_for_missing_lifecycle_result() {
        let pending = tool_call("call_missing", "bash");
        let mut lifecycle = ToolCallLifecycle::default();
        lifecycle.pending_batch(std::slice::from_ref(&pending));

        let batch = ToolExecutionBatch::new(Vec::new(), lifecycle.snapshot());

        assert_eq!(batch.results().len(), 1);
        assert_eq!(batch.results()[0].0.id, "call_missing");
        assert!(!batch.results()[0].1.success);
        assert!(batch.results()[0]
            .1
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("no terminal result was recorded"));
        assert_eq!(batch.failed_count(), 1);
        assert_eq!(
            batch.results()[0].1.data.as_ref().unwrap()["tool_lifecycle_recovery"]
                ["terminal_result"],
            "interrupted"
        );
    }

    #[tokio::test]
    async fn mixed_read_write_round_preserves_tool_call_order() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("call_read_before", "probe_read"),
            tool_call("call_write", "probe_write"),
            tool_call("call_read_after", "probe_read"),
        ];

        let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, _)| call.id.as_str())
                .collect::<Vec<_>>(),
            vec!["call_read_before", "call_write", "call_read_after"]
        );
        assert_eq!(results[0].1.content, "writes_seen=0");
        assert_eq!(results[1].1.content, "writes_before=0");
        assert_eq!(results[2].1.content, "writes_seen=1");
    }

    #[tokio::test]
    async fn tool_results_include_action_decision_metadata() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![tool_call("call_read", "probe_read")];

        let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
        let metadata = batch.results()[0]
            .1
            .data
            .as_ref()
            .expect("tool metadata should be present");

        assert_eq!(
            metadata["action_decision"]["action"]["tool_name"],
            "probe_read"
        );
        assert!(metadata["action_decision"]["scores"]["value"].is_u64());
        assert_eq!(metadata["action_review"]["decision"], "allow");
        assert_eq!(
            metadata["action_review"]["primary_reason"],
            "safe_to_execute"
        );
    }

    #[test]
    fn action_review_metadata_includes_observed_checkpoint_id() {
        let tool = PrematureEditProbeTool {
            executions: Arc::new(AtomicUsize::new(0)),
        };
        let tool_call = ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: json!({"path": "src/lib.rs", "new_string": "updated"}),
        };
        let exposed = HashSet::from(["file_edit".to_string()]);
        let permission_context = crate::permissions::PermissionContext::new(".");
        let review = ActionReview::build(ActionReviewInput {
            tool_call: &tool_call,
            tool: Some(&tool),
            exposed_tool_names: &exposed,
            scheduled_count: 0,
            max_tool_calls: 4,
            action_decision: ActionDecision::for_tool_call(
                &tool_call,
                ActionDecisionInput {
                    task_stage: AgentTaskStage::Edit,
                    route_workflow: Some(WorkflowKind::CodeChange),
                    route_risk: Some(RiskLevel::Medium),
                    action_checkpoint_active: false,
                    has_changes_before_tools: false,
                    no_progress_rounds: 0,
                },
            ),
            permission_context: Some(&permission_context),
            task_state: None,
            working_dir: Some(std::path::Path::new(".")),
            tool_allowed_by_context: true,
            destructive_scope_check: None,
            action_checkpoint_rejection: None,
        });
        let mut result = ToolResult::success_with_data(
            "edit executed",
            json!({"checkpoint": {"id": "cp_test_1"}}),
        );

        attach_action_review_metadata(&mut result, &review);

        let checkpoint = &result.data.as_ref().unwrap()["action_review"]["checkpoint"];
        assert_eq!(checkpoint["status"], "required_and_present");
        assert_eq!(checkpoint["checkpoint_id"], "cp_test_1");
        assert_eq!(checkpoint["observed_result_checkpoint"], true);
    }

    #[tokio::test]
    async fn tool_execution_records_action_review_trace() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![tool_call("call_read", "probe_read")];
        let trace =
            TraceCollector::new(crate::engine::trace::TurnTrace::new("session", 1, "probe"));

        let _batch = execute_probe_tools_with_trace(
            &loop_instance,
            &tool_calls,
            HashMap::new(),
            Some(trace.clone()),
        )
        .await;
        let snapshot = trace.snapshot();

        assert!(snapshot.events.iter().any(|event| matches!(
            event,
            TraceEvent::ActionReviewed {
                tool,
                decision,
                reason,
                ..
            } if tool == "probe_read" && decision == "allow" && reason == "safe_to_execute"
        )));
    }

    #[tokio::test]
    async fn premature_edit_in_understand_stage_revises_before_execution() {
        let executions = Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry.register(PrematureEditProbeTool {
            executions: executions.clone(),
        });
        let loop_instance = ConversationLoop::new(
            Arc::new(NoopProvider),
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let route = IntentRouter::new().route("edit src/lib.rs");
        let mut policy = ResourcePolicy::from_route(&route);
        policy.max_tool_calls = 4;
        let destructive_scope = DestructiveScopeContract::from_user_request(
            "edit src/lib.rs",
            &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        let exposed_tool_names = HashSet::from(["file_edit".to_string(), "file_read".to_string()]);
        let mut lifecycle = ToolCallLifecycle::default();
        let task_state = AgentTaskState::from_initial_context(
            "edit src/lib.rs",
            std::path::Path::new("."),
            &route,
            None,
        );
        let tool_calls = vec![ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: json!({"path": "src/lib.rs", "new_string": "updated"}),
        }];

        let batch =
            ToolExecutionController::new(ToolExecutionContext::from_conversation(&loop_instance))
                .execute_tools_parallel(ToolExecutionRequest {
                    tool_calls: &tool_calls,
                    parent_assistant_content: "",
                    tx: None,
                    pre_executed: HashMap::new(),
                    trace: None,
                    route: Some(&route),
                    resource_policy: &policy,
                    exposed_tool_names: &exposed_tool_names,
                    retained_context: &crate::tools::ToolContextRetainedContext::default(),
                    task_stage: AgentTaskStage::Understand,
                    task_state: Some(&task_state),
                    action_checkpoint_active: false,
                    action_checkpoint_lookup_count: 0,
                    no_progress_rounds: 0,
                    has_changes_before_tools: false,
                    destructive_scope: &destructive_scope,
                    lifecycle: &mut lifecycle,
                })
                .await;
        let result = &batch.results()[0].1;

        assert!(!result.success);
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert_eq!(
            result.data.as_ref().unwrap()["action_review"]["decision"],
            "revise"
        );
        assert_eq!(
            result.data.as_ref().unwrap()["action_review"]["primary_reason"],
            "low_value_action"
        );
        assert!(
            result.data.as_ref().unwrap()["action_review"]["worth"]["premature_mutation"]
                .as_bool()
                .unwrap()
        );
    }

    #[tokio::test]
    async fn consecutive_read_batches_stay_ordered_across_writes() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_1", "probe_read"),
            tool_call("read_2", "probe_read"),
            tool_call("write_1", "probe_write"),
            tool_call("read_3", "probe_read"),
            tool_call("read_4", "probe_read"),
        ];

        let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("read_1", "writes_seen=0"),
                ("read_2", "writes_seen=0"),
                ("write_1", "writes_before=0"),
                ("read_3", "writes_seen=1"),
                ("read_4", "writes_seen=1"),
            ]
        );
    }

    #[tokio::test]
    async fn denied_tool_between_read_batches_preserves_result_order() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_before", "probe_read"),
            tool_call("denied", "probe_denied"),
            tool_call("read_after", "probe_read"),
        ];

        let batch = execute_probe_tools(&loop_instance, &tool_calls, HashMap::new()).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, _)| call.id.as_str())
                .collect::<Vec<_>>(),
            vec!["read_before", "denied", "read_after"]
        );
        assert_eq!(results[0].1.content, "writes_seen=0");
        assert!(!results[1].1.success);
        assert!(results[1].1.content.contains("not found"));
        assert_eq!(
            results[1].1.data.as_ref().unwrap()["action_review"]["decision"],
            "revise"
        );
        assert_eq!(
            results[1].1.data.as_ref().unwrap()["action_review"]["primary_reason"],
            "tool_not_available"
        );
        assert_eq!(results[2].1.content, "writes_seen=0");
        assert_eq!(batch.denied_count(), 1);
    }

    #[tokio::test]
    async fn pre_executed_read_only_result_before_serial_boundary_keeps_original_position() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_pre_executed", "probe_read"),
            tool_call("write", "probe_write"),
            tool_call("read_after", "probe_read"),
        ];
        let pre_executed = HashMap::from([(0usize, ToolResult::success("pre_executed_read"))]);

        let batch = execute_probe_tools(&loop_instance, &tool_calls, pre_executed).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("read_pre_executed", "pre_executed_read"),
                ("write", "writes_before=0"),
                ("read_after", "writes_seen=1"),
            ]
        );
        assert_eq!(batch.pre_executed_count(), 1);
    }

    #[tokio::test]
    async fn pre_executed_read_only_result_after_serial_boundary_is_rerun() {
        let writes = Arc::new(AtomicUsize::new(0));
        let loop_instance = probe_loop(writes);
        let tool_calls = vec![
            tool_call("read_before", "probe_read"),
            tool_call("write", "probe_write"),
            tool_call("read_pre_executed", "probe_read"),
        ];
        let pre_executed = HashMap::from([(2usize, ToolResult::success("pre_executed_read"))]);

        let batch = execute_probe_tools(&loop_instance, &tool_calls, pre_executed).await;
        let results = batch.results();

        assert_eq!(
            results
                .iter()
                .map(|(call, result)| (call.id.as_str(), result.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("read_before", "writes_seen=0"),
                ("write", "writes_before=0"),
                ("read_pre_executed", "writes_seen=1"),
            ]
        );
        assert_eq!(batch.pre_executed_count(), 0);
    }
}

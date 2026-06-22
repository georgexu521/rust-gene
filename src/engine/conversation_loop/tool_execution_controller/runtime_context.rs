//! Tool execution controller support.
//!
//! Separates execution gates, runtime context, and batch state from the conversation-loop control flow.

use super::super::tool_metadata::merge_tool_result_metadata;
use super::super::tool_result_controller::ToolResultNormalizer;
use crate::engine::action_decision::{
    ActionDecision, ActionDecisionInput, ActionScoreModifier, ActionScoreModifierSource,
};
use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::task_context::{AgentTaskStage, AgentTaskState};
use crate::services::api::ToolCall;
use crate::tools::{ToolContextRetainedContext, ToolContextRetentionItem, ToolResult};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub(super) struct ToolRuntimeContext {
    pub(super) has_route: bool,
    pub(super) route_intent: String,
    pub(super) route_workflow: String,
    pub(super) route_retrieval: String,
    pub(super) route_reasoning: String,
    pub(super) route_risk: String,
    pub(super) policy_latency: String,
    pub(super) policy_parallelism_limit: usize,
    pub(super) policy_max_tool_calls: usize,
    pub(super) policy_context_budget_tokens: usize,
    pub(super) policy_allow_fallback_model: bool,
    pub(super) policy_cost_ceiling_usd: String,
    pub(super) action_checkpoint_active: bool,
    pub(super) has_changes_before_tools: bool,
    pub(super) task_stage: AgentTaskStage,
    pub(super) route_workflow_kind: Option<WorkflowKind>,
    pub(super) route_risk_level: Option<RiskLevel>,
    pub(super) no_progress_rounds: usize,
    pub(super) exposed_tools_count: usize,
    pub(super) retained_retrieval_items: usize,
    pub(super) retained_skill_triggers: usize,
    pub(super) retained_context_tokens: usize,
    pub(super) retained_context_provenance: Vec<String>,
    pub(super) retained_context_items: Vec<ToolContextRetentionItem>,
    observer_signal: Option<ObserverActionSignal>,
}

pub(super) struct ToolRuntimeContextInput<'a> {
    pub(super) route: Option<&'a IntentRoute>,
    pub(super) policy: &'a ResourcePolicy,
    pub(super) task_stage: AgentTaskStage,
    pub(super) action_checkpoint_active: bool,
    pub(super) no_progress_rounds: usize,
    pub(super) has_changes_before_tools: bool,
    pub(super) exposed_tools_count: usize,
    pub(super) retained_context: &'a ToolContextRetainedContext,
    pub(super) task_state: Option<&'a AgentTaskState>,
}

#[derive(Debug, Clone, Default)]
struct ObserverActionSignal {
    pub(super) uncertainty_not_reduced_steps: usize,
    pub(super) candidate_focus: Vec<String>,
    pub(super) key_findings: usize,
    pub(super) consecutive_validation_failures: usize,
    pub(super) validation_verified: bool,
    pub(super) risks: usize,
    pub(super) last_progress_signal: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ToolRuntimeTiming {
    pub(super) started_at_unix_ms: Option<u64>,
    pub(super) finished_at_unix_ms: Option<u64>,
}

impl ToolRuntimeTiming {
    pub(super) fn instant() -> Self {
        let now = unix_time_millis();
        Self {
            started_at_unix_ms: now,
            finished_at_unix_ms: now,
        }
    }

    pub(super) fn finished(started_at_unix_ms: Option<u64>) -> Self {
        Self {
            started_at_unix_ms,
            finished_at_unix_ms: unix_time_millis(),
        }
    }
}

impl ToolRuntimeContext {
    pub(super) fn new(input: ToolRuntimeContextInput<'_>) -> Self {
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

    pub(super) fn attach(
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

    pub(super) fn action_decision(&self, tool_call: &ToolCall) -> ActionDecision {
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

    pub(super) fn attach_action_decision(&self, tool_call: &ToolCall, result: &mut ToolResult) {
        let decision = self.action_decision(tool_call);
        if let Ok(value) = serde_json::to_value(&decision) {
            merge_tool_result_metadata(result, "action_decision", value);
        }
        ToolResultNormalizer::attach_observation_metadata(tool_call, result);
    }
}

pub(super) fn unix_time_millis() -> Option<u64> {
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

pub(super) fn apply_memory_action_signal(
    decision: &mut ActionDecision,
    tool_call: &ToolCall,
    retained_items: &[ToolContextRetentionItem],
) {
    let signal = memory_action_signal(tool_call, retained_items);
    if signal.is_empty() {
        return;
    }
    decision.record_score_modifier_evidence(
        ActionScoreModifier::new(
            ActionScoreModifierSource::Memory,
            signal.kind,
            format!(
                "memory evidence only; suggested value_delta={} risk_delta={} uncertainty_delta={} scope_delta={}; {}",
                signal.value_delta,
                signal.risk_delta,
                signal.uncertainty_reduction_delta,
                signal.scope_fit_delta,
                signal.reasons.join("; ")
            ),
        ),
    );
    decision.trace_recommended = true;
    decision.reason_summary = format!(
        "{}; memory evidence value_delta={} risk_delta={} uncertainty_delta={} scope_delta={} not_applied_to_score ({})",
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
    pub(super) kind: String,
    pub(super) value_delta: i8,
    pub(super) risk_delta: i8,
    pub(super) uncertainty_reduction_delta: i8,
    pub(super) scope_fit_delta: i8,
    pub(super) reasons: Vec<String>,
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

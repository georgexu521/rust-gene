use super::closeout_controller::{FinalCloseoutContext, FinalCloseoutController};
use super::runtime_diet::{trace_runtime_diet_report, RuntimeDietSnapshot};
use super::LoopResult;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::{TaskContextBundle, VerificationStatus};
use crate::engine::trace::{control_loop_diagnostic, TraceCollector, TraceEvent, TurnTrace};
use crate::services::api::ToolCall;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::mpsc;

pub(super) struct TurnCompletionContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) required_validation_commands: &'a [String],
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a [ToolCall],
    pub(super) iterations_used: usize,
    pub(super) max_iterations: usize,
    pub(super) tool_calls_made: bool,
    pub(super) evidence_ledger: &'a EvidenceLedger,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) struct TurnCompletionController;

impl TurnCompletionController {
    pub(super) async fn complete(context: TurnCompletionContext<'_>) -> LoopResult {
        FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
            trace: context.trace,
            code_workflow: context.code_workflow,
            task_bundle: context.task_bundle,
            required_validation_commands: context.required_validation_commands,
            runtime_diet: context.runtime_diet,
            final_content: context.final_content,
            final_tool_calls: context.final_tool_calls,
            iterations_used: context.iterations_used,
            max_iterations: context.max_iterations,
            evidence_ledger: context.evidence_ledger,
            tx: context.tx,
        })
        .await;

        trace_runtime_diet_report(
            context.trace,
            context.route,
            context.code_workflow,
            context.runtime_diet,
        );
        record_completion_contract(
            context.trace,
            context.route,
            context.code_workflow,
            context.task_bundle,
            context.required_validation_commands,
            context.final_content,
        );

        context.trace.record(TraceEvent::AssistantResponded {
            chars: context.final_content.chars().count(),
            iterations: context.iterations_used,
        });

        if let Some(tx) = context.tx {
            let _ = tx
                .send(StreamEvent::RuntimeDiagnostic {
                    diagnostic: runtime_diagnostic_payload(context.trace, context.task_bundle),
                })
                .await;
            let _ = tx.send(StreamEvent::Complete).await;
        }

        LoopResult {
            content: std::mem::take(context.final_content),
            tool_calls: Vec::new(),
            tool_calls_made: context.tool_calls_made,
            iterations: context.iterations_used,
            pre_executed_results: HashMap::new(),
        }
    }
}

fn runtime_diagnostic_payload(trace: &TraceCollector, task_bundle: &TaskContextBundle) -> Value {
    let trace_snapshot = trace.snapshot();
    let control_loop = control_loop_diagnostic(&trace_snapshot);
    let covered_phases = control_loop
        .phases
        .iter()
        .filter(|phase| phase.events > 0)
        .count();
    let coverage = format!("{}/{}", covered_phases, control_loop.phases.len());
    let summary = control_loop.compact_summary();

    json!({
        "schema": "desktop_runtime_diagnostic.v1",
        "mva_state_snapshot": serde_json::to_value(task_bundle.mva_state_snapshot()).unwrap_or(Value::Null),
        "task_state": task_state_payload(task_bundle),
        "verification_proof": verification_proof_payload(&trace_snapshot),
        "completion_contract": completion_contract_payload(&trace_snapshot),
        "control_loop": {
            "coverage": coverage,
            "summary": summary,
            "phases": control_loop.phases,
        },
    })
}

fn record_completion_contract(
    trace: &TraceCollector,
    route: &IntentRoute,
    code_workflow: &CodeChangeWorkflowRunner,
    task_bundle: &TaskContextBundle,
    required_validation_commands: &[String],
    final_content: &str,
) {
    let trace_snapshot = trace.snapshot();
    let proof = completion_proof_status(&trace_snapshot);
    let latest_stop = latest_stop_status(&trace_snapshot);
    let changed_files = latest_closeout_changed_files(&trace_snapshot)
        .unwrap_or(task_bundle.agent_state.active_files.len());
    let requires_validation = code_workflow.policy.require_stage_validation
        || !required_validation_commands.is_empty()
        || task_bundle.agent_state.verification_plan.status != VerificationStatus::NotRequired;
    let full_or_high_risk = matches!(
        route.workflow,
        WorkflowKind::CodeChange | WorkflowKind::BugFix
    ) || route.risk == RiskLevel::High;

    let high_risk_blocked_by_answer = route.risk == RiskLevel::High
        && changed_files == 0
        && (final_content_mentions_blocked_action(final_content)
            || task_goal_requests_blocked_destructive_action(&task_bundle.agent_state.main_goal));

    let (status, terminal_status, reason) = if route.risk == RiskLevel::High
        && matches!(
            latest_stop
                .as_ref()
                .map(|stop| stop.terminal_status.as_str()),
            Some("needs_user" | "blocked")
        ) {
        let stop = latest_stop.as_ref().expect("checked above");
        (
            "blocked".to_string(),
            stop.terminal_status.clone(),
            format!(
                "high-risk task ended with stop reason {} and action {}",
                stop.reason, stop.action
            ),
        )
    } else if high_risk_blocked_by_answer {
        (
            "blocked".to_string(),
            "blocked".to_string(),
            "high-risk destructive request was explicitly blocked without workspace changes"
                .to_string(),
        )
    } else if full_or_high_risk && requires_validation {
        match proof.as_str() {
            "verified" => (
                "completed".to_string(),
                "completed".to_string(),
                "required validation proof is verified".to_string(),
            ),
            "failed" => (
                "failed".to_string(),
                "failed".to_string(),
                "verification proof failed".to_string(),
            ),
            "not_applicable" if changed_files == 0 => (
                "partial".to_string(),
                "partial".to_string(),
                "no-diff task has no applicable verification proof".to_string(),
            ),
            _ => (
                "partial".to_string(),
                "partial".to_string(),
                "full or high-risk task lacks verified completion proof".to_string(),
            ),
        }
    } else if final_content.trim().is_empty() {
        (
            "partial".to_string(),
            "partial".to_string(),
            "final answer content is empty".to_string(),
        )
    } else {
        (
            "completed".to_string(),
            "completed".to_string(),
            "direct or light task has a final response".to_string(),
        )
    };

    trace.record(TraceEvent::CompletionContractEvaluated {
        mode: serde_label(&task_bundle.agent_state.mode),
        workflow: serde_label(&route.workflow),
        status,
        terminal_status,
        requires_validation,
        verification_status: serde_label(&task_bundle.agent_state.verification_plan.status),
        verification_proof_status: proof,
        changed_files,
        reason,
    });
    if mva_runtime_profile_enabled() {
        trace.record(TraceEvent::MemoryBoundaryEvaluated {
            read_status: "closeout".to_string(),
            stale_conflict_demotion_status: "not_evaluated".to_string(),
            closeout_write_candidate_status: if final_content.trim().is_empty() {
                "missing".to_string()
            } else {
                "candidate_available".to_string()
            },
            reason: "completion reached memory boundary closeout check".to_string(),
        });
    }
}

fn task_state_payload(task_bundle: &TaskContextBundle) -> Value {
    let state = &task_bundle.agent_state;
    let recent_steps = state
        .completed_steps
        .iter()
        .rev()
        .take(3)
        .map(|step| {
            json!({
                "stage": step.stage,
                "summary": preview_chars(&step.summary, 120),
            })
        })
        .collect::<Vec<_>>();
    let recent_observations = state
        .observations
        .iter()
        .rev()
        .take(3)
        .map(|observation| {
            json!({
                "source": observation.source.as_str(),
                "summary": preview_chars(&observation.summary, 120),
            })
        })
        .collect::<Vec<_>>();
    let recent_edit_snapshots = state
        .edit_snapshots
        .iter()
        .rev()
        .take(3)
        .map(|snapshot| {
            json!({
                "label": preview_chars(&snapshot.label, 100),
                "stage": snapshot.stage,
                "verification": snapshot.verification_status,
                "active_files": snapshot
                    .active_files
                    .iter()
                    .take(5)
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let recent_stage_transitions = state
        .stage_transitions
        .iter()
        .rev()
        .take(5)
        .map(|transition| {
            json!({
                "from": transition.from,
                "to": transition.to,
                "source": transition.source.as_str(),
                "reason": preview_chars(&transition.reason, 140),
                "evidence_items": transition.evidence_items,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "task_id": task_bundle.task_id.as_str(),
        "goal": preview_chars(&state.main_goal, 160),
        "mode": state.mode,
        "mode_score": {
            "confidence": state.mode_score.confidence,
            "complexity": state.mode_score.complexity,
            "risk": state.mode_score.risk,
            "uncertainty": state.mode_score.uncertainty,
            "tool_need": state.mode_score.tool_need,
            "user_impact": state.mode_score.user_impact,
            "reason": preview_chars(&state.mode_score.reason, 180),
        },
        "lightweight_plan": state.lightweight_plan.as_ref().map(|plan| {
            json!({
                "objective": preview_chars(&plan.objective, 160),
                "verification_required": plan.verification_required,
                "heavy_contract_avoided": plan.heavy_contract_avoided,
                "reason": preview_chars(&plan.reason, 180),
                "steps": plan.steps.iter().take(4).map(|step| {
                    json!({
                        "label": step.label.as_str(),
                        "action": preview_chars(&step.action, 120),
                        "expected_observation": preview_chars(&step.expected_observation, 120),
                    })
                }).collect::<Vec<_>>(),
            })
        }),
        "stage": state.stage,
        "terminal_status": state.terminal_status,
        "verification": {
            "status": state.verification_plan.status,
            "required_checks": state
                .verification_plan
                .required_checks
                .iter()
                .take(5)
                .map(|check| preview_chars(check, 120))
                .collect::<Vec<_>>(),
        },
        "done": {
            "satisfied": state.done_condition.satisfied,
            "summary": preview_chars(&state.done_condition.summary, 160),
        },
        "active_files": state
            .active_files
            .iter()
            .take(8)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "risks": state
            .risks
            .iter()
            .take(5)
                .map(|risk| preview_chars(risk, 120))
                .collect::<Vec<_>>(),
        "failure_counters": {
            "uncertainty_not_reduced_steps": state.uncertainty_not_reduced_steps,
            "consecutive_validation_failures": state.consecutive_validation_failures,
            "consecutive_edit_failures": state.consecutive_edit_failures,
            "consecutive_command_failures": state.consecutive_command_failures,
            "consecutive_permission_blocks": state.consecutive_permission_blocks,
            "last_failure_family": state.last_failure_family.as_deref(),
            "last_progress_signal": state.last_progress_signal.as_deref(),
        },
        "rollback_candidates": state
            .rollback_candidates
            .iter()
            .rev()
            .take(3)
            .map(|candidate| {
                json!({
                    "checkpoint_id": candidate.checkpoint_id.as_deref(),
                    "file_change_id": candidate.file_change_id.as_deref(),
                    "tool_round_id": candidate.tool_round_id.as_deref(),
                    "paths": candidate.paths.iter().take(5).cloned().collect::<Vec<_>>(),
                    "reason": preview_chars(&candidate.reason, 160),
                    "confidence": candidate.confidence,
                    "auto_allowed": candidate.auto_allowed,
                })
            })
            .collect::<Vec<_>>(),
        "failed_strategies": state
            .failed_strategies
            .iter()
            .rev()
            .take(5)
            .map(|record| {
                json!({
                    "failed_strategy": record.failed_strategy.as_str(),
                    "reason": preview_chars(&record.reason, 160),
                    "better_strategy": preview_chars(&record.better_strategy, 160),
                    "recovery_plan_id": record.recovery_plan_id.as_deref(),
                    "rollback_status": record.rollback_status.as_deref(),
                })
            })
            .collect::<Vec<_>>(),
        "recent_steps": recent_steps,
        "stage_transitions": recent_stage_transitions,
        "recent_observations": recent_observations,
        "recent_edit_snapshots": recent_edit_snapshots,
        "stop_check": state.stop_checks.last().map(stop_check_payload),
    })
}

fn stop_check_payload(record: &crate::engine::task_context::StopCheckRecord) -> Value {
    json!({
        "status": record.status,
        "terminal_status": record.terminal_status,
        "action": record.action,
        "reason": record.reason,
        "summary": preview_chars(&record.summary, 160),
        "evidence": record
            .evidence
            .iter()
            .take(5)
            .map(|item| preview_chars(item, 160))
            .collect::<Vec<_>>(),
        "failure_type": record.failure_type.as_deref(),
        "recovery_plan_id": record.recovery_plan_id.as_deref(),
        "rollback_candidate": record.rollback_candidate.as_ref().map(|candidate| {
            json!({
                "checkpoint_id": candidate.checkpoint_id.as_deref(),
                "paths": candidate.paths.iter().take(5).cloned().collect::<Vec<_>>(),
                "reason": preview_chars(&candidate.reason, 160),
                "confidence": candidate.confidence,
                "auto_allowed": candidate.auto_allowed,
            })
        }),
        "next_action": record.next_action.as_deref(),
        "no_code_progress_rounds": record.no_code_progress_rounds,
        "action_checkpoint_active": record.action_checkpoint_active,
    })
}

fn verification_proof_payload(trace: &TurnTrace) -> Value {
    trace
        .events
        .iter()
        .rev()
        .find_map(|event| match event {
            TraceEvent::FinalCloseoutPrepared {
                status,
                changed_files,
                validation_items,
                verification_proof_status,
                verification_proof_summary,
                verification_proof_kind_summary,
                verification_proof_support_status,
                verification_proof_support_summary,
                verification_proof_supports_verified,
                verification_proof_residual_risk,
                acceptance_items,
                residual_risks,
                ..
            } => Some(json!({
                "status": verification_proof_status.as_deref().unwrap_or(status),
                "summary": verification_proof_summary
                    .as_deref()
                    .unwrap_or("no verification proof summary recorded"),
                "proof_kinds": verification_proof_kind_summary
                    .as_deref()
                    .unwrap_or("none"),
                "support_status": verification_proof_support_status
                    .as_deref()
                    .unwrap_or("missing"),
                "support_summary": verification_proof_support_summary
                    .as_deref()
                    .unwrap_or("missing"),
                "supports_verified": verification_proof_supports_verified.unwrap_or(false),
                "residual_risk": verification_proof_residual_risk.unwrap_or(false),
                "closeout_status": status,
                "changed_files": changed_files,
                "validation_items": validation_items,
                "acceptance_items": acceptance_items,
                "residual_risks": residual_risks,
            })),
            _ => None,
        })
        .unwrap_or_else(|| {
            json!({
                "status": "unavailable",
                "summary": "no final closeout proof recorded",
            })
        })
}

fn completion_contract_payload(trace: &TurnTrace) -> Value {
    trace
        .events
        .iter()
        .rev()
        .find_map(|event| match event {
            TraceEvent::CompletionContractEvaluated {
                mode,
                workflow,
                status,
                terminal_status,
                requires_validation,
                verification_status,
                verification_proof_status,
                changed_files,
                reason,
            } => Some(json!({
                "mode": mode,
                "workflow": workflow,
                "status": status,
                "terminal_status": terminal_status,
                "requires_validation": requires_validation,
                "verification_status": verification_status,
                "verification_proof_status": verification_proof_status,
                "changed_files": changed_files,
                "reason": preview_chars(reason, 180),
            })),
            _ => None,
        })
        .unwrap_or_else(|| {
            json!({
                "status": "unavailable",
                "reason": "completion contract has not been evaluated",
            })
        })
}

fn completion_proof_status(trace: &TurnTrace) -> String {
    trace
        .events
        .iter()
        .rev()
        .find_map(|event| match event {
            TraceEvent::FinalCloseoutPrepared {
                status,
                verification_proof_status,
                ..
            } => Some(
                verification_proof_status
                    .as_deref()
                    .unwrap_or(status)
                    .to_string(),
            ),
            _ => None,
        })
        .unwrap_or_else(|| "missing".to_string())
}

fn latest_closeout_changed_files(trace: &TurnTrace) -> Option<usize> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::FinalCloseoutPrepared { changed_files, .. } => Some(*changed_files),
        _ => None,
    })
}

struct LatestStopStatus {
    terminal_status: String,
    reason: String,
    action: String,
}

fn latest_stop_status(trace: &TurnTrace) -> Option<LatestStopStatus> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::StopCheckEvaluated {
            terminal_status,
            reason,
            action,
            ..
        } => terminal_status
            .as_ref()
            .map(|terminal_status| LatestStopStatus {
                terminal_status: terminal_status.clone(),
                reason: reason.clone(),
                action: action.clone(),
            }),
        _ => None,
    })
}

fn mva_runtime_profile_enabled() -> bool {
    matches!(
        std::env::var("PRIORITY_AGENT_RUNTIME_PROFILE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "minimum_viable_agent" | "mva"
    )
}

fn final_content_mentions_blocked_action(final_content: &str) -> bool {
    let lower = final_content.to_ascii_lowercase();
    lower.contains("blocked")
        || lower.contains("refused")
        || lower.contains("skipped")
        || final_content.contains("已阻止")
        || final_content.contains("拒绝")
        || final_content.contains("未删除")
        || final_content.contains("保留")
}

fn task_goal_requests_blocked_destructive_action(goal: &str) -> bool {
    let lower = goal.to_ascii_lowercase();
    let destructive = lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("destructive")
        || goal.contains("删除")
        || goal.contains("移除")
        || goal.contains("破坏");
    let blocked = lower.contains("cannot")
        || lower.contains("must not")
        || lower.contains("blocked")
        || lower.contains("refuse")
        || goal.contains("不能")
        || goal.contains("不要")
        || goal.contains("不允许")
        || goal.contains("阻止")
        || goal.contains("拒绝");
    destructive && blocked
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

fn preview_chars(text: &str, max_chars: usize) -> String {
    let mut preview = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        preview.push_str("...");
    }
    preview.replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    #[test]
    fn task_goal_detects_explicit_destructive_block_request() {
        assert!(task_goal_requests_blocked_destructive_action(
            "删除 fixtures/protected 目录。这个请求是高风险破坏性操作，你不能删除它；请说明已阻止。"
        ));
        assert!(!task_goal_requests_blocked_destructive_action(
            "修复删除按钮的样式问题"
        ));
    }

    #[tokio::test]
    async fn completion_records_response_and_returns_loop_result() {
        let route = IntentRouter::new().route("say hello");
        let task_bundle = TaskContextBundle::new("say hello", ".", route.clone(), None);
        let code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let evidence_ledger = EvidenceLedger::new();
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "say hello"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut final_content = "hello".to_string();
        let final_tool_calls = Vec::new();
        let (tx, mut rx) = mpsc::channel(4);

        let result = TurnCompletionController::complete(TurnCompletionContext {
            trace: &trace,
            route: &route,
            code_workflow: &code_workflow,
            task_bundle: &task_bundle,
            required_validation_commands: &[],
            runtime_diet: &mut runtime_diet,
            final_content: &mut final_content,
            final_tool_calls: &final_tool_calls,
            iterations_used: 2,
            max_iterations: 8,
            tool_calls_made: true,
            evidence_ledger: &evidence_ledger,
            tx: Some(&tx),
        })
        .await;

        assert_eq!(result.content, "hello");
        assert!(result.tool_calls.is_empty());
        assert!(result.tool_calls_made);
        assert_eq!(result.iterations, 2);
        assert!(result.pre_executed_results.is_empty());
        assert!(final_content.is_empty());
        let Some(StreamEvent::RuntimeDiagnostic { diagnostic }) = rx.recv().await else {
            panic!("expected runtime diagnostic before completion");
        };
        assert_eq!(
            diagnostic.get("schema").and_then(Value::as_str),
            Some("desktop_runtime_diagnostic.v1")
        );
        assert_eq!(
            diagnostic
                .pointer("/task_state/goal")
                .and_then(Value::as_str),
            Some("say hello")
        );
        assert!(diagnostic
            .pointer("/task_state/mode_score/confidence")
            .and_then(Value::as_u64)
            .is_some());
        assert_eq!(
            diagnostic
                .pointer("/verification_proof/status")
                .and_then(Value::as_str),
            Some("unavailable")
        );
        assert!(diagnostic
            .pointer("/control_loop/coverage")
            .and_then(Value::as_str)
            .is_some());
        assert_eq!(
            diagnostic
                .pointer("/completion_contract/status")
                .and_then(Value::as_str),
            Some("completed")
        );
        assert!(matches!(rx.recv().await, Some(StreamEvent::Complete)));

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::CompletionContractEvaluated {
                status,
                terminal_status,
                ..
            } if status == "completed" && terminal_status == "completed"
        )));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AssistantResponded {
                chars: 5,
                iterations: 2,
            }
        )));
    }
}

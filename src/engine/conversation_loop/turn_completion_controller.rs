use super::closeout_controller::{FinalCloseoutContext, FinalCloseoutController};
use super::runtime_diet::{trace_runtime_diet_report, RuntimeDietSnapshot};
use super::LoopResult;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::IntentRoute;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
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
        "task_state": task_state_payload(task_bundle),
        "verification_proof": verification_proof_payload(&trace_snapshot),
        "control_loop": {
            "coverage": coverage,
            "summary": summary,
            "phases": control_loop.phases,
        },
    })
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
        "recent_steps": recent_steps,
        "recent_observations": recent_observations,
        "recent_edit_snapshots": recent_edit_snapshots,
        "stop_check": state.stop_checks.last().map(stop_check_payload),
    })
}

fn stop_check_payload(record: &crate::engine::task_context::StopCheckRecord) -> Value {
    json!({
        "status": record.status,
        "reason": record.reason,
        "summary": preview_chars(&record.summary, 160),
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
                acceptance_items,
                residual_risks,
                ..
            } => Some(json!({
                "status": verification_proof_status.as_deref().unwrap_or(status),
                "summary": verification_proof_summary
                    .as_deref()
                    .unwrap_or("no verification proof summary recorded"),
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
        assert!(matches!(rx.recv().await, Some(StreamEvent::Complete)));

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AssistantResponded {
                chars: 5,
                iterations: 2,
            }
        )));
    }
}

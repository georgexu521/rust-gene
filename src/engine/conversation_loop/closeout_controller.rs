use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::code_change_workflow::{CodeChangeWorkflowRunner, WorkflowCloseout};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use tokio::sync::mpsc;

pub(super) struct FinalCloseoutContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) required_validation_commands: &'a [String],
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a [ToolCall],
    pub(super) iterations_used: usize,
    pub(super) max_iterations: usize,
    pub(super) evidence_ledger: &'a EvidenceLedger,
    pub(super) tx: Option<&'a mpsc::Sender<super::super::streaming::StreamEvent>>,
}

pub(super) struct CloseoutEvaluation {
    pub(super) closeout: Option<WorkflowCloseout>,
    pub(super) runtime_validation_label: Option<String>,
}

pub(super) struct CloseoutEvaluator;

impl CloseoutEvaluator {
    pub(super) fn evaluate(
        code_workflow: &CodeChangeWorkflowRunner,
        task_bundle: &TaskContextBundle,
        evidence_ledger: &EvidenceLedger,
        required_validation_commands: &[String],
    ) -> CloseoutEvaluation {
        let runtime_validation_label = evidence_ledger
            .runtime_required_validation_label(required_validation_commands)
            .or_else(|| evidence_ledger.runtime_validation_label());
        let closeout = code_workflow.build_closeout_with_runtime_validation(
            task_bundle,
            runtime_validation_label.as_deref(),
        );
        CloseoutEvaluation {
            closeout,
            runtime_validation_label,
        }
    }
}

pub(super) struct VerifiedChangeCloseoutController;

impl VerifiedChangeCloseoutController {
    const VERIFIED_CHANGE_CLOSEOUT_TRACE: &'static str =
        "verified code change passed validation; preparing deterministic closeout";

    pub(super) fn should_break_for_verified_change(
        trace: &TraceCollector,
        should_closeout_after_verified_change: bool,
    ) -> bool {
        if !should_closeout_after_verified_change {
            return false;
        }

        trace.record(TraceEvent::WorkflowFallback {
            error: Self::VERIFIED_CHANGE_CLOSEOUT_TRACE.to_string(),
        });
        true
    }
}

pub(super) struct FinalCloseoutController;

impl FinalCloseoutController {
    pub(super) async fn apply_final_closeout(context: FinalCloseoutContext<'_>) {
        let evaluation = CloseoutEvaluator::evaluate(
            context.code_workflow,
            context.task_bundle,
            context.evidence_ledger,
            context.required_validation_commands,
        );
        if let Some(closeout) = evaluation.closeout {
            let evidence_snapshot = context.evidence_ledger.snapshot();
            context.trace.record(TraceEvent::FinalCloseoutPrepared {
                status: closeout.status.label().to_string(),
                changed_files: closeout.changed_files.len(),
                validation_items: closeout.validation.len(),
                tool_records: evidence_snapshot.tool_execution_records,
                acceptance_items: closeout.acceptance.len(),
                residual_risks: closeout.residual_risks.len(),
            });
            context.runtime_diet.closeout_visibility =
                format!("{:?}", closeout.visibility_from_env()).to_ascii_lowercase();
            context.runtime_diet.validation_evidence = evaluation
                .runtime_validation_label
                .clone()
                .unwrap_or_else(|| closeout.status.label().to_string());
            let closeout_text = closeout.format_for_user_response();
            if !closeout_text.is_empty() && !context.final_content.contains("Closeout:") {
                context.final_content.push_str(&closeout_text);
                if let Some(tx) = context.tx {
                    let _ = tx
                        .send(super::super::streaming::StreamEvent::TextChunk(
                            closeout_text,
                        ))
                        .await;
                }
            }
        }

        if context.runtime_diet.validation_evidence == "none" {
            if let Some(label) = evaluation.runtime_validation_label {
                context.runtime_diet.validation_evidence = label;
            }
        }

        if context.iterations_used >= context.max_iterations
            && !context.final_tool_calls.is_empty()
            && !context.final_content.contains("Closeout:")
        {
            let stop_msg = "\n\n[Stopped after reaching the tool-iteration budget before a final closeout. Review the last tool results and continue if the task is not complete.]\n";
            context.final_content.push_str(stop_msg);
            if let Some(tx) = context.tx {
                let _ = tx
                    .send(super::super::streaming::StreamEvent::TextChunk(
                        stop_msg.to_string(),
                    ))
                    .await;
            }
            context.trace.record(TraceEvent::WorkflowFallback {
                error: "tool iteration budget exhausted before final closeout".to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code_change_workflow::StageValidationStatus;
    use crate::engine::intent_router::{
        IntentKind, IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
    };

    fn audit_route() -> IntentRoute {
        IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.90,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk: RiskLevel::High,
            recommended_tools: Vec::new(),
            reason: "audit/regression eval requires project verification; code diff is optional"
                .to_string(),
        }
    }

    #[test]
    fn evaluator_uses_ledger_runtime_validation_for_no_diff_audit_closeout() {
        let mut bundle = TaskContextBundle::new("审查已有实现", ".", audit_route(), None);
        bundle.add_acceptance_check("required regression checks pass");
        let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
        let mut evidence_ledger = EvidenceLedger::new();
        evidence_ledger.record_validation_result(
            "required_validation",
            Some("cargo test -q memory"),
            true,
            "cargo test -q memory passed",
        );

        let required_commands = vec!["cargo test -q memory".to_string()];
        let evaluation = CloseoutEvaluator::evaluate(
            &code_workflow,
            &bundle,
            &evidence_ledger,
            &required_commands,
        );
        let closeout = evaluation.closeout.expect("closeout");

        assert_eq!(
            evaluation.runtime_validation_label.as_deref(),
            Some("passed:1/1")
        );
        assert_eq!(closeout.status, StageValidationStatus::Passed);
        assert!(closeout.changed_files.is_empty());
        assert!(closeout
            .validation
            .iter()
            .any(|item| item == "required validation: passed (passed:1/1)"));
    }

    #[test]
    fn evaluator_prefers_required_command_success_over_exploratory_validation_failure() {
        let mut bundle = TaskContextBundle::new("检查 Python 包安装", ".", audit_route(), None);
        bundle.add_acceptance_check("test -x .venv/bin/python returns success");
        bundle.add_acceptance_check(
            "python -m core_terminal_demo --self-test outputs core-terminal-demo-ok",
        );
        let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
        let mut evidence_ledger = EvidenceLedger::new();
        evidence_ledger.record_validation_result(
            "bash",
            Some("python3 -c \"import core_terminal_demo\""),
            false,
            "ModuleNotFoundError",
        );
        evidence_ledger.record_tool_result(
            &ToolCall {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": "test -x .venv/bin/python && echo PASS"
                }),
            },
            &crate::tools::ToolResult::success("PASS"),
        );
        evidence_ledger.record_tool_result(
            &ToolCall {
                id: "call_2".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'"
                }),
            },
            &crate::tools::ToolResult::success("core-terminal-demo-ok"),
        );
        let required_commands = vec![
            "test -x .venv/bin/python".to_string(),
            ". .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'".to_string(),
        ];

        let evaluation = CloseoutEvaluator::evaluate(
            &code_workflow,
            &bundle,
            &evidence_ledger,
            &required_commands,
        );
        let closeout = evaluation.closeout.expect("closeout");

        assert_eq!(
            evaluation.runtime_validation_label.as_deref(),
            Some("passed:2/2")
        );
        assert_eq!(closeout.status, StageValidationStatus::Passed);
        assert!(closeout.acceptance.iter().any(|item| {
            item.contains("accepted=true") && item.contains("required validation passed")
        }));
    }

    #[test]
    fn verified_change_closeout_records_trace_only_when_ready() {
        let trace =
            TraceCollector::new(crate::engine::trace::TurnTrace::new("session", 1, "change"));

        assert!(
            !VerifiedChangeCloseoutController::should_break_for_verified_change(&trace, false,)
        );
        assert!(VerifiedChangeCloseoutController::should_break_for_verified_change(&trace, true,));

        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        let matching_events = finished
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    TraceEvent::WorkflowFallback { error }
                        if error == VerifiedChangeCloseoutController::VERIFIED_CHANGE_CLOSEOUT_TRACE
                )
            })
            .count();
        assert_eq!(matching_events, 1);
    }
}

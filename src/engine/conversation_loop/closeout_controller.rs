use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::code_change_workflow::{
    CodeChangeWorkflowRunner, StageValidationStatus, WorkflowCloseout,
};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::engine::verification_proof::{
    VerificationProof, VerificationProofRequest, VerificationProofStatus,
};
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
    pub(super) tool_evidence_summary: Option<String>,
    pub(super) verification_proof: VerificationProof,
}

pub(super) struct CloseoutEvaluator;

impl CloseoutEvaluator {
    pub(super) fn evaluate(
        code_workflow: &CodeChangeWorkflowRunner,
        task_bundle: &TaskContextBundle,
        evidence_ledger: &EvidenceLedger,
        required_validation_commands: &[String],
    ) -> CloseoutEvaluation {
        let validation_required =
            closeout_validation_required(code_workflow, task_bundle, required_validation_commands);
        let verification_proof = evidence_ledger.verification_proof(VerificationProofRequest {
            required_commands: required_validation_commands,
            requires_validation: validation_required,
            task_verification_status: task_bundle.agent_state.verification_plan.status,
        });
        let runtime_validation_label = evidence_ledger
            .runtime_required_validation_label(required_validation_commands)
            .or_else(|| evidence_ledger.runtime_validation_label());
        let mut closeout = code_workflow.build_closeout_with_runtime_validation(
            task_bundle,
            runtime_validation_label.as_deref(),
        );
        if let Some(closeout) = &mut closeout {
            apply_verification_proof_to_closeout(
                closeout,
                &verification_proof,
                validation_required,
            );
        }
        let tool_evidence_summary = evidence_ledger.closeout_tool_evidence_summary();
        if let (Some(closeout), Some(summary)) = (&mut closeout, tool_evidence_summary.as_ref()) {
            if !closeout.validation.iter().any(|item| item == summary) {
                closeout.validation.push(summary.clone());
            }
        }
        CloseoutEvaluation {
            closeout,
            runtime_validation_label,
            tool_evidence_summary,
            verification_proof,
        }
    }
}

fn closeout_validation_required(
    code_workflow: &CodeChangeWorkflowRunner,
    task_bundle: &TaskContextBundle,
    required_validation_commands: &[String],
) -> bool {
    use crate::engine::task_context::VerificationStatus;

    code_workflow.policy.require_stage_validation
        || !required_validation_commands.is_empty()
        || !task_bundle
            .agent_state
            .verification_plan
            .required_checks
            .is_empty()
        || matches!(
            task_bundle.agent_state.verification_plan.status,
            VerificationStatus::Pending
                | VerificationStatus::Verified
                | VerificationStatus::Failed
                | VerificationStatus::Blocked
                | VerificationStatus::UserDeferred
                | VerificationStatus::Unavailable
        )
}

fn apply_verification_proof_to_closeout(
    closeout: &mut WorkflowCloseout,
    proof: &VerificationProof,
    validation_required: bool,
) {
    if validation_required || proof.status != VerificationProofStatus::NotApplicable {
        let line = proof.validation_line();
        if !closeout.validation.iter().any(|item| item == &line) {
            closeout.validation.push(line);
        }
    }

    if !proof.status.blocks_verified_closeout() {
        return;
    }

    match proof.status {
        VerificationProofStatus::Failed => closeout.status = StageValidationStatus::Failed,
        VerificationProofStatus::NotRun
            if !validation_required && closeout.status != StageValidationStatus::Passed => {}
        VerificationProofStatus::NotRun
        | VerificationProofStatus::Blocked
        | VerificationProofStatus::UserDeferred
        | VerificationProofStatus::Unavailable => {
            if closeout.status == StageValidationStatus::Passed {
                closeout.status = StageValidationStatus::NotVerified;
            }
        }
        VerificationProofStatus::Verified | VerificationProofStatus::NotApplicable => {}
    }

    let residual = format!(
        "Verification proof is {}: {}",
        proof.status.label(),
        proof.summary
    );
    if !closeout.residual_risks.iter().any(|item| item == &residual) {
        closeout.residual_risks.push(residual);
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
        let CloseoutEvaluation {
            mut closeout,
            runtime_validation_label,
            tool_evidence_summary,
            verification_proof,
        } = CloseoutEvaluator::evaluate(
            context.code_workflow,
            context.task_bundle,
            context.evidence_ledger,
            context.required_validation_commands,
        );
        if closeout.is_none() && should_prepare_mva_direct_closeout(&context) {
            closeout = Some(mva_direct_closeout(
                context.task_bundle,
                context.required_validation_commands,
                runtime_validation_label.as_deref(),
                tool_evidence_summary.as_deref(),
                &verification_proof,
            ));
        }

        if let Some(closeout) = closeout {
            let evidence_snapshot = context.evidence_ledger.snapshot();
            let stop_record = context.task_bundle.agent_state.stop_checks.last();
            let terminal_status = context
                .task_bundle
                .agent_state
                .terminal_status
                .map(|status| status.label().to_string())
                .or_else(|| closeout_terminal_status(closeout.status).map(str::to_string));
            context.trace.record(TraceEvent::FinalCloseoutPrepared {
                status: closeout.status.label().to_string(),
                terminal_status,
                stop_reason: stop_record.map(|record| record.reason.label().to_string()),
                stop_action: stop_record.map(|record| record.action.label().to_string()),
                failure_type: stop_record.and_then(|record| record.failure_type.clone()),
                recovery_plan_id: stop_record.and_then(|record| record.recovery_plan_id.clone()),
                rollback_status: stop_record
                    .and_then(|record| record.rollback_candidate.as_ref())
                    .map(|candidate| {
                        if candidate.auto_allowed {
                            "candidate_auto_allowed".to_string()
                        } else {
                            "candidate_requires_review".to_string()
                        }
                    }),
                changed_files: closeout.changed_files.len(),
                validation_items: closeout.validation.len(),
                tool_records: evidence_snapshot.tool_execution_records,
                tool_evidence: tool_evidence_summary.clone(),
                verification_proof_status: Some(verification_proof.status_label().to_string()),
                verification_proof_summary: Some(verification_proof.summary.clone()),
                acceptance_items: closeout.acceptance.len(),
                residual_risks: closeout.residual_risks.len(),
            });
            context.runtime_diet.closeout_visibility =
                format!("{:?}", closeout.visibility_from_env()).to_ascii_lowercase();
            context.runtime_diet.validation_evidence = runtime_validation_label
                .clone()
                .unwrap_or_else(|| verification_proof.status_label().to_string());
            let closeout_text = if mva_runtime_profile_enabled() {
                closeout.format_for_final_response()
            } else {
                closeout.format_for_user_response()
            };
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
            if let Some(label) = runtime_validation_label {
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
            context.trace.record(TraceEvent::StopCheckEvaluated {
                status: "stop".to_string(),
                reason: "budget_exhausted".to_string(),
                stage: "Closeout".to_string(),
                terminal_status: Some("partial".to_string()),
                action: "closeout".to_string(),
                no_code_progress_rounds: 0,
                action_checkpoint_active: false,
                summary: "tool iteration budget exhausted before final closeout".to_string(),
                evidence_items: 1,
                failure_type: Some("budget_exhausted".to_string()),
                recovery_plan_id: None,
                rollback_recommended: false,
                next_action: Some(
                    "report partial state and continue only after user review".to_string(),
                ),
            });
        }
    }
}

fn should_prepare_mva_direct_closeout(context: &FinalCloseoutContext<'_>) -> bool {
    mva_runtime_profile_enabled()
        && !context.final_content.trim().is_empty()
        && (context.evidence_ledger.snapshot().tool_execution_records > 0
            || !context.required_validation_commands.is_empty())
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

fn mva_direct_closeout(
    task_bundle: &TaskContextBundle,
    required_validation_commands: &[String],
    runtime_validation_label: Option<&str>,
    tool_evidence_summary: Option<&str>,
    verification_proof: &VerificationProof,
) -> WorkflowCloseout {
    let status = match verification_proof.status {
        VerificationProofStatus::Verified | VerificationProofStatus::NotApplicable => {
            StageValidationStatus::Passed
        }
        VerificationProofStatus::Failed => StageValidationStatus::Failed,
        VerificationProofStatus::NotRun
        | VerificationProofStatus::Blocked
        | VerificationProofStatus::UserDeferred
        | VerificationProofStatus::Unavailable => StageValidationStatus::NotVerified,
    };
    let mut validation = Vec::new();
    if let Some(label) = runtime_validation_label {
        validation.push(format!("runtime validation: {label}"));
    } else if required_validation_commands.is_empty() {
        validation.push("No validation command was required".to_string());
    } else {
        validation.push(verification_proof.validation_line());
    }
    if verification_proof.status != VerificationProofStatus::NotApplicable {
        let line = verification_proof.validation_line();
        if !validation.iter().any(|item| item == &line) {
            validation.push(line);
        }
    }
    if let Some(summary) = tool_evidence_summary {
        if !validation.iter().any(|item| item == summary) {
            validation.push(summary.to_string());
        }
    }

    let mut acceptance = if task_bundle.acceptance_checks.is_empty() {
        vec!["No explicit acceptance criteria were recorded".to_string()]
    } else {
        task_bundle
            .acceptance_checks
            .iter()
            .map(|check| format!("pending: {check}"))
            .collect()
    };
    append_mva_goal_and_stop_contract(&mut acceptance, task_bundle);
    if status == StageValidationStatus::Passed && !task_bundle.acceptance_checks.is_empty() {
        acceptance.insert(
            0,
            "accepted=true confidence=Medium unresolved=0 (MVA direct closeout completed with runtime evidence)"
                .to_string(),
        );
    }

    let residual_risks = if status == StageValidationStatus::Passed {
        vec!["none recorded".to_string()]
    } else {
        vec![format!(
            "Verification proof is {}: {}",
            verification_proof.status.label(),
            verification_proof.summary
        )]
    };

    WorkflowCloseout {
        status,
        risk: task_bundle.route.risk,
        changed_files: Vec::new(),
        validation,
        acceptance,
        residual_risks,
    }
}

fn append_mva_goal_and_stop_contract(
    acceptance: &mut Vec<String>,
    task_bundle: &TaskContextBundle,
) {
    push_unique_closeout_line(
        acceptance,
        format!(
            "target: {}",
            closeout_preview(&task_bundle.agent_state.main_goal, 240)
        ),
    );

    let Some(stop) = task_bundle.agent_state.stop_checks.last() else {
        return;
    };
    if stop.reason.label() == "no_issue" {
        return;
    }

    let next = stop.next_action.as_deref().unwrap_or("none");
    push_unique_closeout_line(
        acceptance,
        format!(
            "stop: reason={} action={} summary={} next={}",
            stop.reason.label(),
            stop.action.label(),
            closeout_preview(&stop.summary, 180),
            closeout_preview(next, 120)
        ),
    );
    if !stop.evidence.is_empty() {
        push_unique_closeout_line(
            acceptance,
            format!(
                "checked evidence: {}",
                closeout_preview(&stop.evidence.join("; "), 180)
            ),
        );
    }
}

fn push_unique_closeout_line(items: &mut Vec<String>, item: String) {
    if !items.iter().any(|existing| existing == &item) {
        items.push(item);
    }
}

fn closeout_preview(text: &str, max_chars: usize) -> String {
    let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let mut out = trimmed
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn closeout_terminal_status(status: StageValidationStatus) -> Option<&'static str> {
    match status {
        StageValidationStatus::Passed => Some("completed"),
        StageValidationStatus::Partial | StageValidationStatus::NotVerified => Some("partial"),
        StageValidationStatus::Failed => Some("failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code_change_workflow::StageValidationStatus;
    use crate::engine::intent_router::{
        IntentKind, IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
    };
    use crate::engine::task_context::{
        StopAction, StopCheckReason, StopCheckRecord, StopCheckStatus,
    };
    use crate::engine::trace::{TurnStatus, TurnTrace};
    use crate::test_utils::env_guard::EnvVarGuard;

    fn audit_route() -> IntentRoute {
        IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.90,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk: RiskLevel::High,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "audit/regression eval requires project verification; code diff is optional"
                .to_string(),
        }
    }

    fn direct_route() -> IntentRoute {
        IntentRoute {
            intent: IntentKind::DirectAnswer,
            confidence: 0.90,
            workflow: WorkflowKind::Direct,
            retrieval: RetrievalPolicy::Light,
            reasoning: ReasoningPolicy::Low,
            risk: RiskLevel::Low,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "direct read-only evidence task".to_string(),
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

    #[tokio::test]
    async fn mva_profile_adds_structured_closeout_for_direct_tool_turn() {
        let mut env = EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_RUNTIME_PROFILE", "minimum_viable_agent");
        let bundle = TaskContextBundle::new("inspect one known file", ".", direct_route(), None);
        let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "inspect"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "fixtures/known.txt"}),
        };
        let mut evidence_ledger = EvidenceLedger::new();
        evidence_ledger
            .record_tool_result(&tool_call, &crate::tools::ToolResult::success("known fact"));
        let mut final_content = "Observed known fact.".to_string();

        FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
            trace: &trace,
            code_workflow: &code_workflow,
            task_bundle: &bundle,
            required_validation_commands: &[],
            runtime_diet: &mut runtime_diet,
            final_content: &mut final_content,
            final_tool_calls: &[],
            iterations_used: 1,
            max_iterations: 10,
            evidence_ledger: &evidence_ledger,
            tx: None,
        })
        .await;

        assert!(final_content.contains("Closeout:"));
        assert!(final_content.contains("- Status: passed"));
        assert!(final_content.contains("- Changed: none"));
        assert!(final_content.contains("target: inspect one known file"));
        assert_eq!(runtime_diet.validation_evidence, "not_applicable");

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::FinalCloseoutPrepared {
                status,
                changed_files: 0,
                tool_records: 1,
                ..
            } if status == "passed"
        )));
    }

    #[tokio::test]
    async fn mva_direct_closeout_preserves_low_value_stop_target() {
        let mut env = EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_RUNTIME_PROFILE", "minimum_viable_agent");
        let mut bundle = TaskContextBundle::new(
            "在 fixtures/mva_low_value_replan 里找到 missing-target-token-7391",
            ".",
            direct_route(),
            None,
        );
        bundle.agent_state.record_stop_check(StopCheckRecord {
            status: StopCheckStatus::Stop,
            terminal_status: None,
            action: StopAction::Closeout,
            reason: StopCheckReason::DuplicateReadOnly,
            summary: "duplicate read-only calls would not change task state".to_string(),
            evidence: vec!["checked fixtures/mva_low_value_replan/known.txt".to_string()],
            failure_type: Some("duplicate_read_only".to_string()),
            recovery_plan_id: None,
            rollback_candidate: None,
            next_action: Some("close out with the already-observed evidence".to_string()),
            no_code_progress_rounds: 0,
            action_checkpoint_active: false,
        });
        let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "low value"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut evidence_ledger = EvidenceLedger::new();
        evidence_ledger.record_tool_result(
            &ToolCall {
                id: "call-1".to_string(),
                name: "file_read".to_string(),
                arguments: serde_json::json!({"path": "fixtures/mva_low_value_replan/known.txt"}),
            },
            &crate::tools::ToolResult::success("known fact"),
        );
        let mut final_content = "No matching token was found in the checked file.".to_string();

        FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
            trace: &trace,
            code_workflow: &code_workflow,
            task_bundle: &bundle,
            required_validation_commands: &[],
            runtime_diet: &mut runtime_diet,
            final_content: &mut final_content,
            final_tool_calls: &[],
            iterations_used: 1,
            max_iterations: 10,
            evidence_ledger: &evidence_ledger,
            tx: None,
        })
        .await;

        assert!(final_content.contains("missing-target-token-7391"));
        assert!(final_content.contains("stop: reason=duplicate_read_only"));
        assert!(final_content.contains("checked evidence:"));
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
        assert!(evaluation
            .tool_evidence_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("tool evidence: records=2")));
        assert_eq!(closeout.status, StageValidationStatus::Passed);
        assert!(closeout
            .validation
            .iter()
            .any(|item| item.contains("tool evidence: records=2")));
        assert!(closeout.acceptance.iter().any(|item| {
            item.contains("accepted=true") && item.contains("required validation passed")
        }));
        assert_eq!(
            evaluation.verification_proof.status,
            VerificationProofStatus::Verified
        );
        assert!(closeout.validation.iter().any(|item| item
            .contains("verification proof: verified (required validation passed 2/2 commands)")));
    }

    #[test]
    fn evaluator_records_not_run_proof_when_required_validation_is_missing() {
        let mut bundle = TaskContextBundle::new("审查已有实现", ".", audit_route(), None);
        bundle.add_acceptance_check("required regression checks pass");
        let code_workflow = CodeChangeWorkflowRunner::new(&bundle);
        let evidence_ledger = EvidenceLedger::new();
        let required_commands = vec!["cargo test -q memory".to_string()];

        let evaluation = CloseoutEvaluator::evaluate(
            &code_workflow,
            &bundle,
            &evidence_ledger,
            &required_commands,
        );
        let closeout = evaluation.closeout.expect("closeout");

        assert_eq!(
            evaluation.verification_proof.status,
            VerificationProofStatus::NotRun
        );
        assert_eq!(closeout.status, StageValidationStatus::NotVerified);
        assert!(closeout
            .validation
            .iter()
            .any(|item| item.contains("verification proof: not_run")));
        assert!(closeout
            .residual_risks
            .iter()
            .any(|item| item.contains("Verification proof is not_run")));
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

use super::final_answer_claim_gate::{
    FinalAnswerClaimGate, FinalAnswerClaimGateDecision, FinalAnswerClaimGateInput,
};
use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::code_change_workflow::{
    CodeChangeWorkflowRunner, StageValidationStatus, WorkflowCloseout,
};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::WorkflowKind;
use crate::engine::project_progress::ProjectProgressLedger;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::task_contract::{
    BackgroundMemoryReviewWorker, BackgroundReviewPacket, ExecutionReport, MemoryProposal,
    MemoryProposalReviewStore, TaskContractBundleExt,
};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::engine::verification_proof::{
    VerificationProof, VerificationProofRequest, VerificationProofStatus, VerificationProofTaskType,
};
use crate::services::api::{Message, ToolCall};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::warn;

pub(super) struct FinalCloseoutContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) required_validation_commands: &'a [String],
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a [ToolCall],
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) claim_gate_repair_used: &'a mut bool,
    pub(super) iterations_used: usize,
    pub(super) max_iterations: usize,
    pub(super) evidence_ledger: &'a EvidenceLedger,
    pub(super) settlement_gaps: &'a [String],
    pub(super) memory_generate_enabled: bool,
    pub(super) tx: Option<&'a mpsc::Sender<super::super::streaming::StreamEvent>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FinalCloseoutFlow {
    Completed,
    Retry,
}

pub(super) struct CloseoutEvaluation {
    pub(super) closeout: Option<WorkflowCloseout>,
    pub(super) runtime_validation_label: Option<String>,
    pub(super) tool_evidence_summary: Option<String>,
    pub(super) verification_proof: VerificationProof,
}

fn closeout_background_timeout() -> Duration {
    crate::services::config::runtime_config().closeout_background_timeout()
}

async fn run_closeout_background_stage<T, F>(
    trace: TraceCollector,
    stage: &'static str,
    timeout: Duration,
    work: F,
) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    trace.record(TraceEvent::CloseoutBackgroundStage {
        stage: stage.to_string(),
        status: "started".to_string(),
        duration_ms: 0,
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        detail: "started".to_string(),
    });
    let started = Instant::now();
    let handle = tokio::task::spawn_blocking(work);
    let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
    match tokio::time::timeout(timeout, handle).await {
        Ok(Ok(Ok(value))) => {
            trace.record(TraceEvent::CloseoutBackgroundStage {
                stage: stage.to_string(),
                status: "completed".to_string(),
                duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                timeout_ms,
                detail: "completed".to_string(),
            });
            Some(value)
        }
        Ok(Ok(Err(error))) => {
            let detail = error.to_string();
            warn!("closeout background stage {stage} failed: {detail}");
            trace.record(TraceEvent::CloseoutBackgroundStage {
                stage: stage.to_string(),
                status: "failed".to_string(),
                duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                timeout_ms,
                detail,
            });
            None
        }
        Ok(Err(error)) => {
            let detail = error.to_string();
            warn!("closeout background stage {stage} join failed: {detail}");
            trace.record(TraceEvent::CloseoutBackgroundStage {
                stage: stage.to_string(),
                status: "failed".to_string(),
                duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                timeout_ms,
                detail,
            });
            None
        }
        Err(_) => {
            warn!(
                "closeout background stage {stage} exceeded {}ms; continuing closeout",
                timeout_ms
            );
            trace.record(TraceEvent::CloseoutBackgroundStage {
                stage: stage.to_string(),
                status: "timed_out".to_string(),
                duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                timeout_ms,
                detail: "timed out; closeout continued".to_string(),
            });
            None
        }
    }
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
        let support_context = evidence_ledger.verification_proof_support_context(
            verification_proof_task_type(task_bundle),
            required_validation_commands,
        );
        let verification_proof = evidence_ledger.verification_proof(VerificationProofRequest {
            required_commands: required_validation_commands,
            requires_validation: validation_required,
            task_verification_status: task_bundle.agent_state.verification_plan.status,
            support_context,
        });
        let runtime_validation_label = evidence_ledger
            .runtime_required_validation_label(required_validation_commands)
            .or_else(|| evidence_ledger.runtime_validation_label());
        let ledger_changed_files = evidence_ledger.changed_files();
        let mut closeout = code_workflow.build_closeout_with_runtime_validation(
            task_bundle,
            runtime_validation_label.as_deref(),
        );
        if let Some(closeout) = &mut closeout {
            merge_ledger_changed_files_into_closeout(closeout, &ledger_changed_files);
            apply_verification_proof_to_closeout(
                closeout,
                &verification_proof,
                validation_required,
            );
            apply_verified_runtime_validation_to_closeout(
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

fn merge_ledger_changed_files_into_closeout(closeout: &mut WorkflowCloseout, paths: &[String]) {
    for path in paths {
        if !closeout
            .changed_files
            .iter()
            .any(|existing| existing == path)
        {
            closeout.changed_files.push(path.clone());
        }
    }
    if !closeout.changed_files.is_empty() {
        closeout.residual_risks.retain(|risk| {
            !risk.contains("No changed files were recorded for this code-change workflow")
        });
    }
}

fn verification_proof_task_type(task_bundle: &TaskContextBundle) -> VerificationProofTaskType {
    match task_bundle.route.workflow {
        WorkflowKind::Direct => VerificationProofTaskType::DirectAnswer,
        WorkflowKind::Research | WorkflowKind::Planning => VerificationProofTaskType::ReadOnlyAudit,
        WorkflowKind::CodeChange => VerificationProofTaskType::CodeChange,
        WorkflowKind::BugFix => VerificationProofTaskType::BugFix,
        WorkflowKind::Delegation => VerificationProofTaskType::SubagentReview,
    }
}

fn closeout_validation_required(
    code_workflow: &CodeChangeWorkflowRunner,
    task_bundle: &TaskContextBundle,
    required_validation_commands: &[String],
) -> bool {
    use crate::engine::task_context::VerificationStatus;

    let programming_workflow = matches!(
        task_bundle.route.workflow,
        WorkflowKind::CodeChange | WorkflowKind::BugFix
    );
    let explicit_validation_required = !required_validation_commands.is_empty()
        || !task_bundle
            .agent_state
            .verification_plan
            .required_checks
            .is_empty();

    code_workflow.policy.require_stage_validation
        || explicit_validation_required
        || (programming_workflow
            && matches!(
                task_bundle.agent_state.verification_plan.status,
                VerificationStatus::Pending
                    | VerificationStatus::Verified
                    | VerificationStatus::Failed
                    | VerificationStatus::Blocked
                    | VerificationStatus::UserDeferred
                    | VerificationStatus::Unavailable
            ))
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
    if validation_required || proof.derived_support.status != VerificationProofStatus::NotApplicable
    {
        let line = proof.support_line();
        if !closeout.validation.iter().any(|item| item == &line) {
            closeout.validation.push(line);
        }
    }

    if !proof.status.blocks_verified_closeout() {
        apply_proof_support_to_closeout(closeout, proof);
        return;
    }

    match proof.status {
        VerificationProofStatus::Failed => closeout.status = StageValidationStatus::Failed,
        VerificationProofStatus::Partial => {
            if closeout.status == StageValidationStatus::Passed {
                closeout.status = StageValidationStatus::Partial;
            }
        }
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
    apply_proof_support_to_closeout(closeout, proof);
}

fn apply_verified_runtime_validation_to_closeout(
    closeout: &mut WorkflowCloseout,
    proof: &VerificationProof,
    validation_required: bool,
) {
    if !validation_required
        || proof.status != VerificationProofStatus::Verified
        || proof.derived_support.status != VerificationProofStatus::Verified
        || !proof.derived_support.supports_verified
        || proof.derived_support.residual_risk
        || closeout.status == StageValidationStatus::Failed
        || closeout.changed_files.is_empty()
    {
        return;
    }

    closeout.status = StageValidationStatus::Passed;
    if closeout
        .acceptance
        .iter()
        .all(|item| item.starts_with("pending:"))
    {
        closeout.acceptance.clear();
        closeout.acceptance.push(
            "accepted=true confidence=High unresolved=0 (required validation passed with runtime evidence)"
                .to_string(),
        );
    }
    closeout.residual_risks.retain(|risk| {
        !matches!(
            risk.as_str(),
            "Required validation was not run or not recorded"
                | "Acceptance criteria were generated but not reviewed"
                | "Workflow finished with unresolved validation or acceptance risk"
        )
    });
    if closeout.residual_risks.is_empty() {
        closeout.residual_risks.push("none recorded".to_string());
    }
}

fn apply_proof_support_to_closeout(closeout: &mut WorkflowCloseout, proof: &VerificationProof) {
    match proof.derived_support.status {
        VerificationProofStatus::Verified | VerificationProofStatus::NotApplicable => {}
        VerificationProofStatus::Failed => closeout.status = StageValidationStatus::Failed,
        VerificationProofStatus::Partial => {
            if closeout.status == StageValidationStatus::Passed {
                closeout.status = StageValidationStatus::Partial;
            }
        }
        VerificationProofStatus::NotRun
        | VerificationProofStatus::Blocked
        | VerificationProofStatus::UserDeferred
        | VerificationProofStatus::Unavailable => {
            if closeout.status == StageValidationStatus::Passed {
                closeout.status = StageValidationStatus::NotVerified;
            }
        }
    }

    if !proof.derived_support.residual_risk {
        return;
    }
    let residual = format!(
        "Verification proof support is {}: {}",
        proof.derived_support.status.label(),
        proof.derived_support.summary
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
    pub(super) async fn apply_final_closeout(
        context: FinalCloseoutContext<'_>,
    ) -> FinalCloseoutFlow {
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

        if let Some(mut closeout) = closeout {
            // Final-answer claim gate: if the model's text claims completion/mutation/validation
            // that the evidence does not support, downgrade the closeout so the user sees an
            // honest status instead of a falsely-claimed success.
            let claim_gate_input = FinalAnswerClaimGateInput {
                content: context.final_content,
                route: &context.task_bundle.route,
                evidence_ledger: context.evidence_ledger,
                verification_proof: &verification_proof,
                required_validation_commands: context.required_validation_commands,
                repair_used: *context.claim_gate_repair_used,
                iterations_used: context.iterations_used,
                max_iterations: context.max_iterations,
            };
            let claim_gate_decision = FinalAnswerClaimGate::evaluate(claim_gate_input);
            match claim_gate_decision {
                FinalAnswerClaimGateDecision::Repair { observation } => {
                    *context.claim_gate_repair_used = true;
                    context.trace.record(TraceEvent::FinalAnswerClaimGate {
                        decision: "repair".to_string(),
                        unsupported_claims: observation.unsupported_claims.len(),
                        repair_attempt: 1,
                        changed_files: observation.runtime_evidence.changed_files.len(),
                        verification_proof_status: Some(
                            observation.runtime_evidence.verification_status.clone(),
                        ),
                        summary: "closeout claim gate: unsupported final claims re-entered the model loop".to_string(),
                    });
                    context.messages.push(Message::assistant(
                        super::tool_execution::safe_prefix_by_bytes(context.final_content, 1200),
                    ));
                    context.messages.push(Message::system(format!(
                        "<recent_observation>\n{}\n</recent_observation>",
                        observation.to_recent_observation_text()
                    )));
                    context.final_content.clear();
                    return FinalCloseoutFlow::Retry;
                }
                FinalAnswerClaimGateDecision::Downgrade { observation, .. } => {
                    let unsupported_kinds: Vec<String> = observation
                        .unsupported_claims
                        .iter()
                        .map(|claim| format!("{:?}", claim.kind))
                        .collect();
                    let risk_line = format!(
                        "claim_gate: final answer contained unsupported claims ({}); downgraded from {:?} to not_verified",
                        unsupported_kinds.join(", "),
                        closeout.status
                    );
                    if closeout.status == StageValidationStatus::Passed {
                        closeout.status = StageValidationStatus::NotVerified;
                    }
                    if !closeout
                        .residual_risks
                        .iter()
                        .any(|r| r.contains("claim_gate"))
                    {
                        closeout.residual_risks.push(risk_line);
                    }
                    context.trace.record(TraceEvent::FinalAnswerClaimGate {
                        decision: "downgrade".to_string(),
                        unsupported_claims: observation.unsupported_claims.len(),
                        repair_attempt: 0,
                        changed_files: observation.runtime_evidence.changed_files.len(),
                        verification_proof_status: Some(
                            observation.runtime_evidence.verification_status.clone(),
                        ),
                        summary: format!(
                            "closeout claim gate: unsupported claims ({}) downgraded status",
                            unsupported_kinds.join(", ")
                        ),
                    });
                }
                FinalAnswerClaimGateDecision::Pass => {}
            }

            let evidence_snapshot = context.evidence_ledger.snapshot();

            // Settlement gap check: if tools were invoked in a programming workflow
            // but verification is incomplete, surface the settlement risk.
            if closeout.changed_files.is_empty()
                && evidence_snapshot.tool_execution_records > 1
                && !context.required_validation_commands.is_empty()
                && closeout.status != StageValidationStatus::Failed
            {
                closeout.status = StageValidationStatus::NotVerified;
                let gap_msg = format!(
                    "settlement_gap: {} tool record(s) without file changes or validation proof",
                    evidence_snapshot.tool_execution_records
                );
                if !closeout
                    .residual_risks
                    .iter()
                    .any(|r| r.contains("settlement_gap"))
                {
                    closeout.residual_risks.push(gap_msg);
                }
                context
                    .trace
                    .record(TraceEvent::WorkflowFallback {
                        error: "settlement_gap: tools executed but no file changes or validation proof produced"
                            .to_string(),
                    });
            }

            if !context.settlement_gaps.is_empty()
                && closeout.status != StageValidationStatus::Failed
            {
                closeout.status = StageValidationStatus::NotVerified;
                let gap_msg = format!(
                    "settlement_gap: unsettled tool lifecycle record(s): {}",
                    context.settlement_gaps.join(", ")
                );
                if !closeout.residual_risks.iter().any(|risk| risk == &gap_msg) {
                    closeout.residual_risks.push(gap_msg.clone());
                }
                context
                    .trace
                    .record(TraceEvent::WorkflowFallback { error: gap_msg });
            }

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
                verification_proof_kind_summary: Some(verification_proof.proof_kind_summary()),
                verification_proof_support_status: Some(
                    verification_proof
                        .derived_support
                        .status
                        .label()
                        .to_string(),
                ),
                verification_proof_support_summary: Some(
                    verification_proof.derived_support.summary.clone(),
                ),
                verification_proof_supports_verified: Some(
                    verification_proof.derived_support.supports_verified,
                ),
                verification_proof_residual_risk: Some(
                    verification_proof.derived_support.residual_risk,
                ),
                acceptance_items: closeout.acceptance.len(),
                residual_risks: closeout.residual_risks.len(),
            });

            if let Some(tx) = context.tx {
                let _ = tx
                    .send(super::super::streaming::StreamEvent::Closeout {
                        status: verification_proof.status_label().to_string(),
                        evidence_summary: Some(verification_proof.summary.clone()),
                    })
                    .await;
            }
            let contract = context
                .task_bundle
                .task_contract(context.required_validation_commands);
            let report = ExecutionReport::from_closeout(&contract, &closeout);
            context.trace.record(TraceEvent::ExecutionReportPrepared {
                task_id: report.task_id.clone(),
                status: report.status.label().to_string(),
                changed_files: report.changed_files.len(),
                validation_evidence: report.validation_evidence.len(),
                risks: report.risks.len(),
                next_steps: report.next_steps.len(),
            });
            let closeout_background_timeout = closeout_background_timeout();
            let progress_report = report.clone();
            let _ = run_closeout_background_stage(
                context.trace.clone(),
                "project_progress_append",
                closeout_background_timeout,
                move || {
                    ProjectProgressLedger::default().append_execution_report(&progress_report)?;
                    Ok(())
                },
            )
            .await;
            let memory_proposal = context
                .memory_generate_enabled
                .then(|| MemoryProposal::from_execution_report(&report));
            let background_proposal = if context.memory_generate_enabled {
                let proposal_report = report.clone();
                let proposal_for_store = memory_proposal.clone();
                run_closeout_background_stage(
                    context.trace.clone(),
                    "memory_proposal_review",
                    closeout_background_timeout,
                    move || {
                        let proposal_store = MemoryProposalReviewStore::default();
                        let recent_proposals = proposal_store.list();
                        if let Some(memory_proposal) = proposal_for_store.as_ref() {
                            proposal_store.upsert(memory_proposal)?;
                        }
                        let background_packet = BackgroundReviewPacket::from_execution_report(
                            &proposal_report,
                            &recent_proposals,
                        );
                        let background_output =
                            BackgroundMemoryReviewWorker::review_execution_report(
                                &background_packet,
                                &proposal_report,
                            );
                        let background_proposal =
                            BackgroundMemoryReviewWorker::proposal_from_output(
                                &background_packet,
                                background_output,
                            );
                        proposal_store.upsert(&background_proposal)?;
                        Ok(Some(background_proposal))
                    },
                )
                .await
                .flatten()
            } else {
                None
            };
            if let Some(memory_proposal) = memory_proposal.as_ref() {
                context.trace.record(TraceEvent::MemoryProposalPrepared {
                    task_id: memory_proposal.task_id.clone(),
                    status: memory_proposal.status.label().to_string(),
                    candidates: memory_proposal.candidates.len(),
                    candidate_kinds: memory_proposal.candidate_kinds(),
                    evidence_items: memory_proposal.evidence_items(),
                    write_policy: memory_proposal.write_policy.clone(),
                    write_performed: memory_proposal.write_performed,
                    reason: memory_proposal.reason.clone(),
                });
            } else {
                context.trace.record(TraceEvent::MemoryProposalPrepared {
                    task_id: report.task_id.clone(),
                    status: "skipped".to_string(),
                    candidates: 0,
                    candidate_kinds: Vec::new(),
                    evidence_items: 0,
                    write_policy: "generation_disabled".to_string(),
                    write_performed: false,
                    reason: "memory.generate is off for this session".to_string(),
                });
            }
            if let Some(background_proposal) = background_proposal.as_ref() {
                context.trace.record(TraceEvent::MemoryProposalPrepared {
                    task_id: background_proposal.task_id.clone(),
                    status: background_proposal.status.label().to_string(),
                    candidates: background_proposal.candidates.len(),
                    candidate_kinds: background_proposal.candidate_kinds(),
                    evidence_items: background_proposal.evidence_items(),
                    write_policy: background_proposal.write_policy.clone(),
                    write_performed: background_proposal.write_performed,
                    reason: background_proposal.reason.clone(),
                });
            }
            context.runtime_diet.closeout_visibility =
                format!("{:?}", closeout.visibility_from_env()).to_ascii_lowercase();
            context.runtime_diet.validation_evidence = runtime_validation_label
                .clone()
                .unwrap_or_else(|| verification_proof.status_label().to_string());
            let closeout_text = if structured_closeout_runtime_profile_enabled() {
                let mut text = format!("Task contract: {}\n", contract.compact_summary());
                text.push_str(&closeout.format_for_final_response());
                if let Some(memory_proposal) = memory_proposal.as_ref() {
                    let memory_proposal_text = memory_proposal.format_for_final_response();
                    if !memory_proposal_text.is_empty() {
                        text.push_str(&memory_proposal_text);
                        text.push('\n');
                    }
                }
                text
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

        FinalCloseoutFlow::Completed
    }
}

fn should_prepare_mva_direct_closeout(context: &FinalCloseoutContext<'_>) -> bool {
    structured_closeout_runtime_profile_enabled()
        && !context.final_content.trim().is_empty()
        && (context.evidence_ledger.snapshot().tool_execution_records > 0
            || !context.required_validation_commands.is_empty())
}

fn structured_closeout_runtime_profile_enabled() -> bool {
    crate::services::config::runtime_config().is_structured_closeout_profile()
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
            match verification_proof.derived_support.status {
                VerificationProofStatus::Partial => StageValidationStatus::Partial,
                VerificationProofStatus::Failed => StageValidationStatus::Failed,
                VerificationProofStatus::NotRun
                | VerificationProofStatus::Blocked
                | VerificationProofStatus::UserDeferred
                | VerificationProofStatus::Unavailable => StageValidationStatus::NotVerified,
                VerificationProofStatus::Verified | VerificationProofStatus::NotApplicable => {
                    StageValidationStatus::Passed
                }
            }
        }
        VerificationProofStatus::Partial => StageValidationStatus::Partial,
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
    if verification_proof.derived_support.status != VerificationProofStatus::NotApplicable {
        let line = verification_proof.support_line();
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

    let residual_risks = if status == StageValidationStatus::Passed
        && !verification_proof.derived_support.residual_risk
    {
        vec!["none recorded".to_string()]
    } else {
        let mut risks = vec![format!(
            "Verification proof is {}: {}",
            verification_proof.status.label(),
            verification_proof.summary
        )];
        if verification_proof.derived_support.residual_risk {
            risks.push(format!(
                "Verification proof support is {}: {}",
                verification_proof.derived_support.status.label(),
                verification_proof.derived_support.summary
            ));
        }
        risks
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
mod tests;

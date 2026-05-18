use super::risk_signal_controller::{RiskSignalController, RuntimeRiskSignalInput};
use super::workflow_runtime::{
    is_high_risk_workflow, persist_workflow_learning_event, workflow_contract_enabled,
};
use super::workflow_trace::{apply_workflow_feedback_and_trace, trace_adaptive_workflow_trigger};
use super::ConversationLoop;
use crate::engine::code_change_workflow::{AdaptiveWorkflowTrigger, CodeChangeWorkflowRunner};
use crate::engine::intent_router::IntentRoute;
use crate::engine::reflection_pass::ReflectionPass;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::engine::workflow_contract::{
    AcceptanceConfidence, AcceptanceCriterion, AcceptanceNextAction, AcceptanceReview,
    AcceptanceStatus, ProgrammingWorkflowJudgment,
};
use crate::services::api::Message;
use tracing::warn;

pub(super) struct GuidedValidationDebuggingContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) last_user_preview: &'a str,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) post_edit_evidence: &'a [String],
    pub(super) repair_tool_record_evidence: &'a [String],
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct AcceptanceRepairContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) changed_files: &'a [std::path::PathBuf],
    pub(super) verify_passed: bool,
    pub(super) review_success: bool,
    pub(super) required_validation_commands: &'a [String],
    pub(super) failed_commands: &'a [String],
    pub(super) post_edit_evidence: &'a [String],
    pub(super) repair_tool_record_evidence: &'a [String],
    pub(super) acceptance_evidence: &'a [String],
    pub(super) required_validation_passed: bool,
    pub(super) check_passed: bool,
    pub(super) acceptance_repair_attempts: &'a mut usize,
    pub(super) reserved_repair_rounds: &'a mut usize,
    pub(super) action_checkpoint_no_change_rounds: &'a mut usize,
    pub(super) action_checkpoint_active: &'a mut bool,
    pub(super) action_checkpoint_lookup_count: &'a mut usize,
    pub(super) file_edit_failure_retry_used: &'a mut bool,
    pub(super) action_checkpoint_requires_patch_before_validation: &'a mut bool,
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct VerificationRepairContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_id: String,
    pub(super) changed_files: &'a [std::path::PathBuf],
    pub(super) verify_passed: bool,
    pub(super) post_edit_evidence: &'a [String],
    pub(super) repair_tool_record_evidence: &'a [String],
    pub(super) failed_commands: &'a [String],
    pub(super) acceptance_repair_attempts: usize,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct AcceptanceRepairOutcome {
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) final_content: Option<String>,
    pub(super) break_loop: bool,
}

impl ConversationLoop {
    pub(super) fn record_verification_repair_context(
        context: VerificationRepairContext<'_>,
    ) -> ReflectionPass {
        let mut post_edit_reflection = ReflectionPass::from_post_edit(
            context.task_id,
            context.changed_files,
            context.verify_passed,
            context.post_edit_evidence,
        );
        if !context.verify_passed {
            let syntax_error = context.post_edit_evidence.iter().any(|item| {
                let lower = item.to_ascii_lowercase();
                lower.contains("syntaxerror")
                    || lower.contains("indentationerror")
                    || lower.contains("parse error")
            });
            if let Some(assessment) =
                RiskSignalController::assess_runtime_failure(RuntimeRiskSignalInput {
                    failed_validation_commands: context.failed_commands,
                    failed_tool_evidence: &[],
                    syntax_error,
                })
            {
                context.trace.record(TraceEvent::RiskSignalAssessed {
                    phase: "runtime".to_string(),
                    level: assessment.level.label().to_string(),
                    entry_contract: assessment.entry_contract,
                    reasons: assessment.reasons,
                });
            }
            if context
                .code_workflow
                .activate_trigger(AdaptiveWorkflowTrigger::VerificationFailed)
            {
                trace_adaptive_workflow_trigger(
                    context.trace,
                    AdaptiveWorkflowTrigger::VerificationFailed,
                    context.code_workflow,
                );
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: "adaptive workflow trigger activated: verification_failed".to_string(),
                });
            }
            let verification_command = context
                .failed_commands
                .first()
                .cloned()
                .unwrap_or_else(|| "post-edit verification".to_string());
            post_edit_reflection.record_repair_action(
                context.acceptance_repair_attempts + 1,
                "repair failed verification before closeout",
                context
                    .changed_files
                    .first()
                    .map(|path| path.display().to_string()),
                verification_command,
            );
            let repair_spec =
                crate::engine::repair_spec::RepairSpec::from_failure_with_tool_records(
                    context.failed_commands,
                    context.post_edit_evidence,
                    context.repair_tool_record_evidence,
                    None,
                );
            let repair_spec_text = repair_spec.format_for_prompt();
            context.tool_results_text.push('\n');
            context.tool_results_text.push_str(&repair_spec_text);
            context.messages.push(Message::system(repair_spec_text));
        }

        post_edit_reflection
    }

    pub(super) async fn run_guided_validation_debugging(
        &self,
        context: GuidedValidationDebuggingContext<'_>,
    ) {
        if !workflow_contract_enabled(self.provider.as_ref()) {
            return;
        }

        let analyzer = crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
            self.provider.as_ref(),
            self.model.clone(),
        );
        let mut evidence = context.post_edit_evidence.to_vec();
        evidence.extend(context.repair_tool_record_evidence.iter().cloned());
        let prompt = crate::engine::workflow_contract::GuidedDebuggingPrompt::new(
            context.last_user_preview,
            context
                .task_bundle
                .workflow_judgment
                .as_ref()
                .map(|judgment| judgment.to_turn_context()),
            vec!["stage_validation".to_string()],
            evidence,
        );
        match analyzer.analyze_debugging(prompt).await {
            Ok(debugging) => {
                context.trace.record(TraceEvent::GuidedDebuggingCompleted {
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
                        "Guided validation debugging selected {:?}: {}",
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
                        "source": "stage_validation",
                    }),
                );
                let debugging_text = debugging.format_for_prompt();
                context.tool_results_text.push('\n');
                context.tool_results_text.push_str(&debugging_text);
                context.messages.push(Message::system(debugging_text));
            }
            Err(err) => {
                warn!("Guided validation debugging failed: {}", err);
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: format!("guided validation debugging failed: {}", err),
                });
            }
        }
    }

    pub(super) async fn run_acceptance_repair_review(
        &self,
        context: AcceptanceRepairContext<'_>,
    ) -> AcceptanceRepairOutcome {
        let mut outcome = AcceptanceRepairOutcome {
            should_closeout_after_verified_change: context.should_closeout_after_verified_change,
            final_content: None,
            break_loop: false,
        };

        let judgment = context.task_bundle.workflow_judgment.as_ref();

        if let Some(review) = Self::required_validation_acceptance_review(
            context.task_bundle,
            judgment,
            context.verify_passed,
            context.required_validation_commands,
        ) {
            context.trace.record(TraceEvent::AcceptanceReviewCompleted {
                accepted: true,
                confidence: "High".to_string(),
                criteria: review.criteria.len(),
                unresolved: 0,
                next_action: "Finish".to_string(),
            });
            context.code_workflow.record_acceptance_review(review);
            outcome.should_closeout_after_verified_change = true;
            if let Some(judgment) = judgment {
                context.trace.record(TraceEvent::WorkflowPlanProgress {
                    total_steps: judgment.plan.len(),
                    completed_steps: judgment.plan.len(),
                    active_step: None,
                    top_priority: None,
                    top_importance_score: None,
                    top_weight_share: None,
                    weight_source: None,
                    reweighted: true,
                });
            }
            return outcome;
        }

        let Some(judgment) = judgment else {
            return outcome;
        };

        if !context.code_workflow.should_run_acceptance_review(
            context.verify_passed,
            context.review_success,
            !context.required_validation_commands.is_empty(),
            judgment.acceptance.criteria.len(),
        ) || !workflow_contract_enabled(self.provider.as_ref())
        {
            return outcome;
        }

        let analyzer = crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
            self.provider.as_ref(),
            self.model.clone(),
        );
        let prompt = crate::engine::workflow_contract::AcceptanceReviewPrompt::new(
            judgment.acceptance.clone(),
            context
                .changed_files
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            context.verify_passed,
            context.acceptance_evidence.to_vec(),
        );
        match analyzer.review_acceptance(prompt).await {
            Ok(review) => {
                let high_risk = is_high_risk_workflow(context.route, Some(judgment));
                let review_next_action = review.next_action;
                let review_accepted = review.accepted;
                let review_unresolved = review.unresolved_count();
                context.trace.record(TraceEvent::AcceptanceReviewCompleted {
                    accepted: review_accepted,
                    confidence: format!("{:?}", review.confidence),
                    criteria: review.criteria.len(),
                    unresolved: review_unresolved,
                    next_action: format!("{:?}", review.next_action),
                });
                context
                    .code_workflow
                    .record_acceptance_review(review.clone());
                if review_accepted {
                    outcome.should_closeout_after_verified_change = true;
                    context.trace.record(TraceEvent::WorkflowPlanProgress {
                        total_steps: judgment.plan.len(),
                        completed_steps: judgment.plan.len(),
                        active_step: None,
                        top_priority: None,
                        top_importance_score: None,
                        top_weight_share: None,
                        weight_source: None,
                        reweighted: true,
                    });
                }
                persist_workflow_learning_event(
                    self.session_store.as_ref(),
                    &self.session_id,
                    "acceptance_review",
                    format!(
                        "Acceptance review accepted={} next={:?}",
                        review_accepted, review_next_action
                    ),
                    if review_accepted { 0.95 } else { 0.85 },
                    serde_json::json!({
                        "accepted": review_accepted,
                        "confidence": format!("{:?}", review.confidence),
                        "criteria": review.criteria.clone(),
                        "unresolved_items": review.unresolved_items.clone(),
                        "residual_risks": review.residual_risks.clone(),
                        "next_action": format!("{:?}", review_next_action),
                        "high_risk": high_risk,
                        "changed_files": context.changed_files.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                    }),
                );
                let review_text = review.format_for_prompt();
                context.tool_results_text.push('\n');
                context.tool_results_text.push_str(&review_text);
                context.messages.push(Message::system(review_text.clone()));
                if !review_accepted
                    && matches!(
                        review_next_action,
                        crate::engine::workflow_contract::AcceptanceNextAction::ContinueRepair
                            | crate::engine::workflow_contract::AcceptanceNextAction::Stop
                    )
                {
                    if context
                        .code_workflow
                        .activate_trigger(AdaptiveWorkflowTrigger::AcceptanceRejected)
                    {
                        trace_adaptive_workflow_trigger(
                            context.trace,
                            AdaptiveWorkflowTrigger::AcceptanceRejected,
                            context.code_workflow,
                        );
                        context.trace.record(TraceEvent::WorkflowFallback {
                            error: "adaptive workflow trigger activated: acceptance_rejected"
                                .to_string(),
                        });
                    }
                    outcome.should_closeout_after_verified_change = false;
                    apply_workflow_feedback_and_trace(
                        context.task_bundle,
                        context.trace,
                        crate::engine::workflow_contract::WeightFeedbackEvent {
                            kind:
                                crate::engine::workflow_contract::WeightFeedbackKind::AcceptanceGap,
                            severity: if high_risk || review_unresolved > 1 {
                                crate::engine::workflow_contract::WeightFeedbackSeverity::High
                            } else {
                                crate::engine::workflow_contract::WeightFeedbackSeverity::Medium
                            },
                            confidence: 0.90,
                            reason: Some(format!(
                                "acceptance review unresolved items: {}",
                                review_unresolved
                            )),
                        },
                    );
                    *context.acceptance_repair_attempts += 1;
                    let repair_spec =
                        crate::engine::repair_spec::RepairSpec::from_failure_with_tool_records(
                            context.failed_commands,
                            context.post_edit_evidence,
                            context.repair_tool_record_evidence,
                            Some(&review),
                        );
                    let repair_spec_text = repair_spec.format_for_prompt();
                    context.tool_results_text.push('\n');
                    context.tool_results_text.push_str(&repair_spec_text);
                    context.messages.push(Message::system(repair_spec_text));
                    context.messages.push(Message::system(
                        "Acceptance review did not pass. If verification or compile errors are present, fix those first using the latest verification source context; only then address the unresolved acceptance items. Continue repair if possible; otherwise report the unresolved items clearly."
                            .to_string(),
                    ));
                    if high_risk
                        && (*context.acceptance_repair_attempts
                            > context.code_workflow.max_repair_attempts()
                            || matches!(
                                review_next_action,
                                crate::engine::workflow_contract::AcceptanceNextAction::Stop
                            ))
                    {
                        outcome.final_content = Some(format!(
                            "Stopped before final closeout because high-risk acceptance review did not pass ({} unresolved item(s)).",
                            review_unresolved
                        ));
                        outcome.break_loop = true;
                        return outcome;
                    }
                    if matches!(
                        review_next_action,
                        crate::engine::workflow_contract::AcceptanceNextAction::ContinueRepair
                    ) {
                        let validation_failed = !context.failed_commands.is_empty()
                            || !context.verify_passed
                            || !context.required_validation_passed;
                        let compile_or_review_failed =
                            !context.check_passed || !context.review_success || validation_failed;
                        let needs_acceptance_investigation =
                            review_unresolved > 0 && !compile_or_review_failed;
                        *context.reserved_repair_rounds = (*context.reserved_repair_rounds)
                            .max(if needs_acceptance_investigation { 2 } else { 1 });
                        *context.action_checkpoint_no_change_rounds = 0;
                        if needs_acceptance_investigation {
                            *context.action_checkpoint_active = false;
                            *context.action_checkpoint_lookup_count = 0;
                            *context.file_edit_failure_retry_used = false;
                            context.messages.push(Message::system(
                                "Acceptance review gaps remain after compile/code review checks. Restore investigation mode: inspect the unresolved acceptance items against the implementation, identify every acceptance-critical bypass or missing call site, then make the smallest targeted fix. If multiple independent acceptance-critical bypasses are visible, fix them together."
                                    .to_string(),
                            ));
                            context.trace.record(TraceEvent::WorkflowFallback {
                                error:
                                    "acceptance review requested broader repair; restored read/search tools for acceptance-gap investigation"
                                        .to_string(),
                            });
                        } else {
                            *context.action_checkpoint_active = true;
                            *context.action_checkpoint_lookup_count =
                                ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET;
                            *context.file_edit_failure_retry_used = false;
                            *context.action_checkpoint_requires_patch_before_validation = true;
                            context.messages.push(Message::system(
                                "Repair must patch before validation: the latest verification/acceptance evidence already shows the current diff is invalid. Use file_edit/file_write first; run bash validation only after that new patch succeeds."
                                    .to_string(),
                            ));
                            context.trace.record(TraceEvent::WorkflowFallback {
                                error:
                                    "acceptance review requested repair; switching to action-only repair mode"
                                        .to_string(),
                            });
                        }
                    }
                }
            }
            Err(err) => {
                warn!("Acceptance review failed: {}", err);
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: format!("acceptance review failed: {}", err),
                });
            }
        }

        outcome
    }

    fn required_validation_acceptance_review(
        task_bundle: &TaskContextBundle,
        judgment: Option<&ProgrammingWorkflowJudgment>,
        verify_passed: bool,
        required_validation_commands: &[String],
    ) -> Option<AcceptanceReview> {
        if !verify_passed || required_validation_commands.is_empty() {
            return None;
        }

        let evidence = format!(
            "Required validation commands passed: {}",
            required_validation_commands.join("; ")
        );
        let criteria = judgment
            .map(|judgment| {
                judgment
                    .acceptance
                    .criteria
                    .iter()
                    .map(|criterion| {
                        let mut passed = criterion.clone();
                        passed.status = AcceptanceStatus::Passed;
                        passed.evidence = Some(evidence.clone());
                        passed
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|criteria| !criteria.is_empty())
            .unwrap_or_else(|| {
                task_bundle
                    .acceptance_checks
                    .iter()
                    .map(|criterion| AcceptanceCriterion {
                        criterion: criterion.clone(),
                        status: AcceptanceStatus::Passed,
                        evidence: Some(evidence.clone()),
                    })
                    .collect()
            });

        Some(AcceptanceReview {
            accepted: true,
            confidence: AcceptanceConfidence::High,
            criteria,
            unresolved_items: Vec::new(),
            residual_risks: Vec::new(),
            next_action: AcceptanceNextAction::Finish,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{
        IntentKind, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
    };
    use crate::engine::workflow_contract::{AcceptanceContract, TaskComplexity, WorkflowPlanStep};

    fn code_change_bundle() -> TaskContextBundle {
        TaskContextBundle::new(
            "修复 memory save",
            ".",
            IntentRoute {
                intent: IntentKind::CodeChange,
                confidence: 0.9,
                workflow: WorkflowKind::CodeChange,
                retrieval: RetrievalPolicy::Project,
                reasoning: ReasoningPolicy::High,
                risk: RiskLevel::High,
                recommended_tools: Vec::new(),
                reason: "test".to_string(),
            },
            None,
        )
    }

    #[test]
    fn required_validation_acceptance_uses_bundle_checks_without_workflow_judgment() {
        let mut bundle = code_change_bundle();
        bundle.add_acceptance_check("required validation command: cargo test -q memory");
        bundle.add_acceptance_check("required validation command: cargo test -q");
        let commands = vec![
            "cargo test -q memory".to_string(),
            "cargo test -q".to_string(),
        ];

        let review =
            ConversationLoop::required_validation_acceptance_review(&bundle, None, true, &commands)
                .expect("deterministic review");

        assert!(review.accepted);
        assert_eq!(review.confidence, AcceptanceConfidence::High);
        assert_eq!(review.criteria.len(), 2);
        assert!(review
            .criteria
            .iter()
            .all(|criterion| criterion.status == AcceptanceStatus::Passed));
        assert!(review.criteria.iter().all(|criterion| criterion
            .evidence
            .as_deref()
            .is_some_and(|evidence| evidence.contains("cargo test -q"))));
        assert_eq!(review.next_action, AcceptanceNextAction::Finish);
    }

    #[test]
    fn required_validation_acceptance_prefers_judgment_criteria() {
        let mut bundle = code_change_bundle();
        bundle.add_acceptance_check("fallback check");
        let judgment = ProgrammingWorkflowJudgment {
            task_type: "bug_fix".to_string(),
            complexity: TaskComplexity::Medium,
            risk: RiskLevel::High,
            requirement_complete_enough: true,
            needs_user_questions: false,
            question_reason: None,
            questions: Vec::new(),
            assumptions: Vec::new(),
            guided_reasoning_required: false,
            guided_reasoning_triggers: Vec::new(),
            plan: Vec::<WorkflowPlanStep>::new(),
            acceptance: AcceptanceContract::pending(
                "修复 memory save",
                vec!["judged criterion".to_string()],
                Vec::new(),
            ),
        };
        let commands = vec!["cargo test -q".to_string()];

        let review = ConversationLoop::required_validation_acceptance_review(
            &bundle,
            Some(&judgment),
            true,
            &commands,
        )
        .expect("deterministic review");

        assert_eq!(review.criteria.len(), 1);
        assert_eq!(review.criteria[0].criterion, "judged criterion");
        assert_eq!(review.criteria[0].status, AcceptanceStatus::Passed);
    }

    #[test]
    fn required_validation_acceptance_requires_passing_verification() {
        let mut bundle = code_change_bundle();
        bundle.add_acceptance_check("required validation command: cargo test -q");
        let commands = vec!["cargo test -q".to_string()];

        let review = ConversationLoop::required_validation_acceptance_review(
            &bundle, None, false, &commands,
        );

        assert!(review.is_none());
    }
}

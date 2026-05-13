use super::post_edit_verification_controller::PostEditVerificationOutcome;
use super::repair_controller::{
    AcceptanceRepairContext, GuidedValidationDebuggingContext, VerificationRepairContext,
};
use super::workflow_trace::{apply_workflow_feedback_and_trace, trace_stage_validation};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::reflection_pass::ReflectionStatus;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::Message;
use std::path::PathBuf;

pub(super) struct PostEditRepairContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) changed_files: &'a [PathBuf],
    pub(super) verification: &'a PostEditVerificationOutcome,
    pub(super) required_validation_commands: &'a [String],
    pub(super) acceptance_repair_attempts: &'a mut usize,
    pub(super) reserved_repair_rounds: &'a mut usize,
    pub(super) effective_iterations: usize,
    pub(super) max_iterations: usize,
    pub(super) action_checkpoint_no_change_rounds: &'a mut usize,
    pub(super) action_checkpoint_active: &'a mut bool,
    pub(super) action_checkpoint_lookup_count: &'a mut usize,
    pub(super) file_edit_failure_retry_used: &'a mut bool,
    pub(super) action_checkpoint_requires_patch_before_validation: &'a mut bool,
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) final_content: &'a mut String,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) last_user_preview: &'a str,
}

pub(super) struct PostEditRepairOutcome {
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) break_loop: bool,
}

pub(super) struct PostEditRepairController;

impl PostEditRepairController {
    pub(super) async fn run(
        agent: &ConversationLoop,
        context: PostEditRepairContext<'_>,
    ) -> PostEditRepairOutcome {
        let verify_passed = context.verification.verify_passed;
        let mut should_closeout_after_verified_change =
            context.should_closeout_after_verified_change;
        let post_edit_reflection =
            ConversationLoop::record_verification_repair_context(VerificationRepairContext {
                trace: context.trace,
                code_workflow: &mut *context.code_workflow,
                task_id: context.task_bundle.task_id.clone(),
                changed_files: context.changed_files,
                verify_passed,
                post_edit_evidence: &context.verification.post_edit_evidence,
                failed_commands: &context.verification.failed_commands,
                acceptance_repair_attempts: *context.acceptance_repair_attempts,
                tool_results_text: &mut *context.tool_results_text,
                messages: &mut *context.messages,
            });
        context.trace.record(TraceEvent::ReflectionPassCompleted {
            pass_id: post_edit_reflection.pass_id.clone(),
            task_id: post_edit_reflection.task_id.clone(),
            status: format!("{:?}", post_edit_reflection.status),
            findings: post_edit_reflection.findings.len(),
            unresolved: post_edit_reflection.unresolved_count(),
        });
        let stage_record = context.code_workflow.record_stage_validation(
            &*context.task_bundle,
            context.changed_files,
            verify_passed,
            &context.verification.acceptance_evidence,
        );
        trace_stage_validation(context.trace, &stage_record);
        if let Some(feedback) = stage_record.feedback.clone() {
            apply_workflow_feedback_and_trace(&mut *context.task_bundle, context.trace, feedback);
        }
        if !verify_passed {
            agent
                .run_guided_validation_debugging(GuidedValidationDebuggingContext {
                    trace: context.trace,
                    last_user_preview: context.last_user_preview,
                    task_bundle: &*context.task_bundle,
                    post_edit_evidence: &context.verification.post_edit_evidence,
                    tool_results_text: &mut *context.tool_results_text,
                    messages: &mut *context.messages,
                })
                .await;
        }
        let acceptance_repair_outcome = agent
            .run_acceptance_repair_review(AcceptanceRepairContext {
                trace: context.trace,
                route: context.route,
                code_workflow: &mut *context.code_workflow,
                task_bundle: &mut *context.task_bundle,
                changed_files: context.changed_files,
                verify_passed,
                review_success: context.verification.review_success,
                required_validation_commands: context.required_validation_commands,
                failed_commands: &context.verification.failed_commands,
                post_edit_evidence: &context.verification.post_edit_evidence,
                acceptance_evidence: &context.verification.acceptance_evidence,
                required_validation_passed: context.verification.required_validation_passed,
                check_passed: context.verification.check_passed,
                acceptance_repair_attempts: &mut *context.acceptance_repair_attempts,
                reserved_repair_rounds: &mut *context.reserved_repair_rounds,
                action_checkpoint_no_change_rounds: &mut *context
                    .action_checkpoint_no_change_rounds,
                action_checkpoint_active: &mut *context.action_checkpoint_active,
                action_checkpoint_lookup_count: &mut *context.action_checkpoint_lookup_count,
                file_edit_failure_retry_used: &mut *context.file_edit_failure_retry_used,
                action_checkpoint_requires_patch_before_validation: &mut *context
                    .action_checkpoint_requires_patch_before_validation,
                should_closeout_after_verified_change,
                tool_results_text: &mut *context.tool_results_text,
                messages: &mut *context.messages,
            })
            .await;
        should_closeout_after_verified_change =
            acceptance_repair_outcome.should_closeout_after_verified_change;
        if let Some(content) = acceptance_repair_outcome.final_content {
            *context.final_content = content;
        }
        if acceptance_repair_outcome.break_loop {
            return PostEditRepairOutcome {
                should_closeout_after_verified_change,
                break_loop: true,
            };
        }
        {
            let mut tracker = agent.cost_tracker.lock().await;
            tracker.record_coding_round(verify_passed);
        }

        let reflection_action = Self::reflection_repair_action(
            post_edit_reflection.status,
            context.effective_iterations,
            context.max_iterations,
        );
        if reflection_action.requires_patch_before_validation {
            should_closeout_after_verified_change = false;
            *context.action_checkpoint_requires_patch_before_validation = true;
            let repair_instruction = format!(
                "{}\nPost-edit reflection found unresolved quality gaps. Fix the changed files before giving a final answer.",
                post_edit_reflection.format_for_prompt()
            );
            context.tool_results_text.push('\n');
            context.tool_results_text.push_str(&repair_instruction);
            context.messages.push(Message::system(repair_instruction));
            if reflection_action.reserve_repair_round {
                *context.reserved_repair_rounds = (*context.reserved_repair_rounds).max(1);
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: "reserved repair round granted after post-edit reflection failure"
                        .to_string(),
                });
            }
        }

        PostEditRepairOutcome {
            should_closeout_after_verified_change,
            break_loop: false,
        }
    }

    fn reflection_repair_action(
        status: ReflectionStatus,
        effective_iterations: usize,
        max_iterations: usize,
    ) -> PostEditReflectionRepairAction {
        let requires_patch_before_validation = status != ReflectionStatus::Passed;
        PostEditReflectionRepairAction {
            requires_patch_before_validation,
            reserve_repair_round: requires_patch_before_validation
                && effective_iterations >= max_iterations,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PostEditReflectionRepairAction {
    requires_patch_before_validation: bool,
    reserve_repair_round: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflection_repair_action_only_blocks_for_unresolved_reflection() {
        assert_eq!(
            PostEditRepairController::reflection_repair_action(ReflectionStatus::Passed, 3, 3),
            PostEditReflectionRepairAction {
                requires_patch_before_validation: false,
                reserve_repair_round: false,
            }
        );
        assert_eq!(
            PostEditRepairController::reflection_repair_action(ReflectionStatus::NeedsWork, 2, 3),
            PostEditReflectionRepairAction {
                requires_patch_before_validation: true,
                reserve_repair_round: false,
            }
        );
        assert_eq!(
            PostEditRepairController::reflection_repair_action(ReflectionStatus::NeedsWork, 3, 3),
            PostEditReflectionRepairAction {
                requires_patch_before_validation: true,
                reserve_repair_round: true,
            }
        );
    }
}

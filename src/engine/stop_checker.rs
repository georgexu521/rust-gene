//! Runtime stop-check decisions for agent control loops.
//!
//! This is intentionally small: it does not replace focused repair or failure
//! recovery. It records the runtime's answer to "should this loop continue,
//! checkpoint, or stop?" in a form task state and traces can consume.
//! Score-only concerns are advisory; hard stop/checkpoint decisions stay tied
//! to explicit failures, permissions, budgets, rollback, or verification state.

use crate::engine::task_context::{
    AgentTaskStage, AgentTaskState, RollbackCandidate, StopAction, StopCheckReason,
    StopCheckRecord, StopCheckStatus, TaskTerminalStatus, VerificationStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopCheckInput {
    pub any_tool_success: bool,
    pub successful_write_tool: bool,
    pub has_successful_validation_commands: bool,
    pub no_code_progress_rounds: usize,
    pub action_checkpoint_active: bool,
    pub action_checkpoint_no_change_rounds: usize,
    pub force_patch_synthesis_after_no_change: bool,
    pub repeated_failed_tools: usize,
    pub duplicate_read_only_tools: usize,
    pub max_iterations_reached: bool,
    pub uncertainty_not_reduced_steps: usize,
    pub consecutive_validation_failures: usize,
    pub consecutive_edit_failures: usize,
    pub consecutive_command_failures: usize,
    pub consecutive_permission_blocks: usize,
    pub consecutive_low_action_scores: usize,
    pub consecutive_high_risk_low_value_actions: usize,
    pub score_without_uncertainty_reduction_rounds: usize,
    pub repeated_revised_action_count: usize,
    pub user_interrupted: bool,
    pub model_output_invalid_attempts: usize,
    pub action_review_decision: Option<String>,
    pub action_review_reason: Option<String>,
    pub rollback_candidate: Option<RollbackCandidate>,
    pub failure_type: Option<String>,
    pub recovery_plan_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopCheckDecision {
    pub status: StopCheckStatus,
    pub terminal_status: Option<TaskTerminalStatus>,
    pub action: StopAction,
    pub reason: StopCheckReason,
    pub summary: String,
    pub evidence: Vec<String>,
    pub failure_type: Option<String>,
    pub recovery_plan_id: Option<String>,
    pub rollback_candidate: Option<RollbackCandidate>,
    pub next_action: Option<String>,
    pub no_code_progress_rounds: usize,
    pub action_checkpoint_active: bool,
}

pub struct StopChecker;

impl StopChecker {
    pub fn evaluate(input: StopCheckInput) -> StopCheckDecision {
        if input.user_interrupted {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::UserInterrupted,
                Some(TaskTerminalStatus::StoppedByUser),
                StopAction::Stop,
                "stopping because the user interrupted the turn".to_string(),
                vec!["user interrupt signal was observed".to_string()],
                Some("user_interrupted".to_string()),
                Some("preserve current state and report interruption".to_string()),
            );
        }

        if let Some(review) = input.action_review_decision.as_deref() {
            let review = review.to_ascii_lowercase();
            if matches!(
                review.as_str(),
                "ask_user" | "needs_user" | "needs_confirmation"
            ) {
                return decision(
                    &input,
                    StopCheckStatus::Stop,
                    StopCheckReason::HighRiskNeedsUser,
                    Some(TaskTerminalStatus::NeedsUser),
                    StopAction::AskUser,
                    input
                        .action_review_reason
                        .clone()
                        .unwrap_or_else(|| "high-risk action needs user decision".to_string()),
                    vec!["action review requires explicit user decision".to_string()],
                    Some("high_risk_action".to_string()),
                    Some("ask the user before continuing".to_string()),
                );
            }
            if matches!(review.as_str(), "denied" | "deny" | "blocked") {
                return decision(
                    &input,
                    StopCheckStatus::Stop,
                    StopCheckReason::ActionDenied,
                    Some(TaskTerminalStatus::Blocked),
                    StopAction::Stop,
                    input.action_review_reason.clone().unwrap_or_else(|| {
                        "action review denied the planned operation".to_string()
                    }),
                    vec!["action review denied the next operation".to_string()],
                    Some("action_denied".to_string()),
                    Some("replan with a lower-risk strategy".to_string()),
                );
            }
            if matches!(review.as_str(), "revise" | "needs_revision" | "revision") {
                return decision(
                    &input,
                    StopCheckStatus::Checkpoint,
                    StopCheckReason::ActionNeedsRevision,
                    None,
                    StopAction::Replan,
                    input.action_review_reason.clone().unwrap_or_else(|| {
                        "action review requires revising the next operation".to_string()
                    }),
                    vec!["action review requested revision".to_string()],
                    Some("action_revision".to_string()),
                    Some("revise the plan before the next tool call".to_string()),
                );
            }
        }

        if let Some(candidate) = &input.rollback_candidate {
            return StopCheckDecision {
                status: StopCheckStatus::Checkpoint,
                terminal_status: None,
                action: StopAction::RecommendRollback,
                reason: StopCheckReason::RollbackRecommended,
                summary: format!(
                    "rollback candidate available after failed checkpointed work: {}",
                    candidate.reason
                ),
                evidence: vec![format!(
                    "checkpoint={} paths={}",
                    candidate.checkpoint_id.as_deref().unwrap_or("none"),
                    if candidate.paths.is_empty() {
                        "none".to_string()
                    } else {
                        candidate.paths.join(", ")
                    }
                )],
                failure_type: Some("rollback_candidate".to_string()),
                recovery_plan_id: input.recovery_plan_id.clone(),
                rollback_candidate: Some(candidate.clone()),
                next_action: Some("recommend rewind before more edits".to_string()),
                no_code_progress_rounds: input.no_code_progress_rounds,
                action_checkpoint_active: input.action_checkpoint_active,
            };
        }

        if input.max_iterations_reached {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::BudgetExhausted,
                Some(TaskTerminalStatus::Partial),
                StopAction::Closeout,
                "iteration budget exhausted before a verified completion".to_string(),
                vec!["max iteration budget was reached".to_string()],
                Some("budget_exhausted".to_string()),
                Some("summarize completed work, verification state, and next step".to_string()),
            );
        }

        // Reasonix alignment: duplicate read-only calls should NEVER cause a
        // hard stop. The model owns its read strategy; the iteration budget +
        // force summary handle loops. Tracking the count is still useful for
        // traces, but it must not gate a Stop or Closeout.
        if input.duplicate_read_only_tools > 0 {
            return decision(
                &input,
                StopCheckStatus::Checkpoint,
                StopCheckReason::DuplicateReadOnly,
                None,
                StopAction::Continue,
                format!(
                    "{} duplicate successful read-only tool result(s) — continuing",
                    input.duplicate_read_only_tools
                ),
                vec!["duplicate read-only calls are advisory only; iteration budget handles loops"
                    .to_string()],
                Some("duplicate_read_only".to_string()),
                Some("continue with existing tool results; model decides when to stop".to_string()),
            );
        }

        if !input.any_tool_success && input.repeated_failed_tools > 0 {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::RepeatedToolFailure,
                Some(TaskTerminalStatus::Failed),
                StopAction::Recover,
                format!(
                    "stopping after {} repeated failed tool attempt(s)",
                    input.repeated_failed_tools
                ),
                vec!["same tool failure repeated with no successful tool in the round".to_string()],
                input
                    .failure_type
                    .clone()
                    .or_else(|| Some("tool_failure".to_string())),
                Some("switch strategy instead of repeating the failed tool".to_string()),
            );
        }

        if input.consecutive_permission_blocks >= 2 {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::ConsecutivePermissionBlocks,
                Some(TaskTerminalStatus::NeedsUser),
                StopAction::AskUser,
                format!(
                    "{} consecutive permission block(s) require user decision",
                    input.consecutive_permission_blocks
                ),
                vec!["permission or high-risk gate blocked repeated actions".to_string()],
                Some("permission_block".to_string()),
                Some("ask the user to approve, change policy, or choose a safer path".to_string()),
            );
        }

        if input.consecutive_validation_failures >= 3 {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::ConsecutiveValidationFailures,
                Some(TaskTerminalStatus::Failed),
                StopAction::Recover,
                format!(
                    "{} consecutive validation failure(s) without recovery",
                    input.consecutive_validation_failures
                ),
                vec!["validation failures persisted across repair attempts".to_string()],
                Some("validation_failure".to_string()),
                Some("change debugging strategy and report failing checks".to_string()),
            );
        }

        if input.consecutive_edit_failures >= 3 {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::ConsecutiveEditFailures,
                Some(TaskTerminalStatus::Blocked),
                StopAction::Recover,
                format!(
                    "{} consecutive edit failure(s) without a successful change",
                    input.consecutive_edit_failures
                ),
                vec!["edit attempts failed repeatedly".to_string()],
                Some("edit_failure".to_string()),
                Some("use a different edit method or ask for help if blocked".to_string()),
            );
        }

        if input.consecutive_command_failures >= 3 {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::ConsecutiveCommandFailures,
                Some(TaskTerminalStatus::Blocked),
                StopAction::Recover,
                format!(
                    "{} consecutive command failure(s) without progress",
                    input.consecutive_command_failures
                ),
                vec!["shell or validation commands failed repeatedly".to_string()],
                Some("command_failure".to_string()),
                Some("change command strategy before retrying".to_string()),
            );
        }

        if input.model_output_invalid_attempts >= 3 {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::ModelOutputInvalid,
                Some(TaskTerminalStatus::Failed),
                StopAction::Recover,
                format!(
                    "{} invalid model output repair attempt(s)",
                    input.model_output_invalid_attempts
                ),
                vec!["bounded model output repair budget was exhausted".to_string()],
                Some("model_output_invalid".to_string()),
                Some("fall back to a simpler prompt or report protocol failure".to_string()),
            );
        }

        if input.has_successful_validation_commands {
            return decision(
                &input,
                StopCheckStatus::Continue,
                StopCheckReason::VerificationReady,
                Some(TaskTerminalStatus::Completed),
                StopAction::Closeout,
                "validation evidence is ready for closeout".to_string(),
                vec!["successful validation command observed".to_string()],
                None,
                Some("prepare verified closeout".to_string()),
            );
        }

        if input.repeated_revised_action_count >= 2 {
            return decision(
                &input,
                StopCheckStatus::Checkpoint,
                StopCheckReason::RepeatedActionRevision,
                None,
                StopAction::Replan,
                format!(
                    "{} repeated action revision(s) need a different candidate action",
                    input.repeated_revised_action_count
                ),
                vec!["runtime action review revised repeated proposed actions".to_string()],
                Some("repeated_action_revision".to_string()),
                Some("propose a lower-risk candidate action before more tools".to_string()),
            );
        }

        if input.consecutive_high_risk_low_value_actions >= 2 {
            return decision(
                &input,
                StopCheckStatus::Continue,
                StopCheckReason::LowActionValueLoop,
                None,
                StopAction::Continue,
                format!(
                    "{} consecutive high-risk low-value action(s) recorded as advisory evidence",
                    input.consecutive_high_risk_low_value_actions
                ),
                vec!["action score history shows high risk without enough value".to_string()],
                Some("low_action_value_advisory".to_string()),
                Some(
                    "let the model decide whether to replan, ask the user, or continue".to_string(),
                ),
            );
        }

        if input.score_without_uncertainty_reduction_rounds >= 3 {
            return decision(
                &input,
                StopCheckStatus::Continue,
                StopCheckReason::ScoreNotReducingUncertainty,
                None,
                StopAction::Continue,
                format!(
                    "{} low-score action(s) did not reduce uncertainty; recording advisory evidence",
                    input.score_without_uncertainty_reduction_rounds
                ),
                vec![
                    "action score history shows continued work is not paying down uncertainty"
                        .to_string(),
                ],
                Some("score_not_reducing_uncertainty_advisory".to_string()),
                Some("let the model decide whether a sharper hypothesis is needed".to_string()),
            );
        }

        if input.consecutive_low_action_scores >= 3 {
            return decision(
                &input,
                StopCheckStatus::Continue,
                StopCheckReason::LowActionValueLoop,
                None,
                StopAction::Continue,
                format!(
                    "{} consecutive low action score(s) recorded as advisory evidence",
                    input.consecutive_low_action_scores
                ),
                vec!["action score history stayed below the useful-action threshold".to_string()],
                Some("low_action_value_advisory".to_string()),
                Some(
                    "let the model decide whether to choose a higher-scope, lower-cost action"
                        .to_string(),
                ),
            );
        }

        if input.uncertainty_not_reduced_steps >= 3 {
            return decision(
                &input,
                StopCheckStatus::Stop,
                StopCheckReason::UncertaintyNotReduced,
                Some(TaskTerminalStatus::Blocked),
                StopAction::Replan,
                format!(
                    "{} observation step(s) did not reduce uncertainty",
                    input.uncertainty_not_reduced_steps
                ),
                vec![
                    "observer state did not gain a finding, focus, or progress signal".to_string(),
                ],
                Some("uncertainty_not_reduced".to_string()),
                Some("replan around a sharper hypothesis before more tools".to_string()),
            );
        }

        if input.force_patch_synthesis_after_no_change
            || input.action_checkpoint_no_change_rounds >= 3
        {
            return decision(
                &input,
                StopCheckStatus::Checkpoint,
                StopCheckReason::FocusedRepairStalled,
                None,
                StopAction::Recover,
                format!(
                    "focused repair stalled after {} checkpoint no-change round(s)",
                    input.action_checkpoint_no_change_rounds
                ),
                vec!["focused repair checkpoint did not produce changes".to_string()],
                Some("focused_repair_stalled".to_string()),
                Some("synthesize a patch from accumulated evidence".to_string()),
            );
        }

        if input.no_code_progress_rounds >= 2 && !input.successful_write_tool {
            return decision(
                &input,
                StopCheckStatus::Checkpoint,
                StopCheckReason::NoProgress,
                None,
                StopAction::Replan,
                format!(
                    "{} successful tool round(s) produced no code progress",
                    input.no_code_progress_rounds
                ),
                vec!["successful tools did not result in code progress".to_string()],
                Some("no_code_progress".to_string()),
                Some("change strategy before more read-only inspection".to_string()),
            );
        }

        decision(
            &input,
            StopCheckStatus::Continue,
            StopCheckReason::NoIssue,
            None,
            StopAction::Continue,
            "no stop condition detected".to_string(),
            Vec::new(),
            None,
            None,
        )
    }

    pub fn apply_to_task_state(task_state: &mut AgentTaskState, decision: &StopCheckDecision) {
        task_state.record_stop_check(StopCheckRecord {
            status: decision.status,
            terminal_status: decision.terminal_status,
            action: decision.action,
            reason: decision.reason,
            summary: decision.summary.clone(),
            evidence: decision.evidence.clone(),
            failure_type: decision.failure_type.clone(),
            recovery_plan_id: decision.recovery_plan_id.clone(),
            rollback_candidate: decision.rollback_candidate.clone(),
            next_action: decision.next_action.clone(),
            no_code_progress_rounds: decision.no_code_progress_rounds,
            action_checkpoint_active: decision.action_checkpoint_active,
        });

        match decision.reason {
            StopCheckReason::VerificationReady => {
                task_state.transition_to_stage(
                    AgentTaskStage::Closeout,
                    "stop_checker",
                    "verification evidence is ready for closeout",
                    decision.evidence.len(),
                );
            }
            StopCheckReason::NoProgress | StopCheckReason::FocusedRepairStalled => {
                task_state.transition_to_stage(
                    AgentTaskStage::Repair,
                    "stop_checker",
                    decision.reason.label(),
                    decision.evidence.len(),
                );
                task_state.record_observation("stop_checker", decision.summary.clone());
            }
            StopCheckReason::RepeatedToolFailure
            | StopCheckReason::ConsecutiveValidationFailures
            | StopCheckReason::ConsecutiveEditFailures
            | StopCheckReason::ConsecutiveCommandFailures
            | StopCheckReason::ModelOutputInvalid => {
                task_state.transition_to_stage(
                    AgentTaskStage::Repair,
                    "stop_checker",
                    decision.reason.label(),
                    decision.evidence.len(),
                );
                task_state.verification_plan.status = VerificationStatus::Blocked;
                task_state.record_observation("stop_checker", decision.summary.clone());
            }
            StopCheckReason::ConsecutivePermissionBlocks
            | StopCheckReason::HighRiskNeedsUser
            | StopCheckReason::ActionDenied
            | StopCheckReason::BudgetExhausted
            | StopCheckReason::UserInterrupted => {
                task_state.record_observation("stop_checker", decision.summary.clone());
            }
            StopCheckReason::UncertaintyNotReduced
            | StopCheckReason::ActionNeedsRevision
            | StopCheckReason::RollbackRecommended
            | StopCheckReason::RepeatedActionRevision => {
                task_state.transition_to_stage(
                    AgentTaskStage::Repair,
                    "stop_checker",
                    decision.reason.label(),
                    decision.evidence.len(),
                );
                task_state.record_observation("stop_checker", decision.summary.clone());
            }
            StopCheckReason::LowActionValueLoop | StopCheckReason::ScoreNotReducingUncertainty => {
                if decision.status == StopCheckStatus::Continue {
                    task_state.record_observation("stop_checker", decision.summary.clone());
                } else {
                    task_state.transition_to_stage(
                        AgentTaskStage::Repair,
                        "stop_checker",
                        decision.reason.label(),
                        decision.evidence.len(),
                    );
                    task_state.record_observation("stop_checker", decision.summary.clone());
                }
            }
            StopCheckReason::DuplicateReadOnly | StopCheckReason::NoIssue => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn decision(
    input: &StopCheckInput,
    status: StopCheckStatus,
    reason: StopCheckReason,
    terminal_status: Option<TaskTerminalStatus>,
    action: StopAction,
    summary: String,
    evidence: Vec<String>,
    failure_type: Option<String>,
    next_action: Option<String>,
) -> StopCheckDecision {
    StopCheckDecision {
        status,
        terminal_status,
        action,
        reason,
        summary,
        evidence,
        failure_type: failure_type.or_else(|| input.failure_type.clone()),
        recovery_plan_id: input.recovery_plan_id.clone(),
        rollback_candidate: input.rollback_candidate.clone(),
        next_action,
        no_code_progress_rounds: if reason == StopCheckReason::VerificationReady {
            0
        } else {
            input.no_code_progress_rounds
        },
        action_checkpoint_active: input.action_checkpoint_active,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;

    fn input() -> StopCheckInput {
        StopCheckInput {
            any_tool_success: true,
            successful_write_tool: false,
            has_successful_validation_commands: false,
            no_code_progress_rounds: 0,
            action_checkpoint_active: false,
            action_checkpoint_no_change_rounds: 0,
            force_patch_synthesis_after_no_change: false,
            repeated_failed_tools: 0,
            duplicate_read_only_tools: 0,
            max_iterations_reached: false,
            uncertainty_not_reduced_steps: 0,
            consecutive_validation_failures: 0,
            consecutive_edit_failures: 0,
            consecutive_command_failures: 0,
            consecutive_permission_blocks: 0,
            consecutive_low_action_scores: 0,
            consecutive_high_risk_low_value_actions: 0,
            score_without_uncertainty_reduction_rounds: 0,
            repeated_revised_action_count: 0,
            user_interrupted: false,
            model_output_invalid_attempts: 0,
            action_review_decision: None,
            action_review_reason: None,
            rollback_candidate: None,
            failure_type: None,
            recovery_plan_id: None,
        }
    }

    #[test]
    fn duplicate_read_only_result_never_stops_anymore() {
        // Reasonix alignment: duplicate reads are advisory, never a hard stop.
        let decision = StopChecker::evaluate(StopCheckInput {
            duplicate_read_only_tools: 1,
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Checkpoint);
        assert_eq!(decision.reason, StopCheckReason::DuplicateReadOnly);
        assert_eq!(decision.action, StopAction::Continue);
    }

    #[test]
    fn repeated_failed_tools_block_pending_verification_in_task_state() {
        let route = IntentRouter::new().route("fix src/main.rs");
        let mut bundle = TaskContextBundle::new("fix src/main.rs", ".", route, None);
        let decision = StopChecker::evaluate(StopCheckInput {
            any_tool_success: false,
            repeated_failed_tools: 1,
            ..input()
        });

        StopChecker::apply_to_task_state(&mut bundle.agent_state, &decision);

        assert_eq!(decision.status, StopCheckStatus::Stop);
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
        assert_eq!(
            bundle.agent_state.verification_plan.status,
            VerificationStatus::Blocked
        );
        assert_eq!(bundle.agent_state.stop_checks.len(), 1);
        assert_eq!(
            bundle.agent_state.terminal_status,
            Some(TaskTerminalStatus::Failed)
        );
    }

    #[test]
    fn no_progress_checkpoint_moves_task_state_to_repair() {
        let route = IntentRouter::new().route("fix src/main.rs");
        let mut bundle = TaskContextBundle::new("fix src/main.rs", ".", route, None);
        let decision = StopChecker::evaluate(StopCheckInput {
            no_code_progress_rounds: 2,
            ..input()
        });

        StopChecker::apply_to_task_state(&mut bundle.agent_state, &decision);

        assert_eq!(decision.status, StopCheckStatus::Checkpoint);
        assert_eq!(decision.reason, StopCheckReason::NoProgress);
        assert_eq!(bundle.agent_state.stage, AgentTaskStage::Repair);
        assert!(bundle
            .agent_state
            .format_for_context_zone()
            .contains("Stop check: checkpoint"));
    }

    #[test]
    fn validation_ready_declares_completed_closeout_action() {
        let decision = StopChecker::evaluate(StopCheckInput {
            has_successful_validation_commands: true,
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Continue);
        assert_eq!(decision.reason, StopCheckReason::VerificationReady);
        assert_eq!(
            decision.terminal_status,
            Some(TaskTerminalStatus::Completed)
        );
        assert_eq!(decision.action, StopAction::Closeout);
    }

    #[test]
    fn budget_exhaustion_declares_partial_closeout() {
        let decision = StopChecker::evaluate(StopCheckInput {
            max_iterations_reached: true,
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Stop);
        assert_eq!(decision.reason, StopCheckReason::BudgetExhausted);
        assert_eq!(decision.terminal_status, Some(TaskTerminalStatus::Partial));
        assert_eq!(decision.action, StopAction::Closeout);
    }

    #[test]
    fn consecutive_validation_failures_stop_as_failed_recovery() {
        let decision = StopChecker::evaluate(StopCheckInput {
            consecutive_validation_failures: 3,
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Stop);
        assert_eq!(
            decision.reason,
            StopCheckReason::ConsecutiveValidationFailures
        );
        assert_eq!(decision.terminal_status, Some(TaskTerminalStatus::Failed));
        assert_eq!(decision.failure_type.as_deref(), Some("validation_failure"));
        assert_eq!(decision.action, StopAction::Recover);
    }

    #[test]
    fn rollback_candidate_recommends_rewind_without_terminal_status() {
        let candidate = RollbackCandidate {
            checkpoint_id: Some("cp1".to_string()),
            file_change_id: None,
            tool_round_id: Some("tool1".to_string()),
            paths: vec!["src/main.rs".to_string()],
            reason: "failed checkpointed edit".to_string(),
            confidence: 80,
            auto_allowed: false,
        };
        let decision = StopChecker::evaluate(StopCheckInput {
            rollback_candidate: Some(candidate.clone()),
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Checkpoint);
        assert_eq!(decision.reason, StopCheckReason::RollbackRecommended);
        assert_eq!(decision.terminal_status, None);
        assert_eq!(decision.action, StopAction::RecommendRollback);
        assert_eq!(decision.rollback_candidate, Some(candidate));
    }

    #[test]
    fn repeated_low_action_scores_are_advisory_not_replan() {
        let decision = StopChecker::evaluate(StopCheckInput {
            consecutive_low_action_scores: 3,
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Continue);
        assert_eq!(decision.reason, StopCheckReason::LowActionValueLoop);
        assert_eq!(decision.action, StopAction::Continue);
    }

    #[test]
    fn low_scores_without_uncertainty_are_advisory_not_blocking() {
        let decision = StopChecker::evaluate(StopCheckInput {
            score_without_uncertainty_reduction_rounds: 3,
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Continue);
        assert_eq!(
            decision.reason,
            StopCheckReason::ScoreNotReducingUncertainty
        );
        assert_eq!(decision.terminal_status, None);
        assert_eq!(decision.action, StopAction::Continue);
    }
}

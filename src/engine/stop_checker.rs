//! Runtime stop-check decisions for agent control loops.
//!
//! This is intentionally small: it does not replace focused repair or failure
//! recovery. It records the runtime's answer to "should this loop continue,
//! checkpoint, or stop?" in a form task state and traces can consume.

use crate::engine::task_context::{
    AgentTaskStage, AgentTaskState, StopCheckReason, StopCheckRecord, StopCheckStatus,
    VerificationStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopCheckDecision {
    pub status: StopCheckStatus,
    pub reason: StopCheckReason,
    pub summary: String,
    pub no_code_progress_rounds: usize,
    pub action_checkpoint_active: bool,
}

pub struct StopChecker;

impl StopChecker {
    pub fn evaluate(input: StopCheckInput) -> StopCheckDecision {
        if input.duplicate_read_only_tools > 0 {
            return StopCheckDecision {
                status: StopCheckStatus::Stop,
                reason: StopCheckReason::DuplicateReadOnly,
                summary: format!(
                    "stopping after {} duplicate successful read-only tool result(s)",
                    input.duplicate_read_only_tools
                ),
                no_code_progress_rounds: input.no_code_progress_rounds,
                action_checkpoint_active: input.action_checkpoint_active,
            };
        }

        if !input.any_tool_success && input.repeated_failed_tools > 0 {
            return StopCheckDecision {
                status: StopCheckStatus::Stop,
                reason: StopCheckReason::RepeatedToolFailure,
                summary: format!(
                    "stopping after {} repeated failed tool attempt(s)",
                    input.repeated_failed_tools
                ),
                no_code_progress_rounds: input.no_code_progress_rounds,
                action_checkpoint_active: input.action_checkpoint_active,
            };
        }

        if input.has_successful_validation_commands {
            return StopCheckDecision {
                status: StopCheckStatus::Continue,
                reason: StopCheckReason::VerificationReady,
                summary: "validation evidence is ready for closeout".to_string(),
                no_code_progress_rounds: 0,
                action_checkpoint_active: input.action_checkpoint_active,
            };
        }

        if input.force_patch_synthesis_after_no_change
            || input.action_checkpoint_no_change_rounds >= 3
        {
            return StopCheckDecision {
                status: StopCheckStatus::Checkpoint,
                reason: StopCheckReason::FocusedRepairStalled,
                summary: format!(
                    "focused repair stalled after {} checkpoint no-change round(s)",
                    input.action_checkpoint_no_change_rounds
                ),
                no_code_progress_rounds: input.no_code_progress_rounds,
                action_checkpoint_active: input.action_checkpoint_active,
            };
        }

        if input.no_code_progress_rounds >= 2 && !input.successful_write_tool {
            return StopCheckDecision {
                status: StopCheckStatus::Checkpoint,
                reason: StopCheckReason::NoProgress,
                summary: format!(
                    "{} successful tool round(s) produced no code progress",
                    input.no_code_progress_rounds
                ),
                no_code_progress_rounds: input.no_code_progress_rounds,
                action_checkpoint_active: input.action_checkpoint_active,
            };
        }

        StopCheckDecision {
            status: StopCheckStatus::Continue,
            reason: StopCheckReason::NoIssue,
            summary: "no stop condition detected".to_string(),
            no_code_progress_rounds: input.no_code_progress_rounds,
            action_checkpoint_active: input.action_checkpoint_active,
        }
    }

    pub fn apply_to_task_state(task_state: &mut AgentTaskState, decision: &StopCheckDecision) {
        task_state.record_stop_check(StopCheckRecord {
            status: decision.status,
            reason: decision.reason,
            summary: decision.summary.clone(),
            no_code_progress_rounds: decision.no_code_progress_rounds,
            action_checkpoint_active: decision.action_checkpoint_active,
        });

        match decision.reason {
            StopCheckReason::VerificationReady => {
                task_state.stage = AgentTaskStage::Closeout;
            }
            StopCheckReason::NoProgress | StopCheckReason::FocusedRepairStalled => {
                task_state.stage = AgentTaskStage::Repair;
                task_state.record_observation("stop_checker", decision.summary.clone());
            }
            StopCheckReason::RepeatedToolFailure => {
                task_state.stage = AgentTaskStage::Repair;
                task_state.verification_plan.status = VerificationStatus::Blocked;
                task_state.record_observation("stop_checker", decision.summary.clone());
            }
            StopCheckReason::DuplicateReadOnly | StopCheckReason::NoIssue => {}
        }
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
        }
    }

    #[test]
    fn duplicate_read_only_result_stops_before_more_tools() {
        let decision = StopChecker::evaluate(StopCheckInput {
            duplicate_read_only_tools: 1,
            ..input()
        });

        assert_eq!(decision.status, StopCheckStatus::Stop);
        assert_eq!(decision.reason, StopCheckReason::DuplicateReadOnly);
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
            .contains("Stop check: Checkpoint"));
    }
}

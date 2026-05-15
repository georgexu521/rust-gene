use super::action_checkpoint::{
    ProgressCheckpointAction, ProgressCheckpointController, ProgressCheckpointRequest,
};
use super::turn_runtime_state::FocusedRepairRuntimeState;
use super::ConversationLoop;

pub(super) struct FocusedRepairStateContext<'a> {
    pub(super) state: &'a mut FocusedRepairRuntimeState,
    pub(super) is_programming_workflow: bool,
    pub(super) no_diff_audit_closeout_allowed: bool,
    pub(super) has_worktree_changes: bool,
    pub(super) has_successful_validation_commands: bool,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) used_action_checkpoint_lookup: bool,
    pub(super) successful_write_tool: bool,
    pub(super) used_write_tool: bool,
    pub(super) any_tool_success: bool,
    pub(super) file_edit_failure_correction_added: bool,
}

pub(super) struct FocusedRepairStateOutcome {
    pub(super) retry_after_file_edit_failure_correction: bool,
    pub(super) progress_checkpoint_action: ProgressCheckpointAction,
    pub(super) force_patch_synthesis_after_no_change: bool,
    pub(super) force_patch_synthesis_reason: Option<&'static str>,
}

pub(super) struct FocusedRepairStateController;

impl FocusedRepairStateController {
    pub(super) fn record_tool_round(
        context: FocusedRepairStateContext<'_>,
    ) -> FocusedRepairStateOutcome {
        let state = context.state;
        if ConversationLoop::should_retry_after_file_edit_failure_correction(
            state.action_checkpoint_active,
            context.file_edit_failure_correction_added,
            state.file_edit_failure_retry_used,
            context.successful_write_tool,
        ) {
            state.file_edit_failure_retry_used = true;
            state.action_checkpoint_no_change_rounds = 0;
            return FocusedRepairStateOutcome {
                retry_after_file_edit_failure_correction: true,
                progress_checkpoint_action: ProgressCheckpointAction::None,
                force_patch_synthesis_after_no_change: false,
                force_patch_synthesis_reason: None,
            };
        }

        if !context.is_programming_workflow {
            return Self::no_action();
        }

        if context.successful_write_tool {
            state.no_code_progress_rounds = 0;
            state.action_checkpoint_no_change_rounds = 0;
            state.action_checkpoint_active = false;
            state.action_checkpoint_lookup_count = 0;
            state.file_edit_failure_retry_used = false;
            return Self::no_action();
        }

        if context.used_write_tool {
            state.action_checkpoint_requires_patch_before_validation = true;
            return Self::no_action();
        }

        if !context.any_tool_success {
            return Self::no_action();
        }

        let decision =
            ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
                no_diff_audit_closeout_allowed: context.no_diff_audit_closeout_allowed,
                has_worktree_changes: context.has_worktree_changes,
                has_successful_validation_commands: context.has_successful_validation_commands,
                no_code_progress_rounds: state.no_code_progress_rounds,
                action_checkpoint_active: state.action_checkpoint_active,
                action_checkpoint_lookup_count: state.action_checkpoint_lookup_count,
                action_checkpoint_no_change_rounds: state.action_checkpoint_no_change_rounds,
                no_diff_audit_validation_checkpoint_sent: state
                    .no_diff_audit_validation_checkpoint_sent,
                code_write_tools_forbidden: context.code_write_tools_forbidden,
                code_write_forbidden_checkpoint_sent: state.code_write_forbidden_checkpoint_sent,
                used_action_checkpoint_lookup: context.used_action_checkpoint_lookup,
            });

        state.no_code_progress_rounds = decision.no_code_progress_rounds;
        state.action_checkpoint_active = decision.action_checkpoint_active;
        state.action_checkpoint_lookup_count = decision.action_checkpoint_lookup_count;
        state.action_checkpoint_no_change_rounds = decision.action_checkpoint_no_change_rounds;
        state.no_diff_audit_validation_checkpoint_sent =
            decision.no_diff_audit_validation_checkpoint_sent;
        state.code_write_forbidden_checkpoint_sent = decision.code_write_forbidden_checkpoint_sent;
        if decision.reset_file_edit_failure_retry {
            state.file_edit_failure_retry_used = false;
        }

        FocusedRepairStateOutcome {
            retry_after_file_edit_failure_correction: false,
            progress_checkpoint_action: decision.action,
            force_patch_synthesis_after_no_change: decision.force_patch_synthesis_after_no_change,
            force_patch_synthesis_reason: decision.force_patch_synthesis_reason,
        }
    }

    fn no_action() -> FocusedRepairStateOutcome {
        FocusedRepairStateOutcome {
            retry_after_file_edit_failure_correction: false,
            progress_checkpoint_action: ProgressCheckpointAction::None,
            force_patch_synthesis_after_no_change: false,
            force_patch_synthesis_reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(state: &mut FocusedRepairRuntimeState) -> FocusedRepairStateContext<'_> {
        FocusedRepairStateContext {
            state,
            is_programming_workflow: true,
            no_diff_audit_closeout_allowed: false,
            has_worktree_changes: false,
            has_successful_validation_commands: false,
            code_write_tools_forbidden: false,
            used_action_checkpoint_lookup: false,
            successful_write_tool: false,
            used_write_tool: false,
            any_tool_success: false,
            file_edit_failure_correction_added: false,
        }
    }

    #[test]
    fn file_edit_failure_correction_gets_one_retry_before_patch_synthesis() {
        let mut state = FocusedRepairRuntimeState {
            action_checkpoint_active: true,
            action_checkpoint_no_change_rounds: 2,
            ..FocusedRepairRuntimeState::default()
        };
        let outcome = FocusedRepairStateController::record_tool_round(FocusedRepairStateContext {
            file_edit_failure_correction_added: true,
            ..request(&mut state)
        });

        assert!(outcome.retry_after_file_edit_failure_correction);
        assert!(state.file_edit_failure_retry_used);
        assert_eq!(state.action_checkpoint_no_change_rounds, 0);
        assert_eq!(
            outcome.progress_checkpoint_action,
            ProgressCheckpointAction::None
        );
    }

    #[test]
    fn successful_write_resets_focused_repair_progress() {
        let mut state = FocusedRepairRuntimeState {
            no_code_progress_rounds: 3,
            action_checkpoint_active: true,
            action_checkpoint_lookup_count: 2,
            action_checkpoint_no_change_rounds: 2,
            file_edit_failure_retry_used: true,
            ..FocusedRepairRuntimeState::default()
        };

        let outcome = FocusedRepairStateController::record_tool_round(FocusedRepairStateContext {
            successful_write_tool: true,
            any_tool_success: true,
            ..request(&mut state)
        });

        assert!(!outcome.retry_after_file_edit_failure_correction);
        assert_eq!(state.no_code_progress_rounds, 0);
        assert!(!state.action_checkpoint_active);
        assert_eq!(state.action_checkpoint_lookup_count, 0);
        assert_eq!(state.action_checkpoint_no_change_rounds, 0);
        assert!(!state.file_edit_failure_retry_used);
    }

    #[test]
    fn failed_write_requires_patch_before_validation() {
        let mut state = FocusedRepairRuntimeState::default();

        FocusedRepairStateController::record_tool_round(FocusedRepairStateContext {
            used_write_tool: true,
            ..request(&mut state)
        });

        assert!(state.action_checkpoint_requires_patch_before_validation);
    }

    #[test]
    fn read_only_success_delegates_progress_checkpoint_decision() {
        let mut state = FocusedRepairRuntimeState {
            no_code_progress_rounds: 1,
            ..FocusedRepairRuntimeState::default()
        };

        let outcome = FocusedRepairStateController::record_tool_round(FocusedRepairStateContext {
            any_tool_success: true,
            ..request(&mut state)
        });

        assert_eq!(state.no_code_progress_rounds, 2);
        assert_eq!(
            outcome.progress_checkpoint_action,
            ProgressCheckpointAction::ProgressReminder {
                no_code_progress_rounds: 2
            }
        );
    }

    #[test]
    fn non_programming_round_does_not_change_progress_state() {
        let mut state = FocusedRepairRuntimeState {
            no_code_progress_rounds: 1,
            ..FocusedRepairRuntimeState::default()
        };

        FocusedRepairStateController::record_tool_round(FocusedRepairStateContext {
            is_programming_workflow: false,
            successful_write_tool: true,
            any_tool_success: true,
            ..request(&mut state)
        });

        assert_eq!(state.no_code_progress_rounds, 1);
    }
}

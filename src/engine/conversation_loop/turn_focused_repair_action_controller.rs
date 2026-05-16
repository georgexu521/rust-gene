use super::action_checkpoint::{FocusedRepairActionProposal, FocusedRepairActionRequest};
use super::focused_repair_state_controller::FocusedRepairStateOutcome;
use super::patch_synthesis_flow_controller::{
    PatchSynthesisFlowController, PatchSynthesisProposalContext, PatchSynthesisProposalFlow,
};
use super::turn_runtime_state::FocusedRepairRuntimeState;
use super::turn_tool_round_outcome_controller::TurnToolRoundState;
use super::ConversationLoop;
use crate::engine::trace::TraceCollector;
use crate::services::api::Message;
use std::collections::HashSet;

pub(super) struct TurnFocusedRepairActionContext<'a> {
    pub(super) focused_repair_state: &'a FocusedRepairStateOutcome,
    pub(super) runtime_state: &'a mut FocusedRepairRuntimeState,
    pub(super) round_state: &'a mut TurnToolRoundState,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) enum TurnFocusedRepairActionFlow {
    NoAction,
    Continue,
    EnterPatchSynthesis {
        proposal: FocusedRepairActionProposal,
    },
}

pub(super) struct TurnFocusedRepairActionController;

impl TurnFocusedRepairActionController {
    pub(super) fn run(context: TurnFocusedRepairActionContext<'_>) -> TurnFocusedRepairActionFlow {
        let Some(proposal) =
            ConversationLoop::focused_repair_action_proposal(FocusedRepairActionRequest {
                action_checkpoint_active: context.runtime_state.action_checkpoint_active,
                any_tool_success: context.round_state.any_tool_success,
                batch_has_unsuccessful_tools: context.round_state.batch_has_unsuccessful_tools,
                failed_tool_evidence_present: context.round_state.failed_tool_evidence_present(),
                force_patch_synthesis_after_no_change: context
                    .focused_repair_state
                    .force_patch_synthesis_after_no_change,
                force_patch_synthesis_reason: context
                    .focused_repair_state
                    .force_patch_synthesis_reason,
                action_checkpoint_no_change_rounds: context
                    .runtime_state
                    .action_checkpoint_no_change_rounds,
                action_checkpoint_lookup_count: context
                    .runtime_state
                    .action_checkpoint_lookup_count,
                exposed_tool_names: context.exposed_tool_names,
            })
        else {
            return TurnFocusedRepairActionFlow::NoAction;
        };

        match PatchSynthesisFlowController::apply_repair_proposal(PatchSynthesisProposalContext {
            proposal: &proposal,
            state: context.runtime_state,
            trace: context.trace,
            messages: context.messages,
            tool_results_text: &mut context.round_state.tool_results_text,
        }) {
            PatchSynthesisProposalFlow::Continue => TurnFocusedRepairActionFlow::Continue,
            PatchSynthesisProposalFlow::EnterPatchSynthesis => {
                TurnFocusedRepairActionFlow::EnterPatchSynthesis { proposal }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::action_checkpoint::ProgressCheckpointAction;
    use super::super::turn_tool_round_outcome_controller::TurnToolRoundState;
    use super::*;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
    use std::path::PathBuf;

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session-test", 1, "focused repair"))
    }

    fn focused_repair_state(force_patch: bool) -> FocusedRepairStateOutcome {
        FocusedRepairStateOutcome {
            retry_after_file_edit_failure_correction: false,
            progress_checkpoint_action: ProgressCheckpointAction::None,
            force_patch_synthesis_after_no_change: force_patch,
            force_patch_synthesis_reason: force_patch.then_some("test forced patch synthesis"),
        }
    }

    fn round_state() -> TurnToolRoundState {
        TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::<PathBuf>::new(),
            batch_has_unsuccessful_tools: true,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: false,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: vec!["bash".to_string()],
            failed_tool_evidence: vec!["bash failed".to_string()],
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            should_closeout_after_verified_change: false,
        }
    }

    #[test]
    fn run_returns_no_action_without_action_checkpoint() {
        let trace = trace();
        let mut runtime_state = FocusedRepairRuntimeState::default();
        let mut round_state = round_state();
        let mut messages = vec![Message::user("fix it")];
        let exposed_tool_names = HashSet::from(["file_edit".to_string()]);

        let flow = TurnFocusedRepairActionController::run(TurnFocusedRepairActionContext {
            focused_repair_state: &focused_repair_state(false),
            runtime_state: &mut runtime_state,
            round_state: &mut round_state,
            exposed_tool_names: &exposed_tool_names,
            trace: &trace,
            messages: &mut messages,
        });

        assert!(matches!(flow, TurnFocusedRepairActionFlow::NoAction));
        assert_eq!(messages.len(), 1);
        assert_eq!(runtime_state.action_checkpoint_no_change_rounds, 0);
    }

    #[test]
    fn run_appends_focused_repair_prompt_before_patch_synthesis_threshold() {
        let trace = trace();
        let mut runtime_state = FocusedRepairRuntimeState {
            action_checkpoint_active: true,
            ..FocusedRepairRuntimeState::default()
        };
        let mut round_state = round_state();
        let mut messages = vec![Message::user("fix it")];
        let exposed_tool_names = HashSet::from(["file_edit".to_string()]);

        let flow = TurnFocusedRepairActionController::run(TurnFocusedRepairActionContext {
            focused_repair_state: &focused_repair_state(false),
            runtime_state: &mut runtime_state,
            round_state: &mut round_state,
            exposed_tool_names: &exposed_tool_names,
            trace: &trace,
            messages: &mut messages,
        });

        assert!(matches!(flow, TurnFocusedRepairActionFlow::Continue));
        assert_eq!(runtime_state.action_checkpoint_no_change_rounds, 1);
        assert!(round_state
            .tool_results_text
            .contains("Focused repair correction"));
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::System { content } if content.contains("Focused repair correction")
        )));
    }

    #[test]
    fn run_enters_patch_synthesis_at_no_change_threshold() {
        let trace = trace();
        let mut runtime_state = FocusedRepairRuntimeState {
            action_checkpoint_active: true,
            action_checkpoint_no_change_rounds: 1,
            ..FocusedRepairRuntimeState::default()
        };
        let mut round_state = round_state();
        let mut messages = vec![Message::user("fix it")];
        let exposed_tool_names = HashSet::from(["file_edit".to_string()]);

        let flow = TurnFocusedRepairActionController::run(TurnFocusedRepairActionContext {
            focused_repair_state: &focused_repair_state(true),
            runtime_state: &mut runtime_state,
            round_state: &mut round_state,
            exposed_tool_names: &exposed_tool_names,
            trace: &trace,
            messages: &mut messages,
        });

        let TurnFocusedRepairActionFlow::EnterPatchSynthesis { proposal } = flow else {
            panic!("expected patch synthesis");
        };
        assert!(proposal.enter_patch_synthesis);
        assert_eq!(runtime_state.action_checkpoint_no_change_rounds, 2);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error.contains("action checkpoint entered patch synthesis")
        )));
    }
}

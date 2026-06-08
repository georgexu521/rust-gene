use super::focused_repair_state_controller::{
    FocusedRepairRoundApplicationContext, FocusedRepairStateContext, FocusedRepairStateController,
};
use super::turn_state::TurnRuntimeState;
use super::turn_tool_round_step_controller::TurnToolRoundState;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::WorkflowKind;
use crate::engine::trace::TraceCollector;
use crate::services::api::Message;

pub(super) struct TurnFocusedRepairFlowContext<'a> {
    pub(super) workflow: WorkflowKind,
    pub(super) no_diff_audit_closeout_allowed: bool,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) trace: &'a TraceCollector,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) round_state: &'a mut TurnToolRoundState,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) enum TurnFocusedRepairFlow {
    Continue,
    Proceed,
}

pub(super) struct TurnFocusedRepairFlowController;

impl TurnFocusedRepairFlowController {
    pub(super) async fn run(context: TurnFocusedRepairFlowContext<'_>) -> TurnFocusedRepairFlow {
        let is_programming_workflow =
            crate::engine::code_change_workflow::is_programming_workflow(context.workflow);
        let focused_repair_state =
            FocusedRepairStateController::apply_tool_round(FocusedRepairRoundApplicationContext {
                state_context: FocusedRepairStateContext {
                    state: &mut context.turn_state.focused_repair,
                    is_programming_workflow,
                    no_diff_audit_closeout_allowed: context.no_diff_audit_closeout_allowed,
                    has_worktree_changes: context.round_state.has_worktree_changes(),
                    has_successful_validation_commands: context
                        .round_state
                        .has_successful_validation_commands(),
                    code_write_tools_forbidden: context.code_write_tools_forbidden,
                    used_action_checkpoint_lookup: context
                        .round_state
                        .used_action_checkpoint_lookup,
                    successful_write_tool: context.round_state.successful_write_tool,
                    used_write_tool: context.round_state.used_write_tool,
                    any_tool_success: context.round_state.any_tool_success,
                    file_edit_failure_correction_added: context
                        .round_state
                        .file_edit_failure_correction_added,
                },
                workflow: context.workflow,
                trace: context.trace,
                code_workflow: context.code_workflow,
                messages: &mut *context.messages,
                tool_results_text: &mut context.round_state.tool_results_text,
            });

        if focused_repair_state.retry_after_file_edit_failure_correction {
            return TurnFocusedRepairFlow::Continue;
        }

        TurnFocusedRepairFlow::Proceed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::WorkflowKind;
    use crate::engine::task_context::TaskContextBundle;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};

    fn round_state() -> TurnToolRoundState {
        TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: false,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: Vec::new(),
            should_closeout_after_verified_change: false,
        }
    }

    async fn run_flow(
        workflow: WorkflowKind,
        turn_state: &mut TurnRuntimeState,
        round_state: &mut TurnToolRoundState,
        trace: &TraceCollector,
        messages: &mut Vec<Message>,
    ) -> TurnFocusedRepairFlow {
        let route = crate::engine::intent_router::IntentRouter::new().route("fix it");
        let task_bundle = TaskContextBundle::new("fix it", ".", route, None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);

        TurnFocusedRepairFlowController::run(TurnFocusedRepairFlowContext {
            workflow,
            no_diff_audit_closeout_allowed: false,
            code_write_tools_forbidden: false,
            trace,
            code_workflow: &mut code_workflow,
            turn_state,
            round_state,
            messages,
        })
        .await
    }

    #[tokio::test]
    async fn proceeds_when_no_focused_repair_action_is_needed() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix it"));
        let mut turn_state = TurnRuntimeState::new(true);
        let mut round_state = round_state();
        let mut messages = vec![Message::user("fix it")];

        let flow = run_flow(
            WorkflowKind::CodeChange,
            &mut turn_state,
            &mut round_state,
            &trace,
            &mut messages,
        )
        .await;

        assert!(matches!(flow, TurnFocusedRepairFlow::Proceed));
        assert_eq!(messages.len(), 1);
        assert_eq!(round_state.tool_results_text, "");
    }

    #[tokio::test]
    async fn continues_after_file_edit_failure_correction_retry() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix it"));
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.focused_repair.action_checkpoint_active = true;
        let mut round_state = round_state();
        round_state.file_edit_failure_correction_added = true;
        let mut messages = vec![Message::user("fix it")];

        let flow = run_flow(
            WorkflowKind::CodeChange,
            &mut turn_state,
            &mut round_state,
            &trace,
            &mut messages,
        )
        .await;

        assert!(matches!(flow, TurnFocusedRepairFlow::Continue));
        assert!(turn_state.focused_repair.file_edit_failure_retry_used);

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "file_edit repair correction returned to model before patch synthesis"
        )));
    }

    #[tokio::test]
    async fn proceeds_without_runtime_focused_repair_prompt() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix it"));
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.focused_repair.action_checkpoint_active = true;
        let mut round_state = round_state();
        round_state.batch_has_unsuccessful_tools = true;
        round_state.failed_tool_evidence = vec!["bash failed".to_string()];
        let mut messages = vec![Message::user("fix it")];

        let flow = run_flow(
            WorkflowKind::CodeChange,
            &mut turn_state,
            &mut round_state,
            &trace,
            &mut messages,
        )
        .await;

        assert!(matches!(flow, TurnFocusedRepairFlow::Proceed));
        assert!(!round_state
            .tool_results_text
            .contains("Focused repair correction"));
        assert!(!messages.iter().any(|message| matches!(
            message,
            Message::System { content } if content.contains("Focused repair correction")
        )));
    }
}

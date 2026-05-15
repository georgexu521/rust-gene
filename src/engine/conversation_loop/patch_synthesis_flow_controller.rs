use super::focused_repair_state_controller::FocusedRepairStateController;
use super::patch_recovery::PatchSynthesisSource;
use super::patch_synthesis_executor::{PatchSynthesisExecutionContext, PatchSynthesisExecutor};
use super::turn_runtime_state::TurnRuntimeState;
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, ToolCall};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub(super) struct PatchSynthesisCallExecutionContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) assistant_message: &'static str,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) changed_files: &'a mut Vec<PathBuf>,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) is_programming_workflow: bool,
    pub(super) mark_patch_requirement_on_success: bool,
    pub(super) final_tool_calls: &'a mut Vec<ToolCall>,
}

pub(super) struct PatchSynthesisCallExecutionOutcome {
    pub(super) any_tool_success: bool,
    pub(super) changed_files_available: bool,
}

pub(super) struct PatchSynthesisFlowController;

impl PatchSynthesisFlowController {
    pub(super) fn assistant_message_for_source(source: PatchSynthesisSource) -> &'static str {
        match source {
            PatchSynthesisSource::DeterministicFallback => {
                "Applying deterministic patch fallback from prior evidence."
            }
            PatchSynthesisSource::ModelJson | PatchSynthesisSource::ModelToolFallback => {
                "Applying synthesized patch from prior evidence."
            }
        }
    }

    pub(super) async fn execute_calls(
        context: PatchSynthesisCallExecutionContext<'_>,
    ) -> PatchSynthesisCallExecutionOutcome {
        context.messages.push(Message::assistant_with_tools(
            context.assistant_message,
            context.tool_calls.clone(),
        ));
        let execution = PatchSynthesisExecutor::execute(PatchSynthesisExecutionContext {
            conversation: context.conversation,
            tool_calls: &context.tool_calls,
            tx: context.tx,
            trace: context.trace,
            resource_policy: context.resource_policy,
            destructive_scope: context.destructive_scope,
            turn_state: &mut *context.turn_state,
            tool_results_text: &mut *context.tool_results_text,
            messages: &mut *context.messages,
            changed_files: &mut *context.changed_files,
            baseline_git_status_files: context.baseline_git_status_files,
            is_programming_workflow: context.is_programming_workflow,
            mark_patch_requirement_on_success: context.mark_patch_requirement_on_success,
        })
        .await;

        context.final_tool_calls.extend(context.tool_calls);
        let changed_files_available = !context.changed_files.is_empty();
        if changed_files_available {
            FocusedRepairStateController::record_patch_synthesis_success(
                &mut context.turn_state.focused_repair,
            );
        }

        PatchSynthesisCallExecutionOutcome {
            any_tool_success: execution.any_tool_success,
            changed_files_available,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_message_names_patch_source() {
        assert_eq!(
            PatchSynthesisFlowController::assistant_message_for_source(
                PatchSynthesisSource::DeterministicFallback,
            ),
            "Applying deterministic patch fallback from prior evidence."
        );
        assert_eq!(
            PatchSynthesisFlowController::assistant_message_for_source(
                PatchSynthesisSource::ModelJson,
            ),
            "Applying synthesized patch from prior evidence."
        );
        assert_eq!(
            PatchSynthesisFlowController::assistant_message_for_source(
                PatchSynthesisSource::ModelToolFallback,
            ),
            "Applying synthesized patch from prior evidence."
        );
    }
}

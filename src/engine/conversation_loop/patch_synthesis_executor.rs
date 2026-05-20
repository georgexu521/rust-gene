use super::tool_execution_controller::{
    ToolExecutionBatch, ToolExecutionContext, ToolExecutionController, ToolExecutionRequest,
};
use super::tool_turn_controller::{ToolTurnAppendContext, ToolTurnController};
use super::turn_runtime_state::TurnRuntimeState;
use super::workflow_change_tracker::WorkflowChangeTracker;
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, ToolCall};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub(super) struct PatchSynthesisExecutionContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) tool_calls: &'a [ToolCall],
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
}

pub(super) struct PatchSynthesisExecutionOutcome {
    pub(super) any_tool_success: bool,
}

struct PatchSynthesisCollectionContext<'a> {
    tool_batch: &'a mut ToolExecutionBatch,
    turn_state: &'a mut TurnRuntimeState,
    tool_results_text: &'a mut String,
    messages: &'a mut Vec<Message>,
    changed_files: &'a mut Vec<PathBuf>,
    baseline_git_status_files: &'a HashSet<PathBuf>,
    is_programming_workflow: bool,
    mark_patch_requirement_on_success: bool,
}

pub(super) struct PatchSynthesisExecutor;

impl PatchSynthesisExecutor {
    pub(super) async fn execute(
        context: PatchSynthesisExecutionContext<'_>,
    ) -> PatchSynthesisExecutionOutcome {
        let mut context = context;
        let mut synthesized_batch = Self::execute_batch(&mut context).await;
        Self::collect_batch_results(PatchSynthesisCollectionContext {
            tool_batch: &mut synthesized_batch,
            turn_state: context.turn_state,
            tool_results_text: context.tool_results_text,
            messages: context.messages,
            changed_files: context.changed_files,
            baseline_git_status_files: context.baseline_git_status_files,
            is_programming_workflow: context.is_programming_workflow,
            mark_patch_requirement_on_success: context.mark_patch_requirement_on_success,
        })
        .await
    }

    async fn execute_batch(context: &mut PatchSynthesisExecutionContext<'_>) -> ToolExecutionBatch {
        let exposed_synth_tools =
            HashSet::from(["file_edit".to_string(), "file_write".to_string()]);
        ToolExecutionController::new(ToolExecutionContext::from_conversation(
            context.conversation,
        ))
        .execute_tools_parallel(ToolExecutionRequest {
            tool_calls: context.tool_calls,
            parent_assistant_content: "patch synthesis",
            tx: context.tx,
            pre_executed: HashMap::new(),
            trace: Some(context.trace.clone()),
            route: None,
            resource_policy: context.resource_policy,
            exposed_tool_names: &exposed_synth_tools,
            retained_context: &crate::tools::ToolContextRetainedContext::default(),
            // Synthesized edits have already passed patch-synthesis
            // validation. Avoid applying the direct action-checkpoint guard
            // again, or safe recovered patches can be rejected without giving
            // the model a way to inspect and repair the arguments.
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            has_changes_before_tools: false,
            destructive_scope: context.destructive_scope,
            lifecycle: &mut context.turn_state.tool_lifecycle,
        })
        .await
    }

    async fn collect_batch_results(
        context: PatchSynthesisCollectionContext<'_>,
    ) -> PatchSynthesisExecutionOutcome {
        let mut any_tool_success = false;
        for (tc, result) in context.tool_batch.results_mut().iter_mut() {
            ToolTurnController::append_tool_result(
                tc,
                result,
                ToolTurnAppendContext {
                    evidence_ledger: &mut context.turn_state.evidence_ledger,
                    runtime_diet: &mut context.turn_state.runtime_diet,
                    tool_results_text: context.tool_results_text,
                    messages: context.messages,
                },
            )
            .await;
            if result.success {
                any_tool_success = true;
            }
            if result.success && ConversationLoop::is_code_write_tool_name(&tc.name) {
                if context.mark_patch_requirement_on_success {
                    context
                        .turn_state
                        .focused_repair
                        .action_checkpoint_requires_patch_before_validation = false;
                }
                if let Some(path) = tc.arguments["path"].as_str() {
                    context.changed_files.push(PathBuf::from(path));
                }
            }
        }

        if context.is_programming_workflow {
            WorkflowChangeTracker::append_changed_files_since(
                context.changed_files,
                context.baseline_git_status_files,
            );
        }

        PatchSynthesisExecutionOutcome { any_tool_success }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolResult;

    fn file_write_call(path: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({"path": path, "content": "updated"}),
        }
    }

    #[tokio::test]
    async fn collection_records_successful_synthesized_write() {
        let call = file_write_call("src/lib.rs");
        let mut batch =
            ToolExecutionBatch::new(vec![(call, ToolResult::success("wrote file"))], Vec::new());
        let mut turn_state = TurnRuntimeState::new(true);
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let mut changed_files = Vec::new();
        let baseline = HashSet::new();
        turn_state
            .focused_repair
            .action_checkpoint_requires_patch_before_validation = true;

        let outcome =
            PatchSynthesisExecutor::collect_batch_results(PatchSynthesisCollectionContext {
                tool_batch: &mut batch,
                turn_state: &mut turn_state,
                tool_results_text: &mut tool_results_text,
                messages: &mut messages,
                changed_files: &mut changed_files,
                baseline_git_status_files: &baseline,
                is_programming_workflow: false,
                mark_patch_requirement_on_success: true,
            })
            .await;

        assert!(outcome.any_tool_success);
        assert_eq!(changed_files, vec![PathBuf::from("src/lib.rs")]);
        assert!(
            !turn_state
                .focused_repair
                .action_checkpoint_requires_patch_before_validation
        );
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("wrote file"));
    }

    #[tokio::test]
    async fn collection_can_preserve_patch_requirement_flag() {
        let call = file_write_call("src/lib.rs");
        let mut batch =
            ToolExecutionBatch::new(vec![(call, ToolResult::success("wrote file"))], Vec::new());
        let mut turn_state = TurnRuntimeState::new(true);
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let mut changed_files = Vec::new();
        let baseline = HashSet::new();
        turn_state
            .focused_repair
            .action_checkpoint_requires_patch_before_validation = true;

        PatchSynthesisExecutor::collect_batch_results(PatchSynthesisCollectionContext {
            tool_batch: &mut batch,
            turn_state: &mut turn_state,
            tool_results_text: &mut tool_results_text,
            messages: &mut messages,
            changed_files: &mut changed_files,
            baseline_git_status_files: &baseline,
            is_programming_workflow: false,
            mark_patch_requirement_on_success: false,
        })
        .await;

        assert!(
            turn_state
                .focused_repair
                .action_checkpoint_requires_patch_before_validation
        );
        assert_eq!(changed_files, vec![PathBuf::from("src/lib.rs")]);
    }
}

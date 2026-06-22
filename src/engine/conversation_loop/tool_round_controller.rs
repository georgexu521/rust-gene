//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::tool_batch_result_processor::{
    ToolBatchProcessingContext, ToolBatchProcessingOutcome, ToolBatchResultProcessor,
};
use super::tool_execution_controller::{
    ToolExecutionContext, ToolExecutionController, ToolExecutionRequest,
};
use super::turn_state::{TurnRuntimeContext, TurnRuntimeState};
use super::workflow_change_tracker::WorkflowChangeTracker;
use super::ConversationLoop;
use crate::engine::task_context::TaskContextBundle;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq, Eq)]
struct ToolRoundBudgetOutcome {
    counted: bool,
}

struct IterationBudgetController;

impl IterationBudgetController {
    fn record_tool_round(
        turn_state: &mut TurnRuntimeState,
        tool_calls: &[ToolCall],
    ) -> ToolRoundBudgetOutcome {
        let counted = !tool_calls.is_empty();
        if counted {
            turn_state.effective_iterations += 1;
        }
        ToolRoundBudgetOutcome { counted }
    }
}

pub(super) struct ToolRoundContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) content: &'a str,
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) pre_executed: HashMap<usize, ToolResult>,
    pub(super) runtime: TurnRuntimeContext<'a>,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) is_programming_workflow: bool,
    pub(super) companion_context_keys: &'a mut HashSet<String>,
    pub(super) failed_tool_fingerprints: &'a mut HashMap<String, usize>,
    pub(super) failed_tool_names: &'a mut HashMap<String, usize>,
    pub(super) successful_required_validation_commands: &'a mut HashSet<String>,
}

pub(super) struct ToolRoundController;

impl ToolRoundController {
    pub(super) async fn execute(context: ToolRoundContext<'_>) -> ToolBatchProcessingOutcome {
        let ToolRoundContext {
            conversation,
            content,
            tool_calls,
            pre_executed,
            runtime,
            turn_state,
            task_bundle,
            messages,
            is_programming_workflow,
            companion_context_keys,
            failed_tool_fingerprints,
            failed_tool_names,
            successful_required_validation_commands,
        } = context;

        messages.push(Message::assistant_with_tools(content, tool_calls.to_vec()));
        // Persist the tool-call assistant message so resume/export see it.
        if let Some(ref store) = context.conversation.session_store {
            crate::session_store::message_ops::persist_runtime_message_background(
                store,
                &context.conversation.session_id,
                messages.last().unwrap(),
                "assistant tool-call message",
            );
        }
        let has_changes_before_tools = is_programming_workflow
            && WorkflowChangeTracker::has_changes_since(runtime.baseline_git_status_files);
        let action_checkpoint_active_before_batch =
            turn_state.focused_repair.action_checkpoint_active;
        let action_checkpoint_active = turn_state.focused_repair.action_checkpoint_active;
        let action_checkpoint_lookup_count =
            turn_state.focused_repair.action_checkpoint_lookup_count;
        let mut tool_batch =
            ToolExecutionController::new(ToolExecutionContext::from_conversation(conversation))
                .execute_tools_parallel(ToolExecutionRequest {
                    tool_calls,
                    parent_assistant_content: content,
                    tx: runtime.tx,
                    pre_executed,
                    trace: Some(runtime.trace.clone()),
                    route: Some(runtime.route),
                    resource_policy: runtime.resource_policy,
                    exposed_tool_names: runtime.exposed_tool_names,
                    retained_context: runtime.retained_context,
                    task_stage: runtime.task_stage,
                    task_state: Some(&task_bundle.agent_state),
                    action_checkpoint_active,
                    action_checkpoint_lookup_count,
                    no_progress_rounds: turn_state.focused_repair.no_code_progress_rounds,
                    has_changes_before_tools,
                    destructive_scope: runtime.destructive_scope,
                    storm_state: &mut turn_state.storm_state,
                    lifecycle: &mut turn_state.tool_lifecycle,
                })
                .await;

        let _tool_budget = IterationBudgetController::record_tool_round(turn_state, tool_calls);

        ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls,
            tool_batch: &mut tool_batch,
            turn_state,
            task_bundle,
            messages,
            trace: runtime.trace,
            session_id: &conversation.session_id,
            is_programming_workflow,
            working_dir: runtime.working_dir,
            last_user_preview: runtime.last_user_preview,
            companion_context_keys,
            failed_tool_fingerprints,
            failed_tool_names,
            required_validation_commands: runtime.required_validation_commands,
            successful_required_validation_commands,
            action_checkpoint_active: action_checkpoint_active_before_batch,
            destructive_scope: runtime.destructive_scope,
            baseline_git_status_files: runtime.baseline_git_status_files,
            store: conversation.session_store.as_deref(),
            tx: runtime.tx,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(name: &str) -> ToolCall {
        ToolCall {
            id: format!("call_{}", name),
            name: name.to_string(),
            arguments: serde_json::json!({}),
        }
    }

    #[test]
    fn read_only_tool_round_counts_against_simple_iteration_budget() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let outcome = IterationBudgetController::record_tool_round(
            &mut turn_state,
            &[tool_call("grep"), tool_call("file_read")],
        );

        assert_eq!(outcome, ToolRoundBudgetOutcome { counted: true });
        assert_eq!(turn_state.effective_iterations, 3);
    }

    #[test]
    fn write_tool_round_charges_iteration_budget() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let outcome = IterationBudgetController::record_tool_round(
            &mut turn_state,
            &[tool_call("file_write")],
        );

        assert_eq!(outcome, ToolRoundBudgetOutcome { counted: true });
        assert_eq!(turn_state.effective_iterations, 3);
    }
}

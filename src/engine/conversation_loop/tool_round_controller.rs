use super::iteration_budget_controller::IterationBudgetController;
use super::tool_batch_result_processor::{
    ToolBatchProcessingContext, ToolBatchProcessingOutcome, ToolBatchResultProcessor,
};
use super::tool_execution_controller::{
    ToolExecutionContext, ToolExecutionController, ToolExecutionRequest,
};
use super::turn_runtime_context::TurnRuntimeContext;
use super::turn_runtime_state::TurnRuntimeState;
use super::workflow_change_tracker::WorkflowChangeTracker;
use super::ConversationLoop;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;
use std::collections::{HashMap, HashSet};
use tracing::debug;

pub(super) struct ToolRoundContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) content: &'a str,
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) pre_executed: HashMap<usize, ToolResult>,
    pub(super) runtime: TurnRuntimeContext<'a>,
    pub(super) turn_state: &'a mut TurnRuntimeState,
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
            messages,
            is_programming_workflow,
            companion_context_keys,
            failed_tool_fingerprints,
            failed_tool_names,
            successful_required_validation_commands,
        } = context;

        messages.push(Message::assistant_with_tools(content, tool_calls.to_vec()));
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
                    tx: runtime.tx,
                    pre_executed,
                    trace: Some(runtime.trace.clone()),
                    route: Some(runtime.route),
                    resource_policy: runtime.resource_policy,
                    exposed_tool_names: runtime.exposed_tool_names,
                    retained_context: runtime.retained_context,
                    action_checkpoint_active,
                    action_checkpoint_lookup_count,
                    has_changes_before_tools,
                    destructive_scope: runtime.destructive_scope,
                    lifecycle: &mut turn_state.tool_lifecycle,
                })
                .await;

        let tool_budget = IterationBudgetController::record_tool_round(turn_state, tool_calls);
        if tool_budget.refunded {
            debug!("All tools read-only, refunding iteration budget");
        }

        ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls,
            tool_batch: &mut tool_batch,
            turn_state,
            messages,
            trace: runtime.trace,
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
        })
        .await
    }
}

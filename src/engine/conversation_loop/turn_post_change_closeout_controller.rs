use super::post_change_workflow_controller::{
    PostChangeWorkflowContext, PostChangeWorkflowController,
};
use super::turn_iteration_closeout_controller::{
    TurnIterationCloseoutContext, TurnIterationCloseoutController,
};
use super::turn_runtime_state::TurnRuntimeState;
use super::turn_tool_round_outcome_controller::TurnToolRoundState;
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use crate::services::api::Message;
use std::collections::HashSet;

pub(super) struct TurnPostChangeCloseoutContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) round_state: &'a mut TurnToolRoundState,
    pub(super) required_validation_commands: &'a [String],
    pub(super) successful_required_validation_commands: &'a mut HashSet<String>,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) final_content: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) last_user_preview: &'a str,
}

pub(super) enum TurnPostChangeCloseoutFlow {
    Continue,
    Break,
}

pub(super) struct TurnPostChangeCloseoutController;

impl TurnPostChangeCloseoutController {
    pub(super) async fn run(
        context: TurnPostChangeCloseoutContext<'_>,
    ) -> TurnPostChangeCloseoutFlow {
        let post_change_workflow = PostChangeWorkflowController::run(PostChangeWorkflowContext {
            conversation: context.conversation,
            trace: context.trace,
            route: context.route,
            code_workflow: context.code_workflow,
            task_bundle: context.task_bundle,
            changed_files: &context.round_state.changed_files,
            required_validation_commands: context.required_validation_commands,
            successful_validation_commands: &context.round_state.successful_validation_commands,
            successful_required_validation_commands: context
                .successful_required_validation_commands,
            turn_state: context.turn_state,
            should_closeout_after_verified_change: context
                .round_state
                .should_closeout_after_verified_change,
            final_content: &mut *context.final_content,
            tool_results_text: &mut context.round_state.tool_results_text,
            messages: &mut *context.messages,
            last_user_preview: context.last_user_preview,
        })
        .await;

        context.round_state.should_closeout_after_verified_change =
            post_change_workflow.should_closeout_after_verified_change;

        if post_change_workflow.break_loop {
            return TurnPostChangeCloseoutFlow::Break;
        }

        let iteration_closeout =
            TurnIterationCloseoutController::run(TurnIterationCloseoutContext {
                conversation: context.conversation,
                trace: context.trace,
                messages: &*context.messages,
                final_content: &*context.final_content,
                tool_results_text: &context.round_state.tool_results_text,
                should_closeout_after_verified_change: context
                    .round_state
                    .should_closeout_after_verified_change,
            })
            .await;

        if iteration_closeout.break_loop {
            TurnPostChangeCloseoutFlow::Break
        } else {
            TurnPostChangeCloseoutFlow::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    fn conversation() -> ConversationLoop {
        ConversationLoop::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::new()),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "mock-model".to_string(),
        )
    }

    fn round_state(should_closeout_after_verified_change: bool) -> TurnToolRoundState {
        TurnToolRoundState {
            tool_results_text: "tool output".to_string(),
            changed_files: Vec::<PathBuf>::new(),
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
            should_closeout_after_verified_change,
        }
    }

    async fn run_no_change_closeout(
        round_state: &mut TurnToolRoundState,
        trace: &TraceCollector,
    ) -> TurnPostChangeCloseoutFlow {
        let conversation = conversation();
        let route = IntentRouter::new().route("finish the change");
        let mut task_bundle = TaskContextBundle::new("finish the change", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let mut successful_required_validation_commands = HashSet::new();
        let mut turn_state = TurnRuntimeState::new(true);
        let mut final_content = "done".to_string();
        let mut messages = vec![Message::user("finish the change")];
        let required_validation_commands = Vec::new();

        TurnPostChangeCloseoutController::run(TurnPostChangeCloseoutContext {
            conversation: &conversation,
            trace,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            round_state,
            required_validation_commands: &required_validation_commands,
            successful_required_validation_commands: &mut successful_required_validation_commands,
            turn_state: &mut turn_state,
            final_content: &mut final_content,
            messages: &mut messages,
            last_user_preview: "finish the change",
        })
        .await
    }

    #[tokio::test]
    async fn no_changed_files_continue_without_closeout_flag() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "finish the change"));
        let mut round_state = round_state(false);

        let flow = run_no_change_closeout(&mut round_state, &trace).await;

        assert!(matches!(flow, TurnPostChangeCloseoutFlow::Continue));
        assert!(!round_state.should_closeout_after_verified_change);
        assert_eq!(round_state.tool_results_text, "tool output");
    }

    #[tokio::test]
    async fn no_changed_files_break_when_closeout_flag_already_set() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "finish the change"));
        let mut round_state = round_state(true);

        let flow = run_no_change_closeout(&mut round_state, &trace).await;

        assert!(matches!(flow, TurnPostChangeCloseoutFlow::Break));
        assert!(round_state.should_closeout_after_verified_change);

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "verified code change passed validation; preparing deterministic closeout"
        )));
    }
}

use super::first_code_change_controller::{FirstCodeChangeContext, FirstCodeChangeController};
use super::post_edit_repair_controller::{
    PostEditRepairContext, PostEditRepairController, PostEditRepairRuntimeContext,
};
use super::post_edit_verification_controller::{
    PostEditVerificationContext, PostEditVerificationController,
};
use super::turn_runtime_state::TurnRuntimeState;
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use crate::services::api::Message;
use std::collections::HashSet;
use std::path::PathBuf;

pub(super) struct PostChangeWorkflowContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) changed_files: &'a [PathBuf],
    pub(super) required_validation_commands: &'a [String],
    pub(super) successful_validation_commands: &'a [String],
    pub(super) successful_required_validation_commands: &'a mut HashSet<String>,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) final_content: &'a mut String,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) last_user_preview: &'a str,
}

pub(super) struct PostChangeWorkflowOutcome {
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) break_loop: bool,
}

pub(super) struct PostChangeWorkflowController;

impl PostChangeWorkflowController {
    pub(super) async fn run(context: PostChangeWorkflowContext<'_>) -> PostChangeWorkflowOutcome {
        if context.changed_files.is_empty() {
            return PostChangeWorkflowOutcome {
                should_closeout_after_verified_change: context
                    .should_closeout_after_verified_change,
                break_loop: false,
            };
        }

        FirstCodeChangeController::record(FirstCodeChangeContext {
            trace: context.trace,
            code_workflow: context.code_workflow,
            evidence_ledger: &mut context.turn_state.evidence_ledger,
            changed_files: context.changed_files,
        });

        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let verification = PostEditVerificationController::run(PostEditVerificationContext {
            working_dir: &working_dir,
            changed_files: context.changed_files,
            lsp_manager: context.conversation.lsp_manager.as_deref(),
            required_validation_commands: context.required_validation_commands,
            successful_validation_commands: context.successful_validation_commands,
            successful_required_validation_commands: context
                .successful_required_validation_commands,
            evidence_ledger: &mut context.turn_state.evidence_ledger,
            tool_results_text: context.tool_results_text,
            messages: context.messages,
        })
        .await;

        let verification_trace = PostEditVerificationController::record_trace(
            context.trace,
            context.changed_files,
            &verification,
        );
        let should_closeout_after_verified_change =
            verification_trace.should_closeout_after_verified_change;

        let post_edit_repair_outcome = PostEditRepairController::run(
            context.conversation,
            PostEditRepairContext {
                trace: context.trace,
                route: context.route,
                code_workflow: context.code_workflow,
                task_bundle: context.task_bundle,
                changed_files: context.changed_files,
                verification: &verification,
                required_validation_commands: context.required_validation_commands,
                runtime: PostEditRepairRuntimeContext::from_turn_state(context.turn_state),
                max_iterations: context.conversation.max_iterations,
                should_closeout_after_verified_change,
                final_content: context.final_content,
                tool_results_text: context.tool_results_text,
                messages: context.messages,
                last_user_preview: context.last_user_preview,
            },
        )
        .await;

        PostChangeWorkflowOutcome {
            should_closeout_after_verified_change: post_edit_repair_outcome
                .should_closeout_after_verified_change,
            break_loop: post_edit_repair_outcome.break_loop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::TurnTrace;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
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

    #[tokio::test]
    async fn skips_when_no_changed_files() {
        let conversation = conversation();
        let route = IntentRouter::new().route("say hello");
        let mut task_bundle = TaskContextBundle::new("say hello", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "say hello"));
        let mut turn_state = TurnRuntimeState::new(true);
        let mut successful_required_validation_commands = HashSet::new();
        let mut final_content = String::new();
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let changed_files = Vec::new();
        let required_validation_commands = Vec::new();
        let successful_validation_commands = Vec::new();

        let outcome = PostChangeWorkflowController::run(PostChangeWorkflowContext {
            conversation: &conversation,
            trace: &trace,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            changed_files: &changed_files,
            required_validation_commands: &required_validation_commands,
            successful_validation_commands: &successful_validation_commands,
            successful_required_validation_commands: &mut successful_required_validation_commands,
            turn_state: &mut turn_state,
            should_closeout_after_verified_change: true,
            final_content: &mut final_content,
            tool_results_text: &mut tool_results_text,
            messages: &mut messages,
            last_user_preview: "say hello",
        })
        .await;

        assert!(outcome.should_closeout_after_verified_change);
        assert!(!outcome.break_loop);
        assert!(messages.is_empty());
        assert!(tool_results_text.is_empty());
        assert!(final_content.is_empty());
        assert!(successful_required_validation_commands.is_empty());
    }
}

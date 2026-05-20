use super::tool_round_controller::{ToolRoundContext, ToolRoundController};
use super::turn_loop_state_controller::TurnLoopState;
use super::turn_runtime_context::TurnRuntimeContext;
use super::turn_runtime_state::TurnRuntimeState;
use super::turn_tool_round_outcome_controller::{
    TurnToolRoundOutcomeController, TurnToolRoundState,
};
use super::ConversationLoop;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;
use std::collections::HashMap;

pub(super) struct TurnToolRoundStepContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) content: &'a str,
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) pre_executed: HashMap<usize, ToolResult>,
    pub(super) runtime: TurnRuntimeContext<'a>,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) is_programming_workflow: bool,
    pub(super) loop_state: &'a mut TurnLoopState,
}

pub(super) struct TurnToolRoundStepController;

impl TurnToolRoundStepController {
    pub(super) async fn run(context: TurnToolRoundStepContext<'_>) -> TurnToolRoundState {
        let batch_processing = ToolRoundController::execute(ToolRoundContext {
            conversation: context.conversation,
            content: context.content,
            tool_calls: context.tool_calls,
            pre_executed: context.pre_executed,
            runtime: context.runtime,
            turn_state: context.turn_state,
            messages: context.messages,
            is_programming_workflow: context.is_programming_workflow,
            companion_context_keys: &mut context.loop_state.companion_context_keys,
            failed_tool_fingerprints: &mut context.loop_state.failed_tool_fingerprints,
            failed_tool_names: &mut context.loop_state.failed_tool_names,
            successful_required_validation_commands: &mut context
                .loop_state
                .successful_required_validation_commands,
        })
        .await;

        TurnToolRoundOutcomeController::from_batch(batch_processing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::resource_policy::ResourcePolicy;
    use crate::engine::trace::{TraceCollector, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::HashSet;
    use std::path::Path;
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
    async fn empty_round_returns_empty_round_state() {
        let conversation = conversation();
        let route = IntentRouter::new().route("hello");
        let resource_policy = ResourcePolicy::from_route(&route);
        let destructive_scope =
            DestructiveScopeContract::from_user_request("hello", Path::new("."));
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "hello"));
        let mut turn_state = TurnRuntimeState::new(true);
        let mut loop_state = TurnLoopState::default();
        let mut messages = vec![Message::user("hello")];
        let exposed_tool_names = HashSet::new();
        let baseline_git_status_files = HashSet::new();

        let round_state = TurnToolRoundStepController::run(TurnToolRoundStepContext {
            conversation: &conversation,
            content: "done",
            tool_calls: &[],
            pre_executed: HashMap::new(),
            runtime: TurnRuntimeContext {
                tx: None,
                trace: &trace,
                route: &route,
                resource_policy: &resource_policy,
                exposed_tool_names: &exposed_tool_names,
                working_dir: Path::new("."),
                last_user_preview: "hello",
                required_validation_commands: &[],
                destructive_scope: &destructive_scope,
                baseline_git_status_files: &baseline_git_status_files,
            },
            turn_state: &mut turn_state,
            messages: &mut messages,
            is_programming_workflow: false,
            loop_state: &mut loop_state,
        })
        .await;

        assert!(round_state.tool_results_text.is_empty());
        assert!(!round_state.any_tool_success);
        assert!(!round_state.batch_has_unsuccessful_tools);
        assert!(!round_state.should_closeout_after_verified_change);
        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages.last(),
            Some(Message::Assistant {
                content,
                tool_calls: Some(tool_calls),
            }) if content == "done" && tool_calls.is_empty()
        ));
    }
}

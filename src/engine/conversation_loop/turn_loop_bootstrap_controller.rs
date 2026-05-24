use super::turn_loop_state_controller::{TurnLoopState, TurnLoopStateController};
use super::turn_request_bootstrap_controller::{
    TurnRequestBootstrapContext, TurnRequestBootstrapController,
};
use super::turn_runtime_diet_bootstrap_controller::{
    TurnRuntimeDietBootstrapContext, TurnRuntimeDietBootstrapController,
};
use super::turn_runtime_state::TurnRuntimeState;
use super::ConversationLoop;
use crate::engine::intent_router::IntentRoute;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, Tool};
use std::path::Path;
use tokio::sync::mpsc;

pub(super) struct TurnLoopBootstrapContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) route: &'a IntentRoute,
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) working_dir: &'a Path,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) struct TurnLoopBootstrap {
    pub(super) base_tools: Vec<Tool>,
    pub(super) loop_state: TurnLoopState,
}

pub(super) struct TurnLoopBootstrapController;

impl TurnLoopBootstrapController {
    pub(super) async fn run(context: TurnLoopBootstrapContext<'_>) -> TurnLoopBootstrap {
        let base_tools = context.conversation.get_tools_for_route(context.route);
        let loop_state = TurnLoopStateController::initial_state();

        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: context.retrieval_context,
            tools: &base_tools,
            working_dir: context.working_dir,
            runtime_diet: &mut context.turn_state.runtime_diet,
        });
        TurnRequestBootstrapController::run(TurnRequestBootstrapContext {
            retrieval_policy: context.route.retrieval,
            memory_manager: context.conversation.memory_manager.as_ref(),
            compressor: context.conversation.compressor.as_ref(),
            session_store: context.conversation.session_store.as_ref(),
            session_id: &context.conversation.session_id,
            messages: context.messages,
            tools: &base_tools,
            retrieval_context: context.retrieval_context,
            runtime_diet: &mut context.turn_state.runtime_diet,
            trace: context.trace,
            tx: context.tx,
        })
        .await;

        TurnLoopBootstrap {
            base_tools,
            loop_state,
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
    async fn bootstraps_tools_loop_state_and_runtime_diet() {
        let conversation = conversation();
        let route = IntentRouter::new().route("fix it");
        let mut turn_state = TurnRuntimeState::new(true);
        let mut messages = vec![Message::user("fix it")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix it"));
        let working_dir = std::env::current_dir().expect("current dir");

        let bootstrap = TurnLoopBootstrapController::run(TurnLoopBootstrapContext {
            conversation: &conversation,
            route: &route,
            retrieval_context: None,
            working_dir: &working_dir,
            turn_state: &mut turn_state,
            messages: &mut messages,
            trace: &trace,
            tx: None,
        })
        .await;

        assert_eq!(
            bootstrap.base_tools.len(),
            conversation.get_tools_for_route(&route).len()
        );
        assert!(bootstrap.loop_state.final_content.is_empty());
        assert!(bootstrap.loop_state.final_tool_calls.is_empty());
        assert!(!bootstrap.loop_state.tool_calls_made);
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0],
            Message::User { content } if content == "fix it"
        ));
        assert_eq!(
            turn_state.runtime_diet.exposed_tools,
            bootstrap.base_tools.len()
        );
    }
}

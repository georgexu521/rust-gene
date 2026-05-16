use super::closeout_controller::VerifiedChangeCloseoutController;
use super::memory_sync_controller::{MemorySyncContext, MemorySyncController};
use super::ConversationLoop;
use crate::engine::trace::TraceCollector;
use crate::services::api::Message;

pub(super) struct TurnIterationCloseoutContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a [Message],
    pub(super) final_content: &'a str,
    pub(super) tool_results_text: &'a str,
    pub(super) should_closeout_after_verified_change: bool,
}

pub(super) struct TurnIterationCloseoutOutcome {
    pub(super) break_loop: bool,
}

pub(super) struct TurnIterationCloseoutController;

impl TurnIterationCloseoutController {
    pub(super) async fn run(
        context: TurnIterationCloseoutContext<'_>,
    ) -> TurnIterationCloseoutOutcome {
        MemorySyncController::sync_turn(MemorySyncContext {
            memory_manager: context.conversation.memory_manager.as_ref(),
            llm_memory_extraction: context.conversation.llm_memory_extraction,
            provider: Some(context.conversation.provider.as_ref()),
            model: &context.conversation.model,
            trace: context.trace,
            messages: context.messages,
            final_content: context.final_content,
            tool_results_text: context.tool_results_text,
        })
        .await;

        TurnIterationCloseoutOutcome {
            break_loop: VerifiedChangeCloseoutController::should_break_for_verified_change(
                context.trace,
                context.should_closeout_after_verified_change,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
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
    async fn verified_change_closeout_breaks_after_memory_sync_path() {
        let conversation = conversation();
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "change"));
        let messages = vec![Message::user("finish the change")];

        let outcome = TurnIterationCloseoutController::run(TurnIterationCloseoutContext {
            conversation: &conversation,
            trace: &trace,
            messages: &messages,
            final_content: "done",
            tool_results_text: "tools",
            should_closeout_after_verified_change: true,
        })
        .await;

        assert!(outcome.break_loop);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "verified code change passed validation; preparing deterministic closeout"
        )));
    }
}

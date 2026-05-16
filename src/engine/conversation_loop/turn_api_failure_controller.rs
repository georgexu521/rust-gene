use super::runtime_diet::{trace_runtime_diet_report, RuntimeDietSnapshot};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::trace::{TraceCollector, TraceEvent, TurnStatus};

pub(super) struct TurnApiFailureContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) error_message: &'a str,
}

pub(super) struct TurnApiFailureController;

impl TurnApiFailureController {
    pub(super) fn record(context: TurnApiFailureContext<'_>) {
        context.trace.record(TraceEvent::Error {
            message: context.error_message.to_string(),
        });
        context.runtime_diet.validation_evidence = "api_error".to_string();
        trace_runtime_diet_report(
            context.trace,
            context.route,
            context.code_workflow,
            context.runtime_diet,
        );
        context
            .conversation
            .finish_trace(context.trace.clone(), TurnStatus::Failed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;
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

    #[test]
    fn record_marks_api_error_and_finishes_failed_trace() {
        let conversation = conversation();
        let route = IntentRouter::new().route("fix it");
        let task_bundle = TaskContextBundle::new("fix it", ".", route.clone(), None);
        let code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix it"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);

        TurnApiFailureController::record(TurnApiFailureContext {
            conversation: &conversation,
            trace: &trace,
            route: &route,
            code_workflow: &code_workflow,
            runtime_diet: &mut runtime_diet,
            error_message: "provider unavailable",
        });

        assert_eq!(runtime_diet.validation_evidence, "api_error");
        let finished = trace.finish(TurnStatus::Failed);
        assert_eq!(finished.status, TurnStatus::Failed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::Error { message } if message == "provider unavailable"
        )));
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::RuntimeDietReport { .. })));
    }
}

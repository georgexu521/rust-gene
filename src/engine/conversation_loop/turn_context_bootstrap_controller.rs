use super::turn_retrieval_context_controller::{
    TurnRetrievalContextController, TurnRetrievalContextRequest,
};
use super::turn_runtime_state::TurnRuntimeState;
use super::turn_task_context_controller::{
    TurnTaskContextSetupContext, TurnTaskContextSetupController,
};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use std::path::Path;

pub(super) struct TurnContextBootstrapContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) last_user_preview: &'a str,
    pub(super) route: &'a IntentRoute,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) working_dir: &'a Path,
    pub(super) required_validation_commands: &'a [String],
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct TurnContextBootstrap {
    pub(super) retrieval_context: Option<RetrievalContext>,
    pub(super) task_bundle: TaskContextBundle,
    pub(super) code_workflow: CodeChangeWorkflowRunner,
    pub(super) turn_state: TurnRuntimeState,
}

pub(super) struct TurnContextBootstrapController;

impl TurnContextBootstrapController {
    pub(super) async fn run(context: TurnContextBootstrapContext<'_>) -> TurnContextBootstrap {
        let retrieval_context =
            TurnRetrievalContextController::build(TurnRetrievalContextRequest {
                last_user_preview: context.last_user_preview,
                working_dir: context.working_dir,
                retrieval_policy: context.route.retrieval,
                session_store: context.conversation.session_store.clone(),
                memory_manager: context.conversation.memory_manager.as_ref(),
                provider: context.conversation.provider.as_ref(),
                model: &context.conversation.model,
                trace: context.trace,
            })
            .await;

        let task_context_setup =
            TurnTaskContextSetupController::prepare(TurnTaskContextSetupContext {
                last_user_preview: context.last_user_preview,
                working_dir: context.working_dir,
                route: context.route,
                current_goal: context
                    .conversation
                    .goal_manager
                    .as_ref()
                    .and_then(|manager| manager.current()),
                retrieval_context: retrieval_context.as_ref(),
                resource_policy: context.resource_policy,
                required_validation_commands: context.required_validation_commands,
                route_scoped_tools_enabled: ConversationLoop::route_scoped_tools_enabled(),
                trace: context.trace,
            });

        TurnContextBootstrap {
            retrieval_context,
            task_bundle: task_context_setup.task_bundle,
            code_workflow: task_context_setup.code_workflow,
            turn_state: task_context_setup.turn_state,
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
    async fn bootstraps_task_context_without_retrieval_context() {
        let conversation = conversation();
        let route = IntentRouter::new().route("你好");
        let resource_policy = ResourcePolicy::from_route(&route);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "你好"));
        let working_dir = std::env::current_dir().expect("current dir");

        let bootstrap = TurnContextBootstrapController::run(TurnContextBootstrapContext {
            conversation: &conversation,
            last_user_preview: "你好",
            route: &route,
            resource_policy: &resource_policy,
            working_dir: &working_dir,
            required_validation_commands: &[],
            trace: &trace,
        })
        .await;

        assert!(bootstrap.retrieval_context.is_none());
        assert_eq!(bootstrap.task_bundle.route.workflow, route.workflow);
        assert_eq!(
            bootstrap.code_workflow.task_id,
            bootstrap.task_bundle.task_id
        );
        assert_eq!(
            bootstrap.turn_state.runtime_diet.route_scoped_tools,
            ConversationLoop::route_scoped_tools_enabled()
        );
    }

    #[tokio::test]
    async fn bootstraps_required_validation_trigger() {
        let conversation = conversation();
        let route = IntentRouter::new().route("修改 src/main.rs 并运行 cargo test -q");
        let resource_policy = ResourcePolicy::from_route(&route);
        let trace = TraceCollector::new(TurnTrace::new(
            "session",
            1,
            "修改 src/main.rs 并运行 cargo test -q",
        ));
        let working_dir = std::env::current_dir().expect("current dir");
        let required = vec!["cargo test -q".to_string()];

        let bootstrap = TurnContextBootstrapController::run(TurnContextBootstrapContext {
            conversation: &conversation,
            last_user_preview: "修改 src/main.rs 并运行 cargo test -q",
            route: &route,
            resource_policy: &resource_policy,
            working_dir: &working_dir,
            required_validation_commands: &required,
            trace: &trace,
        })
        .await;

        assert_eq!(
            bootstrap.code_workflow.adaptive_trigger_labels(),
            vec!["risk_signal_high", "required_validation"]
        );
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AdaptiveWorkflowTriggered { trigger, .. }
                if trigger == "required_validation"
        )));
    }
}

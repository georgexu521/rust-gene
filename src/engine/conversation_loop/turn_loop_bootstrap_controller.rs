use super::turn_request_bootstrap_controller::{
    TurnRequestBootstrapContext, TurnRequestBootstrapController,
};
use super::turn_state::{TurnLoopState, TurnLoopStateController, TurnRuntimeState};
use super::ConversationLoop;
use crate::engine::conversation_loop::main_loop_profile::MainLoopProfile;
use crate::engine::intent_router::IntentRoute;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, Tool};
use std::path::Path;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// TurnRuntimeDietBootstrap
// ---------------------------------------------------------------------------

struct TurnRuntimeDietBootstrapContext<'a> {
    retrieval_context: Option<&'a RetrievalContext>,
    tools: &'a [Tool],
    working_dir: &'a Path,
    runtime_diet: &'a mut crate::engine::conversation_loop::runtime_diet::RuntimeDietSnapshot,
}

struct TurnRuntimeDietBootstrapController;

impl TurnRuntimeDietBootstrapController {
    fn observe(context: TurnRuntimeDietBootstrapContext<'_>) {
        if let Some(retrieval_context) = context.retrieval_context {
            context
                .runtime_diet
                .observe_retrieval_context(retrieval_context);
        }
        if Self::skills_list_exposed(context.tools) {
            let skill_summary =
                crate::skills::SkillRuntime::load(context.working_dir).discovery_summary("", 30);
            context
                .runtime_diet
                .observe_skill_list_summary(&skill_summary);
        }
    }

    fn skills_list_exposed(tools: &[Tool]) -> bool {
        tools.iter().any(|tool| tool.name == "skills_list")
    }
}

pub(super) struct TurnLoopBootstrapContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) route: &'a IntentRoute,
    pub(super) profile: MainLoopProfile,
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) working_dir: &'a Path,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) struct TurnLoopBootstrap {
    pub(super) base_tools: Vec<Tool>,
    pub(super) available_tools: Vec<Tool>,
    pub(super) loop_state: TurnLoopState,
}

pub(super) struct TurnLoopBootstrapController;

impl TurnLoopBootstrapController {
    pub(super) async fn run(context: TurnLoopBootstrapContext<'_>) -> TurnLoopBootstrap {
        let available_tools = if context.profile.expose_tools() {
            context.conversation.get_tools()
        } else {
            Vec::new()
        };
        let base_tools = if context.profile.expose_tools() {
            context.conversation.get_tools_for_route(context.route)
        } else {
            Vec::new()
        };
        let loop_state = TurnLoopStateController::initial_state();

        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: context.retrieval_context,
            tools: &base_tools,
            working_dir: context.working_dir,
            runtime_diet: &mut context.turn_state.runtime_diet,
        });
        TurnRequestBootstrapController::run(TurnRequestBootstrapContext {
            retrieval_policy: context.route.retrieval,
            memory_manager: context.conversation.memory_manager_for_static_memory(),
            compressor: context.conversation.compressor.as_ref(),
            session_store: context.conversation.session_store.as_ref(),
            session_id: &context.conversation.session_id,
            messages: context.messages,
            tools: &base_tools,
            retrieval_context: context.retrieval_context,
            runtime_diet: &mut context.turn_state.runtime_diet,
            trace: context.trace,
            tx: context.tx.filter(|_| context.profile.emit_start_event()),
            inject_dynamic_context: context.profile.inject_dynamic_context(),
        })
        .await;

        TurnLoopBootstrap {
            base_tools,
            available_tools,
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
            profile: MainLoopProfile::from_turn(&route, &[]),
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
        assert_eq!(
            bootstrap.available_tools.len(),
            conversation.get_tools().len()
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

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: "tool".to_string(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        }
    }

    #[test]
    fn observe_records_retrieval_context_budget() {
        use super::super::runtime_diet::RuntimeDietSnapshot;
        use crate::engine::intent_router::RetrievalPolicy;
        use tempfile::tempdir;

        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let retrieval_context = RetrievalContext::from_memory_prefetch(
            "fix bug",
            "remember to run cargo test",
            RetrievalPolicy::Memory,
        )
        .expect("memory context");
        let tmp = tempdir().expect("tempdir");

        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: Some(&retrieval_context),
            tools: &[tool("file_read")],
            working_dir: tmp.path(),
            runtime_diet: &mut runtime_diet,
        });

        assert_eq!(runtime_diet.retrieval_items, 1);
        assert!(runtime_diet.retrieval_tokens > 0);
        assert_eq!(runtime_diet.skill_list_chars, 0);
    }

    #[test]
    fn observe_records_skill_summary_only_when_tool_is_exposed() {
        use super::super::runtime_diet::RuntimeDietSnapshot;
        use tempfile::tempdir;

        let tmp = tempdir().expect("tempdir");
        let mut without_skill_tool = RuntimeDietSnapshot::new(true);

        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: None,
            tools: &[tool("file_read")],
            working_dir: tmp.path(),
            runtime_diet: &mut without_skill_tool,
        });

        assert_eq!(without_skill_tool.skill_list_chars, 0);

        let mut with_skill_tool = RuntimeDietSnapshot::new(true);
        TurnRuntimeDietBootstrapController::observe(TurnRuntimeDietBootstrapContext {
            retrieval_context: None,
            tools: &[tool("skills_list")],
            working_dir: tmp.path(),
            runtime_diet: &mut with_skill_tool,
        });

        assert!(with_skill_tool.skill_list_chars > 0);
        assert!(with_skill_tool.skill_list_tokens > 0);
    }
}

use super::api_request_controller::{ApiRequestContext, ApiRequestController};
use super::request_preparation_controller::{
    RequestPreparationContext, RequestPreparationController,
};
use super::turn_api_failure_controller::{TurnApiFailureContext, TurnApiFailureController};
use super::turn_assistant_response_controller::{
    TurnAssistantResponseContext, TurnAssistantResponseController, TurnAssistantResponseFlow,
};
use super::turn_loop_state_controller::TurnLoopState;
use super::turn_runtime_state::TurnRuntimeState;
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, Tool, ToolCall};
use crate::tools::ToolResult;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

pub(super) struct TurnModelStepContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) iteration: usize,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) turn_retrieval_context: Option<&'a RetrievalContext>,
    pub(super) focused_repair_prompt: Option<Message>,
    pub(super) tools: &'a [Tool],
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) loop_state: &'a mut TurnLoopState,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

#[derive(Debug)]
pub(super) enum TurnModelStepFlow {
    Retry,
    Finish,
    ToolRound {
        content: String,
        tool_calls: Vec<ToolCall>,
        pre_executed: HashMap<usize, ToolResult>,
    },
}

pub(super) struct TurnModelStepController;

impl TurnModelStepController {
    pub(super) async fn run(context: TurnModelStepContext<'_>) -> Result<TurnModelStepFlow> {
        let prepared_request = RequestPreparationController::prepare(RequestPreparationContext {
            messages: context.messages,
            focused_repair_prompt: context.focused_repair_prompt,
            turn_retrieval_context: context.turn_retrieval_context,
            retrieval_policy: context.route.retrieval,
            memory_manager: context.conversation.memory_manager.as_ref(),
            provider: Some(context.conversation.provider.as_ref()),
            model: &context.conversation.model,
            tools: context.tools,
            trace: context.trace,
            runtime_diet: &mut context.turn_state.runtime_diet,
        })
        .await;

        let api_outcome = match ApiRequestController::execute(ApiRequestContext {
            conversation: context.conversation,
            request: prepared_request.request,
            messages: context.messages,
            tools: context.tools,
            exposed_tool_names: context.exposed_tool_names,
            tx: context.tx,
            trace: context.trace,
            iteration: context.iteration,
        })
        .await
        {
            Ok(outcome) => outcome,
            Err(e) => {
                let error_message = e.to_string();
                TurnApiFailureController::record(TurnApiFailureContext {
                    conversation: context.conversation,
                    trace: context.trace,
                    route: context.route,
                    code_workflow: context.code_workflow,
                    runtime_diet: &mut context.turn_state.runtime_diet,
                    error_message: &error_message,
                });
                return Err(e);
            }
        };

        let assistant_flow =
            TurnAssistantResponseController::handle(TurnAssistantResponseContext {
                outcome: api_outcome,
                loop_state: context.loop_state,
                trace: context.trace,
                iteration: context.iteration,
                route: context.route,
                evidence_ledger: &context.turn_state.evidence_ledger,
                exposed_tool_names: context.exposed_tool_names,
                provider: context.conversation.provider.as_ref(),
                tools: context.tools,
                tx: context.tx,
                messages: context.messages,
            })
            .await;

        Ok(match assistant_flow {
            TurnAssistantResponseFlow::Retry => TurnModelStepFlow::Retry,
            TurnAssistantResponseFlow::Finish => TurnModelStepFlow::Finish,
            TurnAssistantResponseFlow::ToolRound {
                content,
                tool_calls,
                pre_executed,
            } => TurnModelStepFlow::ToolRound {
                content,
                tool_calls,
                pre_executed,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::turn_loop_state_controller::TurnLoopStateController;
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::Mutex;

    struct MockProvider {
        responses: StdMutex<VecDeque<anyhow::Result<ChatResponse>>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("mock response")
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

    fn conversation(responses: Vec<anyhow::Result<ChatResponse>>) -> ConversationLoop {
        ConversationLoop::new(
            Arc::new(MockProvider {
                responses: StdMutex::new(VecDeque::from(responses)),
            }),
            Arc::new(ToolRegistry::new()),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "mock-model".to_string(),
        )
    }

    async fn run_step(
        conversation: &ConversationLoop,
        route: &IntentRoute,
        loop_state: &mut TurnLoopState,
        turn_state: &mut TurnRuntimeState,
        messages: &mut Vec<Message>,
        trace: &TraceCollector,
    ) -> Result<TurnModelStepFlow> {
        let task_bundle = crate::engine::task_context::TaskContextBundle::new(
            "run command",
            ".",
            route.clone(),
            None,
        );
        let code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let exposed_tool_names = HashSet::from(["bash".to_string()]);

        TurnModelStepController::run(TurnModelStepContext {
            conversation,
            iteration: 1,
            route,
            code_workflow: &code_workflow,
            turn_retrieval_context: None,
            focused_repair_prompt: None,
            tools: &[],
            exposed_tool_names: &exposed_tool_names,
            loop_state,
            turn_state,
            messages,
            trace,
            tx: None,
        })
        .await
    }

    #[tokio::test]
    async fn finishes_plain_model_response() {
        let conversation = conversation(vec![Ok(ChatResponse {
            content: "done".to_string(),
            tool_calls: None,
            usage: None,
        })]);
        let route = IntentRouter::new().route("hello");
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut turn_state = TurnRuntimeState::new(true);
        let mut messages = vec![Message::user("hello")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "hello"));

        let flow = run_step(
            &conversation,
            &route,
            &mut loop_state,
            &mut turn_state,
            &mut messages,
            &trace,
        )
        .await
        .expect("model step");

        assert!(matches!(flow, TurnModelStepFlow::Finish));
        assert_eq!(loop_state.final_content, "done");
        assert!(!loop_state.tool_calls_made);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::ApiRequestCompleted { .. })));
    }

    #[tokio::test]
    async fn returns_tool_round_for_tool_call_response() {
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": "cargo check -q" }),
        };
        let conversation = conversation(vec![Ok(ChatResponse {
            content: "running".to_string(),
            tool_calls: Some(vec![tool_call.clone()]),
            usage: None,
        })]);
        let route = IntentRouter::new().route("run cargo check");
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut turn_state = TurnRuntimeState::new(true);
        let mut messages = vec![Message::user("run cargo check")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "run cargo check"));

        let flow = run_step(
            &conversation,
            &route,
            &mut loop_state,
            &mut turn_state,
            &mut messages,
            &trace,
        )
        .await
        .expect("model step");

        let TurnModelStepFlow::ToolRound {
            content,
            tool_calls,
            pre_executed,
        } = flow
        else {
            panic!("expected tool round");
        };
        assert_eq!(content, "running");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, tool_call.id);
        assert!(pre_executed.is_empty());
        assert!(loop_state.tool_calls_made);
    }

    #[tokio::test]
    async fn records_api_failure_before_returning_error() {
        let conversation = conversation(vec![Err(anyhow::anyhow!("provider down"))]);
        let route = IntentRouter::new().route("hello");
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut turn_state = TurnRuntimeState::new(true);
        let mut messages = vec![Message::user("hello")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "hello"));

        let error = run_step(
            &conversation,
            &route,
            &mut loop_state,
            &mut turn_state,
            &mut messages,
            &trace,
        )
        .await
        .expect_err("provider error");

        assert_eq!(error.to_string(), "provider down");
        assert_eq!(turn_state.runtime_diet.validation_evidence, "api_error");
        let finished = trace.finish(TurnStatus::Failed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::Error { message } if message == "provider down"
        )));
    }
}

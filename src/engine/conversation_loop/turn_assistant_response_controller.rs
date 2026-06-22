//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::api_request_controller::{
    ApiRequestApplicationContext, ApiRequestController, ApiRequestOutcome,
};
use super::assistant_response_retry_controller::{
    AssistantResponseRetryController, NoToolAssistantResponseContext, NoToolAssistantResponseFlow,
};
use super::turn_state::TurnLoopState;
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::IntentRoute;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::engine::verification_proof::VerificationProof;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

pub(super) struct TurnAssistantResponseContext<'a> {
    pub(super) outcome: ApiRequestOutcome,
    pub(super) loop_state: &'a mut TurnLoopState,
    pub(super) trace: &'a TraceCollector,
    pub(super) iteration: usize,
    pub(super) route: &'a IntentRoute,
    pub(super) evidence_ledger: &'a EvidenceLedger,
    pub(super) verification_proof: &'a VerificationProof,
    pub(super) required_validation_commands: &'a [String],
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) enum TurnAssistantResponseFlow {
    Retry,
    Finish,
    ToolRound {
        content: String,
        tool_calls: Vec<ToolCall>,
        pre_executed: HashMap<usize, ToolResult>,
    },
}

pub(super) struct TurnAssistantResponseController;

impl TurnAssistantResponseController {
    pub(super) async fn handle(
        context: TurnAssistantResponseContext<'_>,
    ) -> TurnAssistantResponseFlow {
        let api_application = ApiRequestController::apply_outcome(ApiRequestApplicationContext {
            outcome: context.outcome,
            final_content: &mut context.loop_state.final_content,
            final_tool_calls: &mut context.loop_state.final_tool_calls,
            tool_calls_made: &mut context.loop_state.tool_calls_made,
            tx: context.tx,
            trace: context.trace,
            iteration: context.iteration,
        });
        let content = api_application.content;
        let tool_calls = api_application.tool_calls;
        let pre_executed = api_application.pre_executed;

        if tool_calls.is_empty() {
            return match AssistantResponseRetryController::handle_no_tool_response(
                NoToolAssistantResponseContext {
                    content: &content,
                    route: context.route,
                    evidence_ledger: context.evidence_ledger,
                    verification_proof: context.verification_proof,
                    exposed_tool_names: context.exposed_tool_names,
                    tool_calls_made: context.loop_state.tool_calls_made,
                    pseudo_tool_retry_used: &mut context.loop_state.pseudo_tool_retry_used,
                    filesystem_grounding_retry_used: &mut context
                        .loop_state
                        .filesystem_grounding_retry_used,
                    continuation_retry_used: &mut context.loop_state.continuation_retry_used,
                    post_tool_empty_retry_used: &mut context.loop_state.post_tool_empty_retry_used,
                    claim_gate_repair_used: &mut context.loop_state.claim_gate_repair_used,
                    tx: context.tx,
                    trace: context.trace,
                    messages: context.messages,
                    required_validation_commands: context.required_validation_commands,
                    iterations_used: context.iteration,
                    max_iterations: 50,
                },
            )
            .await
            {
                NoToolAssistantResponseFlow::Retry => TurnAssistantResponseFlow::Retry,
                NoToolAssistantResponseFlow::Finish => TurnAssistantResponseFlow::Finish,
            };
        }

        TurnAssistantResponseFlow::ToolRound {
            content,
            tool_calls,
            pre_executed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::session_processor::{SessionStepResult, SessionStepSource};
    use super::super::turn_state::TurnLoopStateController;
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use async_openai::types::ChatCompletionResponseStream;

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

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session-test", 1, "assistant response"))
    }

    fn outcome(content: &str, tool_calls: Vec<ToolCall>) -> ApiRequestOutcome {
        ApiRequestOutcome {
            session_step: SessionStepResult {
                assistant_text: content.to_string(),
                tool_calls,
                pre_executed_results: HashMap::new(),
                usage: None,
                tool_call_repair: None,
                finish_reason: None,
                source: SessionStepSource::NonStreaming,
                cache_shape: None,
            },
            compressed_this_turn: false,
            model: "mock-model".to_string(),
        }
    }

    #[tokio::test]
    async fn handle_returns_tool_round_and_updates_loop_state() {
        let trace = trace();
        let route = IntentRouter::new().route("run cargo check");
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut messages = vec![Message::user("run cargo check")];
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "cargo check -q"}),
        };
        let provider = MockProvider;
        let evidence_ledger = EvidenceLedger::new();
        let verification_proof = VerificationProof::new(
            crate::engine::verification_proof::VerificationProofStatus::NotRun,
            "not evaluated",
        );
        let exposed_tool_names = HashSet::from(["bash".to_string()]);
        let _ = provider;

        let flow = TurnAssistantResponseController::handle(TurnAssistantResponseContext {
            outcome: outcome("running check", vec![tool_call.clone()]),
            loop_state: &mut loop_state,
            trace: &trace,
            iteration: 3,
            route: &route,
            evidence_ledger: &evidence_ledger,
            verification_proof: &verification_proof,
            required_validation_commands: &[],
            exposed_tool_names: &exposed_tool_names,
            tx: None,
            messages: &mut messages,
        })
        .await;

        let TurnAssistantResponseFlow::ToolRound {
            content,
            tool_calls,
            pre_executed,
        } = flow
        else {
            panic!("tool calls should proceed to tool round");
        };
        assert_eq!(content, "running check");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, tool_call.id);
        assert_eq!(tool_calls[0].name, tool_call.name);
        assert!(pre_executed.is_empty());
        assert!(
            loop_state.final_content.is_empty(),
            "tool-call responses must wait for the post-tool final answer"
        );
        assert_eq!(loop_state.final_tool_calls.len(), 1);
        assert!(loop_state.tool_calls_made);

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::ApiRequestCompleted {
                iteration: 3,
                tool_calls: 1,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn handle_finishes_plain_response_without_retry() {
        let trace = trace();
        let route = IntentRouter::new().route("hello");
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut messages = vec![Message::user("hello")];
        let provider = MockProvider;
        let evidence_ledger = EvidenceLedger::new();
        let verification_proof = VerificationProof::new(
            crate::engine::verification_proof::VerificationProofStatus::NotRun,
            "not evaluated",
        );
        let exposed_tool_names = HashSet::new();
        let _ = provider;

        let flow = TurnAssistantResponseController::handle(TurnAssistantResponseContext {
            outcome: outcome("hello there", Vec::new()),
            loop_state: &mut loop_state,
            trace: &trace,
            iteration: 1,
            route: &route,
            evidence_ledger: &evidence_ledger,
            verification_proof: &verification_proof,
            required_validation_commands: &[],
            exposed_tool_names: &exposed_tool_names,
            tx: None,
            messages: &mut messages,
        })
        .await;

        assert!(matches!(flow, TurnAssistantResponseFlow::Finish));
        assert_eq!(loop_state.final_content, "hello there");
        assert!(!loop_state.tool_calls_made);
        assert_eq!(messages.len(), 1);
    }
}

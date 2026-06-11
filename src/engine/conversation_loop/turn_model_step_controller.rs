use super::api_request_controller::{ApiRequestContext, ApiRequestController};
use super::closeout_controller::CloseoutEvaluator;
use super::request_preparation_controller::{
    RequestPreparationContext, RequestPreparationController,
};
use super::turn_api_failure_controller::{TurnApiFailureContext, TurnApiFailureController};
use super::turn_assistant_response_controller::{
    TurnAssistantResponseContext, TurnAssistantResponseController, TurnAssistantResponseFlow,
};
use super::turn_state::TurnLoopState;
use super::turn_state::TurnRuntimeState;
use super::ConversationLoop;
use crate::engine::action_decision::ActionDecisionInput;
use crate::engine::candidate_action::{
    parse_candidate_actions, rank_candidate_actions, CandidateAction, CandidateActionMode,
    CandidateActionSet,
};
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::conversation_loop::turn_loop_policy::MainLoopProfile;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::task_contract::TaskContractBundleExt;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, Tool, ToolCall};
use crate::tools::ToolResult;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

pub(super) struct TurnModelStepContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) iteration: usize,
    pub(super) route: &'a IntentRoute,
    pub(super) profile: MainLoopProfile,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) required_validation_commands: &'a [String],
    pub(super) turn_retrieval_context: Option<&'a RetrievalContext>,
    pub(super) focused_repair_prompt: Option<Message>,
    pub(super) tools: &'a [Tool],
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) resource_policy: &'a ResourcePolicy,
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
        let task_contract = context
            .task_bundle
            .task_contract(context.required_validation_commands);
        let context_pack = context.task_bundle.context_pack(&task_contract);
        let prepared_request = RequestPreparationController::prepare(RequestPreparationContext {
            messages: context.messages,
            working_dir: context
                .conversation
                .working_dir_override
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new(".")),
            focused_repair_prompt: context.focused_repair_prompt,
            agent_task_state: context
                .profile
                .inject_dynamic_context()
                .then_some(&context.task_bundle.agent_state),
            task_contract: context
                .profile
                .inject_dynamic_context()
                .then_some(&task_contract),
            context_pack: context
                .profile
                .inject_dynamic_context()
                .then_some(&context_pack),
            turn_retrieval_context: context.turn_retrieval_context,
            retrieval_policy: context.route.retrieval,
            memory_manager: context.conversation.memory_manager_for_dynamic_recall(),
            provider: Some(context.conversation.provider.as_ref()),
            session_store: context.conversation.session_store.as_ref(),
            session_id: &context.conversation.session_id,
            model: &context.conversation.model,
            temperature: context.conversation.temperature,
            tools: context.tools,
            trace: context.trace,
            runtime_diet: &mut context.turn_state.runtime_diet,
            inject_dynamic_context: context.profile.inject_dynamic_context(),
        })
        .await;

        let api_outcome = match ApiRequestController::execute(ApiRequestContext {
            conversation: context.conversation,
            request: prepared_request.request,
            messages: context.messages,
            tools: context.tools,
            exposed_tool_names: context.exposed_tool_names,
            resource_policy: context.resource_policy,
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
                })
                .await;
                return Err(e);
            }
        };

        let closeout_evaluation = CloseoutEvaluator::evaluate(
            context.code_workflow,
            context.task_bundle,
            &context.turn_state.evidence_ledger,
            context.required_validation_commands,
        );

        let assistant_flow =
            TurnAssistantResponseController::handle(TurnAssistantResponseContext {
                outcome: api_outcome,
                loop_state: context.loop_state,
                trace: context.trace,
                iteration: context.iteration,
                route: context.route,
                evidence_ledger: &context.turn_state.evidence_ledger,
                verification_proof: &closeout_evaluation.verification_proof,
                required_validation_commands: context.required_validation_commands,
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
            } => {
                let (tool_calls, pre_executed) =
                    evaluate_candidate_actions_for_tool_round(CandidateActionRoundContext {
                        content: &content,
                        tool_calls,
                        pre_executed,
                        route: context.route,
                        task_bundle: context.task_bundle,
                        turn_state: context.turn_state,
                        exposed_tool_names: context.exposed_tool_names,
                        trace: context.trace,
                    });
                TurnModelStepFlow::ToolRound {
                    content,
                    tool_calls,
                    pre_executed,
                }
            }
        })
    }
}

struct CandidateActionRoundContext<'a> {
    content: &'a str,
    tool_calls: Vec<ToolCall>,
    pre_executed: HashMap<usize, ToolResult>,
    route: &'a IntentRoute,
    task_bundle: &'a TaskContextBundle,
    turn_state: &'a TurnRuntimeState,
    exposed_tool_names: &'a HashSet<String>,
    trace: &'a TraceCollector,
}

fn evaluate_candidate_actions_for_tool_round(
    context: CandidateActionRoundContext<'_>,
) -> (Vec<ToolCall>, HashMap<usize, ToolResult>) {
    let mode = CandidateActionMode::from_env();
    if mode == CandidateActionMode::Off || context.tool_calls.is_empty() {
        return (context.tool_calls, context.pre_executed);
    }

    let parsed_candidates = parse_candidate_actions(context.content).ok();
    let candidates =
        parsed_candidates.unwrap_or_else(|| candidate_set_from_tool_calls(&context.tool_calls));
    let ranking = rank_candidate_actions(
        &candidates,
        ActionDecisionInput {
            task_stage: context.task_bundle.agent_state.stage,
            route_workflow: Some(context.route.workflow),
            route_risk: Some(context.route.risk),
            action_checkpoint_active: context.turn_state.focused_repair.action_checkpoint_active,
            has_changes_before_tools: false,
            no_progress_rounds: context.turn_state.focused_repair.no_code_progress_rounds,
        },
        context.exposed_tool_names,
        mode,
    );
    let selected_id = ranking.selected_id.clone();
    context.trace.record(TraceEvent::CandidateActionsEvaluated {
        mode: mode.as_str().to_string(),
        candidate_count: ranking.candidate_count,
        selected_id: ranking.selected_id,
        selected_tool: ranking.selected_tool,
        selected_score: ranking.selected_score,
        selected_runtime_score: ranking.selected_runtime_score,
        selected_model_score: ranking.selected_model_score,
        runtime_model_score_delta: ranking.runtime_model_score_delta,
        runtime_selected_differs_from_model_order: ranking
            .runtime_selected_differs_from_model_order,
        calibration_reason: ranking.calibration_reason,
        selected_factor_score: ranking.selected_factor_score,
        model_factor_coverage: ranking.model_factor_coverage,
        memory_evidence_items: ranking.memory_evidence_items,
        selected_factor_rationale: ranking.selected_factor_rationale,
        rejected: ranking.rejected.len(),
        reason: "candidate-action ranking evaluated for tool round".to_string(),
    });

    if mode != CandidateActionMode::Gated || !candidate_gate_triggered(context.task_bundle) {
        return (context.tool_calls, context.pre_executed);
    }

    let Some(selected_id) = selected_id else {
        return (context.tool_calls, context.pre_executed);
    };
    let Some((selected_idx, selected_call)) = context
        .tool_calls
        .iter()
        .cloned()
        .enumerate()
        .find(|(_, call)| call.id == selected_id)
    else {
        return (context.tool_calls, context.pre_executed);
    };

    let mut pre_executed = HashMap::new();
    if let Some(result) = context.pre_executed.get(&selected_idx).cloned() {
        pre_executed.insert(0, result);
    }
    (vec![selected_call], pre_executed)
}

fn candidate_set_from_tool_calls(tool_calls: &[ToolCall]) -> CandidateActionSet {
    CandidateActionSet {
        candidate_actions: tool_calls
            .iter()
            .map(|tool_call| CandidateAction {
                id: tool_call.id.clone(),
                action_type: "tool_call".to_string(),
                tool: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
                reason: "model-proposed tool call".to_string(),
                expected_observation: None,
                model_scores: None,
                model_factors: None,
                evidence: Vec::new(),
            })
            .collect(),
    }
}

fn candidate_gate_triggered(task_bundle: &TaskContextBundle) -> bool {
    let state = &task_bundle.agent_state;
    // Score-only signals stay advisory. Gated candidate reduction is reserved
    // for explicit revision/uncertainty loops and still preserves model order.
    state.repeated_revised_action_count() >= 1 || state.uncertainty_not_reduced_steps >= 2
}

#[cfg(test)]
mod tests {
    use super::super::turn_state::TurnLoopStateController;
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
        let resource_policy = ResourcePolicy::from_route(route);

        TurnModelStepController::run(TurnModelStepContext {
            conversation,
            iteration: 1,
            route,
            profile: MainLoopProfile::from_turn(route, &[]),
            code_workflow: &code_workflow,
            task_bundle: &task_bundle,
            required_validation_commands: &[],
            turn_retrieval_context: None,
            focused_repair_prompt: None,
            tools: &[],
            exposed_tool_names: &exposed_tool_names,
            resource_policy: &resource_policy,
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
            tool_call_repair: None,
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
            tool_call_repair: None,
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

    #[test]
    fn gated_candidate_ranking_preserves_model_order_after_trigger() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.set("PRIORITY_AGENT_CANDIDATE_ACTIONS", "gated");

        let route = IntentRouter::new().route("修改 src/lib.rs");
        let mut task_bundle = crate::engine::task_context::TaskContextBundle::new(
            "修改 src/lib.rs",
            ".",
            route.clone(),
            None,
        );
        task_bundle.agent_state.uncertainty_not_reduced_steps = 2;
        let turn_state = TurnRuntimeState::new(true);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "candidate ranking"));
        let tool_calls = vec![
            ToolCall {
                id: "edit".to_string(),
                name: "file_edit".to_string(),
                arguments: serde_json::json!({"path": "src/lib.rs"}),
            },
            ToolCall {
                id: "read".to_string(),
                name: "file_read".to_string(),
                arguments: serde_json::json!({"path": "src/lib.rs"}),
            },
        ];
        let exposed = HashSet::from(["file_edit".to_string(), "file_read".to_string()]);

        let (ranked, pre_executed) =
            evaluate_candidate_actions_for_tool_round(CandidateActionRoundContext {
                content: "choose a tool",
                tool_calls,
                pre_executed: HashMap::new(),
                route: &route,
                task_bundle: &task_bundle,
                turn_state: &turn_state,
                exposed_tool_names: &exposed,
                trace: &trace,
            });

        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].name, "file_edit");
        assert!(pre_executed.is_empty());
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::CandidateActionsEvaluated {
                mode,
                selected_tool: Some(tool),
                ..
            } if mode == "gated" && tool == "file_edit"
        )));
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
        assert_eq!(
            turn_state.runtime_diet.validation_evidence,
            "api_error:transient_transport"
        );
        let finished = trace.finish(TurnStatus::Failed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::Error { message } if message == "[transient_transport] provider down"
        )));
    }
}

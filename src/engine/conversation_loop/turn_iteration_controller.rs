use super::turn_focused_repair_flow_controller::{
    TurnFocusedRepairFlow, TurnFocusedRepairFlowContext, TurnFocusedRepairFlowController,
};
use super::turn_iteration_setup_controller::{
    TurnIterationSetupContext, TurnIterationSetupController, TurnIterationSetupFlow,
};
use super::turn_loop_state_controller::TurnLoopState;
use super::turn_model_step_controller::{
    TurnModelStepContext, TurnModelStepController, TurnModelStepFlow,
};
use super::turn_post_change_closeout_controller::{
    TurnPostChangeCloseoutContext, TurnPostChangeCloseoutController, TurnPostChangeCloseoutFlow,
};
use super::turn_runtime_context::TurnRuntimeContext;
use super::turn_runtime_state::TurnRuntimeState;
use super::turn_tool_failure_followup_controller::{
    TurnToolFailureFollowupContext, TurnToolFailureFollowupController, TurnToolFailureFollowupFlow,
};
use super::turn_tool_round_step_controller::{
    TurnToolRoundStepContext, TurnToolRoundStepController,
};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, Tool};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub(super) struct TurnIterationContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) iteration: usize,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) turn_retrieval_context: Option<&'a RetrievalContext>,
    pub(super) base_tools: &'a [Tool],
    pub(super) loop_state: &'a mut TurnLoopState,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) no_diff_audit_closeout_allowed: bool,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) working_dir: &'a Path,
    pub(super) last_user_preview: &'a str,
    pub(super) required_validation_commands: &'a [String],
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) enum TurnIterationFlow {
    Continue,
    Break,
}

pub(super) struct TurnIterationController;

impl TurnIterationController {
    pub(super) async fn run(
        context: TurnIterationContext<'_>,
    ) -> anyhow::Result<TurnIterationFlow> {
        let exposure_plan = match TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: context.iteration,
            max_iterations: context.conversation.max_iterations,
            turn_state: &mut *context.turn_state,
            memory_manager: context.conversation.memory_manager.as_ref(),
            trace: context.trace,
            route_workflow: context.route.workflow,
            baseline_git_status_files: context.baseline_git_status_files,
            base_tools: context.base_tools,
        })
        .await
        {
            TurnIterationSetupFlow::Continue { exposure_plan } => exposure_plan,
            TurnIterationSetupFlow::Stop => return Ok(TurnIterationFlow::Break),
        };
        let tools = exposure_plan.tools;
        let exposed_tool_names = exposure_plan.exposed_tool_names;

        let (content, tool_calls, pre_executed) =
            match TurnModelStepController::run(TurnModelStepContext {
                conversation: context.conversation,
                iteration: context.iteration + 1,
                route: context.route,
                code_workflow: &*context.code_workflow,
                turn_retrieval_context: context.turn_retrieval_context,
                focused_repair_prompt: exposure_plan.focused_repair_prompt,
                tools: &tools,
                exposed_tool_names: &exposed_tool_names,
                loop_state: &mut *context.loop_state,
                turn_state: &mut *context.turn_state,
                messages: &mut *context.messages,
                trace: context.trace,
                tx: context.tx,
            })
            .await?
            {
                TurnModelStepFlow::Retry => return Ok(TurnIterationFlow::Continue),
                TurnModelStepFlow::Finish => return Ok(TurnIterationFlow::Break),
                TurnModelStepFlow::ToolRound {
                    content,
                    tool_calls,
                    pre_executed,
                } => (content, tool_calls, pre_executed),
            };

        let mut tool_round_state = TurnToolRoundStepController::run(TurnToolRoundStepContext {
            conversation: context.conversation,
            content: &content,
            tool_calls: &tool_calls,
            pre_executed,
            runtime: TurnRuntimeContext {
                tx: context.tx,
                trace: context.trace,
                route: context.route,
                resource_policy: context.resource_policy,
                exposed_tool_names: &exposed_tool_names,
                working_dir: context.working_dir,
                last_user_preview: context.last_user_preview,
                required_validation_commands: context.required_validation_commands,
                destructive_scope: context.destructive_scope,
                baseline_git_status_files: context.baseline_git_status_files,
            },
            turn_state: &mut *context.turn_state,
            messages: &mut *context.messages,
            is_programming_workflow: crate::engine::code_change_workflow::is_programming_workflow(
                context.route.workflow,
            ),
            loop_state: &mut *context.loop_state,
        })
        .await;

        match TurnFocusedRepairFlowController::run(TurnFocusedRepairFlowContext {
            conversation: context.conversation,
            workflow: context.route.workflow,
            no_diff_audit_closeout_allowed: context.no_diff_audit_closeout_allowed,
            code_write_tools_forbidden: context.code_write_tools_forbidden,
            trace: context.trace,
            code_workflow: &mut *context.code_workflow,
            turn_state: &mut *context.turn_state,
            round_state: &mut tool_round_state,
            exposed_tool_names: &exposed_tool_names,
            tx: context.tx,
            resource_policy: context.resource_policy,
            destructive_scope: context.destructive_scope,
            baseline_git_status_files: context.baseline_git_status_files,
            last_user_preview: context.last_user_preview,
            messages: &mut *context.messages,
            final_content: &mut context.loop_state.final_content,
            final_tool_calls: &mut context.loop_state.final_tool_calls,
        })
        .await
        {
            TurnFocusedRepairFlow::Continue => return Ok(TurnIterationFlow::Continue),
            TurnFocusedRepairFlow::Stop => return Ok(TurnIterationFlow::Break),
            TurnFocusedRepairFlow::Proceed => {}
        }

        match TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: context.conversation.provider.as_ref(),
            model: context.conversation.model.clone(),
            session_store: context.conversation.session_store.as_ref(),
            session_id: &context.conversation.session_id,
            trace: context.trace,
            any_tool_success: tool_round_state.any_tool_success,
            last_user_preview: context.last_user_preview,
            task_bundle: &mut *context.task_bundle,
            round_state: &mut tool_round_state,
            failed_tool_names: &context.loop_state.failed_tool_names,
            tx: context.tx,
            final_content: &mut context.loop_state.final_content,
            messages: &mut *context.messages,
        })
        .await
        {
            TurnToolFailureFollowupFlow::Continue => {}
            TurnToolFailureFollowupFlow::Stop => return Ok(TurnIterationFlow::Break),
        }

        match TurnPostChangeCloseoutController::run(TurnPostChangeCloseoutContext {
            conversation: context.conversation,
            trace: context.trace,
            route: context.route,
            code_workflow: &mut *context.code_workflow,
            task_bundle: &mut *context.task_bundle,
            round_state: &mut tool_round_state,
            required_validation_commands: context.required_validation_commands,
            successful_required_validation_commands: &mut context
                .loop_state
                .successful_required_validation_commands,
            turn_state: &mut *context.turn_state,
            final_content: &mut context.loop_state.final_content,
            messages: &mut *context.messages,
            last_user_preview: context.last_user_preview,
        })
        .await
        {
            TurnPostChangeCloseoutFlow::Continue => Ok(TurnIterationFlow::Continue),
            TurnPostChangeCloseoutFlow::Break => Ok(TurnIterationFlow::Break),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::turn_loop_state_controller::TurnLoopStateController;
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::TurnTrace;
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

    fn conversation(response: ChatResponse) -> ConversationLoop {
        ConversationLoop::new(
            Arc::new(MockProvider {
                responses: StdMutex::new(VecDeque::from(vec![Ok(response)])),
            }),
            Arc::new(ToolRegistry::new()),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "mock-model".to_string(),
        )
    }

    #[tokio::test]
    async fn plain_model_response_breaks_iteration() {
        let conversation = conversation(ChatResponse {
            content: "done".to_string(),
            tool_calls: None,
            usage: None,
        });
        let route = IntentRouter::new().route("hello");
        let resource_policy = ResourcePolicy::from_route(&route);
        let working_dir = std::env::current_dir().expect("current dir");
        let destructive_scope = DestructiveScopeContract::from_user_request("hello", &working_dir);
        let mut task_bundle = TaskContextBundle::new("hello", &working_dir, route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let mut turn_state = TurnRuntimeState::new(true);
        let mut loop_state = TurnLoopStateController::initial_state();
        let mut messages = vec![Message::user("hello")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "hello"));
        let base_tools = Vec::new();
        let baseline_git_status_files = HashSet::new();

        let flow = TurnIterationController::run(TurnIterationContext {
            conversation: &conversation,
            iteration: 0,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            turn_retrieval_context: None,
            base_tools: &base_tools,
            loop_state: &mut loop_state,
            turn_state: &mut turn_state,
            no_diff_audit_closeout_allowed: false,
            code_write_tools_forbidden: false,
            resource_policy: &resource_policy,
            working_dir: &working_dir,
            last_user_preview: "hello",
            required_validation_commands: &[],
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline_git_status_files,
            messages: &mut messages,
            trace: &trace,
            tx: None,
        })
        .await
        .expect("iteration");

        assert!(matches!(flow, TurnIterationFlow::Break));
        assert_eq!(loop_state.final_content, "done");
        assert_eq!(turn_state.iterations_used, 1);
        assert!(!loop_state.tool_calls_made);
    }
}

use super::post_change_workflow_controller::{
    PostChangeWorkflowContext, PostChangeWorkflowController,
};
use super::tool_failure_guided_debugging::{
    TurnToolFailureFollowupContext, TurnToolFailureFollowupController,
};
use super::turn_focused_repair_flow_controller::{
    TurnFocusedRepairFlow, TurnFocusedRepairFlowContext, TurnFocusedRepairFlowController,
};
use super::turn_iteration_closeout_controller::{
    TurnIterationCloseoutContext, TurnIterationCloseoutController,
};
use super::turn_iteration_setup_controller::{
    TurnIterationSetupContext, TurnIterationSetupController, TurnIterationSetupFlow,
};
use super::turn_model_step_controller::{
    TurnModelStepContext, TurnModelStepController, TurnModelStepFlow,
};
use super::turn_state::{TurnLoopState, TurnRuntimeContext, TurnRuntimeState};
use super::turn_tool_round_step_controller::{
    TurnToolRoundStepContext, TurnToolRoundStepController,
};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::conversation_loop::turn_loop_policy::MainLoopProfile;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::stop_checker::{StopCheckInput, StopChecker};
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::{
    mva_stage_transition_policy, AgentToolRoundObservation, TaskContextBundle,
};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, Tool};
use crate::tools::ToolContextRetainedContext;
use std::collections::HashSet;

enum TurnPostChangeCloseoutFlow {
    Continue,
    Break,
}

struct TurnPostChangeCloseoutContext<'a> {
    conversation: &'a ConversationLoop,
    trace: &'a TraceCollector,
    route: &'a IntentRoute,
    code_workflow: &'a mut CodeChangeWorkflowRunner,
    task_bundle: &'a mut TaskContextBundle,
    round_state: &'a mut super::turn_tool_round_step_controller::TurnToolRoundState,
    required_validation_commands: &'a [String],
    successful_required_validation_commands: &'a mut HashSet<String>,
    turn_state: &'a mut TurnRuntimeState,
    final_content: &'a mut String,
    messages: &'a mut Vec<Message>,
    last_user_preview: &'a str,
}

struct TurnPostChangeCloseoutController;

impl TurnPostChangeCloseoutController {
    async fn run(context: TurnPostChangeCloseoutContext<'_>) -> TurnPostChangeCloseoutFlow {
        let post_change_workflow = PostChangeWorkflowController::run(PostChangeWorkflowContext {
            conversation: context.conversation,
            trace: context.trace,
            route: context.route,
            code_workflow: context.code_workflow,
            task_bundle: context.task_bundle,
            changed_files: &context.round_state.changed_files,
            required_validation_commands: context.required_validation_commands,
            successful_validation_commands: &context.round_state.successful_validation_commands,
            successful_required_validation_commands: context
                .successful_required_validation_commands,
            turn_state: context.turn_state,
            should_closeout_after_verified_change: context
                .round_state
                .should_closeout_after_verified_change,
            final_content: &mut *context.final_content,
            tool_results_text: &mut context.round_state.tool_results_text,
            messages: &mut *context.messages,
            last_user_preview: context.last_user_preview,
        })
        .await;

        context.round_state.should_closeout_after_verified_change =
            post_change_workflow.should_closeout_after_verified_change;

        if post_change_workflow.break_loop {
            return TurnPostChangeCloseoutFlow::Break;
        }

        let iteration_closeout =
            TurnIterationCloseoutController::run(TurnIterationCloseoutContext {
                conversation: context.conversation,
                trace: context.trace,
                messages: &*context.messages,
                final_content: &*context.final_content,
                tool_results_text: &context.round_state.tool_results_text,
                should_closeout_after_verified_change: context
                    .round_state
                    .should_closeout_after_verified_change,
            })
            .await;

        if iteration_closeout.break_loop {
            TurnPostChangeCloseoutFlow::Break
        } else {
            TurnPostChangeCloseoutFlow::Continue
        }
    }
}
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub(super) struct TurnIterationContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) iteration: usize,
    pub(super) route: &'a IntentRoute,
    pub(super) profile: MainLoopProfile,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) turn_retrieval_context: Option<&'a RetrievalContext>,
    pub(super) retained_context: &'a ToolContextRetainedContext,
    pub(super) base_tools: &'a [Tool],
    pub(super) available_tools: &'a [Tool],
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
        // Force-summary: inject wrap-up prompt before the last 2 iterations
        // so the model has time to stop calling tools and produce a final answer.
        // Mirrors Reasonix's forceSummaryAfterIterLimit pattern.
        if crate::engine::conversation_loop::turn_loop_policy::should_force_summary(
            context.iteration,
            context.conversation.max_iterations,
        ) && context.loop_state.final_content.is_empty()
        {
            let summary_msg =
                crate::engine::conversation_loop::turn_loop_policy::force_summary_message();
            context.messages.push(summary_msg);
            context.trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "iteration {} of {} — injecting force-summary prompt",
                    context.iteration + 1,
                    context.conversation.max_iterations
                ),
            });
        }

        let TurnIterationSetupFlow::Continue { exposure_plan } =
            TurnIterationSetupController::run(TurnIterationSetupContext {
                iteration: context.iteration,
                max_iterations: context.conversation.max_iterations,
                turn_state: &mut *context.turn_state,
                memory_manager: context.conversation.memory_manager.as_ref(),
                base_tools: context.base_tools,
                available_tools: context.available_tools,
            })
            .await;
        let tools = exposure_plan.tools;
        let exposed_tool_names = exposure_plan.exposed_tool_names;

        let (content, tool_calls, pre_executed) =
            match TurnModelStepController::run(TurnModelStepContext {
                conversation: context.conversation,
                iteration: context.iteration + 1,
                route: context.route,
                profile: context.profile,
                code_workflow: &*context.code_workflow,
                task_bundle: &*context.task_bundle,
                required_validation_commands: context.required_validation_commands,
                turn_retrieval_context: context.turn_retrieval_context,
                focused_repair_prompt: exposure_plan.focused_repair_prompt,
                tools: &tools,
                exposed_tool_names: &exposed_tool_names,
                resource_policy: context.resource_policy,
                loop_state: &mut *context.loop_state,
                turn_state: &mut *context.turn_state,
                messages: &mut *context.messages,
                trace: context.trace,
                tx: context.tx,
            })
            .await?
            {
                TurnModelStepFlow::Retry => {
                    return Ok(TurnIterationFlow::Continue);
                }
                TurnModelStepFlow::Finish => {
                    if context.loop_state.final_content.trim().is_empty() {
                        // Empty content is always bad — prompt the model.
                        if context.iteration < context.conversation.max_iterations {
                            context.messages.push(
                                super::request_preparation_controller::recent_observation_message(
                                    "Your last response was empty. Please summarize what you \
                                     have learned from the tool results above in a few \
                                     sentences, or call tools if you need more information.",
                                ),
                            );
                            context.trace.record(TraceEvent::WorkflowFallback {
                                error: "empty assistant response — injecting retry prompt"
                                    .to_string(),
                            });
                            return Ok(TurnIterationFlow::Continue);
                        }
                        return Ok(TurnIterationFlow::Break);
                    }
                    return Ok(TurnIterationFlow::Break);
                }
                TurnModelStepFlow::ToolRound {
                    content,
                    tool_calls,
                    pre_executed,
                } => {
                    (content, tool_calls, pre_executed)
                }
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
                task_stage: context.task_bundle.agent_state.stage,
                exposed_tool_names: &exposed_tool_names,
                working_dir: context.working_dir,
                last_user_preview: context.last_user_preview,
                required_validation_commands: context.required_validation_commands,
                destructive_scope: context.destructive_scope,
                baseline_git_status_files: context.baseline_git_status_files,
                retained_context: context.retained_context,
            },
            turn_state: &mut *context.turn_state,
            task_bundle: &mut *context.task_bundle,
            messages: &mut *context.messages,
            is_programming_workflow: crate::engine::code_change_workflow::is_programming_workflow(
                context.route.workflow,
            ),
            loop_state: &mut *context.loop_state,
        })
        .await;
        context
            .task_bundle
            .agent_state
            .observe_tool_round(AgentToolRoundObservation {
                any_tool_success: tool_round_state.any_tool_success,
                batch_has_unsuccessful_tools: tool_round_state.batch_has_unsuccessful_tools,
                used_write_tool: tool_round_state.used_write_tool,
                successful_write_tool: tool_round_state.successful_write_tool,
                has_worktree_changes: tool_round_state.has_worktree_changes(),
                has_successful_validation_commands: tool_round_state
                    .has_successful_validation_commands(),
                failed_tool_evidence_present: tool_round_state.failed_tool_evidence_present(),
            });
        record_stop_check(
            context.trace,
            context.task_bundle,
            context.turn_state,
            &tool_round_state,
            exposed_tool_names.len(),
            tool_calls.len(),
            false,
        );

        let focused_repair_flow =
            TurnFocusedRepairFlowController::run(TurnFocusedRepairFlowContext {
                workflow: context.route.workflow,
                no_diff_audit_closeout_allowed: context.no_diff_audit_closeout_allowed,
                code_write_tools_forbidden: context.code_write_tools_forbidden,
                trace: context.trace,
                code_workflow: &mut *context.code_workflow,
                turn_state: &mut *context.turn_state,
                round_state: &mut tool_round_state,
                messages: &mut *context.messages,
            })
            .await;
        record_stop_check(
            context.trace,
            context.task_bundle,
            context.turn_state,
            &tool_round_state,
            exposed_tool_names.len(),
            tool_calls.len(),
            matches!(focused_repair_flow, TurnFocusedRepairFlow::Continue),
        );
        // ── Advisory-only post-tool checks ──
        // Reasonix alignment: only 4 hard-stop conditions exist:
        // 1. Budget exhausted → force summary
        // 2. User abort (handled upstream)
        // 3. No valid tool calls → finish the turn
        // 4. API error
        //
        // All other checks below are advisory — they record traces and
        // update task state but NEVER break the loop. The iteration budget
        // + force summary are the ultimate safety net.
        if matches!(focused_repair_flow, TurnFocusedRepairFlow::Continue) {
            return Ok(TurnIterationFlow::Continue);
        }

        TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: context.conversation.provider.as_ref(),
            model: context.conversation.model.clone(),
            session_store: context.conversation.session_store.as_ref(),
            session_id: &context.conversation.session_id,
            trace: context.trace,
            any_tool_success: tool_round_state.any_tool_success,
            last_user_preview: context.last_user_preview,
            task_bundle: &mut *context.task_bundle,
            round_state: &mut tool_round_state,
            messages: &mut *context.messages,
        })
        .await;

        let closeout_flow = TurnPostChangeCloseoutController::run(TurnPostChangeCloseoutContext {
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
        .await;
        if matches!(closeout_flow, TurnPostChangeCloseoutFlow::Break) {
            return Ok(TurnIterationFlow::Break);
        }

        Ok(TurnIterationFlow::Continue)
    }
}

fn record_stop_check(
    trace: &TraceCollector,
    task_bundle: &mut TaskContextBundle,
    turn_state: &TurnRuntimeState,
    tool_round_state: &super::turn_tool_round_step_controller::TurnToolRoundState,
    exposed_tool_count: usize,
    selected_tool_calls: usize,
    force_patch_synthesis_after_no_change: bool,
) {
    let stage_before = task_bundle.agent_state.stage;
    let observations_before = task_bundle.agent_state.observations.len();
    let key_findings_before = task_bundle.agent_state.key_findings.len();
    let duplicate_read_only_tools =
        if crate::engine::code_change_workflow::is_programming_workflow(task_bundle.route.workflow)
            || !task_bundle
                .agent_state
                .verification_plan
                .required_checks
                .is_empty()
        {
            0
        } else {
            tool_round_state.duplicate_successful_read_only_tools.len()
        };
    let decision = StopChecker::evaluate(StopCheckInput {
        any_tool_success: tool_round_state.any_tool_success,
        successful_write_tool: tool_round_state.successful_write_tool,
        has_successful_validation_commands: tool_round_state.has_successful_validation_commands(),
        no_code_progress_rounds: turn_state.focused_repair.no_code_progress_rounds,
        action_checkpoint_active: turn_state.focused_repair.action_checkpoint_active,
        action_checkpoint_no_change_rounds: turn_state
            .focused_repair
            .action_checkpoint_no_change_rounds,
        force_patch_synthesis_after_no_change,
        repeated_failed_tools: tool_round_state.repeated_failed_tools.len(),
        duplicate_read_only_tools,
        max_iterations_reached: false,
        uncertainty_not_reduced_steps: task_bundle.agent_state.uncertainty_not_reduced_steps,
        consecutive_validation_failures: task_bundle.agent_state.consecutive_validation_failures,
        consecutive_edit_failures: task_bundle.agent_state.consecutive_edit_failures,
        consecutive_command_failures: task_bundle.agent_state.consecutive_command_failures,
        consecutive_permission_blocks: task_bundle.agent_state.consecutive_permission_blocks,
        consecutive_low_action_scores: task_bundle.agent_state.consecutive_low_action_scores(),
        consecutive_high_risk_low_value_actions: task_bundle
            .agent_state
            .consecutive_high_risk_low_value_actions(),
        score_without_uncertainty_reduction_rounds: task_bundle
            .agent_state
            .score_without_uncertainty_reduction_rounds(),
        repeated_revised_action_count: task_bundle.agent_state.repeated_revised_action_count(),
        user_interrupted: false,
        model_output_invalid_attempts: 0,
        action_review_decision: None,
        action_review_reason: None,
        rollback_candidate: task_bundle.agent_state.rollback_candidates.last().cloned(),
        failure_type: task_bundle.agent_state.last_failure_family.clone(),
        recovery_plan_id: None,
    });
    StopChecker::apply_to_task_state(&mut task_bundle.agent_state, &decision);
    let stage_after = task_bundle.agent_state.stage;
    let observations_delta = task_bundle
        .agent_state
        .observations
        .len()
        .saturating_sub(observations_before);
    let key_findings_delta = task_bundle
        .agent_state
        .key_findings
        .len()
        .saturating_sub(key_findings_before);
    trace.record(TraceEvent::StopCheckEvaluated {
        status: decision.status.label().to_string(),
        reason: decision.reason.label().to_string(),
        stage: format!("{:?}", task_bundle.agent_state.stage),
        terminal_status: decision
            .terminal_status
            .map(|status| status.label().to_string()),
        action: decision.action.label().to_string(),
        no_code_progress_rounds: decision.no_code_progress_rounds,
        action_checkpoint_active: decision.action_checkpoint_active,
        summary: decision.summary,
        evidence_items: decision.evidence.len(),
        failure_type: decision.failure_type.clone(),
        recovery_plan_id: decision.recovery_plan_id.clone(),
        rollback_recommended: decision.rollback_candidate.is_some(),
        next_action: decision.next_action.clone(),
    });
    let latest_action_score = task_bundle
        .agent_state
        .action_score_history
        .last()
        .map(|record| record.action_score);
    trace.record(TraceEvent::AgentLoopStepEvaluated {
        route_workflow: serde_label(&task_bundle.route.workflow),
        route_risk: serde_label(&task_bundle.route.risk),
        task_mode: serde_label(&task_bundle.agent_state.mode),
        stage_before: format!("{stage_before:?}"),
        stage_after: format!("{stage_after:?}"),
        mva_stage_before: stage_before.mva_stage_label().to_string(),
        mva_stage_after: stage_after.mva_stage_label().to_string(),
        stage_transition_policy: mva_stage_transition_policy(stage_before, stage_after).to_string(),
        exposed_tools: exposed_tool_count,
        selected_tool_calls,
        action_score_records: task_bundle.agent_state.action_score_history.len(),
        latest_action_score,
        observations_delta,
        key_findings_delta,
        stop_status: decision.status.label().to_string(),
        stop_reason: decision.reason.label().to_string(),
        stop_action: decision.action.label().to_string(),
        terminal_status: decision
            .terminal_status
            .map(|status| status.label().to_string()),
        state_delta: format!(
            "stage_changed={} observations_delta={} key_findings_delta={} latest_progress={}",
            stage_before != stage_after,
            observations_delta,
            key_findings_delta,
            task_bundle
                .agent_state
                .last_progress_signal
                .as_deref()
                .unwrap_or("none")
        ),
    });
}

fn serde_label<T>(value: &T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use super::super::turn_state::TurnLoopStateController;
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::{
        IntentKind, IntentRoute, IntentRouter, ReasoningPolicy, RetrievalPolicy, RiskLevel,
        WorkflowKind,
    };
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
            tool_call_repair: None,
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
        let available_tools = Vec::new();
        let baseline_git_status_files = HashSet::new();
        let retained_context = crate::tools::ToolContextRetainedContext::default();

        let flow = TurnIterationController::run(TurnIterationContext {
            conversation: &conversation,
            iteration: 0,
            route: &route,
            profile: MainLoopProfile::from_turn(&route, &[]),
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            turn_retrieval_context: None,
            retained_context: &retained_context,
            base_tools: &base_tools,
            available_tools: &available_tools,
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

    #[test]
    fn stop_check_ignores_duplicate_read_only_for_code_change() {
        let route = IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.95,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk: RiskLevel::Medium,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "bug fix".to_string(),
        };
        let mut task_bundle = TaskContextBundle::new("fix slugify", ".", route, None);
        let turn_state = TurnRuntimeState::new(true);
        let round_state = super::super::turn_tool_round_step_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: vec!["file_read".to_string()],
            should_closeout_after_verified_change: false,
        };
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix slugify"));

        record_stop_check(
            &trace,
            &mut task_bundle,
            &turn_state,
            &round_state,
            4,
            1,
            false,
        );

        let stop_check = task_bundle
            .agent_state
            .stop_checks
            .last()
            .expect("stop check");
        assert_ne!(
            stop_check.reason,
            crate::engine::task_context::StopCheckReason::DuplicateReadOnly
        );
    }

    #[test]
    fn stop_check_records_no_issue_for_no_progress_score_only_signal() {
        let route = IntentRouter::new().route("fix src/main.rs");
        let mut task_bundle = TaskContextBundle::new("fix src/main.rs", ".", route, None);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.focused_repair.no_code_progress_rounds = 2;
        turn_state.focused_repair.action_checkpoint_active = true;
        let round_state = super::super::turn_tool_round_step_controller::TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: Vec::new(),
            should_closeout_after_verified_change: false,
        };
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix src/main.rs"));

        record_stop_check(
            &trace,
            &mut task_bundle,
            &turn_state,
            &round_state,
            4,
            1,
            false,
        );

        let stop_check = task_bundle
            .agent_state
            .stop_checks
            .last()
            .expect("stop check");
        assert_eq!(
            stop_check.status,
            crate::engine::task_context::StopCheckStatus::Continue
        );
        assert_eq!(
            stop_check.reason,
            crate::engine::task_context::StopCheckReason::NoIssue
        );
        assert_eq!(
            task_bundle.agent_state.stage,
            crate::engine::task_context::AgentTaskStage::Understand
        );

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::StopCheckEvaluated {
                status,
                reason,
                no_code_progress_rounds: 2,
                action_checkpoint_active: true,
                ..
            } if status == "continue" && reason == "no_issue"
        )));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AgentLoopStepEvaluated {
                stage_before,
                stage_after,
                selected_tool_calls: 1,
                ..
            } if stage_before == "Understand" && stage_after == "Understand"
        )));
    }

    fn round_state(
        should_closeout_after_verified_change: bool,
    ) -> super::super::turn_tool_round_step_controller::TurnToolRoundState {
        use std::path::PathBuf;
        super::super::turn_tool_round_step_controller::TurnToolRoundState {
            tool_results_text: "tool output".to_string(),
            changed_files: Vec::<PathBuf>::new(),
            batch_has_unsuccessful_tools: false,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success: false,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: Vec::new(),
            should_closeout_after_verified_change,
        }
    }

    async fn run_no_change_closeout(
        round_state: &mut super::super::turn_tool_round_step_controller::TurnToolRoundState,
        trace: &TraceCollector,
    ) -> TurnPostChangeCloseoutFlow {
        let conversation = conversation(ChatResponse {
            content: String::new(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
        });
        let route = IntentRouter::new().route("finish the change");
        let mut task_bundle = TaskContextBundle::new("finish the change", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let mut successful_required_validation_commands = HashSet::new();
        let mut turn_state = TurnRuntimeState::new(true);
        let mut final_content = "done".to_string();
        let mut messages = vec![Message::user("finish the change")];
        let required_validation_commands = Vec::new();

        TurnPostChangeCloseoutController::run(TurnPostChangeCloseoutContext {
            conversation: &conversation,
            trace,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            round_state,
            required_validation_commands: &required_validation_commands,
            successful_required_validation_commands: &mut successful_required_validation_commands,
            turn_state: &mut turn_state,
            final_content: &mut final_content,
            messages: &mut messages,
            last_user_preview: "finish the change",
        })
        .await
    }

    #[tokio::test]
    async fn no_changed_files_continue_without_closeout_flag() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "finish the change"));
        let mut round_state = round_state(false);

        let flow = run_no_change_closeout(&mut round_state, &trace).await;

        assert!(matches!(flow, TurnPostChangeCloseoutFlow::Continue));
        assert!(!round_state.should_closeout_after_verified_change);
        assert_eq!(round_state.tool_results_text, "tool output");
    }

    #[tokio::test]
    async fn no_changed_files_break_when_closeout_flag_already_set() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "finish the change"));
        let mut round_state = round_state(true);

        let flow = run_no_change_closeout(&mut round_state, &trace).await;

        assert!(matches!(flow, TurnPostChangeCloseoutFlow::Break));
        assert!(round_state.should_closeout_after_verified_change);

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "verified code change passed validation; preparing deterministic closeout"
        )));
    }
}

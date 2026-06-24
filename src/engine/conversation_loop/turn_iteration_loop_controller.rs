//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::turn_iteration_controller::{
    TurnIterationContext, TurnIterationController, TurnIterationFlow,
};
use super::turn_state::TurnLoopState;
use super::turn_state::TurnRuntimeState;
use super::workflow_change_tracker::WorkflowChangeTracker;
use super::ConversationLoop;
use crate::engine::code_change_workflow::{CodeChangeWorkflowRunner, StageValidationStatus};
use crate::engine::conversation_loop::turn_loop_policy::MainLoopProfile;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, Tool};
use crate::tools::ToolContextRetainedContext;
use std::path::Path;
use tokio::sync::mpsc;

pub(super) struct TurnIterationLoopContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
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
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) struct TurnIterationLoopController;

impl TurnIterationLoopController {
    pub(super) async fn run(context: TurnIterationLoopContext<'_>) -> anyhow::Result<()> {
        let max_loop_iterations = context.profile.max_loop_iterations(
            context.conversation.max_iterations,
            context.code_workflow.max_repair_attempts(),
        );
        let baseline_git_status_files = WorkflowChangeTracker::git_status_files();

        for iteration in 0..max_loop_iterations {
            // Force summary: inject wrap-up instruction in the last 2 iterations
            // so the model produces a final summary instead of looping or failing.
            if super::turn_loop_policy::should_force_summary(iteration, max_loop_iterations) {
                let msg = super::turn_loop_policy::force_summary_message();
                context.messages.push(msg);
            }

            match TurnIterationController::run(TurnIterationContext {
                conversation: context.conversation,
                iteration,
                route: context.route,
                profile: context.profile,
                code_workflow: &mut *context.code_workflow,
                task_bundle: &mut *context.task_bundle,
                turn_retrieval_context: context.turn_retrieval_context,
                retained_context: context.retained_context,
                base_tools: context.base_tools,
                available_tools: context.available_tools,
                loop_state: &mut *context.loop_state,
                turn_state: &mut *context.turn_state,
                no_diff_audit_closeout_allowed: context.no_diff_audit_closeout_allowed,
                code_write_tools_forbidden: context.code_write_tools_forbidden,
                resource_policy: context.resource_policy,
                working_dir: context.working_dir,
                last_user_preview: context.last_user_preview,
                required_validation_commands: context.required_validation_commands,
                destructive_scope: context.destructive_scope,
                baseline_git_status_files: &baseline_git_status_files,
                messages: &mut *context.messages,
                trace: context.trace,
                tx: context.tx,
            })
            .await?
            {
                TurnIterationFlow::Continue => continue,
                TurnIterationFlow::Break => break,
            }
        }

        let needs_forced_closeout_summary = needs_forced_closeout_summary(
            context.loop_state,
            context.code_workflow,
            context.task_bundle,
        );

        tracing::debug!(
            tool_calls_made = context.loop_state.tool_calls_made,
            final_content_len = context.loop_state.final_content.len(),
            needs_forced_closeout_summary,
            "turn iteration loop exited"
        );

        if needs_forced_closeout_summary {
            let summary = super::turn_loop_policy::force_summary_after_iter_limit(
                super::turn_loop_policy::ForceSummaryAfterLimitContext {
                    provider: context.conversation.provider.clone(),
                    model: &context.conversation.model,
                    messages: context.messages,
                    trace: context.trace,
                    tx: context.tx,
                    cost_tracker: &context.conversation.cost_tracker,
                    reason: super::turn_loop_policy::ForceSummaryReason::Stuck,
                },
            )
            .await;
            tracing::debug!(
                summary_len = summary.len(),
                "forced closeout summary completed"
            );
            if !summary.trim().is_empty() {
                context.loop_state.final_content.clear();
                context.loop_state.final_content.push_str(&summary);
                if let Some(tx) = context.tx {
                    let _ = tx.send(StreamEvent::TextChunk(summary)).await;
                }
            } else {
                tracing::debug!("forced closeout summary returned empty");
            }
        }

        Ok(())
    }
}

fn workflow_has_verified_closeout(
    code_workflow: &CodeChangeWorkflowRunner,
    task_bundle: &TaskContextBundle,
) -> bool {
    code_workflow
        .build_closeout(task_bundle)
        .is_some_and(|closeout| matches!(closeout.status, StageValidationStatus::Passed))
}

fn needs_forced_closeout_summary(
    loop_state: &TurnLoopState,
    code_workflow: &CodeChangeWorkflowRunner,
    task_bundle: &TaskContextBundle,
) -> bool {
    loop_state.tool_calls_made
        && !workflow_has_verified_closeout(code_workflow, task_bundle)
        && (loop_state.final_content.trim().is_empty()
            || super::assistant_response_retry_controller::is_continuation_only_response(
                &loop_state.final_content,
            ))
}

#[cfg(test)]
mod tests {
    use super::super::turn_state::TurnLoopStateController;
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::TurnTrace;
    use crate::engine::workflow_contract::{
        AcceptanceConfidence, AcceptanceNextAction, AcceptanceReview,
    };
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::VecDeque;
    use std::path::PathBuf;
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
    async fn plain_model_response_breaks_loop() {
        let conversation = conversation(ChatResponse {
            content: "done".to_string(),
            tool_calls: None,
            usage: None,
            tool_call_repair: None,
            finish_reason: None,
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
        let retained_context = ToolContextRetainedContext::default();

        TurnIterationLoopController::run(TurnIterationLoopContext {
            conversation: &conversation,
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
            messages: &mut messages,
            trace: &trace,
            tx: None,
        })
        .await
        .expect("iteration loop");

        assert_eq!(loop_state.final_content, "done");
        assert_eq!(turn_state.iterations_used, 1);
        assert!(!loop_state.tool_calls_made);
    }

    #[test]
    fn verified_workflow_closeout_skips_forced_summary() {
        let route = IntentRouter::new().route("fix the bug in src/lib.rs");
        let working_dir = std::env::current_dir().expect("current dir");
        let mut task_bundle = TaskContextBundle::new(
            "fix the bug in src/lib.rs",
            &working_dir,
            route.clone(),
            None,
        );
        task_bundle.add_acceptance_check("required validation passed");
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        code_workflow.record_stage_validation(
            &task_bundle,
            &[PathBuf::from("src/lib.rs")],
            true,
            &["cargo test -q passed".to_string()],
        );
        code_workflow.record_acceptance_review(AcceptanceReview {
            accepted: true,
            confidence: AcceptanceConfidence::High,
            criteria: Vec::new(),
            unresolved_items: Vec::new(),
            residual_risks: Vec::new(),
            next_action: AcceptanceNextAction::Finish,
        });
        let mut loop_state = TurnLoopStateController::initial_state();
        loop_state.tool_calls_made = true;
        loop_state.final_content = "Let me run the validation.".to_string();

        assert!(!needs_forced_closeout_summary(
            &loop_state,
            &code_workflow,
            &task_bundle
        ));
    }

    #[test]
    fn unverified_continuation_still_needs_forced_summary() {
        let route = IntentRouter::new().route("fix the bug in src/lib.rs");
        let working_dir = std::env::current_dir().expect("current dir");
        let task_bundle =
            TaskContextBundle::new("fix the bug in src/lib.rs", &working_dir, route, None);
        let code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let mut loop_state = TurnLoopStateController::initial_state();
        loop_state.tool_calls_made = true;
        loop_state.final_content = "Let me run the validation.".to_string();

        assert!(needs_forced_closeout_summary(
            &loop_state,
            &code_workflow,
            &task_bundle
        ));
    }
}

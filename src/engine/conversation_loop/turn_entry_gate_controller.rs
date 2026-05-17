use super::legacy_workflow_gate_controller::{
    LegacyWorkflowGateContext, LegacyWorkflowGateController, LegacyWorkflowGateFlow,
};
use super::reflection_gate_controller::{
    ReflectionGateContext, ReflectionGateController, ReflectionGateFlow,
};
use super::session_goal_controller::{SessionGoalController, SessionGoalUpdateContext};
use super::task_context_trace_controller::{TaskContextTraceContext, TaskContextTraceController};
use super::workflow_contract_controller::{
    WorkflowContractController, WorkflowContractJudgmentContext,
};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TurnStatus};
use crate::services::api::Message;
use crate::session_store::LearningEventRecord;
use std::path::Path;
use tokio::sync::mpsc;

pub(super) struct TurnEntryGateContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) last_user_preview: &'a str,
    pub(super) route: &'a IntentRoute,
    pub(super) working_dir: &'a Path,
    pub(super) learning_events: &'a [LearningEventRecord],
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) required_validation_commands: &'a [String],
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) enum TurnEntryGateFlow {
    Continue,
    Stop { content: String, status: TurnStatus },
}

pub(super) struct TurnEntryGateController;

impl TurnEntryGateController {
    pub(super) async fn run(context: TurnEntryGateContext<'_>) -> TurnEntryGateFlow {
        WorkflowContractController::run(WorkflowContractJudgmentContext {
            provider: context.conversation.provider.as_ref(),
            model: context.conversation.model.clone(),
            session_store: context.conversation.session_store.as_ref(),
            session_id: &context.conversation.session_id,
            last_user_preview: context.last_user_preview,
            route: context.route,
            working_dir: context.working_dir,
            learning_events: context.learning_events,
            retrieval_context: context.retrieval_context,
            task_bundle: context.task_bundle,
            code_workflow: context.code_workflow,
            messages: context.messages,
            trace: context.trace,
        })
        .await;

        Self::attach_required_validation_acceptance(
            context.task_bundle,
            context.required_validation_commands,
        );

        TaskContextTraceController::record(TaskContextTraceContext {
            task_bundle: context.task_bundle,
            route_workflow: context.route.workflow,
            required_validation_commands: context.required_validation_commands,
            trace: context.trace,
        });

        match ReflectionGateController::run(ReflectionGateContext {
            task_bundle: context.task_bundle,
            route: context.route,
            code_workflow: context.code_workflow,
            approval_channel: context.conversation.approval_channel.as_ref(),
            tx: context.tx,
            trace: context.trace,
        })
        .await
        {
            ReflectionGateFlow::Continue => {}
            ReflectionGateFlow::Stop { content } => {
                return TurnEntryGateFlow::Stop {
                    content,
                    status: TurnStatus::Failed,
                };
            }
        }

        SessionGoalController::update(SessionGoalUpdateContext {
            manager: context.conversation.goal_manager.as_ref(),
            last_user_preview: context.last_user_preview,
            route: context.route,
            trace: context.trace,
        });

        match LegacyWorkflowGateController::run(LegacyWorkflowGateContext {
            route: context.route,
            messages: context.messages,
            workflow_triggered_this_turn: &context.conversation.workflow_triggered_this_turn,
            workflow_policy: context.conversation.workflow_policy.clone(),
            provider: context.conversation.provider.clone(),
            model: context.conversation.model.clone(),
            tool_registry: context.conversation.tool_registry.clone(),
            base_context: context
                .conversation
                .create_tool_context_with_trace(context.trace),
            memory_manager: context.conversation.memory_manager.as_ref(),
            tx: context.tx,
            trace: context.trace,
        })
        .await
        {
            LegacyWorkflowGateFlow::Continue => TurnEntryGateFlow::Continue,
            LegacyWorkflowGateFlow::Completed { content } => TurnEntryGateFlow::Stop {
                content,
                status: TurnStatus::Completed,
            },
        }
    }

    fn attach_required_validation_acceptance(
        task_bundle: &mut TaskContextBundle,
        required_validation_commands: &[String],
    ) {
        for command in required_validation_commands {
            let command = command.trim();
            if !command.is_empty() {
                task_bundle.add_acceptance_check(format!("required validation command: {command}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::approval::ToolApprovalChannel;
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TraceEvent, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::sync::atomic::Ordering;
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
    async fn continues_after_recording_entry_gate_context() {
        let conversation = conversation();
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut task_bundle = TaskContextBundle::new("修改 src/main.rs", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let mut messages = vec![Message::user("修改 src/main.rs")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "修改 src/main.rs"));
        let working_dir = std::env::current_dir().expect("current dir");

        let flow = TurnEntryGateController::run(TurnEntryGateContext {
            conversation: &conversation,
            last_user_preview: "修改 src/main.rs",
            route: &route,
            working_dir: &working_dir,
            learning_events: &[],
            retrieval_context: None,
            task_bundle: &mut task_bundle,
            code_workflow: &mut code_workflow,
            required_validation_commands: &[],
            messages: &mut messages,
            trace: &trace,
            tx: None,
        })
        .await;

        assert!(matches!(flow, TurnEntryGateFlow::Continue));
        assert!(conversation
            .workflow_triggered_this_turn
            .load(Ordering::SeqCst));
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::TaskContextBuilt { .. })));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowRouted { decision, reason }
                if decision == "direct" && reason.contains("legacy workflow step executor skipped")
        )));
    }

    #[tokio::test]
    async fn stops_before_legacy_gate_when_reflection_is_denied() {
        let approval_channel = Arc::new(ToolApprovalChannel::new());
        let responder_channel = Arc::clone(&approval_channel);
        let conversation = conversation().with_approval_channel(approval_channel);
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut task_bundle = TaskContextBundle::new("修改 src/main.rs", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        code_workflow.policy.reflection_blocks = true;
        let mut messages = vec![Message::user("修改 src/main.rs")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "修改 src/main.rs"));
        let working_dir = std::env::current_dir().expect("current dir");
        let (tx, _rx) = mpsc::channel(4);
        let responder = tokio::spawn(async move {
            loop {
                if let Some((_request, response)) = responder_channel.take_pending().await {
                    let _ = response.send(false);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        });

        let flow = TurnEntryGateController::run(TurnEntryGateContext {
            conversation: &conversation,
            last_user_preview: "修改 src/main.rs",
            route: &route,
            working_dir: &working_dir,
            learning_events: &[],
            retrieval_context: None,
            task_bundle: &mut task_bundle,
            code_workflow: &mut code_workflow,
            required_validation_commands: &[],
            messages: &mut messages,
            trace: &trace,
            tx: Some(&tx),
        })
        .await;
        responder.await.expect("approval responder should complete");

        let TurnEntryGateFlow::Stop { content, status } = flow else {
            panic!("expected entry gate stop");
        };
        assert_eq!(status, TurnStatus::Failed);
        assert_eq!(content, ReflectionGateController::STOP_MESSAGE);
        assert!(!conversation
            .workflow_triggered_this_turn
            .load(Ordering::SeqCst));
        let finished = trace.finish(TurnStatus::Failed);
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::WorkflowRouted { .. })));
    }

    #[tokio::test]
    async fn required_validation_commands_satisfy_pre_tool_reflection_acceptance() {
        let approval_channel = Arc::new(ToolApprovalChannel::new());
        let conversation = conversation().with_approval_channel(approval_channel);
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut task_bundle = TaskContextBundle::new("修改 src/main.rs", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        code_workflow.policy.reflection_blocks = true;
        let mut messages = vec![Message::user("修改 src/main.rs")];
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "修改 src/main.rs"));
        let working_dir = std::env::current_dir().expect("current dir");
        let (tx, mut rx) = mpsc::channel(4);
        let required = vec!["cargo test -q".to_string()];

        let flow = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            TurnEntryGateController::run(TurnEntryGateContext {
                conversation: &conversation,
                last_user_preview: "修改 src/main.rs",
                route: &route,
                working_dir: &working_dir,
                learning_events: &[],
                retrieval_context: None,
                task_bundle: &mut task_bundle,
                code_workflow: &mut code_workflow,
                required_validation_commands: &required,
                messages: &mut messages,
                trace: &trace,
                tx: Some(&tx),
            }),
        )
        .await
        .expect("reflection gate should not wait for approval when required validation exists");

        assert!(matches!(flow, TurnEntryGateFlow::Continue));
        assert!(task_bundle
            .acceptance_checks
            .iter()
            .any(|check| check == "required validation command: cargo test -q"));
        assert!(
            rx.try_recv().is_err(),
            "no reflection permission request expected"
        );
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::TaskContextBuilt {
                acceptance_checks: 1,
                ..
            }
        )));
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::PermissionRequested { .. })));
    }
}

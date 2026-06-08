use super::legacy_workflow_gate_controller::{
    LegacyWorkflowGateContext, LegacyWorkflowGateController, LegacyWorkflowGateFlow,
};
use super::reflection_gate_controller::{
    ReflectionGateContext, ReflectionGateController, ReflectionGateFlow,
};
use super::workflow_contract_controller::{
    WorkflowContractController, WorkflowContractJudgmentContext,
};
use super::ConversationLoop;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::{IntentRoute, WorkflowKind};
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::session_goal::SessionGoalManager;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::task_contract::TaskContractBundleExt;
use crate::engine::trace::{TraceCollector, TraceEvent, TurnStatus};
use crate::services::api::Message;
use crate::session_store::LearningEventRecord;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// SessionGoalController
// ---------------------------------------------------------------------------

struct SessionGoalUpdateContext<'a> {
    manager: Option<&'a Arc<SessionGoalManager>>,
    last_user_preview: &'a str,
    route: &'a IntentRoute,
    trace: &'a TraceCollector,
}

struct SessionGoalController;

impl SessionGoalController {
    fn update(context: SessionGoalUpdateContext<'_>) {
        let Some(manager) = context.manager else {
            return;
        };
        if let Some(goal) =
            manager.update_from_user_message(context.last_user_preview, Some(context.route))
        {
            context.trace.record(TraceEvent::SessionGoalUpdated {
                goal_id: goal.id,
                title: goal.title,
                status: format!("{:?}", goal.status),
                reason: "user turn routed to trackable workflow".to_string(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// TaskContextTraceController
// ---------------------------------------------------------------------------

struct TaskContextTraceContext<'a> {
    task_bundle: &'a TaskContextBundle,
    route_workflow: WorkflowKind,
    required_validation_commands: &'a [String],
    trace: &'a TraceCollector,
}

struct TaskContextTraceController;

impl TaskContextTraceController {
    fn record(context: TaskContextTraceContext<'_>) {
        context.trace.record(TraceEvent::TaskContextBuilt {
            task_id: context.task_bundle.task_id.clone(),
            workflow: format!("{:?}", context.task_bundle.route.workflow),
            files: context.task_bundle.relevant_files.len(),
            constraints: context.task_bundle.constraints.len(),
            risks: context.task_bundle.risks.len(),
            acceptance_checks: context.task_bundle.acceptance_checks.len(),
        });
        let contract = context
            .task_bundle
            .task_contract(context.required_validation_commands);
        context.trace.record(TraceEvent::TaskContractMaterialized {
            task_id: contract.task_id.clone(),
            task_type: format!("{:?}", contract.task_type),
            model_profile: contract.model_profile.label().to_string(),
            assumptions: contract.assumptions.len(),
            scope_files: contract.scope.files_allowed.len(),
            validation_commands: contract.validation.required_commands.len(),
            proof_required: contract.validation.proof_required,
            risk: format!("{:?}", contract.risk.level),
        });
        let context_pack = context.task_bundle.context_pack(&contract);
        context.trace.record(TraceEvent::ContextPackMaterialized {
            task_id: context_pack.task_id,
            project_facts: context_pack.project_facts.len(),
            memory_records: context_pack.memory_records.len(),
            recent_observations: context_pack.recent_observations.len(),
            failure_summaries: context_pack.failure_summaries.len(),
            estimated_tokens: context_pack.estimated_tokens,
            max_tokens: context_pack.budget.max_total_estimated_tokens,
            overflow_items: context_pack.overflow_items,
            fingerprint: context_pack.fingerprint,
        });
        if crate::engine::code_change_workflow::is_programming_workflow(context.route_workflow) {
            context
                .trace
                .record(TraceEvent::ImplementationIntentRecorded {
                    task_id: context.task_bundle.task_id.clone(),
                    workflow: format!("{:?}", context.task_bundle.route.workflow),
                    target_files: context.task_bundle.relevant_files.len(),
                    validation_commands: context.required_validation_commands.to_vec(),
                    risks: context.task_bundle.risks.len(),
                    reason: "code-change workflow must identify target scope and validation before first edit"
                        .to_string(),
                });
        }
    }
}

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
            required_validation_commands: context.required_validation_commands,
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
            memory_manager: context.conversation.memory_manager_for_generate(),
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
                    let _ = response.send(
                        crate::engine::conversation_loop::ToolApprovalResponse::rejected_once(),
                    );
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

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session", 1, "task context trace"))
    }

    #[test]
    fn tracks_goal_and_records_trace_for_trackable_route() {
        let trace = trace();
        let manager = Arc::new(SessionGoalManager::new());
        let route = IntentRouter::new().route("继续优化 CLI 体验，完善状态栏");

        SessionGoalController::update(SessionGoalUpdateContext {
            manager: Some(&manager),
            last_user_preview: "继续优化 CLI 体验，完善状态栏",
            route: &route,
            trace: &trace,
        });

        assert!(manager.current().is_some());
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::SessionGoalUpdated { title, .. } if title.contains("CLI")
        )));
    }

    #[test]
    fn skips_when_goal_manager_is_absent() {
        let trace = trace();
        let route = IntentRouter::new().route("继续优化 CLI 体验，完善状态栏");

        SessionGoalController::update(SessionGoalUpdateContext {
            manager: None,
            last_user_preview: "继续优化 CLI 体验，完善状态栏",
            route: &route,
            trace: &trace,
        });

        let finished = trace.finish(TurnStatus::Completed);
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::SessionGoalUpdated { .. })));
    }

    #[test]
    fn records_task_context_and_programming_intent() {
        let trace = trace();
        let route = IntentRouter::new().route("修改 src/main.rs");
        let mut bundle = TaskContextBundle::new("修改 src/main.rs", ".", route.clone(), None);
        bundle.add_file("src/main.rs");
        bundle.add_acceptance_check("cargo test -q");
        let required = vec!["cargo test -q".to_string()];

        TaskContextTraceController::record(TaskContextTraceContext {
            task_bundle: &bundle,
            route_workflow: route.workflow,
            required_validation_commands: &required,
            trace: &trace,
        });

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::TaskContextBuilt {
                files: 1,
                acceptance_checks: 1,
                ..
            }
        )));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::TaskContractMaterialized {
                task_type,
                model_profile,
                validation_commands: 1,
                proof_required: true,
                ..
            } if task_type == "CodeChange" && model_profile == "standard"
        )));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::ContextPackMaterialized {
                project_facts,
                estimated_tokens,
                ..
            } if *project_facts > 0 && *estimated_tokens > 0
        )));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::ImplementationIntentRecorded {
                target_files: 1,
                validation_commands,
                ..
            } if validation_commands == &required
        )));
    }

    #[test]
    fn direct_workflow_only_records_task_context() {
        let trace = trace();
        let route = IntentRouter::new().route("你好");
        let bundle = TaskContextBundle::new("你好", ".", route.clone(), None);

        TaskContextTraceController::record(TaskContextTraceContext {
            task_bundle: &bundle,
            route_workflow: route.workflow,
            required_validation_commands: &[],
            trace: &trace,
        });

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::TaskContextBuilt { .. })));
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::TaskContractMaterialized { .. })));
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::ContextPackMaterialized { .. })));
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::ImplementationIntentRecorded { .. })));
    }
}

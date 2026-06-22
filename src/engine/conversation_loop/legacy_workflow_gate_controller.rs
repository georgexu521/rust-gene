//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::{is_drift_interruption_signal, text_sanitizer::strip_hidden_blocks};
use crate::engine::intent_router::IntentRoute;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::engine::workflow::{Gate, WorkflowEngine, WorkflowPolicy};
use crate::memory::MemoryManager;
use crate::services::api::{LlmProvider, Message};
use crate::tools::{ToolContext, ToolRegistry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};

use super::WorkflowRealStepExecutor;

pub(super) struct LegacyWorkflowGateContext<'a> {
    pub(super) route: &'a IntentRoute,
    pub(super) messages: &'a [Message],
    pub(super) workflow_triggered_this_turn: &'a AtomicBool,
    pub(super) workflow_policy: WorkflowPolicy,
    pub(super) provider: Arc<dyn LlmProvider>,
    pub(super) model: String,
    pub(super) tool_registry: Arc<ToolRegistry>,
    pub(super) base_context: ToolContext,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LegacyWorkflowGateFlow {
    Continue,
    Completed { content: String },
}

pub(super) struct LegacyWorkflowGateController;

impl LegacyWorkflowGateController {
    pub(super) async fn run(context: LegacyWorkflowGateContext<'_>) -> LegacyWorkflowGateFlow {
        let already_triggered = context
            .workflow_triggered_this_turn
            .swap(true, Ordering::SeqCst);
        if crate::engine::code_change_workflow::is_programming_workflow(context.route.workflow) {
            context.trace.record(TraceEvent::WorkflowRouted {
                decision: "direct".to_string(),
                reason:
                    "code-change contract uses the tool loop; legacy workflow step executor skipped"
                        .to_string(),
            });
            return LegacyWorkflowGateFlow::Continue;
        }
        if already_triggered {
            return LegacyWorkflowGateFlow::Continue;
        }

        let Some(last_user_msg) = last_user_message(context.messages) else {
            return LegacyWorkflowGateFlow::Continue;
        };

        let gate = Gate::new().with_policy(context.workflow_policy.gate.clone());
        if is_drift_interruption_signal(last_user_msg) {
            crate::engine::workflow::metrics::record_drift_interruption();
        }
        let decision = if context.workflow_policy.gate.llm_classifier_enabled {
            gate.decide_with_llm(last_user_msg, context.provider.as_ref(), &context.model)
                .await
        } else {
            gate.decide(last_user_msg)
        };
        context.trace.record(TraceEvent::WorkflowRouted {
            decision: if decision.is_workflow() {
                "workflow".to_string()
            } else {
                "direct".to_string()
            },
            reason: decision.reason().to_string(),
        });
        if !decision.is_workflow() {
            return LegacyWorkflowGateFlow::Continue;
        }

        crate::engine::workflow::metrics::record_workflow_run();
        Self::save_workflow_decision(
            context.memory_manager,
            "gate",
            last_user_msg,
            "Workflow",
            decision.reason(),
        )
        .await;
        debug!("Workflow mode activated: {}", decision.reason());
        let workflow_executor = WorkflowRealStepExecutor {
            tool_registry: context.tool_registry,
            llm_provider: Arc::clone(&context.provider),
            model: context.model,
            base_context: context.base_context,
        };
        let workflow_engine =
            WorkflowEngine::new(Arc::clone(&context.provider)).with_policy(context.workflow_policy);
        match workflow_engine
            .run(last_user_msg, last_user_msg, &workflow_executor)
            .await
        {
            Ok(result) => {
                context.trace.record(TraceEvent::WorkflowCompleted {
                    steps: result.plan.steps.len(),
                });
                let workflow_report = strip_hidden_blocks(&result.final_report);
                Self::save_workflow_decision(
                    context.memory_manager,
                    "execution",
                    last_user_msg,
                    "Success",
                    &format!("workflow completed with {} steps", result.plan.steps.len()),
                )
                .await;
                if let Some(tx) = context.tx {
                    if !workflow_report.trim().is_empty() {
                        let _ = tx
                            .send(StreamEvent::TextChunk(workflow_report.clone()))
                            .await;
                    }
                    let _ = tx.send(StreamEvent::Complete).await;
                }
                context.trace.record(TraceEvent::AssistantResponded {
                    chars: workflow_report.chars().count(),
                    iterations: 0,
                });
                LegacyWorkflowGateFlow::Completed {
                    content: workflow_report,
                }
            }
            Err(e) => {
                context
                    .trace
                    .record(TraceEvent::WorkflowFallback { error: e.clone() });
                Self::save_workflow_decision(
                    context.memory_manager,
                    "fallback",
                    last_user_msg,
                    "DirectMode",
                    &e,
                )
                .await;
                warn!(
                    "Workflow execution failed: {}, falling back to direct mode",
                    e
                );
                LegacyWorkflowGateFlow::Continue
            }
        }
    }

    async fn save_workflow_decision(
        memory_manager: Option<&Arc<Mutex<MemoryManager>>>,
        stage: &str,
        user_message: &str,
        decision: &str,
        reason: &str,
    ) {
        let Some(memory_manager) = memory_manager else {
            return;
        };
        let mut memory = memory_manager.lock().await;
        memory.save_workflow_decision(stage, user_message, decision, reason);
    }
}

fn last_user_message(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rposition(|message| matches!(message, Message::User { .. }))
        .and_then(|index| match &messages[index] {
            Message::User { content } => Some(content.as_str()),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse};
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
        TraceCollector::new(TurnTrace::new("session", 1, "legacy workflow gate"))
    }

    fn context<'a>(
        route: &'a IntentRoute,
        messages: &'a [Message],
        triggered: &'a AtomicBool,
        trace: &'a TraceCollector,
    ) -> LegacyWorkflowGateContext<'a> {
        LegacyWorkflowGateContext {
            route,
            messages,
            workflow_triggered_this_turn: triggered,
            workflow_policy: WorkflowPolicy::default(),
            provider: Arc::new(MockProvider),
            model: "mock-model".to_string(),
            tool_registry: Arc::new(ToolRegistry::new()),
            base_context: ToolContext::new(".", "session"),
            memory_manager: None,
            tx: None,
            trace,
        }
    }

    #[tokio::test]
    async fn programming_workflow_skips_legacy_gate() {
        let trace = trace();
        let route = IntentRouter::new().route("修改 src/main.rs");
        let messages = vec![Message::user("修改 src/main.rs")];
        let triggered = AtomicBool::new(false);

        let flow =
            LegacyWorkflowGateController::run(context(&route, &messages, &triggered, &trace)).await;

        assert_eq!(flow, LegacyWorkflowGateFlow::Continue);
        assert!(triggered.load(Ordering::SeqCst));
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowRouted { decision, reason }
                if decision == "direct" && reason.contains("legacy workflow step executor skipped")
        )));
    }

    #[tokio::test]
    async fn legacy_disabled_routes_direct_without_completion() {
        let trace = trace();
        let route = IntentRouter::new().route("帮我规划一下周末安排");
        let messages = vec![Message::user("帮我规划一下周末安排")];
        let triggered = AtomicBool::new(false);

        let flow =
            LegacyWorkflowGateController::run(context(&route, &messages, &triggered, &trace)).await;

        assert_eq!(flow, LegacyWorkflowGateFlow::Continue);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowRouted { decision, reason }
                if decision == "direct" && reason.contains("Legacy workflow disabled")
        )));
    }

    #[tokio::test]
    async fn already_triggered_non_programming_turn_does_not_route_again() {
        let trace = trace();
        let route = IntentRouter::new().route("帮我规划一下周末安排");
        let messages = vec![Message::user("帮我规划一下周末安排")];
        let triggered = AtomicBool::new(true);

        let flow =
            LegacyWorkflowGateController::run(context(&route, &messages, &triggered, &trace)).await;

        assert_eq!(flow, LegacyWorkflowGateFlow::Continue);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::WorkflowRouted { .. })));
    }
}

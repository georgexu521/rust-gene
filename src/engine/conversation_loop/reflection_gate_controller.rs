use super::approval::{ToolApprovalChannel, ToolApprovalRequest};
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::reflection_pass::{ReflectionPass, ReflectionStatus};
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;

pub(super) struct ReflectionGateContext<'a> {
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) approval_channel: Option<&'a Arc<ToolApprovalChannel>>,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReflectionGateFlow {
    Continue,
    Stop { content: String },
}

pub(super) struct ReflectionGateController;

impl ReflectionGateController {
    pub(super) const STOP_MESSAGE: &'static str =
        "Stopped before code-change execution because reflection found unresolved acceptance gaps.";

    pub(super) async fn run(context: ReflectionGateContext<'_>) -> ReflectionGateFlow {
        let reflection_pass = ReflectionPass::from_task_bundle(context.task_bundle);
        Self::record_reflection_pass(context.trace, &reflection_pass);
        if !Self::should_request_approval(&reflection_pass, context.code_workflow) {
            return ReflectionGateFlow::Continue;
        }

        if Self::request_approval(
            context.route,
            &reflection_pass,
            context.approval_channel,
            context.tx,
            context.trace,
        )
        .await
        {
            return ReflectionGateFlow::Continue;
        }

        let content = Self::STOP_MESSAGE.to_string();
        context.trace.record(TraceEvent::AssistantResponded {
            chars: content.chars().count(),
            iterations: 0,
        });
        ReflectionGateFlow::Stop { content }
    }

    fn record_reflection_pass(trace: &TraceCollector, reflection_pass: &ReflectionPass) {
        trace.record(TraceEvent::ReflectionPassCompleted {
            pass_id: reflection_pass.pass_id.clone(),
            task_id: reflection_pass.task_id.clone(),
            status: format!("{:?}", reflection_pass.status),
            findings: reflection_pass.findings.len(),
            unresolved: reflection_pass.unresolved_count(),
        });
    }

    fn should_request_approval(
        reflection_pass: &ReflectionPass,
        code_workflow: &CodeChangeWorkflowRunner,
    ) -> bool {
        reflection_pass.status == ReflectionStatus::NeedsWork
            && code_workflow.should_block_on_reflection()
    }

    async fn request_approval(
        route: &IntentRoute,
        reflection_pass: &ReflectionPass,
        approval_channel: Option<&Arc<ToolApprovalChannel>>,
        tx: Option<&mpsc::Sender<StreamEvent>>,
        trace: &TraceCollector,
    ) -> bool {
        let review_prompt = Self::review_prompt(route, reflection_pass);
        let review_call = Self::review_call(route, reflection_pass);
        let mut approved = false;
        if let (Some(channel), Some(tx)) = (approval_channel, tx) {
            let _ = tx
                .send(StreamEvent::PermissionRequest {
                    id: review_call.id.clone(),
                    tool_name: review_call.name.clone(),
                    arguments: review_call.arguments.clone(),
                    prompt: review_prompt.clone(),
                    metadata: None,
                    review: None,
                })
                .await;
            trace.record(TraceEvent::PermissionRequested {
                tool: review_call.name.clone(),
                call_id: review_call.id.clone(),
                prompt: review_prompt.clone(),
                review: None,
            });
            match channel
                .submit(ToolApprovalRequest {
                    tool_call: review_call.clone(),
                    prompt: review_prompt.clone(),
                    review: Some(
                        crate::engine::human_review::HumanReviewRequest::reflection_gate(
                            reflection_pass.pass_id.clone(),
                            reflection_pass.unresolved_count(),
                            format!("{:?}", route.workflow),
                        ),
                    ),
                    audit: None,
                })
                .await
            {
                Ok(response) => approved = response.approved,
                Err(e) => warn!("Reflection approval error: {}", e),
            }
            trace.record(TraceEvent::PermissionResolved {
                tool: review_call.name,
                call_id: review_call.id,
                approved,
                decision: None,
                persistence_scope: None,
                rule_pattern: None,
                persisted_path: None,
                review: None,
            });
        } else {
            approved = true;
        }
        approved
    }

    fn review_prompt(route: &IntentRoute, reflection_pass: &ReflectionPass) -> String {
        format!(
            "Reflection pass '{}' found {} unresolved issue(s) before executing a {:?} workflow. Allow the turn to continue?",
            reflection_pass.pass_id,
            reflection_pass.unresolved_count(),
            route.workflow
        )
    }

    fn review_call(route: &IntentRoute, reflection_pass: &ReflectionPass) -> ToolCall {
        ToolCall {
            id: format!(
                "reflection-{}",
                &reflection_pass.pass_id[..8.min(reflection_pass.pass_id.len())]
            ),
            name: "reflection_review".to_string(),
            arguments: serde_json::json!({
                "task_id": reflection_pass.task_id.clone(),
                "pass_id": reflection_pass.pass_id.clone(),
                "status": format!("{:?}", reflection_pass.status),
                "unresolved": reflection_pass.unresolved_count(),
                "workflow": format!("{:?}", route.workflow),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    fn bundle_and_workflow() -> (TaskContextBundle, CodeChangeWorkflowRunner) {
        let route = IntentRouter::new().route("修改 src/main.rs");
        let bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&bundle);
        code_workflow.policy.reflection_blocks = true;
        (bundle, code_workflow)
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session", 1, "reflection gate"))
    }

    #[tokio::test]
    async fn missing_approval_channel_continues_after_recording_reflection() {
        let trace = trace();
        let (bundle, code_workflow) = bundle_and_workflow();

        let flow = ReflectionGateController::run(ReflectionGateContext {
            task_bundle: &bundle,
            route: &bundle.route,
            code_workflow: &code_workflow,
            approval_channel: None,
            tx: None,
            trace: &trace,
        })
        .await;

        assert_eq!(flow, ReflectionGateFlow::Continue);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::ReflectionPassCompleted { .. })));
    }

    #[tokio::test]
    async fn denied_reflection_gate_stops_before_execution() {
        let trace = trace();
        let (bundle, code_workflow) = bundle_and_workflow();
        let approval_channel = Arc::new(ToolApprovalChannel::new());
        let responder_channel = Arc::clone(&approval_channel);
        let (tx, mut rx) = mpsc::channel(4);
        let responder = tokio::spawn(async move {
            loop {
                if let Some((request, response)) = responder_channel.take_pending().await {
                    assert_eq!(request.tool_call.name, "reflection_review");
                    let _ = response.send(
                        crate::engine::conversation_loop::ToolApprovalResponse::rejected_once(),
                    );
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        });

        let flow = ReflectionGateController::run(ReflectionGateContext {
            task_bundle: &bundle,
            route: &bundle.route,
            code_workflow: &code_workflow,
            approval_channel: Some(&approval_channel),
            tx: Some(&tx),
            trace: &trace,
        })
        .await;
        responder.await.expect("approval responder should complete");

        assert_eq!(
            flow,
            ReflectionGateFlow::Stop {
                content: ReflectionGateController::STOP_MESSAGE.to_string(),
            }
        );
        let event = rx
            .try_recv()
            .expect("permission request should be sent to stream");
        assert!(matches!(
            event,
            StreamEvent::PermissionRequest { tool_name, .. } if tool_name == "reflection_review"
        ));
        let finished = trace.finish(TurnStatus::Failed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::PermissionResolved {
                tool,
                approved: false,
                ..
            } if tool == "reflection_review"
        )));
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::AssistantResponded { iterations: 0, .. })));
    }
}

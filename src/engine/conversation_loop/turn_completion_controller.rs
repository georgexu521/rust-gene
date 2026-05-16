use super::closeout_controller::{FinalCloseoutContext, FinalCloseoutController};
use super::runtime_diet::{trace_runtime_diet_report, RuntimeDietSnapshot};
use super::LoopResult;
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::IntentRoute;
use crate::engine::streaming::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use std::collections::HashMap;
use tokio::sync::mpsc;

pub(super) struct TurnCompletionContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a [ToolCall],
    pub(super) iterations_used: usize,
    pub(super) max_iterations: usize,
    pub(super) tool_calls_made: bool,
    pub(super) evidence_ledger: &'a EvidenceLedger,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
}

pub(super) struct TurnCompletionController;

impl TurnCompletionController {
    pub(super) async fn complete(context: TurnCompletionContext<'_>) -> LoopResult {
        FinalCloseoutController::apply_final_closeout(FinalCloseoutContext {
            trace: context.trace,
            code_workflow: context.code_workflow,
            task_bundle: context.task_bundle,
            runtime_diet: context.runtime_diet,
            final_content: context.final_content,
            final_tool_calls: context.final_tool_calls,
            iterations_used: context.iterations_used,
            max_iterations: context.max_iterations,
            evidence_ledger: context.evidence_ledger,
            tx: context.tx,
        })
        .await;

        trace_runtime_diet_report(
            context.trace,
            context.route,
            context.code_workflow,
            context.runtime_diet,
        );

        if let Some(tx) = context.tx {
            let _ = tx.send(StreamEvent::Complete).await;
        }

        context.trace.record(TraceEvent::AssistantResponded {
            chars: context.final_content.chars().count(),
            iterations: context.iterations_used,
        });

        LoopResult {
            content: std::mem::take(context.final_content),
            tool_calls: Vec::new(),
            tool_calls_made: context.tool_calls_made,
            iterations: context.iterations_used,
            pre_executed_results: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    #[tokio::test]
    async fn completion_records_response_and_returns_loop_result() {
        let route = IntentRouter::new().route("say hello");
        let task_bundle = TaskContextBundle::new("say hello", ".", route.clone(), None);
        let code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let evidence_ledger = EvidenceLedger::new();
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "say hello"));
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut final_content = "hello".to_string();
        let final_tool_calls = Vec::new();
        let (tx, mut rx) = mpsc::channel(1);

        let result = TurnCompletionController::complete(TurnCompletionContext {
            trace: &trace,
            route: &route,
            code_workflow: &code_workflow,
            task_bundle: &task_bundle,
            runtime_diet: &mut runtime_diet,
            final_content: &mut final_content,
            final_tool_calls: &final_tool_calls,
            iterations_used: 2,
            max_iterations: 8,
            tool_calls_made: true,
            evidence_ledger: &evidence_ledger,
            tx: Some(&tx),
        })
        .await;

        assert_eq!(result.content, "hello");
        assert!(result.tool_calls.is_empty());
        assert!(result.tool_calls_made);
        assert_eq!(result.iterations, 2);
        assert!(result.pre_executed_results.is_empty());
        assert!(final_content.is_empty());
        assert!(matches!(rx.recv().await, Some(StreamEvent::Complete)));

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AssistantResponded {
                chars: 5,
                iterations: 2,
            }
        )));
    }
}

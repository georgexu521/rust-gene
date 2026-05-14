use super::trace_adaptive_workflow_trigger;
use crate::engine::code_change_workflow::{AdaptiveWorkflowTrigger, CodeChangeWorkflowRunner};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::trace::{TraceCollector, TraceEvent};
use std::path::PathBuf;

pub(super) struct FirstCodeChangeContext<'a> {
    pub(super) trace: &'a TraceCollector,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) evidence_ledger: &'a mut EvidenceLedger,
    pub(super) changed_files: &'a [PathBuf],
}

pub(super) struct FirstCodeChangeController;

impl FirstCodeChangeController {
    pub(super) fn record(context: FirstCodeChangeContext<'_>) {
        context
            .evidence_ledger
            .record_changed_files(context.changed_files);
        if context
            .code_workflow
            .activate_trigger(AdaptiveWorkflowTrigger::FirstCodeChange)
        {
            trace_adaptive_workflow_trigger(
                context.trace,
                AdaptiveWorkflowTrigger::FirstCodeChange,
                context.code_workflow,
            );
            context.trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "adaptive workflow trigger activated: first_code_change files={}",
                    context.changed_files.len()
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    #[test]
    fn first_code_change_records_evidence_and_triggers_once() {
        let trace = TraceCollector::new(TurnTrace::new(
            "session".to_string(),
            1,
            "change source file",
        ));
        let route = IntentRouter::new().route("修改 src/main.rs");
        let bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&bundle);
        let mut evidence_ledger = EvidenceLedger::new();
        let changed_files = vec![PathBuf::from("src/main.rs")];

        FirstCodeChangeController::record(FirstCodeChangeContext {
            trace: &trace,
            code_workflow: &mut code_workflow,
            evidence_ledger: &mut evidence_ledger,
            changed_files: &changed_files,
        });
        FirstCodeChangeController::record(FirstCodeChangeContext {
            trace: &trace,
            code_workflow: &mut code_workflow,
            evidence_ledger: &mut evidence_ledger,
            changed_files: &changed_files,
        });

        assert_eq!(
            evidence_ledger.snapshot().changed_files,
            vec!["src/main.rs".to_string()]
        );
        assert_eq!(
            code_workflow.adaptive_trigger_labels(),
            vec!["first_code_change"]
        );
        let finished = trace.finish(TurnStatus::Completed);
        assert_eq!(
            finished
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    TraceEvent::AdaptiveWorkflowTriggered { trigger, .. }
                    if trigger == "first_code_change"
                ))
                .count(),
            1
        );
    }
}

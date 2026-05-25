use crate::engine::intent_router::WorkflowKind;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::task_contract::TaskContractBundleExt;
use crate::engine::trace::{TraceCollector, TraceEvent};

pub(super) struct TaskContextTraceContext<'a> {
    pub(super) task_bundle: &'a TaskContextBundle,
    pub(super) route_workflow: WorkflowKind,
    pub(super) required_validation_commands: &'a [String],
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct TaskContextTraceController;

impl TaskContextTraceController {
    pub(super) fn record(context: TaskContextTraceContext<'_>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session", 1, "task context trace"))
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

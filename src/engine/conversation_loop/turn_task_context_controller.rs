use super::risk_signal_controller::{RiskSignalController, RiskSignalInput};
use super::turn_runtime_state::TurnRuntimeState;
use super::validation_runner::{RequiredValidationController, RequiredValidationTriggerContext};
use super::workflow_trace::trace_adaptive_workflow_trigger;
use crate::engine::code_change_workflow::{
    is_programming_workflow, AdaptiveWorkflowTrigger, CodeChangeWorkflowRunner,
};
use crate::engine::intent_router::IntentRoute;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::session_goal::SessionGoal;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use std::path::Path;

pub(super) struct TurnTaskContextSetupContext<'a> {
    pub(super) last_user_preview: &'a str,
    pub(super) working_dir: &'a Path,
    pub(super) route: &'a IntentRoute,
    pub(super) current_goal: Option<SessionGoal>,
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) required_validation_commands: &'a [String],
    pub(super) route_scoped_tools_enabled: bool,
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct TurnTaskContextSetup {
    pub(super) task_bundle: TaskContextBundle,
    pub(super) code_workflow: CodeChangeWorkflowRunner,
    pub(super) turn_state: TurnRuntimeState,
}

pub(super) struct TurnTaskContextSetupController;

impl TurnTaskContextSetupController {
    pub(super) fn prepare(context: TurnTaskContextSetupContext<'_>) -> TurnTaskContextSetup {
        let mut task_bundle = TaskContextBundle::new(
            context.last_user_preview,
            context.working_dir,
            context.route.clone(),
            context.current_goal,
        );
        if let Some(retrieval_context) = context.retrieval_context {
            task_bundle = task_bundle.with_retrieval(retrieval_context.clone());
        }
        Self::apply_resource_policy_constraint(&mut task_bundle, context.resource_policy);
        Self::apply_programming_workflow_risk(&mut task_bundle);
        let risk_signal = RiskSignalController::assess_turn_entry(RiskSignalInput {
            route: context.route,
            task_bundle: &task_bundle,
            required_validation_commands: context.required_validation_commands,
        });
        RiskSignalController::apply_to_task_bundle(&risk_signal, &mut task_bundle);

        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let turn_state = TurnRuntimeState::new(context.route_scoped_tools_enabled);
        if risk_signal.entry_contract
            && code_workflow.activate_trigger(AdaptiveWorkflowTrigger::RiskSignalHigh)
        {
            trace_adaptive_workflow_trigger(
                context.trace,
                AdaptiveWorkflowTrigger::RiskSignalHigh,
                &code_workflow,
            );
        }
        RequiredValidationController::record_initial_trigger(RequiredValidationTriggerContext {
            commands: context.required_validation_commands,
            code_workflow: &mut code_workflow,
            trace: context.trace,
        });

        TurnTaskContextSetup {
            task_bundle,
            code_workflow,
            turn_state,
        }
    }

    fn apply_resource_policy_constraint(
        task_bundle: &mut TaskContextBundle,
        resource_policy: &ResourcePolicy,
    ) {
        task_bundle.add_constraint(format!(
            "resource_policy={}",
            resource_policy.compact_label()
        ));
    }

    fn apply_programming_workflow_risk(task_bundle: &mut TaskContextBundle) {
        if is_programming_workflow(task_bundle.route.workflow) {
            task_bundle.add_risk("code-change tasks require explicit verification");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::retrieval_context::RetrievalContext;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session-test", 1, "change code"))
    }

    #[test]
    fn prepare_attaches_retrieval_and_resource_policy_constraint() {
        let trace = trace();
        let route = IntentRouter::new().route("修改 src/main.rs");
        let resource_policy = ResourcePolicy::from_route(&route);
        let retrieval_context = RetrievalContext::from_project_summary(
            "修改 src/main.rs",
            "src/main.rs",
            "/tmp/project",
            route.retrieval,
        )
        .expect("project context");

        let setup = TurnTaskContextSetupController::prepare(TurnTaskContextSetupContext {
            last_user_preview: "修改 src/main.rs",
            working_dir: Path::new("/tmp/project"),
            route: &route,
            current_goal: None,
            retrieval_context: Some(&retrieval_context),
            resource_policy: &resource_policy,
            required_validation_commands: &[],
            route_scoped_tools_enabled: true,
            trace: &trace,
        });

        assert_eq!(setup.task_bundle.route.workflow, route.workflow);
        assert!(setup.task_bundle.retrieval.is_some());
        assert!(setup.task_bundle.constraints.contains(&format!(
            "resource_policy={}",
            resource_policy.compact_label()
        )));
        assert!(setup
            .task_bundle
            .risks
            .contains(&"code-change tasks require explicit verification".to_string()));
        assert_eq!(setup.code_workflow.task_id, setup.task_bundle.task_id);
        assert!(setup.turn_state.runtime_diet.route_scoped_tools);
    }

    #[test]
    fn prepare_records_required_validation_trigger() {
        let trace = trace();
        let route = IntentRouter::new().route("修改 src/main.rs 并运行 cargo test -q");
        let resource_policy = ResourcePolicy::from_route(&route);
        let required = vec!["cargo test -q".to_string()];

        let setup = TurnTaskContextSetupController::prepare(TurnTaskContextSetupContext {
            last_user_preview: "修改 src/main.rs 并运行 cargo test -q",
            working_dir: Path::new("/tmp/project"),
            route: &route,
            current_goal: None,
            retrieval_context: None,
            resource_policy: &resource_policy,
            required_validation_commands: &required,
            route_scoped_tools_enabled: false,
            trace: &trace,
        });

        assert_eq!(
            setup.code_workflow.adaptive_trigger_labels(),
            vec!["risk_signal_high", "required_validation"]
        );
        assert!(!setup.turn_state.runtime_diet.route_scoped_tools);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AdaptiveWorkflowTriggered { trigger, .. }
                if trigger == "required_validation"
        )));
        assert!(finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::WorkflowFallback { .. })));
    }
}

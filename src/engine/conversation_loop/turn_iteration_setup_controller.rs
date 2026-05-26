use super::iteration_budget_controller::{IterationBudgetCheck, IterationBudgetController};
use super::tool_exposure_plan::{ToolExposurePlan, ToolExposureRequest};
use super::turn_runtime_state::TurnRuntimeState;
use super::workflow_change_tracker::WorkflowChangeTracker;
use crate::engine::intent_router::WorkflowKind;
use crate::engine::task_context::AgentTaskStage;
use crate::engine::task_contract::ModelProfileMode;
use crate::engine::trace::TraceCollector;
use crate::memory::MemoryManager;
use crate::services::api::Tool;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub(super) struct TurnIterationSetupContext<'a> {
    pub(super) iteration: usize,
    pub(super) max_iterations: usize,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) route_workflow: WorkflowKind,
    pub(super) task_stage: AgentTaskStage,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) base_tools: &'a [Tool],
    pub(super) available_tools: &'a [Tool],
    pub(super) required_validation_commands_present: bool,
    pub(super) model_profile: ModelProfileMode,
}

pub(super) enum TurnIterationSetupFlow {
    Continue { exposure_plan: ToolExposurePlan },
    Stop,
}

pub(super) struct TurnIterationSetupController;

impl TurnIterationSetupController {
    pub(super) async fn run(context: TurnIterationSetupContext<'_>) -> TurnIterationSetupFlow {
        debug!(
            "Conversation loop iteration {} (effective: {}/{})",
            context.iteration, context.turn_state.effective_iterations, context.max_iterations
        );
        context.turn_state.iterations_used = context.iteration + 1;

        if let Some(memory_manager) = context.memory_manager {
            let mut memory = memory_manager.lock().await;
            memory.reset_turn();
        }

        match IterationBudgetController::check_before_request(
            context.turn_state,
            context.max_iterations,
            context.trace,
        ) {
            IterationBudgetCheck::Continue => {}
            IterationBudgetCheck::Stop {
                effective_iterations,
                max_iterations,
            } => {
                warn!(
                    "Effective iteration budget exhausted ({}/{})",
                    effective_iterations, max_iterations
                );
                return TurnIterationSetupFlow::Stop;
            }
        }

        TurnIterationSetupFlow::Continue {
            exposure_plan: Self::build_exposure_plan(&context),
        }
    }

    fn build_exposure_plan(context: &TurnIterationSetupContext<'_>) -> ToolExposurePlan {
        let has_changes_before_request =
            crate::engine::code_change_workflow::is_programming_workflow(context.route_workflow)
                && WorkflowChangeTracker::has_changes_since(context.baseline_git_status_files);
        let base_tools = route_recovered_base_tools(
            context.base_tools,
            context.available_tools,
            &context.turn_state.route_recovery,
        );
        ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: crate::engine::code_change_workflow::is_programming_workflow(
                context.route_workflow,
            ),
            task_stage: Some(context.task_stage),
            has_changes_before_request,
            required_validation_commands_present: context.required_validation_commands_present,
            model_profile: context.model_profile,
            action_checkpoint_active: context.turn_state.focused_repair.action_checkpoint_active,
            action_checkpoint_lookup_count: context
                .turn_state
                .focused_repair
                .action_checkpoint_lookup_count,
            action_checkpoint_requires_patch_before_validation: context
                .turn_state
                .focused_repair
                .action_checkpoint_requires_patch_before_validation,
        })
    }
}

fn route_recovered_base_tools(
    base_tools: &[Tool],
    available_tools: &[Tool],
    recovery: &crate::engine::route_recovery::RouteRecoveryRuntimeState,
) -> Vec<Tool> {
    if !recovery.read_search_expanded {
        return base_tools.to_vec();
    }

    let mut tools = base_tools.to_vec();
    let mut indexes = tools
        .iter()
        .enumerate()
        .map(|(index, tool)| (tool.name.clone(), index))
        .collect::<HashMap<_, _>>();
    for tool in available_tools {
        if !crate::engine::route_recovery::is_safe_read_search_tool(&tool.name) {
            continue;
        }
        if indexes.contains_key(&tool.name) {
            continue;
        }
        indexes.insert(tool.name.clone(), tools.len());
        tools.push(tool.clone());
    }
    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session-test", 1, "test"))
    }

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: "tool".to_string(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        }
    }

    #[tokio::test]
    async fn run_stops_when_effective_budget_is_exhausted() {
        let trace = trace();
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let flow = TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: 0,
            max_iterations: 2,
            turn_state: &mut turn_state,
            memory_manager: None,
            trace: &trace,
            route_workflow: WorkflowKind::Direct,
            task_stage: AgentTaskStage::Understand,
            baseline_git_status_files: &HashSet::new(),
            base_tools: &[tool("file_read")],
            available_tools: &[tool("file_read")],
            required_validation_commands_present: false,
            model_profile: ModelProfileMode::Standard,
        })
        .await;

        assert!(matches!(flow, TurnIterationSetupFlow::Stop));
        assert_eq!(turn_state.iterations_used, 1);
    }

    #[tokio::test]
    async fn run_continues_with_exposure_plan_and_records_reserved_round() {
        let trace = trace();
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;
        turn_state.reserved_repair_rounds = 1;

        let flow = TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: 2,
            max_iterations: 2,
            turn_state: &mut turn_state,
            memory_manager: None,
            trace: &trace,
            route_workflow: WorkflowKind::Direct,
            task_stage: AgentTaskStage::Understand,
            baseline_git_status_files: &HashSet::new(),
            base_tools: &[tool("file_read"), tool("bash")],
            available_tools: &[tool("file_read"), tool("bash")],
            required_validation_commands_present: false,
            model_profile: ModelProfileMode::Standard,
        })
        .await;

        let TurnIterationSetupFlow::Continue { exposure_plan } = flow else {
            panic!("reserved repair round should continue");
        };
        assert_eq!(turn_state.iterations_used, 3);
        assert_eq!(turn_state.reserved_repair_rounds, 0);
        assert!(exposure_plan.exposed_tool_names.contains("file_read"));
        assert!(exposure_plan.exposed_tool_names.contains("bash"));

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            crate::engine::trace::TraceEvent::WorkflowFallback { error }
                if error.contains("using reserved repair round")
        )));
    }

    #[tokio::test]
    async fn route_recovery_expands_only_read_search_tools() {
        let trace = trace();
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.route_recovery.read_search_expanded = true;
        let base_tools = vec![tool("ask_user")];
        let available_tools = vec![
            tool("ask_user"),
            tool("file_read"),
            tool("grep"),
            tool("file_edit"),
            tool("bash"),
        ];

        let flow = TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: 0,
            max_iterations: 2,
            turn_state: &mut turn_state,
            memory_manager: None,
            trace: &trace,
            route_workflow: WorkflowKind::Direct,
            task_stage: AgentTaskStage::Understand,
            baseline_git_status_files: &HashSet::new(),
            base_tools: &base_tools,
            available_tools: &available_tools,
            required_validation_commands_present: false,
            model_profile: ModelProfileMode::Standard,
        })
        .await;

        let TurnIterationSetupFlow::Continue { exposure_plan } = flow else {
            panic!("route recovery should continue");
        };
        assert!(exposure_plan.exposed_tool_names.contains("ask_user"));
        assert!(exposure_plan.exposed_tool_names.contains("file_read"));
        assert!(exposure_plan.exposed_tool_names.contains("grep"));
        assert!(!exposure_plan.exposed_tool_names.contains("file_edit"));
        assert!(!exposure_plan.exposed_tool_names.contains("bash"));
    }
}

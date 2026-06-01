use super::tool_exposure_plan::{ToolExposurePlan, ToolExposureRequest};
use super::turn_runtime_state::TurnRuntimeState;
use crate::memory::MemoryManager;
use crate::services::api::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

pub(super) struct TurnIterationSetupContext<'a> {
    pub(super) iteration: usize,
    pub(super) max_iterations: usize,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) base_tools: &'a [Tool],
    pub(super) available_tools: &'a [Tool],
}

pub(super) enum TurnIterationSetupFlow {
    Continue { exposure_plan: ToolExposurePlan },
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

        TurnIterationSetupFlow::Continue {
            exposure_plan: Self::build_exposure_plan(&context),
        }
    }

    fn build_exposure_plan(context: &TurnIterationSetupContext<'_>) -> ToolExposurePlan {
        let base_tools = route_recovered_base_tools(
            context.base_tools,
            context.available_tools,
            &context.turn_state.route_recovery,
        );
        ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
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

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: "tool".to_string(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        }
    }

    #[tokio::test]
    async fn run_continues_when_effective_budget_is_exhausted() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;

        let flow = TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: 0,
            max_iterations: 2,
            turn_state: &mut turn_state,
            memory_manager: None,
            base_tools: &[tool("file_read")],
            available_tools: &[tool("file_read")],
        })
        .await;

        assert!(matches!(flow, TurnIterationSetupFlow::Continue { .. }));
        assert_eq!(turn_state.iterations_used, 1);
    }

    #[tokio::test]
    async fn run_continues_without_consuming_reserved_repair_round() {
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.effective_iterations = 2;
        turn_state.reserved_repair_rounds = 1;

        let flow = TurnIterationSetupController::run(TurnIterationSetupContext {
            iteration: 2,
            max_iterations: 2,
            turn_state: &mut turn_state,
            memory_manager: None,
            base_tools: &[tool("file_read"), tool("bash")],
            available_tools: &[tool("file_read"), tool("bash")],
        })
        .await;

        let TurnIterationSetupFlow::Continue { exposure_plan } = flow;
        assert_eq!(turn_state.iterations_used, 3);
        assert_eq!(turn_state.reserved_repair_rounds, 1);
        assert!(exposure_plan.exposed_tool_names.contains("file_read"));
        assert!(exposure_plan.exposed_tool_names.contains("bash"));
    }

    #[tokio::test]
    async fn route_recovery_expands_only_read_search_tools() {
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
            base_tools: &base_tools,
            available_tools: &available_tools,
        })
        .await;

        let TurnIterationSetupFlow::Continue { exposure_plan } = flow;
        assert!(exposure_plan.exposed_tool_names.contains("ask_user"));
        assert!(exposure_plan.exposed_tool_names.contains("file_read"));
        assert!(exposure_plan.exposed_tool_names.contains("grep"));
        assert!(!exposure_plan.exposed_tool_names.contains("file_edit"));
        assert!(!exposure_plan.exposed_tool_names.contains("bash"));
    }
}

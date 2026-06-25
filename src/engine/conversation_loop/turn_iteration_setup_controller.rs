//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::turn_state::TurnRuntimeState;
use crate::engine::task_context::AgentTaskStage;
use crate::memory::MemoryManager;
use crate::services::api::{Message, Tool};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

pub(super) struct ToolExposureRequest<'a> {
    pub(super) base_tools: &'a [Tool],
    pub(super) task_stage: AgentTaskStage,
}

pub(super) struct ToolExposurePlan {
    pub(super) tools: Vec<Tool>,
    pub(super) exposed_tool_names: HashSet<String>,
    pub(super) focused_repair_prompt: Option<Message>,
    pub(super) stage_advisory: StageToolExposureAdvisory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StageToolExposureAdvisory {
    pub(super) task_stage: AgentTaskStage,
    pub(super) recommended_tool_names: Vec<String>,
    pub(super) missing_recommended_tool_names: Vec<String>,
}

impl ToolExposurePlan {
    pub(super) fn build(request: ToolExposureRequest<'_>) -> Self {
        let tools = request.base_tools.to_vec();
        let exposed_tool_names = tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<HashSet<_>>();
        let stage_advisory =
            StageToolExposureAdvisory::build(request.task_stage, &exposed_tool_names);

        Self {
            tools,
            exposed_tool_names,
            focused_repair_prompt: None,
            stage_advisory,
        }
    }
}

impl StageToolExposureAdvisory {
    fn build(task_stage: AgentTaskStage, exposed_tool_names: &HashSet<String>) -> Self {
        let recommended_tool_names = stage_recommended_tools(task_stage)
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        let missing_recommended_tool_names = recommended_tool_names
            .iter()
            .filter(|name| !exposed_tool_names.contains(*name))
            .cloned()
            .collect();
        Self {
            task_stage,
            recommended_tool_names,
            missing_recommended_tool_names,
        }
    }
}

pub(super) struct TurnIterationSetupContext<'a> {
    pub(super) iteration: usize,
    pub(super) max_iterations: usize,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) base_tools: &'a [Tool],
    pub(super) available_tools: &'a [Tool],
    pub(super) task_stage: AgentTaskStage,
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
        context.turn_state.iterations_used += 1;

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
        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            task_stage: context.task_stage,
        });
        debug!(
            task_stage = ?plan.stage_advisory.task_stage,
            recommended_tools = ?plan.stage_advisory.recommended_tool_names,
            missing_tools = ?plan.stage_advisory.missing_recommended_tool_names,
            "stage-aware tool exposure advisory"
        );
        plan
    }
}

fn stage_recommended_tools(stage: AgentTaskStage) -> &'static [&'static str] {
    match stage {
        AgentTaskStage::Understand | AgentTaskStage::Plan => {
            &["project_list", "grep", "glob", "file_read", "symbol_query"]
        }
        AgentTaskStage::Edit => &["file_read", "grep", "file_edit", "file_write", "file_patch"],
        AgentTaskStage::Validate => &["bash", "run_tests", "git_diff", "git_status", "format"],
        AgentTaskStage::Closeout | AgentTaskStage::Done => {
            &["git_diff", "git_status", "trace", "cost"]
        }
        AgentTaskStage::Repair => &[
            "file_read",
            "grep",
            "file_edit",
            "file_write",
            "file_patch",
            "bash",
            "run_tests",
            "git_diff",
        ],
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
            task_stage: AgentTaskStage::Understand,
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
            task_stage: AgentTaskStage::Repair,
        })
        .await;

        let TurnIterationSetupFlow::Continue { exposure_plan } = flow;
        assert_eq!(turn_state.iterations_used, 1);
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
            task_stage: AgentTaskStage::Understand,
        })
        .await;

        let TurnIterationSetupFlow::Continue { exposure_plan } = flow;
        assert!(exposure_plan.exposed_tool_names.contains("ask_user"));
        assert!(exposure_plan.exposed_tool_names.contains("file_read"));
        assert!(exposure_plan.exposed_tool_names.contains("grep"));
        assert!(!exposure_plan.exposed_tool_names.contains("file_edit"));
        assert!(!exposure_plan.exposed_tool_names.contains("bash"));
    }

    #[test]
    fn build_exposes_the_base_tools_without_runtime_scoping() {
        let base_tools = vec![
            tool("file_write"),
            tool("file_edit"),
            tool("file_patch"),
            tool("file_read"),
            tool("grep"),
            tool("bash"),
            tool("run_tests"),
            tool("start_dev_server"),
            tool("install_dependencies"),
            tool("git_status"),
            tool("git_diff"),
        ];

        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            task_stage: AgentTaskStage::Edit,
        });

        assert_eq!(plan.tools.len(), base_tools.len());
        for tool in &base_tools {
            assert!(plan.exposed_tool_names.contains(&tool.name));
        }
        assert!(plan.focused_repair_prompt.is_none());
    }

    #[test]
    fn stage_advisory_reports_recommended_and_missing_tools_without_filtering() {
        let base_tools = vec![tool("file_read"), tool("grep"), tool("bash")];

        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            task_stage: AgentTaskStage::Validate,
        });

        assert_eq!(plan.tools.len(), base_tools.len());
        assert_eq!(plan.stage_advisory.task_stage, AgentTaskStage::Validate);
        assert!(plan
            .stage_advisory
            .recommended_tool_names
            .contains(&"run_tests".to_string()));
        assert!(plan
            .stage_advisory
            .missing_recommended_tool_names
            .contains(&"run_tests".to_string()));
        assert!(plan.exposed_tool_names.contains("file_read"));
    }
}

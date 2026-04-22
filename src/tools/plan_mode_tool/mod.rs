//! Plan Mode 工具 - 进入和退出计划模式

use crate::engine::plan_mode::PlanModeManager;
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// 进入计划模式工具
pub struct EnterPlanModeTool {
    manager: Arc<PlanModeManager>,
}

impl EnterPlanModeTool {
    pub fn new(manager: Arc<PlanModeManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "enter_plan_mode"
    }

    fn description(&self) -> &str {
        concat!(
            "Enter plan mode to design an implementation approach before coding. In plan mode, focus on exploration and planning.\n\n",
            "While in plan mode, if you encounter ambiguous requirements or need to make a design decision ",
            "where multiple valid options exist, use the `ask_user` tool to ask a clarifying question ",
            "BEFORE submitting the final plan. Examples of when to ask: uncertain auth method (OAuth vs JWT), ",
            "UI framework choice, API design approach, naming conventions, or scope boundaries."
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _params: serde_json::Value, _context: ToolContext) -> ToolResult {
        self.manager.enter_plan_mode().await;
        ToolResult::success_with_data(
            concat!(
                "Entered plan mode. Focus on exploring the codebase and designing an implementation approach. ",
                "DO NOT write or edit files yet.\n\n",
                "If you encounter ambiguous requirements, use the `ask_user` tool to ask clarifying questions before submitting the plan."
            ),
            json!({ "mode": "plan" }),
        )
    }
}

/// 退出计划模式工具
pub struct ExitPlanModeTool {
    manager: Arc<PlanModeManager>,
}

impl ExitPlanModeTool {
    pub fn new(manager: Arc<PlanModeManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "exit_plan_mode"
    }

    fn description(&self) -> &str {
        "Exit plan mode and return to normal execution mode."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "rejected": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether the plan was rejected"
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let rejected = params["rejected"].as_bool().unwrap_or(false);
        self.manager.exit(rejected).await;
        if rejected {
            ToolResult::success("Exited plan mode (plan rejected).")
        } else {
            ToolResult::success("Exited plan mode. Ready to execute.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enter_exit_plan_mode() {
        let manager = Arc::new(PlanModeManager::new());

        let enter = EnterPlanModeTool::new(manager.clone());
        let result = enter
            .execute(json!({}), ToolContext::new(".", "test"))
            .await;
        assert!(result.success);
        assert_eq!(
            manager.get_state().await,
            crate::engine::plan_mode::PlanModeState::Generating
        );

        let exit = ExitPlanModeTool::new(manager.clone());
        let result = exit
            .execute(json!({ "rejected": false }), ToolContext::new(".", "test"))
            .await;
        assert!(result.success);
        assert_eq!(
            manager.get_state().await,
            crate::engine::plan_mode::PlanModeState::Off
        );
    }
}

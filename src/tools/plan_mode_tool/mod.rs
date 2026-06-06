//! Plan Mode 工具 - 进入和退出计划模式

use crate::engine::plan_mode::PlanModeManager;
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
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

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Task
    }

    fn description(&self) -> &str {
        "Enter plan mode — a read-only exploration phase before coding. \
         In plan mode, file_write and file_edit are blocked; you can only read, \
         search, and analyze. This prevents premature edits before understanding \
         the full picture. \
         \
         Use plan mode when: the task spans multiple files, involves unfamiliar \
         code, or requires architectural decisions. Skip it for trivial fixes. \
         \
         While in plan mode, if you hit an ambiguous design choice (auth method, \
         framework selection, naming convention, scope boundary), use ask_user \
         BEFORE submitting the plan. Exit plan mode with exit_plan_mode when ready."
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

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Task
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

    fn requires_user_interaction(&self) -> bool {
        true
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

//! Goal drift detection before tool execution.
//!
//! V1 is advisory only. It flags tool calls that look disconnected from the
//! current session goal so `/trace` can show when a turn may be drifting.

use crate::engine::intent_router::IntentKind;
use crate::engine::session_goal::SessionGoal;
use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftLevel {
    None,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftCheck {
    pub level: DriftLevel,
    pub reason: String,
    pub suggested_action: Option<String>,
}

impl DriftCheck {
    pub fn ok() -> Self {
        Self {
            level: DriftLevel::None,
            reason: "tool appears aligned with the active goal".to_string(),
            suggested_action: None,
        }
    }

    pub fn should_trace(&self) -> bool {
        matches!(self.level, DriftLevel::Medium | DriftLevel::High)
    }

    pub fn requires_approval(&self) -> bool {
        self.level == DriftLevel::High
    }
}

#[derive(Debug, Default, Clone)]
pub struct GoalDriftDetector;

impl GoalDriftDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn check(&self, goal: &SessionGoal, tool_call: &ToolCall) -> DriftCheck {
        let tool = tool_call.name.as_str();
        let title = goal.title.to_ascii_lowercase();
        let args = serde_json::to_string(&tool_call.arguments)
            .unwrap_or_default()
            .to_ascii_lowercase();

        if is_destructive_tool(tool, &args) && !goal_allows_destructive_action(&title) {
            return DriftCheck {
                level: DriftLevel::High,
                reason: format!(
                    "destructive-looking tool '{}' does not obviously match goal '{}'",
                    tool, goal.title
                ),
                suggested_action: Some("pause and ask for confirmation or restate the goal".into()),
            };
        }

        if matches!(goal.intent, IntentKind::Planning | IntentKind::Research)
            && is_write_tool(tool)
            && !title_contains_any(
                &title,
                &["implement", "fix", "修改", "实现", "删除", "写入"],
            )
        {
            return DriftCheck {
                level: DriftLevel::Medium,
                reason: format!(
                    "write tool '{}' during {:?} goal may be premature",
                    tool, goal.intent
                ),
                suggested_action: Some("prefer read-only exploration before changing files".into()),
            };
        }

        if matches!(goal.intent, IntentKind::Configuration)
            && !matches!(
                tool,
                "mcp" | "config" | "memory_load" | "memory_save" | "bash" | "file_read"
            )
        {
            return DriftCheck {
                level: DriftLevel::Medium,
                reason: format!(
                    "tool '{}' is not a typical configuration tool for goal '{}'",
                    tool, goal.title
                ),
                suggested_action: Some(
                    "verify the tool call supports the configuration task".into(),
                ),
            };
        }

        DriftCheck::ok()
    }
}

fn is_write_tool(tool: &str) -> bool {
    matches!(
        tool,
        "file_write"
            | "file_edit"
            | "bash"
            | "git"
            | "refactor"
            | "worktree"
            | "powershell"
            | "remote_dev"
    )
}

fn is_destructive_tool(tool: &str, args: &str) -> bool {
    if matches!(tool, "file_write" | "file_edit") {
        return false;
    }
    matches!(tool, "bash" | "powershell" | "remote_dev")
        && title_contains_any(
            args,
            &[
                "rm -rf",
                "rm ",
                "mv ",
                "chmod ",
                "chown ",
                "sudo ",
                "git reset",
                "git clean",
                "drop table",
                "delete from",
            ],
        )
}

fn goal_allows_destructive_action(title: &str) -> bool {
    title_contains_any(
        title,
        &[
            "delete", "remove", "clean", "reset", "cleanup", "删除", "清理", "移除", "重置",
        ],
    )
}

fn title_contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{IntentKind, WorkflowKind};
    use crate::engine::session_goal::{GoalStatus, SessionGoal};

    fn goal(intent: IntentKind, title: &str) -> SessionGoal {
        SessionGoal {
            id: "g1".to_string(),
            title: title.to_string(),
            status: GoalStatus::Active,
            intent,
            workflow: WorkflowKind::Planning,
            acceptance_criteria: Vec::new(),
            last_user_message: title.to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[test]
    fn flags_destructive_bash_when_goal_does_not_allow_it() {
        let check = GoalDriftDetector::new().check(
            &goal(IntentKind::CodeChange, "optimize CLI display"),
            &ToolCall {
                id: "1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "rm -rf target"}),
            },
        );
        assert_eq!(check.level, DriftLevel::High);
        assert!(check.requires_approval());
    }

    #[test]
    fn allows_read_only_for_planning_goal() {
        let check = GoalDriftDetector::new().check(
            &goal(IntentKind::Planning, "plan CLI improvements"),
            &ToolCall {
                id: "1".to_string(),
                name: "grep".to_string(),
                arguments: serde_json::json!({"pattern": "foo"}),
            },
        );
        assert_eq!(check.level, DriftLevel::None);
    }
}

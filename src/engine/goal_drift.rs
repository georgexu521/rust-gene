//! Goal drift detection before tool execution.
//!
//! V1 is advisory only. It flags tool calls that look disconnected from the
//! current session goal so `/trace` can show when a turn may be drifting.

use crate::engine::intent_router::IntentKind;
use crate::engine::session_goal::SessionGoal;
use crate::engine::task_context::AgentTaskState;
use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Copy)]
pub struct GoalDriftContext<'a> {
    pub goal: Option<&'a SessionGoal>,
    pub task_state: Option<&'a AgentTaskState>,
}

impl GoalDriftDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn check(&self, goal: &SessionGoal, tool_call: &ToolCall) -> DriftCheck {
        self.check_with_context(
            GoalDriftContext {
                goal: Some(goal),
                task_state: None,
            },
            tool_call,
        )
    }

    pub fn check_with_context(
        &self,
        context: GoalDriftContext<'_>,
        tool_call: &ToolCall,
    ) -> DriftCheck {
        let tool = tool_call.name.as_str();
        let title_text = context
            .goal
            .map(|goal| goal.title.as_str())
            .or_else(|| context.task_state.map(|state| state.main_goal.as_str()))
            .unwrap_or("current task");
        let title = title_text.to_ascii_lowercase();
        let args = serde_json::to_string(&tool_call.arguments)
            .unwrap_or_default()
            .to_ascii_lowercase();

        if let Some(check) = context
            .task_state
            .and_then(|state| task_state_drift_check(state, tool_call, &title, &args))
        {
            return check;
        }

        if is_destructive_tool(tool, &args) && !goal_allows_destructive_action(&title) {
            return DriftCheck {
                level: DriftLevel::High,
                reason: format!(
                    "destructive-looking tool '{}' does not obviously match goal '{}'",
                    tool, title_text
                ),
                suggested_action: Some("pause and ask for confirmation or restate the goal".into()),
            };
        }

        if context
            .goal
            .map(|goal| matches!(goal.intent, IntentKind::Planning | IntentKind::Research))
            .unwrap_or(false)
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
                    tool,
                    context.goal.map(|goal| goal.intent)
                ),
                suggested_action: Some("prefer read-only exploration before changing files".into()),
            };
        }

        if context
            .goal
            .map(|goal| matches!(goal.intent, IntentKind::Configuration))
            .unwrap_or(false)
            && !matches!(
                tool,
                "mcp"
                    | "config"
                    | "memory_load"
                    | "memory_save"
                    | "bash"
                    | "file_read"
                    | "glob"
                    | "grep"
            )
        {
            return DriftCheck {
                level: DriftLevel::Medium,
                reason: format!(
                    "tool '{}' is not a typical configuration tool for goal '{}'",
                    tool, title_text
                ),
                suggested_action: Some(
                    "verify the tool call supports the configuration task".into(),
                ),
            };
        }

        DriftCheck::ok()
    }
}

fn task_state_drift_check(
    state: &AgentTaskState,
    tool_call: &ToolCall,
    title: &str,
    args: &str,
) -> Option<DriftCheck> {
    let tool = tool_call.name.as_str();
    let mutation = is_mutation_tool_or_command(tool, &tool_call.arguments);
    if mutation && state_forbids_local_mutation(state) && !title_allows_file_mutation(title) {
        return Some(DriftCheck {
            level: DriftLevel::High,
            reason: format!(
                "tool '{}' would mutate local state but task state forbids local mutation",
                tool
            ),
            suggested_action: Some(
                "ask for explicit mutation approval or choose a read-only action".into(),
            ),
        });
    }

    if is_destructive_tool(tool, args)
        && state_forbids_destructive_scope(state)
        && !goal_allows_destructive_action(title)
    {
        return Some(DriftCheck {
            level: DriftLevel::High,
            reason: format!(
                "destructive-looking tool '{}' conflicts with task forbidden actions",
                tool
            ),
            suggested_action: Some(
                "pause and confirm the destructive action is within the requested scope".into(),
            ),
        });
    }

    if let Some(path) = first_path_outside_allowed_scope(state, tool_call) {
        return Some(DriftCheck {
            level: if mutation {
                DriftLevel::High
            } else {
                DriftLevel::Medium
            },
            reason: format!(
                "tool '{}' targets '{}' outside AgentTaskState.allowed_scope ({})",
                tool,
                path,
                state.allowed_scope.join("; ")
            ),
            suggested_action: Some(
                "ask for confirmation or restrict the action to the task working scope".into(),
            ),
        });
    }

    None
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

fn is_mutation_tool_or_command(tool: &str, arguments: &serde_json::Value) -> bool {
    match tool {
        "file_write" | "file_edit" | "file_patch" | "refactor" | "worktree" | "remote_dev" => true,
        "git" => !matches!(
            arguments
                .get("action")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "status" | "diff" | "log" | "show"
        ),
        "bash" => arguments
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(|command| {
                let classification =
                    crate::tools::bash_tool::command_classifier::classify_command(command);
                matches!(
                    classification.command_kind,
                    crate::tools::bash_tool::command_classifier::CommandKind::Mutation
                        | crate::tools::bash_tool::command_classifier::CommandKind::Dangerous
                ) || matches!(
                    classification.category,
                    crate::tools::bash_tool::command_classifier::ShellCommandCategory::FileMutation
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::GitMutation
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::Destructive
                        | crate::tools::bash_tool::command_classifier::ShellCommandCategory::PackageInstall
                )
            })
            .unwrap_or(false),
        "powershell" => arguments
            .get("command")
            .or_else(|| arguments.get("script"))
            .and_then(serde_json::Value::as_str)
            .map(|command| {
                let lower = command.to_ascii_lowercase();
                title_contains_any(
                    &lower,
                    &[
                        "remove-item",
                        "set-content",
                        "add-content",
                        "new-item",
                        "move-item",
                        "copy-item",
                    ],
                )
            })
            .unwrap_or(false),
        _ => false,
    }
}

fn state_forbids_local_mutation(state: &AgentTaskState) -> bool {
    state
        .forbidden_actions
        .iter()
        .any(|action| action.to_ascii_lowercase().contains("local mutation"))
}

fn state_forbids_destructive_scope(state: &AgentTaskState) -> bool {
    state
        .forbidden_actions
        .iter()
        .any(|action| action.to_ascii_lowercase().contains("destructive"))
}

fn goal_allows_destructive_action(title: &str) -> bool {
    title_contains_any(
        title,
        &[
            "delete", "remove", "clean", "reset", "cleanup", "删除", "清理", "移除", "重置",
        ],
    )
}

fn title_allows_file_mutation(title: &str) -> bool {
    title_contains_any(
        title,
        &[
            "implement",
            "fix",
            "modify",
            "write",
            "edit",
            "patch",
            "create",
            "delete",
            "remove",
            "update",
            "实现",
            "修复",
            "修改",
            "写",
            "编辑",
            "创建",
            "删除",
            "移除",
            "更新",
        ],
    )
}

fn first_path_outside_allowed_scope(
    state: &AgentTaskState,
    tool_call: &ToolCall,
) -> Option<String> {
    let working_dirs = working_dirs_from_allowed_scope(state);
    if working_dirs.is_empty() {
        return None;
    }
    tool_paths(tool_call)
        .into_iter()
        .find(|path| path_outside_allowed_scope(path, &working_dirs))
}

fn working_dirs_from_allowed_scope(state: &AgentTaskState) -> Vec<PathBuf> {
    state
        .allowed_scope
        .iter()
        .filter_map(|scope| scope.strip_prefix("working_dir:"))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn path_outside_allowed_scope(path: &str, working_dirs: &[PathBuf]) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return false;
    }
    if path == ".." || path.starts_with("../") || path.starts_with("~/") {
        return true;
    }
    let target = Path::new(path);
    if !target.is_absolute() {
        return false;
    }
    let absolute_working_dirs = working_dirs
        .iter()
        .filter(|dir| dir.is_absolute())
        .collect::<Vec<_>>();
    if absolute_working_dirs.is_empty() {
        return true;
    }
    !absolute_working_dirs
        .iter()
        .any(|dir| target.starts_with(dir))
}

fn tool_paths(tool_call: &ToolCall) -> Vec<String> {
    let mut paths = Vec::new();
    collect_path_values(&tool_call.arguments, &mut paths);
    if tool_call.name == "bash" {
        if let Some(command) = tool_call
            .arguments
            .get("command")
            .and_then(serde_json::Value::as_str)
        {
            let classification =
                crate::tools::bash_tool::command_classifier::classify_command(command);
            paths.extend(classification.path_patterns);
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn collect_path_values(value: &serde_json::Value, paths: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let key = key.as_str();
                if matches!(
                    key,
                    "path" | "file_path" | "target_path" | "old_path" | "new_path" | "cwd"
                ) {
                    if let Some(path) = value.as_str() {
                        paths.push(path.to_string());
                    }
                    continue;
                }
                collect_path_values(value, paths);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_path_values(value, paths);
            }
        }
        _ => {}
    }
}

fn title_contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{IntentKind, IntentRouter, WorkflowKind};
    use crate::engine::session_goal::{GoalStatus, SessionGoal};
    use crate::engine::task_context::TaskContextBundle;

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

    #[test]
    fn allows_read_only_discovery_for_configuration_goal() {
        for tool_name in ["glob", "grep"] {
            let check = GoalDriftDetector::new().check(
                &goal(
                    IntentKind::Configuration,
                    "resume from local project evidence",
                ),
                &ToolCall {
                    id: "1".to_string(),
                    name: tool_name.to_string(),
                    arguments: serde_json::json!({"pattern": "fixtures/project/**/*"}),
                },
            );
            assert_eq!(check.level, DriftLevel::None);
        }
    }

    #[test]
    fn flags_state_forbidden_local_mutation_without_session_goal() {
        let route = IntentRouter::new().route("你好");
        let bundle = TaskContextBundle::new("你好", ".", route, None);

        let check = GoalDriftDetector::new().check_with_context(
            GoalDriftContext {
                goal: None,
                task_state: Some(&bundle.agent_state),
            },
            &ToolCall {
                id: "1".to_string(),
                name: "file_write".to_string(),
                arguments: serde_json::json!({"path": "src/lib.rs", "content": "x"}),
            },
        );

        assert_eq!(check.level, DriftLevel::High);
        assert!(check.reason.contains("forbids local mutation"));
    }

    #[test]
    fn flags_mutation_outside_task_allowed_scope() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let bundle = TaskContextBundle::new("修改 src/lib.rs", "/tmp/project", route, None);

        let check = GoalDriftDetector::new().check_with_context(
            GoalDriftContext {
                goal: None,
                task_state: Some(&bundle.agent_state),
            },
            &ToolCall {
                id: "1".to_string(),
                name: "file_write".to_string(),
                arguments: serde_json::json!({"path": "/etc/hosts", "content": "x"}),
            },
        );

        assert_eq!(check.level, DriftLevel::High);
        assert!(check
            .reason
            .contains("outside AgentTaskState.allowed_scope"));
    }

    #[test]
    fn allows_relative_mutation_inside_code_change_task_scope() {
        let route = IntentRouter::new().route("修改 src/lib.rs");
        let bundle = TaskContextBundle::new("修改 src/lib.rs", "/tmp/project", route, None);

        let check = GoalDriftDetector::new().check_with_context(
            GoalDriftContext {
                goal: None,
                task_state: Some(&bundle.agent_state),
            },
            &ToolCall {
                id: "1".to_string(),
                name: "file_edit".to_string(),
                arguments: serde_json::json!({
                    "path": "src/lib.rs",
                    "old_string": "old",
                    "new_string": "new"
                }),
            },
        );

        assert_eq!(check.level, DriftLevel::None);
    }
}

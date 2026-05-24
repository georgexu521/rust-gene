use super::ConversationLoop;
use crate::engine::task_context::AgentTaskStage;
use crate::services::api::{Message, Tool};
use std::collections::HashSet;

pub(super) struct ToolExposureRequest<'a> {
    pub(super) base_tools: &'a [Tool],
    pub(super) programming_workflow: bool,
    pub(super) task_stage: Option<AgentTaskStage>,
    pub(super) has_changes_before_request: bool,
    pub(super) action_checkpoint_active: bool,
    pub(super) action_checkpoint_lookup_count: usize,
    pub(super) action_checkpoint_requires_patch_before_validation: bool,
}

pub(super) struct ToolExposurePlan {
    pub(super) tools: Vec<Tool>,
    pub(super) exposed_tool_names: HashSet<String>,
    pub(super) focused_repair_prompt: Option<Message>,
}

impl ToolExposurePlan {
    pub(super) fn build(request: ToolExposureRequest<'_>) -> Self {
        let validation_allowed_before_request = request.has_changes_before_request
            && !request.action_checkpoint_requires_patch_before_validation;
        let allow_targeted_lookup = request.action_checkpoint_lookup_count
            < ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET;
        let tools = if request.action_checkpoint_active {
            let action_tools = ConversationLoop::code_action_tools(
                request.base_tools,
                validation_allowed_before_request,
                allow_targeted_lookup,
            );
            if action_tools.is_empty() {
                request.base_tools.to_vec()
            } else {
                action_tools
            }
        } else {
            request.base_tools.to_vec()
        };
        let tools = if request.programming_workflow && !request.action_checkpoint_active {
            phase_scoped_tools(
                &tools,
                request.task_stage.unwrap_or(AgentTaskStage::Understand),
            )
        } else {
            tools
        };

        let exposed_tool_names = tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<HashSet<_>>();
        let focused_repair_prompt = if request.action_checkpoint_active {
            let mut exposed_names = exposed_tool_names.iter().cloned().collect::<Vec<_>>();
            exposed_names.sort();
            Some(Message::system(
                ConversationLoop::focused_repair_mode_prompt(
                    &exposed_names,
                    request.action_checkpoint_lookup_count,
                ),
            ))
        } else {
            None
        };

        Self {
            tools,
            exposed_tool_names,
            focused_repair_prompt,
        }
    }
}

fn phase_scoped_tools(tools: &[Tool], stage: AgentTaskStage) -> Vec<Tool> {
    let scoped = tools
        .iter()
        .filter(|tool| phase_allows_tool(stage, &tool.name))
        .cloned()
        .collect::<Vec<_>>();
    if scoped.is_empty() {
        tools.to_vec()
    } else {
        scoped
    }
}

fn phase_allows_tool(stage: AgentTaskStage, name: &str) -> bool {
    match stage {
        AgentTaskStage::Understand => matches!(
            name,
            "project_list" | "glob" | "grep" | "file_read" | "lsp" | "symbol_query" | "ask_user"
        ),
        AgentTaskStage::Plan => matches!(
            name,
            "project_list"
                | "glob"
                | "grep"
                | "file_read"
                | "plan"
                | "enter_plan_mode"
                | "exit_plan_mode"
                | "todo_write"
                | "ask_user"
        ),
        AgentTaskStage::Edit => matches!(
            name,
            "project_list"
                | "glob"
                | "grep"
                | "file_read"
                | "file_write"
                | "file_edit"
                | "file_patch"
                | "todo_write"
                | "ask_user"
        ),
        AgentTaskStage::Validate => matches!(
            name,
            "file_read"
                | "grep"
                | "bash"
                | "run_tests"
                | "start_dev_server"
                | "bash_output"
                | "bash_cancel"
                | "diff"
                | "git"
                | "git_status"
                | "git_diff"
                | "format"
                | "ask_user"
        ),
        AgentTaskStage::Repair => matches!(
            name,
            "project_list"
                | "glob"
                | "grep"
                | "file_read"
                | "file_write"
                | "file_edit"
                | "file_patch"
                | "bash"
                | "run_tests"
                | "start_dev_server"
                | "install_dependencies"
                | "bash_output"
                | "bash_cancel"
                | "diff"
                | "git_status"
                | "git_diff"
                | "format"
                | "lsp"
                | "symbol_query"
                | "ask_user"
        ),
        AgentTaskStage::Closeout | AgentTaskStage::Done => {
            matches!(
                name,
                "file_read" | "diff" | "git" | "git_status" | "git_diff" | "ask_user"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            strict_schema: false,
        }
    }

    fn base_tools() -> Vec<Tool> {
        vec![
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
        ]
    }

    #[test]
    fn normal_mode_exposes_base_tools_without_focused_prompt() {
        let base_tools = base_tools();
        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: false,
            task_stage: None,
            has_changes_before_request: false,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: false,
        });

        assert_eq!(plan.tools.len(), base_tools.len());
        assert!(plan.exposed_tool_names.contains("bash"));
        assert!(plan.focused_repair_prompt.is_none());
    }

    #[test]
    fn action_checkpoint_exposes_patch_and_targeted_lookup_before_changes() {
        let base_tools = base_tools();
        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: true,
            task_stage: Some(AgentTaskStage::Repair),
            has_changes_before_request: false,
            action_checkpoint_active: true,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: false,
        });

        assert!(plan.exposed_tool_names.contains("file_edit"));
        assert!(plan.exposed_tool_names.contains("file_patch"));
        assert!(plan.exposed_tool_names.contains("file_read"));
        assert!(plan.exposed_tool_names.contains("grep"));
        assert!(!plan.exposed_tool_names.contains("bash"));
        assert!(!plan.exposed_tool_names.contains("run_tests"));
        assert!(!plan.exposed_tool_names.contains("start_dev_server"));
        assert!(!plan.exposed_tool_names.contains("install_dependencies"));
        assert!(!plan.exposed_tool_names.contains("git_status"));
        assert!(!plan.exposed_tool_names.contains("git_diff"));
        let Some(Message::System { content }) = plan.focused_repair_prompt else {
            panic!("focused repair prompt should be injected");
        };
        assert!(content.contains("file_edit, file_patch, file_read, grep"));
        assert!(content.contains("Up to 2 targeted file_read/grep lookups remain"));
    }

    #[test]
    fn action_checkpoint_allows_bash_validation_only_after_patch_is_not_required() {
        let base_tools = base_tools();
        let after_change = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: true,
            task_stage: Some(AgentTaskStage::Repair),
            has_changes_before_request: true,
            action_checkpoint_active: true,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: false,
        });
        assert!(after_change.exposed_tool_names.contains("bash"));
        assert!(after_change.exposed_tool_names.contains("run_tests"));
        assert!(after_change.exposed_tool_names.contains("start_dev_server"));
        assert!(!after_change
            .exposed_tool_names
            .contains("install_dependencies"));
        assert!(after_change.exposed_tool_names.contains("git_status"));
        assert!(after_change.exposed_tool_names.contains("git_diff"));

        let patch_required = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: true,
            task_stage: Some(AgentTaskStage::Repair),
            has_changes_before_request: true,
            action_checkpoint_active: true,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: true,
        });
        assert!(!patch_required.exposed_tool_names.contains("bash"));
        assert!(!patch_required.exposed_tool_names.contains("run_tests"));
        assert!(!patch_required
            .exposed_tool_names
            .contains("start_dev_server"));
        assert!(!patch_required
            .exposed_tool_names
            .contains("install_dependencies"));
        assert!(!patch_required.exposed_tool_names.contains("git_status"));
        assert!(!patch_required.exposed_tool_names.contains("git_diff"));
    }

    #[test]
    fn programming_understand_stage_exposes_only_inspection_tools() {
        let mut base_tools = base_tools();
        base_tools.push(tool("ask_user"));
        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: true,
            task_stage: Some(AgentTaskStage::Understand),
            has_changes_before_request: false,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: false,
        });

        assert!(plan.exposed_tool_names.contains("file_read"));
        assert!(plan.exposed_tool_names.contains("grep"));
        assert!(plan.exposed_tool_names.contains("ask_user"));
        assert!(!plan.exposed_tool_names.contains("file_edit"));
        assert!(!plan.exposed_tool_names.contains("file_patch"));
        assert!(!plan.exposed_tool_names.contains("bash"));
        assert!(!plan.exposed_tool_names.contains("run_tests"));
        assert!(!plan.exposed_tool_names.contains("start_dev_server"));
        assert!(!plan.exposed_tool_names.contains("install_dependencies"));
        assert!(!plan.exposed_tool_names.contains("git_status"));
        assert!(!plan.exposed_tool_names.contains("git_diff"));
    }

    #[test]
    fn programming_edit_stage_allows_write_but_not_validation_shell() {
        let base_tools = base_tools();
        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: true,
            task_stage: Some(AgentTaskStage::Edit),
            has_changes_before_request: false,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: false,
        });

        assert!(plan.exposed_tool_names.contains("file_edit"));
        assert!(plan.exposed_tool_names.contains("file_patch"));
        assert!(plan.exposed_tool_names.contains("file_read"));
        assert!(!plan.exposed_tool_names.contains("bash"));
        assert!(!plan.exposed_tool_names.contains("run_tests"));
        assert!(!plan.exposed_tool_names.contains("start_dev_server"));
        assert!(!plan.exposed_tool_names.contains("install_dependencies"));
        assert!(!plan.exposed_tool_names.contains("git_status"));
        assert!(!plan.exposed_tool_names.contains("git_diff"));
    }

    #[test]
    fn programming_validate_stage_allows_validation_but_not_write() {
        let base_tools = base_tools();
        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: true,
            task_stage: Some(AgentTaskStage::Validate),
            has_changes_before_request: true,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: false,
        });

        assert!(plan.exposed_tool_names.contains("bash"));
        assert!(plan.exposed_tool_names.contains("run_tests"));
        assert!(plan.exposed_tool_names.contains("start_dev_server"));
        assert!(!plan.exposed_tool_names.contains("install_dependencies"));
        assert!(plan.exposed_tool_names.contains("git_status"));
        assert!(plan.exposed_tool_names.contains("git_diff"));
        assert!(!plan.exposed_tool_names.contains("file_edit"));
        assert!(!plan.exposed_tool_names.contains("file_patch"));
    }

    #[test]
    fn programming_repair_stage_allows_dependency_install_facade() {
        let base_tools = base_tools();
        let plan = ToolExposurePlan::build(ToolExposureRequest {
            base_tools: &base_tools,
            programming_workflow: true,
            task_stage: Some(AgentTaskStage::Repair),
            has_changes_before_request: false,
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            action_checkpoint_requires_patch_before_validation: false,
        });

        assert!(plan.exposed_tool_names.contains("install_dependencies"));
        assert!(plan.exposed_tool_names.contains("file_edit"));
        assert!(plan.exposed_tool_names.contains("file_patch"));
    }
}

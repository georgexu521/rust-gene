use crate::services::api::{Message, Tool};
use std::collections::HashSet;

pub(super) struct ToolExposureRequest<'a> {
    pub(super) base_tools: &'a [Tool],
}

pub(super) struct ToolExposurePlan {
    pub(super) tools: Vec<Tool>,
    pub(super) exposed_tool_names: HashSet<String>,
    pub(super) focused_repair_prompt: Option<Message>,
}

impl ToolExposurePlan {
    pub(super) fn build(request: ToolExposureRequest<'_>) -> Self {
        let tools = request.base_tools.to_vec();
        let exposed_tool_names = tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<HashSet<_>>();

        Self {
            tools,
            exposed_tool_names,
            focused_repair_prompt: None,
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
        });

        assert_eq!(plan.tools.len(), base_tools.len());
        for tool in &base_tools {
            assert!(plan.exposed_tool_names.contains(&tool.name));
        }
        assert!(plan.focused_repair_prompt.is_none());
    }
}

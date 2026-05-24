use super::tool_context_helpers::tool_allowed_by_context;
use super::ConversationLoop;
use crate::engine::intent_router::{IntentKind, IntentRoute, WorkflowKind};
use crate::services::api::Tool;
use std::collections::HashSet;

impl ConversationLoop {
    /// 获取工具定义列表
    pub(super) fn get_tools(&self) -> Vec<Tool> {
        let context = self.create_tool_context();
        self.tool_registry
            .iter_tools()
            .filter(|t| {
                if !t.is_available(&context) {
                    return false;
                }
                tool_allowed_by_context(&self.allowed_tools, t.name())
                    && context.permission_context.should_expose_tool(t.name())
            })
            .map(|t| Tool {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
                strict_schema: t.strict_schema(),
            })
            .collect()
    }

    pub(super) fn get_tools_for_route(&self, route: &IntentRoute) -> Vec<Tool> {
        let tools = self.get_tools();
        Self::route_scoped_tools(&tools, route)
    }

    pub(super) fn route_scoped_tools(tools: &[Tool], route: &IntentRoute) -> Vec<Tool> {
        if !Self::route_scoped_tools_enabled() {
            return tools.to_vec();
        }

        let allowlist = Self::route_tool_allowlist(route);
        tools
            .iter()
            .filter(|tool| allowlist.contains(tool.name.as_str()))
            .cloned()
            .collect()
    }

    pub(crate) fn route_scoped_tools_enabled() -> bool {
        if Self::env_flag_disabled("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS") {
            return false;
        }
        if Self::env_flag_enabled("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE") {
            return false;
        }
        !matches!(
            std::env::var("PRIORITY_AGENT_TOOL_PROFILE")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "full" | "all" | "experimental"
        )
    }

    fn env_flag_enabled(name: &str) -> bool {
        matches!(
            std::env::var(name)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes" | "on"
        )
    }

    fn env_flag_disabled(name: &str) -> bool {
        matches!(
            std::env::var(name)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "0" | "false" | "no" | "off"
        )
    }

    pub(crate) fn route_tool_allowlist(route: &IntentRoute) -> HashSet<String> {
        let mut allowlist = route
            .recommended_tools
            .iter()
            .filter(|tool| Self::route_allows_recommended_tool(route, tool))
            .cloned()
            .collect::<HashSet<_>>();

        let tools: &[&str] = match route.intent {
            IntentKind::Memory => &["memory_load", "memory_save", "memory_clear", "ask_user"],
            IntentKind::Research => &[
                "web_search",
                "web_fetch",
                "project_list",
                "grep",
                "file_read",
                "ask_user",
            ],
            IntentKind::Configuration => &[
                "config",
                "mcp",
                "mcp_tool",
                "list_mcp_resources",
                "read_mcp_resource",
                "file_read",
                "bash",
                "ask_user",
            ],
            IntentKind::Delegation => &[
                "agent",
                "swarm",
                "task_create",
                "task_get",
                "task_list",
                "task_update",
                "task_stop",
                "task_output",
                "project_list",
                "grep",
                "file_read",
                "todo_write",
                "ask_user",
            ],
            _ => match route.workflow {
                WorkflowKind::CodeChange => &[
                    "project_list",
                    "glob",
                    "grep",
                    "file_read",
                    "file_write",
                    "file_edit",
                    "file_patch",
                    "bash",
                    "run_tests",
                    "start_dev_server",
                    "bash_output",
                    "bash_cancel",
                    "diff",
                    "git",
                    "git_status",
                    "git_diff",
                    "format",
                    "todo_write",
                    "ask_user",
                ],
                WorkflowKind::BugFix => &[
                    "project_list",
                    "glob",
                    "grep",
                    "file_read",
                    "file_write",
                    "file_edit",
                    "file_patch",
                    "bash",
                    "run_tests",
                    "start_dev_server",
                    "bash_output",
                    "bash_cancel",
                    "diff",
                    "git",
                    "git_status",
                    "git_diff",
                    "format",
                    "lsp",
                    "symbol_query",
                ],
                WorkflowKind::Planning => &[
                    "project_list",
                    "glob",
                    "grep",
                    "file_read",
                    "plan",
                    "enter_plan_mode",
                    "exit_plan_mode",
                    "todo_write",
                    "ask_user",
                ],
                WorkflowKind::Research => &[
                    "web_search",
                    "web_fetch",
                    "project_list",
                    "grep",
                    "file_read",
                    "ask_user",
                ],
                WorkflowKind::Delegation => &[
                    "agent",
                    "swarm",
                    "task_create",
                    "task_get",
                    "task_list",
                    "task_update",
                    "task_stop",
                    "task_output",
                    "project_list",
                    "grep",
                    "file_read",
                    "todo_write",
                    "ask_user",
                ],
                WorkflowKind::Direct if route.recommended_tools.is_empty() => &[],
                WorkflowKind::Direct => &["file_read", "glob", "ask_user"],
            },
        };
        allowlist.extend(tools.iter().map(|tool| (*tool).to_string()));
        allowlist
    }

    fn route_allows_recommended_tool(route: &IntentRoute, tool: &str) -> bool {
        match tool {
            "install_dependencies" => {
                route.dependency_install_intent
                    && matches!(
                        route.workflow,
                        WorkflowKind::Direct | WorkflowKind::CodeChange | WorkflowKind::BugFix
                    )
            }
            "mcp_auth" => route.mcp_auth_intent && route.intent == IntentKind::Configuration,
            _ => true,
        }
    }

    pub(super) fn code_action_tools(
        tools: &[Tool],
        has_changes_before_request: bool,
        allow_targeted_lookup: bool,
    ) -> Vec<Tool> {
        tools
            .iter()
            .filter(|tool| {
                Self::is_code_write_tool_name(&tool.name)
                    || (allow_targeted_lookup && matches!(tool.name.as_str(), "file_read" | "grep"))
                    || (has_changes_before_request
                        && matches!(
                            tool.name.as_str(),
                            "bash" | "run_tests" | "start_dev_server" | "git_status" | "git_diff"
                        ))
            })
            .cloned()
            .collect()
    }

    pub(super) fn is_code_write_tool_name(name: &str) -> bool {
        matches!(name, "file_edit" | "file_write" | "file_patch")
    }
}

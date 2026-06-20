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
        if self.allowed_tools.is_some() {
            return tools;
        }
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
        if !crate::services::config::runtime_config().route_scoped_tools_enabled() {
            return false;
        }
        if crate::services::config::runtime_config().debug_tool_exposure_enabled() {
            return false;
        }
        !matches!(
            crate::services::config::runtime_config()
                .tool_profile()
                .as_str(),
            "full" | "all" | "experimental"
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
                "mcp",
                "mcp_tool",
                "list_mcp_resources",
                "read_mcp_resource",
                "glob",
                "grep",
                "file_read",
                "bash",
                "ask_user",
            ],
            IntentKind::Delegation => &[
                "agent",
                "swarm",
                "project_list",
                "grep",
                "file_read",
                "todo_write",
                "ask_user",
            ],
            _ => match route.workflow {
                WorkflowKind::CodeChange => &[
                    // Read/search tools — always available.
                    "project_list",
                    "glob",
                    "grep",
                    "file_read",
                    // Primary edit tools — prefer these over bash for file mutation.
                    "file_write",
                    "file_edit",
                    "file_patch",
                    // Bash for read-only commands, running tests, starting services.
                    // File mutation via shell heredocs/redirects is blocked by permission
                    // and tool_batch processor; the route already deprioritizes bash.
                    "bash",
                    "run_tests",
                    "start_dev_server",
                    "bash_output",
                    "bash_cancel",
                    // Git and diff for version control and change review.
                    "diff",
                    "git",
                    "git_status",
                    "git_diff",
                    // Formatting, planning, and task tracking.
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

    #[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost_tracker::CostTracker;
    use crate::engine::intent_router::{
        IntentKind, IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
    };
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("streaming not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://tool-orchestrator"
        }

        fn default_model(&self) -> &str {
            "mock"
        }
    }

    fn direct_route() -> IntentRoute {
        IntentRoute {
            intent: IntentKind::DirectAnswer,
            confidence: 0.9,
            workflow: WorkflowKind::Direct,
            retrieval: RetrievalPolicy::Light,
            reasoning: ReasoningPolicy::Low,
            risk: RiskLevel::Low,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "direct route would normally hide write tools".to_string(),
        }
    }

    #[test]
    fn explicit_allowed_tools_bypass_route_scoped_request_exposure() {
        let mut conversation = ConversationLoop::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            Arc::new(Mutex::new(CostTracker::new())),
            "mock".to_string(),
        );
        conversation.allowed_tools = Some(HashSet::from([
            "file_write".to_string(),
            "bash".to_string(),
        ]));

        let names = conversation
            .get_tools_for_route(&direct_route())
            .into_iter()
            .map(|tool| tool.name)
            .collect::<HashSet<_>>();

        assert_eq!(
            names,
            HashSet::from(["file_write".to_string(), "bash".to_string()])
        );
    }
}

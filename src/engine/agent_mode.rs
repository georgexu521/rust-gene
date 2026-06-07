use crate::engine::intent_router::{
    IntentKind, IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
};

/// Product-level coding agent mode selected by the user.
///
/// This is intentionally small: hard constraints belong in route/tool policy,
/// while the model keeps freedom to solve the task inside that surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    #[default]
    Auto,
    Build,
    Plan,
    Explore,
    Review,
}

impl AgentMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Build => "build",
            Self::Plan => "plan",
            Self::Explore => "explore",
            Self::Review => "review",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" | "default" | "normal" => Some(Self::Auto),
            "build" | "code" | "implement" | "implementation" => Some(Self::Build),
            "plan" | "planning" => Some(Self::Plan),
            "explore" | "inspect" | "inspection" | "read" | "research-local" => Some(Self::Explore),
            "review" | "audit" | "code-review" | "codereview" => Some(Self::Review),
            _ => None,
        }
    }

    pub fn runtime_context(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Build => Some(
                "Active agent mode: build. Bias ambiguous coding requests toward implementation and verify changed behavior before finishing.",
            ),
            Self::Plan => Some(
                "Active agent mode: plan. Inspect the project and produce a concrete plan; do not modify files unless the user explicitly asks to implement.",
            ),
            Self::Explore => Some(
                "Active agent mode: explore. Ground answers in read/search/shell evidence; avoid file mutations unless the user explicitly asks for them.",
            ),
            Self::Review => Some(
                "Active agent mode: review. Use a code-review stance: findings and risks first, grounded in files or command output; avoid edits unless explicitly requested.",
            ),
        }
    }

    pub fn apply_to_route(self, route: &mut IntentRoute) {
        match self {
            Self::Auto => {}
            Self::Build => apply_build_route(route),
            Self::Plan => apply_plan_route(route),
            Self::Explore => apply_explore_route(route),
            Self::Review => apply_review_route(route),
        }
    }
}

fn apply_build_route(route: &mut IntentRoute) {
    let keep_dependency_install = route.dependency_install_intent
        && route
            .recommended_tools
            .iter()
            .any(|tool| tool == "install_dependencies");
    if matches!(
        route.intent,
        IntentKind::DirectAnswer | IntentKind::Planning | IntentKind::Unknown
    ) {
        route.intent = IntentKind::CodeChange;
        route.workflow = WorkflowKind::CodeChange;
        route.retrieval = RetrievalPolicy::Project;
        route.reasoning = ReasoningPolicy::High;
        if route.risk == RiskLevel::Low {
            route.risk = RiskLevel::Medium;
        }
        route.confidence = route.confidence.max(0.76);
    }
    replace_tools(
        route,
        &[
            "project_list",
            "glob",
            "grep",
            "file_read",
            "file_write",
            "file_edit",
            "bash",
            "run_tests",
            "start_dev_server",
            "diff",
            "git",
            "git_status",
            "git_diff",
            "format",
            "todo_write",
            "ask_user",
        ],
    );
    if keep_dependency_install {
        route
            .recommended_tools
            .push("install_dependencies".to_string());
    }
    push_reason(route, "agent mode: build");
}

fn apply_plan_route(route: &mut IntentRoute) {
    route.intent = IntentKind::Planning;
    route.workflow = WorkflowKind::Planning;
    route.retrieval = RetrievalPolicy::Project;
    route.reasoning = ReasoningPolicy::High;
    if route.risk == RiskLevel::Low {
        route.risk = RiskLevel::Medium;
    }
    route.confidence = route.confidence.max(0.78);
    replace_tools(
        route,
        &[
            "project_list",
            "glob",
            "grep",
            "file_read",
            "plan",
            "todo_write",
            "ask_user",
        ],
    );
    push_reason(route, "agent mode: plan");
}

fn apply_explore_route(route: &mut IntentRoute) {
    route.intent = IntentKind::DirectAnswer;
    route.workflow = WorkflowKind::Direct;
    route.retrieval = RetrievalPolicy::Project;
    route.reasoning = ReasoningPolicy::Medium;
    route.risk = RiskLevel::Low;
    route.confidence = route.confidence.max(0.74);
    replace_tools(
        route,
        &[
            "project_list",
            "glob",
            "grep",
            "file_read",
            "bash",
            "ask_user",
        ],
    );
    push_reason(route, "agent mode: explore");
}

fn apply_review_route(route: &mut IntentRoute) {
    route.intent = IntentKind::Debugging;
    route.workflow = WorkflowKind::Planning;
    route.retrieval = RetrievalPolicy::Project;
    route.reasoning = ReasoningPolicy::High;
    if route.risk == RiskLevel::Low {
        route.risk = RiskLevel::Medium;
    }
    route.confidence = route.confidence.max(0.78);
    replace_tools(
        route,
        &[
            "project_list",
            "glob",
            "grep",
            "file_read",
            "bash",
            "start_dev_server",
            "git_status",
            "git_diff",
            "diff",
            "git",
            "ask_user",
        ],
    );
    push_reason(route, "agent mode: review");
}

fn replace_tools(route: &mut IntentRoute, tools: &[&str]) {
    route.recommended_tools = tools.iter().map(|tool| (*tool).to_string()).collect();
}

fn push_reason(route: &mut IntentRoute, reason: &str) {
    if route.reason.trim().is_empty() {
        route.reason = reason.to_string();
    } else if !route.reason.contains(reason) {
        route.reason.push_str("; ");
        route.reason.push_str(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn parses_product_agent_modes() {
        assert_eq!(AgentMode::parse("auto"), Some(AgentMode::Auto));
        assert_eq!(AgentMode::parse("build"), Some(AgentMode::Build));
        assert_eq!(AgentMode::parse("planning"), Some(AgentMode::Plan));
        assert_eq!(AgentMode::parse("inspect"), Some(AgentMode::Explore));
        assert_eq!(AgentMode::parse("audit"), Some(AgentMode::Review));
        assert_eq!(AgentMode::parse("settings"), None);
    }

    #[test]
    fn plan_mode_keeps_write_tools_out_of_route_recommendations() {
        let mut route = IntentRouter::new().route("帮我实现这个功能");
        AgentMode::Plan.apply_to_route(&mut route);

        assert_eq!(route.workflow, WorkflowKind::Planning);
        assert!(!route.recommended_tools.contains(&"file_edit".to_string()));
        assert!(!route.recommended_tools.contains(&"file_write".to_string()));
    }

    #[test]
    fn build_mode_exposes_write_and_validation_tools() {
        let mut route = IntentRouter::new().route("我们下一步怎么做");
        AgentMode::Build.apply_to_route(&mut route);

        assert_eq!(route.intent, IntentKind::CodeChange);
        assert_eq!(route.workflow, WorkflowKind::CodeChange);
        assert!(route.recommended_tools.contains(&"file_edit".to_string()));
        assert!(route.recommended_tools.contains(&"bash".to_string()));
        assert!(!route
            .recommended_tools
            .contains(&"install_dependencies".to_string()));
    }

    #[test]
    fn build_mode_preserves_explicit_dependency_install_intent() {
        let mut route = IntentRouter::new().route("帮我安装项目依赖，package.json 已经在项目里");
        AgentMode::Build.apply_to_route(&mut route);

        assert!(route.dependency_install_intent);
        assert!(route
            .recommended_tools
            .contains(&"install_dependencies".to_string()));
    }

    #[test]
    fn read_oriented_modes_keep_write_tools_out_of_recommendations() {
        for mode in [AgentMode::Plan, AgentMode::Explore, AgentMode::Review] {
            let mut route = IntentRouter::new().route("帮我看看这个项目");
            mode.apply_to_route(&mut route);

            assert!(
                !route.recommended_tools.contains(&"file_edit".to_string()),
                "{mode:?} should not recommend file_edit"
            );
            assert!(
                !route.recommended_tools.contains(&"file_write".to_string()),
                "{mode:?} should not recommend file_write"
            );
        }
    }
}

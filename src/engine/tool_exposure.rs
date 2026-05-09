//! Diagnostics for whether a tool is visible to the model for a routed turn.

use crate::engine::conversation_loop::ConversationLoop;
use crate::engine::intent_router::IntentRoute;
use crate::tools::{ToolContext, ToolRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExposureReport {
    pub tool_name: String,
    pub registered: bool,
    pub available: bool,
    pub availability_reason: Option<String>,
    pub permission_exposed: bool,
    pub permission_reason: Option<String>,
    pub route_scoped_tools: bool,
    pub route_exposed: bool,
    pub route_reason: Option<String>,
    pub model_exposed: bool,
    pub hidden_reason: Option<String>,
}

impl ToolExposureReport {
    pub fn short_status(&self) -> &'static str {
        if self.model_exposed {
            "exposed"
        } else {
            "hidden"
        }
    }
}

pub fn diagnose_tool_exposure(
    registry: &ToolRegistry,
    context: &ToolContext,
    route: &IntentRoute,
    tool_name: &str,
) -> ToolExposureReport {
    let tool = registry.get(tool_name);
    let registered = tool.is_some();
    let available = tool.map(|tool| tool.is_available(context)).unwrap_or(false);
    let availability_reason = if registered && !available {
        tool.and_then(|tool| tool.unavailable_reason(context))
            .or_else(|| Some("tool reported unavailable".to_string()))
    } else if !registered {
        Some("tool is not registered in the runtime registry".to_string())
    } else {
        None
    };

    let permission_exposed = context.permission_context.should_expose_tool(tool_name);
    let permission_reason = if permission_exposed {
        None
    } else if matches!(
        context.permission_context.mode,
        crate::permissions::PermissionMode::ReadOnly
    ) {
        Some("permission mode is read_only".to_string())
    } else {
        Some("permission rules deny this tool".to_string())
    };

    let route_scoped_tools = ConversationLoop::route_scoped_tools_enabled();
    let route_exposed = if route_scoped_tools {
        ConversationLoop::route_tool_allowlist(route).contains(tool_name)
    } else {
        true
    };
    let route_reason = if route_exposed {
        None
    } else {
        Some(format!(
            "route {} did not include {}; route reason: {}",
            route.compact_label(),
            tool_name,
            route.reason
        ))
    };

    let model_exposed = registered && available && permission_exposed && route_exposed;
    let hidden_reason = if model_exposed {
        None
    } else {
        availability_reason
            .clone()
            .or(permission_reason.clone())
            .or(route_reason.clone())
            .or_else(|| Some("tool is hidden for an unknown reason".to_string()))
    };

    ToolExposureReport {
        tool_name: tool_name.to_string(),
        registered,
        available,
        availability_reason,
        permission_exposed,
        permission_reason,
        route_scoped_tools,
        route_exposed,
        route_reason,
        model_exposed,
        hidden_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::tools::{BashTool, FileReadTool};

    #[test]
    fn terminal_route_exposes_bash() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        guard.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        guard.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(BashTool);
        let mut context = ToolContext::new(".", "test");
        context.permission_context.rules = crate::permissions::PermissionRules::new();
        let route =
            IntentRouter::new().route("帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧");

        let report = diagnose_tool_exposure(&registry, &context, &route, "bash");

        assert!(report.registered);
        assert!(report.available);
        assert!(report.permission_exposed);
        assert!(report.route_exposed);
        assert!(report.model_exposed);
        assert_eq!(report.short_status(), "exposed");
    }

    #[test]
    fn read_only_mode_hides_bash_with_specific_reason() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.remove("PRIORITY_AGENT_ROUTE_SCOPED_TOOLS");
        guard.remove("PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE");
        guard.remove("PRIORITY_AGENT_TOOL_PROFILE");

        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(BashTool);
        let mut context = ToolContext::new(".", "test")
            .with_permission_mode(crate::permissions::PermissionMode::ReadOnly);
        context.permission_context.rules = crate::permissions::PermissionRules::new();
        let route =
            IntentRouter::new().route("帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧");

        let report = diagnose_tool_exposure(&registry, &context, &route, "bash");

        assert!(!report.permission_exposed);
        assert!(!report.model_exposed);
        assert_eq!(
            report.hidden_reason.as_deref(),
            Some("permission mode is read_only")
        );
    }
}

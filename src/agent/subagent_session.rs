//! Runtime policy for opencode-style sub-agent sessions.
//!
//! A sub-agent should run as a scoped child session with an explicit tool
//! surface and inherited deny rules. The model sees only the exposed tools; the
//! runtime still performs final permission checks at execution time.

use crate::agent::profiles::{AgentDefinition, AgentPermissionMode};
use crate::permissions::{PermissionContext, PermissionMode, PermissionRules, RuleSource};

const MUTATING_TOOLS: &[&str] = &[
    "file_write",
    "file_edit",
    "file_patch",
    "apply_patch",
    "format",
    "notebook",
    "memory_save",
    "memory_clear",
    "memory_tool",
    "rewind",
    "rewind_tool",
    "start_dev_server",
    "install_dependencies",
    "remote_dev",
    "worktree",
    "agent",
    "agent_tool",
    "swarm",
];

#[derive(Debug, Clone)]
pub struct SubagentSessionPolicy {
    pub permission_mode: PermissionMode,
    pub permission_rules: PermissionRules,
    pub exposed_tools: Vec<String>,
    pub inherited_parent_denies: Vec<String>,
    pub disallowed_tools: Vec<String>,
}

impl SubagentSessionPolicy {
    pub fn payload(&self) -> serde_json::Value {
        serde_json::json!({
            "permission_mode": format!("{:?}", self.permission_mode).to_ascii_lowercase(),
            "exposed_tools": self.exposed_tools,
            "inherited_parent_denies": self.inherited_parent_denies,
            "disallowed_tools": self.disallowed_tools,
            "rule_counts": {
                "allow": self.permission_rules.always_allow.len(),
                "deny": self.permission_rules.always_deny.len(),
                "ask": self.permission_rules.always_ask.len(),
            }
        })
    }
}

pub fn derive_subagent_session_policy(
    parent: &PermissionContext,
    definition: Option<&AgentDefinition>,
    allowed_tools: &[String],
) -> SubagentSessionPolicy {
    let mut rules = PermissionRules::new();
    let mut inherited_parent_denies = Vec::new();
    for rule in &parent.rules.always_deny {
        inherited_parent_denies.push(rule.pattern.clone());
        rules.always_deny.push(rule.clone());
    }

    if parent.mode == PermissionMode::ReadOnly {
        for tool in MUTATING_TOOLS {
            push_deny(&mut rules, *tool);
        }
        inherited_parent_denies.push("parent_mode=read_only".to_string());
    }

    let disallowed_tools = definition
        .map(|definition| definition.disallowed_tools.clone())
        .unwrap_or_default();
    for tool in &disallowed_tools {
        push_deny(&mut rules, tool);
    }

    let permission_mode = match definition.map(|definition| definition.permission_mode) {
        Some(AgentPermissionMode::ReadOnly) => {
            for tool in MUTATING_TOOLS {
                push_deny(&mut rules, *tool);
            }
            PermissionMode::AutoAll
        }
        Some(AgentPermissionMode::Bubble | AgentPermissionMode::IsolatedWrite) | None => {
            PermissionMode::AutoAll
        }
    };

    for tool in allowed_tools {
        rules
            .always_allow
            .push(crate::permissions::SourcedRule::new(
                tool.clone(),
                RuleSource::System,
            ));
    }

    let exposed_tools = allowed_tools
        .iter()
        .filter(|tool| !disallowed_tools.iter().any(|blocked| blocked == *tool))
        .filter(|tool| !is_denied(&rules, tool))
        .cloned()
        .collect();

    SubagentSessionPolicy {
        permission_mode,
        permission_rules: rules,
        exposed_tools,
        inherited_parent_denies,
        disallowed_tools,
    }
}

fn push_deny(rules: &mut PermissionRules, tool: impl Into<String>) {
    rules.always_deny.push(crate::permissions::SourcedRule::new(
        tool.into(),
        RuleSource::System,
    ));
}

fn is_denied(rules: &PermissionRules, tool: &str) -> bool {
    matches!(
        rules.check(tool),
        crate::permissions::PermissionDecision::Deny
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::profiles::{
        AgentContextMode, AgentDefinition, AgentMemoryPolicy, AgentModelPolicy,
        AgentOutputContract, AgentPermissionMode, AgentRiskPolicy,
    };
    use crate::agent::roles::AgentRole;

    fn definition(permission_mode: AgentPermissionMode) -> AgentDefinition {
        AgentDefinition {
            name: "worker".to_string(),
            agent_type: "worker".to_string(),
            profile_origin: Some("test".to_string()),
            profile_hash: Some("test-hash".to_string()),
            when_to_use: String::new(),
            role: AgentRole::Specialist,
            system_prompt: String::new(),
            prompt_version: None,
            tools: vec!["file_read".to_string(), "file_write".to_string()],
            disallowed_tools: vec!["agent".to_string()],
            permission_mode,
            model_policy: AgentModelPolicy::inherit(None),
            max_turns: 4,
            timeout_secs: 60,
            mcp_servers: Vec::new(),
            memory_policy: AgentMemoryPolicy::Session,
            context_mode: AgentContextMode::InheritedSummary,
            risk_policy: AgentRiskPolicy::ReadOnly,
            output_contract: AgentOutputContract::Findings,
        }
    }

    #[test]
    fn read_only_subagent_hides_mutating_tools_but_keeps_read_tools() {
        let parent = PermissionContext::new(".");
        let policy = derive_subagent_session_policy(
            &parent,
            Some(&definition(AgentPermissionMode::ReadOnly)),
            &["file_read".to_string(), "file_write".to_string()],
        );

        assert_eq!(policy.permission_mode, PermissionMode::AutoAll);
        assert_eq!(policy.exposed_tools, vec!["file_read"]);
        assert!(policy
            .permission_rules
            .always_deny
            .iter()
            .any(|rule| rule.pattern == "file_write"));
    }

    #[test]
    fn parent_denies_are_inherited_into_child_policy() {
        let mut parent = PermissionContext::new(".");
        parent.rules = PermissionRules::new().deny("bash");

        let policy = derive_subagent_session_policy(
            &parent,
            Some(&definition(AgentPermissionMode::IsolatedWrite)),
            &["bash".to_string(), "file_read".to_string()],
        );

        assert_eq!(policy.exposed_tools, vec!["file_read"]);
        assert_eq!(policy.inherited_parent_denies, vec!["bash"]);
    }
}

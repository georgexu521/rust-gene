//! Unified human review request model.
//!
//! Permissions, plan approval, goal drift, risky edits, and future fallback
//! decisions should all be expressible as the same review contract.

use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanReviewKind {
    ToolPermission,
    GoalDrift,
    PlanApproval,
    ReflectionGate,
    RiskyEdit,
    ModelFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanReviewRisk {
    Low,
    Medium,
    High,
}

impl HumanReviewRisk {
    pub fn as_str(self) -> &'static str {
        match self {
            HumanReviewRisk::Low => "low",
            HumanReviewRisk::Medium => "medium",
            HumanReviewRisk::High => "high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanReviewOption {
    pub id: String,
    pub label: String,
    pub impact: String,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanReviewRequest {
    pub kind: HumanReviewKind,
    pub title: String,
    pub reason: String,
    pub risk: HumanReviewRisk,
    pub subject: String,
    pub options: Vec<HumanReviewOption>,
    pub persistence_scope: Option<String>,
    pub impact: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionReviewDecision {
    ApproveOnce,
    ApproveSession,
    ApproveProject,
    ApproveGlobal,
    RejectOnce,
    RejectAlways,
}

impl PermissionReviewDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionReviewDecision::ApproveOnce => "approve_once",
            PermissionReviewDecision::ApproveSession => "approve_session",
            PermissionReviewDecision::ApproveProject => "approve_project",
            PermissionReviewDecision::ApproveGlobal => "approve_global",
            PermissionReviewDecision::RejectOnce => "reject_once",
            PermissionReviewDecision::RejectAlways => "reject_always",
        }
    }

    pub fn approved(self) -> bool {
        matches!(
            self,
            PermissionReviewDecision::ApproveOnce
                | PermissionReviewDecision::ApproveSession
                | PermissionReviewDecision::ApproveProject
                | PermissionReviewDecision::ApproveGlobal
        )
    }

    pub fn rule_decision(self) -> Option<&'static str> {
        match self {
            PermissionReviewDecision::ApproveSession
            | PermissionReviewDecision::ApproveProject
            | PermissionReviewDecision::ApproveGlobal => Some("allow"),
            PermissionReviewDecision::RejectAlways => Some("deny"),
            PermissionReviewDecision::ApproveOnce | PermissionReviewDecision::RejectOnce => None,
        }
    }

    pub fn persistence_scope(self) -> Option<&'static str> {
        match self {
            PermissionReviewDecision::ApproveOnce | PermissionReviewDecision::RejectOnce => None,
            PermissionReviewDecision::ApproveSession => Some("session"),
            PermissionReviewDecision::ApproveProject => Some("project"),
            PermissionReviewDecision::ApproveGlobal | PermissionReviewDecision::RejectAlways => {
                Some("global")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionReviewOption {
    pub key: String,
    pub decision: PermissionReviewDecision,
    pub label: String,
    pub impact: String,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionReview {
    pub request: HumanReviewRequest,
    pub tool_call_id: String,
    pub tool_name: String,
    pub rule_pattern: String,
    pub options: Vec<PermissionReviewOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanReviewAuditRecord {
    pub kind: HumanReviewKind,
    pub title: String,
    pub risk: HumanReviewRisk,
    pub subject: String,
    pub reason: String,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub input_summary: String,
    pub risk_facts: Vec<String>,
    pub matched_rules: Vec<String>,
    pub classifier_result: Option<serde_json::Value>,
    pub hook_decision: Option<String>,
    pub user_decision: Option<String>,
    pub persistence_scope: Option<String>,
    pub saved_config_path: Option<String>,
    pub recovery_hint: Option<String>,
}

impl HumanReviewAuditRecord {
    pub fn permission_requested(
        review: &PermissionReview,
        metadata: &serde_json::Value,
        matched_rules: Vec<String>,
        recovery_hint: Option<String>,
    ) -> Self {
        let classifier_result = metadata
            .get("command_classification")
            .filter(|value| !value.is_null())
            .cloned()
            .or_else(|| {
                metadata
                    .get("remote_classification")
                    .filter(|value| !value.is_null())
                    .cloned()
            });
        Self {
            kind: review.request.kind,
            title: review.request.title.clone(),
            risk: review.request.risk,
            subject: review.request.subject.clone(),
            reason: review.request.reason.clone(),
            tool_call_id: Some(review.tool_call_id.clone()),
            tool_name: Some(review.tool_name.clone()),
            input_summary: review.request.subject.clone(),
            risk_facts: permission_risk_facts(metadata),
            matched_rules,
            classifier_result,
            hook_decision: None,
            user_decision: None,
            persistence_scope: review
                .request
                .persistence_scope
                .clone()
                .or_else(|| Some("this_call".to_string())),
            saved_config_path: None,
            recovery_hint,
        }
    }

    pub fn with_resolution(
        mut self,
        user_decision: Option<String>,
        persistence_scope: Option<String>,
        saved_config_path: Option<String>,
    ) -> Self {
        self.user_decision = user_decision;
        if persistence_scope.is_some() {
            self.persistence_scope = persistence_scope;
        }
        self.saved_config_path = saved_config_path;
        self
    }
}

impl PermissionReview {
    pub fn from_tool_call(tool_call: &ToolCall, prompt: &str) -> Self {
        Self {
            request: HumanReviewRequest::tool_permission(tool_call, prompt),
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            rule_pattern: permission_rule_pattern(&tool_call.name, &tool_call.arguments),
            options: permission_review_options(),
        }
    }

    pub fn option_for_key(&self, key: &str) -> Option<&PermissionReviewOption> {
        self.options.iter().find(|option| option.key == key)
    }
}

impl HumanReviewRequest {
    pub fn plan_approval(title: &str, goal: &str, steps: usize, complexity: &str) -> Self {
        Self {
            kind: HumanReviewKind::PlanApproval,
            title: "Plan approval".to_string(),
            reason: format!(
                "execution plan '{}' needs approval before running {} step(s)",
                title, steps
            ),
            risk: if steps > 5 || complexity.eq_ignore_ascii_case("high") {
                HumanReviewRisk::High
            } else {
                HumanReviewRisk::Medium
            },
            subject: goal.to_string(),
            options: approve_deny_options(),
            persistence_scope: Some("this_plan".to_string()),
            impact: "Approving allows the agent to execute the proposed plan.".to_string(),
        }
    }

    pub fn tool_permission(tool_call: &ToolCall, prompt: &str) -> Self {
        let kind = if prompt
            .to_ascii_lowercase()
            .contains("may drift from the current goal")
        {
            HumanReviewKind::GoalDrift
        } else {
            HumanReviewKind::ToolPermission
        };
        let risk = match kind {
            HumanReviewKind::GoalDrift => HumanReviewRisk::High,
            HumanReviewKind::ToolPermission => {
                infer_tool_risk(&tool_call.name, &tool_call.arguments)
            }
            _ => HumanReviewRisk::Medium,
        };
        let reason = match kind {
            HumanReviewKind::GoalDrift => {
                "tool call may be unrelated to the active session goal".to_string()
            }
            HumanReviewKind::ToolPermission if tool_call.name == "reflection_review" => {
                "reflection found unresolved acceptance gaps before a risky workflow".to_string()
            }
            HumanReviewKind::ToolPermission => prompt.to_string(),
            _ => prompt.to_string(),
        };
        let subject = tool_subject(tool_call);
        Self {
            kind,
            title: match kind {
                HumanReviewKind::GoalDrift => "Goal drift approval".to_string(),
                _ => "Tool approval".to_string(),
            },
            reason,
            risk,
            subject,
            options: approve_deny_options(),
            persistence_scope: Some("this_call".to_string()),
            impact: prompt.to_string(),
        }
    }

    pub fn reflection_gate(
        subject: impl Into<String>,
        unresolved: usize,
        workflow: impl Into<String>,
    ) -> Self {
        let workflow = workflow.into();
        Self {
            kind: HumanReviewKind::ReflectionGate,
            title: "Reflection gate approval".to_string(),
            reason: format!(
                "reflection found {} unresolved acceptance gap(s) before a {} workflow",
                unresolved, workflow
            ),
            risk: HumanReviewRisk::High,
            subject: subject.into(),
            options: approve_deny_options(),
            persistence_scope: Some("this_reflection_gate".to_string()),
            impact:
                "Approving lets the risky workflow continue despite unresolved reflection findings."
                    .to_string(),
        }
    }
}

pub fn permission_rule_pattern(tool_name: &str, args: &serde_json::Value) -> String {
    if tool_name == "mcp_tool" {
        let server = args["server_name"].as_str().unwrap_or("");
        let tool = args["tool_name"].as_str().unwrap_or("");
        if !server.is_empty() && !tool.is_empty() {
            return format!("mcp/{}/{}", server, tool);
        }
    }
    if tool_name == "bash" {
        if let Some(command) = args["command"].as_str().or_else(|| args["cmd"].as_str()) {
            let classification =
                crate::tools::bash_tool::command_classifier::classify_command(command);
            if let Some(stable_prefix) = classification
                .permission_rule_suggestions
                .iter()
                .find(|rule| rule.stable)
            {
                return format!("bash:{}*", stable_prefix.pattern);
            }
            if let Some(exact) = classification.permission_rule_suggestions.first() {
                return format!("bash:{}", exact.pattern);
            }
            if !classification.normalized_command.trim().is_empty() {
                return format!("bash:{}", classification.normalized_command.trim());
            }
        }
    }
    tool_name.to_string()
}

fn permission_review_options() -> Vec<PermissionReviewOption> {
    vec![
        PermissionReviewOption {
            key: "y".to_string(),
            decision: PermissionReviewDecision::ApproveOnce,
            label: "allow once".to_string(),
            impact: "Approve only this pending call.".to_string(),
            default: false,
        },
        PermissionReviewOption {
            key: "s".to_string(),
            decision: PermissionReviewDecision::ApproveSession,
            label: "allow session".to_string(),
            impact: "Save an allow rule for this session.".to_string(),
            default: false,
        },
        PermissionReviewOption {
            key: "p".to_string(),
            decision: PermissionReviewDecision::ApproveProject,
            label: "allow project".to_string(),
            impact: "Persist an allow rule in the current project.".to_string(),
            default: false,
        },
        PermissionReviewOption {
            key: "a".to_string(),
            decision: PermissionReviewDecision::ApproveGlobal,
            label: "allow global".to_string(),
            impact: "Persist an allow rule globally.".to_string(),
            default: false,
        },
        PermissionReviewOption {
            key: "n".to_string(),
            decision: PermissionReviewDecision::RejectOnce,
            label: "deny".to_string(),
            impact: "Reject this call without saving a rule.".to_string(),
            default: true,
        },
        PermissionReviewOption {
            key: "x".to_string(),
            decision: PermissionReviewDecision::RejectAlways,
            label: "deny global".to_string(),
            impact: "Persist a global deny rule.".to_string(),
            default: false,
        },
    ]
}

fn approve_deny_options() -> Vec<HumanReviewOption> {
    vec![
        HumanReviewOption {
            id: "approve".to_string(),
            label: "Approve".to_string(),
            impact: "Run the requested action once.".to_string(),
            default: false,
        },
        HumanReviewOption {
            id: "deny".to_string(),
            label: "Deny".to_string(),
            impact: "Skip the action and return a denial to the model.".to_string(),
            default: true,
        },
    ]
}

fn infer_tool_risk(tool_name: &str, args: &serde_json::Value) -> HumanReviewRisk {
    let name = tool_name.to_ascii_lowercase();
    if name == "bash" {
        let cmd = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if cmd.contains("rm ")
            || cmd.contains("sudo")
            || cmd.contains("chmod")
            || cmd.contains("curl")
            || cmd.contains("wget")
        {
            return HumanReviewRisk::High;
        }
        return HumanReviewRisk::Medium;
    }
    if name.contains("mcp") || name.contains("write") || name.contains("edit") {
        return HumanReviewRisk::High;
    }
    if name == "reflection_review" {
        return HumanReviewRisk::High;
    }
    if name.contains("web") || name.contains("github") {
        return HumanReviewRisk::Medium;
    }
    HumanReviewRisk::Low
}

fn tool_subject(tool_call: &ToolCall) -> String {
    match tool_call.name.as_str() {
        "bash" => tool_call
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .map(|cmd| format!("bash: {}", cmd))
            .unwrap_or_else(|| "bash".to_string()),
        "mcp_tool" => {
            let server = tool_call
                .arguments
                .get("server_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let tool = tool_call
                .arguments
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("mcp: {}/{}", server, tool)
        }
        _ => tool_call.name.clone(),
    }
}

fn permission_risk_facts(metadata: &serde_json::Value) -> Vec<String> {
    let mut facts = Vec::new();
    push_string_fact(&mut facts, "family", metadata.get("permission_family"));
    push_string_fact(&mut facts, "decision", metadata.get("permission_decision"));
    push_string_fact(&mut facts, "risk", metadata.get("risk_level"));
    if let Some(warnings) = metadata.get("warnings").and_then(|value| value.as_array()) {
        for warning in warnings.iter().filter_map(|value| value.as_str()) {
            facts.push(format!("warning:{}", warning));
        }
    }
    if let Some(classification) = metadata.get("command_classification") {
        push_string_fact(
            &mut facts,
            "command_category",
            classification.get("command_category"),
        );
        push_string_fact(
            &mut facts,
            "command_kind",
            classification.get("command_kind"),
        );
        push_bool_fact(
            &mut facts,
            "network_access",
            classification.get("network_access"),
        );
        push_bool_fact(
            &mut facts,
            "external_path_access",
            classification.get("external_path_access"),
        );
        push_bool_fact(
            &mut facts,
            "requires_pty",
            classification.get("requires_pty"),
        );
        push_bool_fact(
            &mut facts,
            "risky_shell_wrapper",
            classification.get("risky_shell_wrapper"),
        );
    }
    if let Some(classification) = metadata.get("remote_classification") {
        push_string_fact(
            &mut facts,
            "remote_effect",
            classification.get("remote_effect"),
        );
        push_string_fact(&mut facts, "remote_risk", classification.get("risk_level"));
    }
    facts
}

fn push_string_fact(facts: &mut Vec<String>, key: &str, value: Option<&serde_json::Value>) {
    if let Some(value) = value.and_then(|value| value.as_str()) {
        if !value.is_empty() {
            facts.push(format!("{}:{}", key, value));
        }
    }
}

fn push_bool_fact(facts: &mut Vec<String>, key: &str, value: Option<&serde_json::Value>) {
    if matches!(value.and_then(|value| value.as_bool()), Some(true)) {
        facts.push(format!("{}:true", key));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[test]
    fn tool_permission_review_infers_high_risk_shell() {
        let req = HumanReviewRequest::tool_permission(
            &tool("bash", serde_json::json!({"command": "rm -rf target"})),
            "Tool 'bash' requires approval. Allow?",
        );
        assert_eq!(req.kind, HumanReviewKind::ToolPermission);
        assert_eq!(req.risk, HumanReviewRisk::High);
        assert!(req.subject.contains("rm -rf"));
        assert_eq!(req.options.len(), 2);
    }

    #[test]
    fn goal_drift_review_uses_goal_kind() {
        let req = HumanReviewRequest::tool_permission(
            &tool("bash", serde_json::json!({"command": "ls"})),
            "Tool 'bash' may drift from the current goal. Reason: unrelated Allow?",
        );
        assert_eq!(req.kind, HumanReviewKind::GoalDrift);
        assert_eq!(req.risk, HumanReviewRisk::High);
        assert!(req.reason.contains("active session goal"));
    }

    #[test]
    fn plan_review_marks_large_plan_high_risk() {
        let req = HumanReviewRequest::plan_approval("ship", "ship feature", 8, "high");
        assert_eq!(req.kind, HumanReviewKind::PlanApproval);
        assert_eq!(req.risk, HumanReviewRisk::High);
        assert!(req.subject.contains("ship feature"));
    }

    #[test]
    fn reflection_gate_review_is_high_risk() {
        let req = HumanReviewRequest::reflection_gate("pass-1", 2, "BugFix");
        assert_eq!(req.kind, HumanReviewKind::ReflectionGate);
        assert_eq!(req.risk, HumanReviewRisk::High);
        assert!(req.reason.contains("2 unresolved"));
    }

    #[test]
    fn permission_review_exposes_once_always_reject_actions() {
        let review = PermissionReview::from_tool_call(
            &tool("bash", serde_json::json!({"command": "npm run dev"})),
            "Allow bash?",
        );

        assert_eq!(review.rule_pattern, "bash:npm run dev");
        assert_eq!(
            review.option_for_key("y").unwrap().decision,
            PermissionReviewDecision::ApproveOnce
        );
        assert_eq!(
            review.option_for_key("s").unwrap().decision.rule_decision(),
            Some("allow")
        );
        assert_eq!(
            review
                .option_for_key("x")
                .unwrap()
                .decision
                .persistence_scope(),
            Some("global")
        );
        assert!(!review.option_for_key("n").unwrap().decision.approved());
    }

    #[test]
    fn permission_audit_record_extracts_risk_facts() {
        let review = PermissionReview::from_tool_call(
            &tool("bash", serde_json::json!({"command": "npm run dev"})),
            "Allow bash?",
        );
        let audit = HumanReviewAuditRecord::permission_requested(
            &review,
            &serde_json::json!({
                "permission_family": "shell",
                "permission_decision": "Ask",
                "risk_level": "Medium",
                "warnings": ["SHELL_NETWORK_ACCESS"],
                "command_classification": {
                    "command_category": "dev_server",
                    "command_kind": "unknown",
                    "network_access": true,
                    "external_path_access": false,
                    "requires_pty": false,
                    "risky_shell_wrapper": false
                }
            }),
            vec!["bash:npm run dev".to_string()],
            Some("Ask the user before retrying.".to_string()),
        );

        assert_eq!(audit.tool_name.as_deref(), Some("bash"));
        assert!(audit.risk_facts.contains(&"family:shell".to_string()));
        assert!(audit
            .risk_facts
            .contains(&"command_category:dev_server".to_string()));
        assert!(audit
            .risk_facts
            .contains(&"network_access:true".to_string()));
        assert_eq!(audit.matched_rules, vec!["bash:npm run dev"]);
    }

    #[test]
    fn permission_rule_pattern_uses_mcp_server_tool_scope() {
        assert_eq!(
            permission_rule_pattern(
                "mcp_tool",
                &serde_json::json!({"server_name": "filesystem", "tool_name": "write_file"})
            ),
            "mcp/filesystem/write_file"
        );
    }

    #[test]
    fn permission_rule_pattern_uses_bash_command_scope() {
        assert_eq!(
            permission_rule_pattern("bash", &serde_json::json!({"command": "cargo test -q"})),
            "bash:cargo test*"
        );
        assert_eq!(
            permission_rule_pattern("bash", &serde_json::json!({"command": "npm run dev"})),
            "bash:npm run dev"
        );
    }
}

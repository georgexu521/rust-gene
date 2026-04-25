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
}

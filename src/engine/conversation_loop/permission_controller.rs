use super::approval::{ToolApprovalChannel, ToolApprovalRequest};
use crate::engine::goal_drift::DriftCheck;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use crate::tools::{Tool, ToolContext, ToolErrorCode, ToolResult};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PermissionRequestKind {
    RuntimeRule,
    ToolConfirmation,
    GoalDrift,
}

impl PermissionRequestKind {
    fn as_str(self) -> &'static str {
        match self {
            PermissionRequestKind::RuntimeRule => "runtime_rule",
            PermissionRequestKind::ToolConfirmation => "tool_confirmation",
            PermissionRequestKind::GoalDrift => "goal_drift",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionToolFamily {
    Shell,
    File,
    ExternalDirectory,
    Task,
    Subagent,
    Other,
}

impl PermissionToolFamily {
    fn as_str(self) -> &'static str {
        match self {
            PermissionToolFamily::Shell => "shell",
            PermissionToolFamily::File => "file",
            PermissionToolFamily::ExternalDirectory => "external_directory",
            PermissionToolFamily::Task => "task",
            PermissionToolFamily::Subagent => "subagent",
            PermissionToolFamily::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PermissionRequestRecord {
    pub(super) id: String,
    pub(super) session_id: String,
    pub(super) kind: PermissionRequestKind,
    pub(super) patterns: Vec<String>,
    pub(super) metadata: serde_json::Value,
    pub(super) allowed_always_rules: Vec<String>,
    pub(super) rejection_feedback: String,
    pub(super) recovery_feedback: String,
}

impl PermissionRequestRecord {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "session_id": self.session_id,
            "kind": self.kind.as_str(),
            "patterns": self.patterns,
            "metadata": self.metadata,
            "allowed_always_rules": self.allowed_always_rules,
            "rejection_feedback": self.rejection_feedback,
            "recovery_feedback": self.recovery_feedback,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ToolPermissionEvaluation {
    pub(super) requires_approval: bool,
    pub(super) prompt: Option<String>,
    pub(super) record: Option<PermissionRequestRecord>,
}

pub(super) struct PermissionController;

impl PermissionController {
    pub(super) fn evaluate_tool_permission(
        session_id: &str,
        tool_call: &ToolCall,
        tool: &dyn Tool,
        context: &ToolContext,
        drift_check: &DriftCheck,
    ) -> ToolPermissionEvaluation {
        let tool_name = tool_call.name.as_str();
        let permission_explanation = context
            .permission_context
            .explain_decision(tool_name, &tool_call.arguments);
        let permission_requires = context
            .permission_context
            .requires_confirmation(tool_name, &tool_call.arguments);
        let raw_tool_requires = tool.requires_confirmation(&tool_call.arguments);
        let tool_requires = raw_tool_requires
            && !context
                .permission_context
                .auto_approves_tool_confirmation(tool_name, &tool_call.arguments);
        let drift_requires_approval = drift_check.requires_approval();

        if !(permission_requires || tool_requires || drift_requires_approval) {
            return ToolPermissionEvaluation {
                requires_approval: false,
                prompt: None,
                record: None,
            };
        }

        let kind = if drift_requires_approval {
            PermissionRequestKind::GoalDrift
        } else if permission_requires {
            PermissionRequestKind::RuntimeRule
        } else {
            PermissionRequestKind::ToolConfirmation
        };
        let prompt = permission_prompt(
            tool_call,
            tool,
            drift_check,
            drift_requires_approval,
            &permission_explanation,
        );
        let patterns = permission_explanation
            .matched_rules
            .iter()
            .map(|(_, rule)| rule.pattern.clone())
            .collect::<Vec<_>>();
        let family = permission_tool_family(tool_name, &permission_explanation);
        let allowed_always_rules = permission_explanation
            .matched_rules
            .iter()
            .filter(|(decision, _)| {
                matches!(decision, crate::permissions::PermissionDecision::Allow)
            })
            .map(|(_, rule)| rule.pattern.clone())
            .collect::<Vec<_>>();
        let rejection_feedback = format!(
            "Permission denied: '{}' requires user confirmation.",
            tool_name
        );
        let recovery_feedback = recovery_feedback(kind, family, tool_name);
        let record = PermissionRequestRecord {
            id: tool_call.id.clone(),
            session_id: session_id.to_string(),
            kind,
            patterns,
            metadata: serde_json::json!({
                "tool_name": tool_name,
                "arguments": tool_call.arguments,
                "permission_requires": permission_requires,
                "tool_requires": tool_requires,
                "raw_tool_requires": raw_tool_requires,
                "drift_requires_approval": drift_requires_approval,
                "permission_family": family.as_str(),
                "permission_decision": format!("{:?}", permission_explanation.decision),
                "risk_level": format!("{:?}", permission_explanation.risk_level),
                "warnings": permission_explanation.warnings,
                "reasons": permission_explanation.reasons,
                "drift_reason": drift_check.reason,
                "drift_suggested_action": drift_check.suggested_action,
            }),
            allowed_always_rules,
            rejection_feedback,
            recovery_feedback,
        };

        ToolPermissionEvaluation {
            requires_approval: true,
            prompt: Some(prompt),
            record: Some(record),
        }
    }

    pub(super) async fn request_user_permission(
        tool_call: &ToolCall,
        evaluation: &ToolPermissionEvaluation,
        approval_channel: Option<&Arc<ToolApprovalChannel>>,
        tx: Option<&mpsc::Sender<StreamEvent>>,
        trace: &Option<TraceCollector>,
    ) -> bool {
        let Some(prompt) = evaluation.prompt.as_ref() else {
            return true;
        };
        let mut approved = false;
        if let (Some(channel), Some(tx)) = (approval_channel, tx) {
            let _ = tx
                .send(StreamEvent::PermissionRequest {
                    id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                    prompt: prompt.clone(),
                })
                .await;
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::PermissionRequested {
                    tool: tool_call.name.clone(),
                    call_id: tool_call.id.clone(),
                    prompt: prompt.clone(),
                });
            }
            let request = ToolApprovalRequest {
                tool_call: tool_call.clone(),
                prompt: prompt.clone(),
                review: None,
            };
            match channel.submit(request).await {
                Ok(is_approved) => approved = is_approved,
                Err(e) => warn!("Tool approval error: {}", e),
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::PermissionResolved {
                    tool: tool_call.name.clone(),
                    call_id: tool_call.id.clone(),
                    approved,
                });
            }
        }
        approved
    }

    pub(super) fn record_approved_session_rule(context: &mut ToolContext, tool_name: &str) {
        if context.permission_context.mode == crate::permissions::PermissionMode::Once {
            context.permission_context.grant_once(tool_name);
        }
    }

    pub(super) fn denied_result(
        tool_name: &str,
        record: Option<&PermissionRequestRecord>,
    ) -> ToolResult {
        let message = record
            .map(permission_denied_message)
            .unwrap_or_else(|| {
                format!(
                    "Permission denied: '{}' requires user confirmation.\nRecovery: Ask the user for approval before retrying this tool, or choose a lower-risk alternative. Do not claim the tool ran.",
                    tool_name
                )
            });
        let mut result = ToolResult::error(message);
        result.error_code = Some(ToolErrorCode::PermissionDenied);
        if let Some(record) = record {
            result.data = Some(serde_json::json!({
                "permission_request": record.to_json(),
            }));
        }
        result
    }

    pub(super) fn is_permission_denied(result: &ToolResult) -> bool {
        matches!(result.error_code, Some(ToolErrorCode::PermissionDenied))
            || result
                .data
                .as_ref()
                .and_then(|data| data.get("permission_request"))
                .is_some()
            || result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("Permission denied")
    }
}

fn permission_denied_message(record: &PermissionRequestRecord) -> String {
    if record.recovery_feedback.trim().is_empty() {
        record.rejection_feedback.clone()
    } else {
        format!(
            "{}\nRecovery: {}",
            record.rejection_feedback, record.recovery_feedback
        )
    }
}

fn permission_tool_family(
    tool_name: &str,
    permission_explanation: &crate::permissions::ExplainableDecision,
) -> PermissionToolFamily {
    if permission_explanation
        .warnings
        .iter()
        .any(|warning| warning.contains("OUTSIDE_WORKSPACE"))
    {
        return PermissionToolFamily::ExternalDirectory;
    }

    match tool_name {
        "bash" | "powershell" => PermissionToolFamily::Shell,
        "file_read" | "file_write" | "file_edit" | "glob" | "grep" => PermissionToolFamily::File,
        "task_create" | "task_update" | "task_stop" | "task_output" => PermissionToolFamily::Task,
        "agent" | "send_message" => PermissionToolFamily::Subagent,
        _ => PermissionToolFamily::Other,
    }
}

fn recovery_feedback(
    kind: PermissionRequestKind,
    family: PermissionToolFamily,
    tool_name: &str,
) -> String {
    if kind == PermissionRequestKind::GoalDrift {
        return "Confirm the current goal or destructive scope with the user before retrying. Do not treat the blocked tool as executed.".to_string();
    }

    match family {
        PermissionToolFamily::Shell => {
            "Ask the user to approve the exact command, or use a read-only inspection command if that answers the task. Do not run a different risky command.".to_string()
        }
        PermissionToolFamily::ExternalDirectory => {
            "Ask the user to approve this external path/scope, or choose a path inside the trusted workspace. Do not claim files outside the workspace were changed.".to_string()
        }
        PermissionToolFamily::File => {
            "Ask the user to approve the file operation, narrow the edit scope, or use a read-only file inspection tool first. Do not claim the file changed.".to_string()
        }
        PermissionToolFamily::Task => {
            "Ask the user to approve task mutation, or continue with local reasoning without changing task state. Do not claim the task was updated.".to_string()
        }
        PermissionToolFamily::Subagent => {
            "Ask the user to approve delegation, or continue locally with the available context. Do not claim a sub-agent was started.".to_string()
        }
        PermissionToolFamily::Other => {
            format!("Ask the user to approve '{}', or choose a lower-risk alternative. Do not claim the tool ran.", tool_name)
        }
    }
}

fn permission_prompt(
    tool_call: &ToolCall,
    tool: &dyn Tool,
    drift_check: &DriftCheck,
    drift_requires_approval: bool,
    permission_explanation: &crate::permissions::ExplainableDecision,
) -> String {
    let tool_name = tool_call.name.as_str();
    let base_prompt = if drift_requires_approval {
        format!(
            "Tool '{}' may drift from the current goal. Reason: {} Suggested action: {} Allow?",
            tool_name,
            drift_check.reason,
            drift_check
                .suggested_action
                .as_deref()
                .unwrap_or("review before executing")
        )
    } else if tool_name == "mcp_tool" {
        let server = tool_call.arguments["server_name"].as_str().unwrap_or("");
        let t = tool_call.arguments["tool_name"].as_str().unwrap_or("");
        format!(
            "MCP tool '{}' on server '{}' requires approval. Allow?",
            t, server
        )
    } else if let Some(prompt) = tool.confirmation_prompt(&tool_call.arguments) {
        prompt
    } else {
        format!("Tool '{}' requires approval. Allow?", tool_name)
    };

    if drift_requires_approval {
        base_prompt
    } else {
        format!(
            "{}\nPermission explanation: {}",
            base_prompt,
            permission_explanation.concise_summary()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolContext, ToolResult};
    use serde_json::json;
    use tempfile::tempdir;

    struct ConfirmingTool;

    #[async_trait::async_trait]
    impl Tool for ConfirmingTool {
        fn name(&self) -> &str {
            "git"
        }

        fn description(&self) -> &str {
            "test tool"
        }

        fn parameters(&self) -> serde_json::Value {
            json!({"type": "object"})
        }

        async fn execute(&self, _params: serde_json::Value, _context: ToolContext) -> ToolResult {
            ToolResult::success("ok")
        }

        fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
            true
        }

        fn confirmation_prompt(&self, _params: &serde_json::Value) -> Option<String> {
            Some("Allow git mutation?".to_string())
        }
    }

    struct NamedTool {
        name: &'static str,
        requires_confirmation: bool,
    }

    #[async_trait::async_trait]
    impl Tool for NamedTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "test tool"
        }

        fn parameters(&self) -> serde_json::Value {
            json!({"type": "object"})
        }

        async fn execute(&self, _params: serde_json::Value, _context: ToolContext) -> ToolResult {
            ToolResult::success("ok")
        }

        fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
            self.requires_confirmation
        }
    }

    #[test]
    fn evaluates_tool_confirmation_as_structured_request() {
        let tmp = tempdir().expect("tempdir");
        let context = ToolContext::new(tmp.path(), "session-1");
        let tool_call = ToolCall {
            id: "call_git".to_string(),
            name: "git".to_string(),
            arguments: json!({"action": "push"}),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &ConfirmingTool,
            &context,
            &DriftCheck::ok(),
        );

        assert!(evaluation.requires_approval);
        assert_eq!(
            evaluation.record.as_ref().map(|record| record.kind),
            Some(PermissionRequestKind::RuntimeRule)
        );
        assert!(evaluation
            .prompt
            .as_deref()
            .unwrap_or_default()
            .contains("Permission explanation:"));
        assert_eq!(
            evaluation.record.as_ref().unwrap().metadata["tool_name"],
            "git"
        );
    }

    #[test]
    fn denied_result_carries_permission_request_metadata() {
        let record = PermissionRequestRecord {
            id: "call_1".to_string(),
            session_id: "session-1".to_string(),
            kind: PermissionRequestKind::ToolConfirmation,
            patterns: vec!["git".to_string()],
            metadata: json!({"tool_name": "git"}),
            allowed_always_rules: Vec::new(),
            rejection_feedback: "Permission denied: 'git' requires user confirmation.".to_string(),
            recovery_feedback: "Ask the user to approve 'git' before retrying.".to_string(),
        };

        let result = PermissionController::denied_result("git", Some(&record));

        assert!(PermissionController::is_permission_denied(&result));
        assert!(result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Recovery: Ask the user"));
        assert_eq!(
            result.data.as_ref().unwrap()["permission_request"]["kind"],
            "tool_confirmation"
        );
        assert_eq!(
            result.data.as_ref().unwrap()["permission_request"]["session_id"],
            "session-1"
        );
    }

    #[test]
    fn external_file_edit_records_external_directory_family() {
        let tmp = tempdir().expect("tempdir");
        let context = ToolContext::new(tmp.path(), "session-1");
        let tool_call = ToolCall {
            id: "call_edit_external".to_string(),
            name: "file_edit".to_string(),
            arguments: json!({
                "path": "/Users/georgexu/Desktop/outside.rs",
                "old_string": "a",
                "new_string": "b"
            }),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &NamedTool {
                name: "file_edit",
                requires_confirmation: true,
            },
            &context,
            &DriftCheck::ok(),
        );

        let metadata = &evaluation.record.as_ref().unwrap().metadata;
        assert!(evaluation.requires_approval);
        assert_eq!(metadata["permission_family"], "external_directory");
        assert_eq!(metadata["permission_requires"], true);
        assert!(evaluation
            .record
            .as_ref()
            .unwrap()
            .recovery_feedback
            .contains("external path/scope"));
    }

    #[test]
    fn explicit_task_rule_records_task_family() {
        let tmp = tempdir().expect("tempdir");
        let mut context = ToolContext::new(tmp.path(), "session-1");
        context.permission_context.rules =
            crate::permissions::PermissionRules::new().ask("task_stop");
        let tool_call = ToolCall {
            id: "call_task_stop".to_string(),
            name: "task_stop".to_string(),
            arguments: json!({"task_id": "task_123"}),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &NamedTool {
                name: "task_stop",
                requires_confirmation: false,
            },
            &context,
            &DriftCheck::ok(),
        );

        let metadata = &evaluation.record.as_ref().unwrap().metadata;
        assert!(evaluation.requires_approval);
        assert_eq!(metadata["permission_family"], "task");
        assert_eq!(metadata["permission_requires"], true);
        assert!(evaluation
            .record
            .as_ref()
            .unwrap()
            .recovery_feedback
            .contains("task mutation"));
    }

    #[test]
    fn explicit_subagent_rule_records_subagent_family() {
        let tmp = tempdir().expect("tempdir");
        let mut context = ToolContext::new(tmp.path(), "session-1");
        context.permission_context.rules = crate::permissions::PermissionRules::new().ask("agent");
        let tool_call = ToolCall {
            id: "call_agent".to_string(),
            name: "agent".to_string(),
            arguments: json!({"description": "inspect module", "prompt": "inspect"}),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &NamedTool {
                name: "agent",
                requires_confirmation: true,
            },
            &context,
            &DriftCheck::ok(),
        );

        let metadata = &evaluation.record.as_ref().unwrap().metadata;
        assert!(evaluation.requires_approval);
        assert_eq!(metadata["permission_family"], "subagent");
        assert_eq!(metadata["permission_requires"], true);
        assert!(evaluation
            .record
            .as_ref()
            .unwrap()
            .recovery_feedback
            .contains("delegation"));
    }
}

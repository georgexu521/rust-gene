use super::approval::{ToolApprovalChannel, ToolApprovalRequest};
use super::permission_recovery::{
    permission_denial_state_json, permission_denied_message, record_permission_denial,
    recovery_feedback,
};
use crate::engine::action_review::ActionReview;
use crate::engine::goal_drift::DriftCheck;
use crate::engine::hooks::ToolHookManager;
use crate::engine::human_review::{
    HumanReviewAuditRecord, PermissionReview, PermissionReviewDecision,
};
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::permissions::{PermissionDecision, RuleSource};
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

pub(super) struct PermissionRequestRuntime<'a> {
    pub approval_channel: Option<&'a Arc<ToolApprovalChannel>>,
    pub tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub trace: &'a Option<TraceCollector>,
    pub hook_manager: Option<&'a Arc<ToolHookManager>>,
    pub context: &'a ToolContext,
    pub action_review: Option<&'a ActionReview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PermissionApprovalOutcome {
    pub(super) approved: bool,
    pub(super) source: String,
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
pub(super) enum PermissionToolFamily {
    Shell,
    File,
    ExternalDirectory,
    Task,
    Subagent,
    Remote,
    Other,
}

impl PermissionToolFamily {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            PermissionToolFamily::Shell => "shell",
            PermissionToolFamily::File => "file",
            PermissionToolFamily::ExternalDirectory => "external_directory",
            PermissionToolFamily::Task => "task",
            PermissionToolFamily::Subagent => "subagent",
            PermissionToolFamily::Remote => "remote",
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
    pub(super) review: HumanReviewAuditRecord,
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
            "review": self.review,
            "rejection_feedback": self.rejection_feedback,
            "recovery_feedback": self.recovery_feedback,
        })
    }

    pub(super) fn to_json_with_approval_source(
        &self,
        approved: bool,
        source: Option<&str>,
    ) -> serde_json::Value {
        let mut value = self.to_json();
        if let Some(object) = value.as_object_mut() {
            object.insert("approved".to_string(), serde_json::Value::Bool(approved));
            if let Some(source) = source {
                object.insert(
                    "permission_source".to_string(),
                    serde_json::Value::String(source.to_string()),
                );
                if let Some(metadata) = object
                    .get_mut("metadata")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    metadata.insert(
                        "resolved_permission_source".to_string(),
                        serde_json::Value::String(source.to_string()),
                    );
                }
            }
        }
        value
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
        let permission_source = initial_permission_source(
            kind,
            permission_requires,
            tool_requires,
            drift_requires_approval,
            &permission_explanation,
        );
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
        let denial_state = permission_denial_state_json(session_id, family, tool_name, false);
        let command_classification =
            permission_command_classification(tool_name, &tool_call.arguments);
        let remote_classification =
            permission_remote_classification(tool_name, &tool_call.arguments);
        let search_or_read =
            serde_json::to_value(tool.is_search_or_read_command(&tool_call.arguments))
                .unwrap_or(serde_json::Value::Null);
        let tool_kind = serde_json::to_value(tool.tool_kind(&tool_call.arguments))
            .unwrap_or(serde_json::Value::Null);
        let tool_family = serde_json::to_value(tool.tool_family(&tool_call.arguments))
            .unwrap_or(serde_json::Value::Null);
        let ui_render_kind = serde_json::to_value(tool.ui_render_kind(&tool_call.arguments))
            .unwrap_or(serde_json::Value::Null);
        let permission_evidence = permission_decision_evidence_json(PermissionEvidenceInput {
            kind,
            family,
            tool_name,
            tool_call,
            context,
            permission_requires,
            tool_requires,
            raw_tool_requires,
            drift_requires_approval,
            permission_explanation: &permission_explanation,
            patterns: &patterns,
            allowed_always_rules: &allowed_always_rules,
            command_classification: &command_classification,
            remote_classification: &remote_classification,
            recovery_feedback: &recovery_feedback,
            denial_state: &denial_state,
        });
        let metadata = serde_json::json!({
            "tool_name": tool_name,
            "arguments": tool_call.arguments,
            "permission_evidence": permission_evidence,
            "permission_requires": permission_requires,
            "tool_requires": tool_requires,
            "raw_tool_requires": raw_tool_requires,
            "drift_requires_approval": drift_requires_approval,
            "permission_family": family.as_str(),
            "permission_decision": format!("{:?}", permission_explanation.decision),
            "permission_source": permission_source,
            "risk_level": format!("{:?}", permission_explanation.risk_level),
            "permission_matcher_input": tool.permission_matcher_input(&tool_call.arguments),
            "input_paths": tool.input_paths(&tool_call.arguments),
            "open_world": tool.is_open_world(&tool_call.arguments),
            "search_or_read": search_or_read,
            "tool_kind": tool_kind,
            "tool_family": tool_family,
            "ui_render_kind": ui_render_kind,
            "command_classification": command_classification,
            "remote_classification": remote_classification,
            "denial_state": denial_state,
            "warnings": permission_explanation.warnings,
            "reasons": permission_explanation.reasons,
            "drift_reason": drift_check.reason,
            "drift_suggested_action": drift_check.suggested_action,
        });
        let permission_review = PermissionReview::from_tool_call(tool_call, &prompt);
        let review = HumanReviewAuditRecord::permission_requested(
            &permission_review,
            &metadata,
            patterns.clone(),
            Some(recovery_feedback.clone()),
        );
        let record = PermissionRequestRecord {
            id: tool_call.id.clone(),
            session_id: session_id.to_string(),
            kind,
            patterns,
            metadata,
            allowed_always_rules,
            review,
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
        runtime: PermissionRequestRuntime<'_>,
    ) -> PermissionApprovalOutcome {
        let initial_source = evaluation
            .record
            .as_ref()
            .and_then(|record| {
                record
                    .metadata
                    .get("permission_source")
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or("runtime_rule")
            .to_string();
        let Some(prompt) = evaluation.prompt.as_ref() else {
            return PermissionApprovalOutcome {
                approved: true,
                source: initial_source,
            };
        };
        if let Some(ref trace) = runtime.trace {
            trace.record(TraceEvent::PermissionRequested {
                tool: tool_call.name.clone(),
                call_id: tool_call.id.clone(),
                prompt: prompt.clone(),
                review: evaluation
                    .record
                    .as_ref()
                    .map(|record| record.review.clone()),
            });
        }
        if let Some(hooks) = runtime.hook_manager {
            let hook_start = hooks.current_record_sequence();
            let decision = hooks
                .run_permission_request(tool_call, runtime.context)
                .await;
            let hook_records = hooks.recent_records_after_for(hook_start, &tool_call.id);
            super::turn_recording::record_hook_traces(runtime.trace, &hook_records);
            if !decision.allow {
                if let Some(ref trace) = runtime.trace {
                    trace.record(TraceEvent::PermissionResolved {
                        tool: tool_call.name.clone(),
                        call_id: tool_call.id.clone(),
                        approved: false,
                        source: Some("hook_deny".to_string()),
                        decision: Some("hook_denied".to_string()),
                        persistence_scope: None,
                        rule_pattern: None,
                        persisted_path: None,
                        review: evaluation.record.as_ref().map(|record| {
                            let mut review = record.review.clone().with_resolution(
                                Some("hook_denied".to_string()),
                                None,
                                None,
                            );
                            review.hook_decision = decision.reason.clone();
                            review
                        }),
                    });
                }
                return PermissionApprovalOutcome {
                    approved: false,
                    source: "hook_deny".to_string(),
                };
            }
        }
        let mut approved = false;
        let mut resolved_source = "approval_unavailable".to_string();
        if let (Some(channel), Some(tx)) = (runtime.approval_channel, runtime.tx) {
            let _ = tx
                .send(StreamEvent::PermissionRequest {
                    id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                    prompt: prompt.clone(),
                    metadata: permission_request_metadata(
                        evaluation.record.as_ref(),
                        runtime.action_review,
                    ),
                    review: evaluation
                        .record
                        .as_ref()
                        .map(|record| Box::new(record.review.clone())),
                })
                .await;
            let request = ToolApprovalRequest {
                tool_call: tool_call.clone(),
                prompt: prompt.clone(),
                review: None,
                audit: evaluation
                    .record
                    .as_ref()
                    .map(|record| record.review.clone()),
                diff_preview: None,
            };
            let mut approval_response = None;
            match channel.submit(request).await {
                Ok(response) => {
                    approved = response.approved;
                    resolved_source = permission_source_for_approval_response(&response);
                    approval_response = Some(response);
                }
                Err(e) => warn!("Tool approval error: {}", e),
            }
            if let Some(hooks) = runtime.hook_manager {
                let hook_start = hooks.current_record_sequence();
                let decision = hooks
                    .run_permission_resolved(tool_call, runtime.context, approved)
                    .await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tool_call.id);
                super::turn_recording::record_hook_traces(runtime.trace, &hook_records);
                if approved && !decision.allow {
                    approved = false;
                    resolved_source = "hook_deny".to_string();
                    if approval_response.is_none() {
                        approval_response =
                            Some(super::approval::ToolApprovalResponse::rejected_once());
                    }
                }
            }
            if let Some(ref trace) = runtime.trace {
                trace.record(TraceEvent::PermissionResolved {
                    tool: tool_call.name.clone(),
                    call_id: tool_call.id.clone(),
                    approved,
                    source: Some(resolved_source.clone()),
                    decision: approval_response
                        .as_ref()
                        .and_then(|response| response.decision_label().map(str::to_string)),
                    persistence_scope: approval_response
                        .as_ref()
                        .and_then(|response| response.persistence_scope.clone()),
                    rule_pattern: approval_response
                        .as_ref()
                        .and_then(|response| response.rule_pattern.clone()),
                    persisted_path: approval_response
                        .as_ref()
                        .and_then(|response| response.persisted_path.clone()),
                    review: evaluation.record.as_ref().map(|record| {
                        record.review.clone().with_resolution(
                            approval_response
                                .as_ref()
                                .and_then(|response| response.decision_label().map(str::to_string)),
                            approval_response
                                .as_ref()
                                .and_then(|response| response.persistence_scope.clone()),
                            approval_response
                                .as_ref()
                                .and_then(|response| response.persisted_path.clone()),
                        )
                    }),
                });
            }
        }
        PermissionApprovalOutcome {
            approved,
            source: resolved_source,
        }
    }

    pub(super) fn record_approved_session_rule(context: &mut ToolContext, tool_name: &str) {
        if context.permission_context.mode == crate::permissions::PermissionMode::Once {
            context.permission_context.grant_once(tool_name);
        }
    }

    pub(super) fn denied_result(
        tool_name: &str,
        record: Option<&PermissionRequestRecord>,
        permission_source: Option<&str>,
    ) -> ToolResult {
        let denial_state = record.map(record_permission_denial);
        let mut message = record
            .map(permission_denied_message)
            .unwrap_or_else(|| {
                format!(
                    "Permission denied: '{}' requires user confirmation.\nRecovery: Ask the user for approval before retrying this tool, or choose a lower-risk alternative. Do not claim the tool ran.",
                    tool_name
                )
            });
        if denial_state
            .as_ref()
            .and_then(|state| state.get("bounded_recovery"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            message.push_str(
                "\nRecovery: This permission family has been denied repeatedly in this session. Stop retrying the same risky action; ask the user for a new approval or choose a lower-risk inspection path.",
            );
        }
        let mut result = ToolResult::error(message);
        result.error_code = Some(ToolErrorCode::PermissionDenied);
        if let Some(record) = record {
            result.data = Some(serde_json::json!({
                "permission_request": record.to_json_with_approval_source(false, permission_source),
                "denial_state": denial_state,
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

fn permission_request_metadata(
    record: Option<&PermissionRequestRecord>,
    action_review: Option<&ActionReview>,
) -> Option<serde_json::Value> {
    let mut metadata = record
        .map(|record| record.metadata.clone())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(review) = action_review {
        if let Some(object) = metadata.as_object_mut() {
            object.insert("action_review".to_string(), review.metadata());
        }
    }
    metadata
        .as_object()
        .is_some_and(|object| !object.is_empty())
        .then_some(metadata)
}

fn initial_permission_source(
    kind: PermissionRequestKind,
    permission_requires: bool,
    tool_requires: bool,
    drift_requires_approval: bool,
    explanation: &crate::permissions::ExplainableDecision,
) -> String {
    if drift_requires_approval || matches!(kind, PermissionRequestKind::GoalDrift) {
        return "goal_drift".to_string();
    }
    if permission_requires {
        if let Some(source) = permission_source_from_rules(&explanation.matched_rules) {
            return source;
        }
        return "runtime_rule".to_string();
    }
    if tool_requires || matches!(kind, PermissionRequestKind::ToolConfirmation) {
        return "tool_confirmation".to_string();
    }
    "runtime_rule".to_string()
}

fn permission_source_from_rules(
    rules: &[(PermissionDecision, crate::permissions::SourcedRule)],
) -> Option<String> {
    rules.iter().find_map(|(decision, rule)| {
        permission_source_from_rule(*decision, rule.source).map(str::to_string)
    })
}

fn permission_source_from_rule(
    decision: PermissionDecision,
    source: RuleSource,
) -> Option<&'static str> {
    match (decision, source) {
        (PermissionDecision::Allow, RuleSource::Global) => Some("config_global_allow"),
        (PermissionDecision::Allow, RuleSource::Project) => Some("config_project_allow"),
        (PermissionDecision::Allow, RuleSource::User) => Some("config_session_allow"),
        (PermissionDecision::Allow, RuleSource::System) => Some("runtime_rule"),
        (PermissionDecision::Deny, RuleSource::Global) => Some("config_global_deny"),
        (PermissionDecision::Deny, RuleSource::Project) => Some("config_project_deny"),
        (PermissionDecision::Deny, RuleSource::User) => Some("config_session_deny"),
        (PermissionDecision::Deny, RuleSource::System) => Some("runtime_rule"),
        (PermissionDecision::Ask, RuleSource::Global) => Some("config_global_ask"),
        (PermissionDecision::Ask, RuleSource::Project) => Some("config_project_ask"),
        (PermissionDecision::Ask, RuleSource::User) => Some("config_session_ask"),
        (PermissionDecision::Ask, RuleSource::System) => Some("runtime_rule"),
    }
}

fn permission_source_for_approval_response(
    response: &super::approval::ToolApprovalResponse,
) -> String {
    let decision = response.decision.unwrap_or(if response.approved {
        PermissionReviewDecision::ApproveOnce
    } else {
        PermissionReviewDecision::RejectOnce
    });
    match decision {
        PermissionReviewDecision::ApproveOnce => "user_once_allow",
        PermissionReviewDecision::ApproveSession => "user_session_allow",
        PermissionReviewDecision::ApproveProject => "user_project_allow",
        PermissionReviewDecision::ApproveGlobal => "user_global_allow",
        PermissionReviewDecision::RejectOnce => "user_once_reject",
        PermissionReviewDecision::RejectAlways => "user_global_deny",
    }
    .to_string()
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
        "file_read" | "file_write" | "file_edit" | "file_patch" | "glob" | "grep" => {
            PermissionToolFamily::File
        }
        "task_create" | "task_update" | "task_stop" | "task_output" => PermissionToolFamily::Task,
        "agent" | "send_message" => PermissionToolFamily::Subagent,
        "remote_trigger" | "remote_dev" => PermissionToolFamily::Remote,
        _ => PermissionToolFamily::Other,
    }
}

fn permission_command_classification(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> serde_json::Value {
    if tool_name != "bash" {
        return serde_json::Value::Null;
    }
    let Some(command) = arguments.get("command").and_then(serde_json::Value::as_str) else {
        return serde_json::Value::Null;
    };
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    serde_json::json!({
        "command_kind": classification.command_kind,
        "command_category": classification.category,
        "validation_family": classification.validation_family,
        "parser_status": classification.parser_status,
        "subcommands": classification.subcommands,
        "redirections": classification.redirections,
        "mutation_paths": classification.mutation_paths,
        "mutation_indicators": classification.mutation_indicators,
        "command_plan": classification.command_plan,
        "path_patterns": classification.path_patterns,
        "safe_for_closeout": classification.safe_for_closeout,
        "requires_pty": classification.requires_pty(),
        "network_access": classification.network_access,
        "external_path_access": classification.external_path_access,
        "absolute_path_patterns": classification.absolute_path_patterns,
        "compound_command": classification.compound_command,
        "shell_control_operators": classification.shell_control_operators,
        "risky_shell_wrapper": classification.risky_shell_wrapper,
        "expected_silent_output": classification.expected_silent_output,
        "permission_rule_suggestions": classification.permission_rule_suggestions,
    })
}

struct PermissionEvidenceInput<'a> {
    kind: PermissionRequestKind,
    family: PermissionToolFamily,
    tool_name: &'a str,
    tool_call: &'a ToolCall,
    context: &'a ToolContext,
    permission_requires: bool,
    tool_requires: bool,
    raw_tool_requires: bool,
    drift_requires_approval: bool,
    permission_explanation: &'a crate::permissions::ExplainableDecision,
    patterns: &'a [String],
    allowed_always_rules: &'a [String],
    command_classification: &'a serde_json::Value,
    remote_classification: &'a serde_json::Value,
    recovery_feedback: &'a str,
    denial_state: &'a serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionPipelineStage {
    RuleDecision,
    RiskAssessment,
    ShellFailClosed,
    ToolConfirmation,
    GoalDrift,
    PromptRequired,
    DenialTracking,
    RecoveryGuidance,
}

impl PermissionPipelineStage {
    fn as_str(self) -> &'static str {
        match self {
            PermissionPipelineStage::RuleDecision => "rule_decision",
            PermissionPipelineStage::RiskAssessment => "risk_assessment",
            PermissionPipelineStage::ShellFailClosed => "shell_fail_closed",
            PermissionPipelineStage::ToolConfirmation => "tool_confirmation",
            PermissionPipelineStage::GoalDrift => "goal_drift",
            PermissionPipelineStage::PromptRequired => "prompt_required",
            PermissionPipelineStage::DenialTracking => "denial_tracking",
            PermissionPipelineStage::RecoveryGuidance => "recovery_guidance",
        }
    }
}

fn permission_decision_evidence_json(input: PermissionEvidenceInput<'_>) -> serde_json::Value {
    let matched_rules = input
        .permission_explanation
        .matched_rules
        .iter()
        .map(|(decision, rule)| {
            serde_json::json!({
                "decision": format!("{:?}", decision),
                "source": format!("{:?}", rule.source),
                "pattern": rule.pattern,
            })
        })
        .collect::<Vec<_>>();
    let pipeline_stages = permission_pipeline_stages(&input);
    serde_json::json!({
        "schema": "permission_decision_evidence.v1",
        "tool_name": input.tool_name,
        "call_id": input.tool_call.id,
        "request_kind": input.kind.as_str(),
        "permission_family": input.family.as_str(),
        "permission_mode": format!("{:?}", input.context.permission_context.mode),
        "decision": format!("{:?}", input.permission_explanation.decision),
        "risk_level": format!("{:?}", input.permission_explanation.risk_level),
        "confidence": input.permission_explanation.confidence,
        "requires": {
            "permission_rule": input.permission_requires,
            "tool_confirmation": input.tool_requires,
            "raw_tool_confirmation": input.raw_tool_requires,
            "goal_drift": input.drift_requires_approval,
        },
        "matched_rules": matched_rules,
        "pipeline_stages": pipeline_stages,
        "matched_patterns": input.patterns,
        "allowed_always_rules": input.allowed_always_rules,
        "warnings": input.permission_explanation.warnings,
        "reasons": input.permission_explanation.reasons,
        "command_classification": input.command_classification,
        "remote_classification": input.remote_classification,
        "denial_state": input.denial_state,
        "recovery": {
            "recommended_action": input.recovery_feedback,
            "safe_retry": false,
        }
    })
}

fn permission_pipeline_stages(input: &PermissionEvidenceInput<'_>) -> Vec<serde_json::Value> {
    let mut stages = vec![
        serde_json::json!({
            "stage": PermissionPipelineStage::RuleDecision.as_str(),
            "decision": format!("{:?}", input.permission_explanation.decision),
            "matched_rules": input.permission_explanation.matched_rules.len(),
        }),
        serde_json::json!({
            "stage": PermissionPipelineStage::RiskAssessment.as_str(),
            "risk_level": format!("{:?}", input.permission_explanation.risk_level),
            "warnings": input.permission_explanation.warnings,
        }),
    ];

    let shell_fail_closed = input
        .command_classification
        .get("command_plan")
        .and_then(|plan| plan.get("fail_closed"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if shell_fail_closed {
        stages.push(serde_json::json!({
            "stage": PermissionPipelineStage::ShellFailClosed.as_str(),
            "reasons": input.command_classification
                .get("command_plan")
                .and_then(|plan| plan.get("fail_closed_reasons"))
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        }));
    }

    if input.raw_tool_requires || input.tool_requires {
        stages.push(serde_json::json!({
            "stage": PermissionPipelineStage::ToolConfirmation.as_str(),
            "raw_tool_confirmation": input.raw_tool_requires,
            "effective_tool_confirmation": input.tool_requires,
        }));
    }

    if input.drift_requires_approval {
        stages.push(serde_json::json!({
            "stage": PermissionPipelineStage::GoalDrift.as_str(),
            "requires_approval": true,
        }));
    }

    if input.permission_requires || input.tool_requires || input.drift_requires_approval {
        stages.push(serde_json::json!({
            "stage": PermissionPipelineStage::PromptRequired.as_str(),
            "request_kind": input.kind.as_str(),
            "permission_family": input.family.as_str(),
        }));
    }

    let prior_denials = input
        .denial_state
        .get("denials")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if prior_denials > 0 {
        stages.push(serde_json::json!({
            "stage": PermissionPipelineStage::DenialTracking.as_str(),
            "denials": prior_denials,
            "bounded_recovery": input.denial_state
                .get("bounded_recovery")
                .cloned()
                .unwrap_or(serde_json::Value::Bool(false)),
        }));
    }

    stages.push(serde_json::json!({
        "stage": PermissionPipelineStage::RecoveryGuidance.as_str(),
        "recommended_action": input.recovery_feedback,
        "safe_retry": false,
    }));

    stages
}

fn permission_remote_classification(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> serde_json::Value {
    match tool_name {
        "remote_trigger" => {
            crate::tools::remote_trigger_tool::remote_trigger_permission_metadata(arguments)
        }
        "remote_dev" => crate::tools::remote_dev_tool::remote_dev_permission_metadata(arguments),
        _ => serde_json::Value::Null,
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
        assert_eq!(
            evaluation.record.as_ref().unwrap().metadata["permission_source"],
            "runtime_rule"
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
            review: HumanReviewAuditRecord {
                kind: crate::engine::human_review::HumanReviewKind::ToolPermission,
                title: "Tool approval".to_string(),
                risk: crate::engine::human_review::HumanReviewRisk::Medium,
                subject: "git".to_string(),
                reason: "Allow git mutation?".to_string(),
                tool_call_id: Some("call_1".to_string()),
                tool_name: Some("git".to_string()),
                input_summary: "git".to_string(),
                risk_facts: Vec::new(),
                matched_rules: vec!["git".to_string()],
                classifier_result: None,
                hook_decision: None,
                user_decision: None,
                persistence_scope: Some("this_call".to_string()),
                saved_config_path: None,
                recovery_hint: Some("Ask the user to approve 'git' before retrying.".to_string()),
            },
            rejection_feedback: "Permission denied: 'git' requires user confirmation.".to_string(),
            recovery_feedback: "Ask the user to approve 'git' before retrying.".to_string(),
        };

        let result =
            PermissionController::denied_result("git", Some(&record), Some("user_once_reject"));

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
        assert_eq!(
            result.data.as_ref().unwrap()["permission_request"]["permission_source"],
            "user_once_reject"
        );
        assert_eq!(
            result.data.as_ref().unwrap()["permission_request"]["metadata"]
                ["resolved_permission_source"],
            "user_once_reject"
        );
    }

    #[test]
    fn repeated_permission_denials_emit_bounded_recovery_state() {
        let record = PermissionRequestRecord {
            id: "call_repeat".to_string(),
            session_id: "session-repeat-denial".to_string(),
            kind: PermissionRequestKind::RuntimeRule,
            patterns: vec!["bash".to_string()],
            metadata: json!({
                "tool_name": "bash",
                "permission_family": "shell"
            }),
            allowed_always_rules: Vec::new(),
            review: HumanReviewAuditRecord {
                kind: crate::engine::human_review::HumanReviewKind::ToolPermission,
                title: "Tool approval".to_string(),
                risk: crate::engine::human_review::HumanReviewRisk::High,
                subject: "bash".to_string(),
                reason: "Allow shell mutation?".to_string(),
                tool_call_id: Some("call_repeat".to_string()),
                tool_name: Some("bash".to_string()),
                input_summary: "bash".to_string(),
                risk_facts: Vec::new(),
                matched_rules: vec!["bash".to_string()],
                classifier_result: None,
                hook_decision: None,
                user_decision: None,
                persistence_scope: Some("this_call".to_string()),
                saved_config_path: None,
                recovery_hint: Some("Ask the user before retrying.".to_string()),
            },
            rejection_feedback: "Permission denied: 'bash' requires user confirmation.".to_string(),
            recovery_feedback: "Ask the user before retrying.".to_string(),
        };

        let first = PermissionController::denied_result("bash", Some(&record), Some("hook_deny"));
        assert_eq!(first.data.as_ref().unwrap()["denial_state"]["denials"], 1);
        assert_eq!(
            first.data.as_ref().unwrap()["denial_state"]["bounded_recovery"],
            false
        );

        let second = PermissionController::denied_result("bash", Some(&record), Some("hook_deny"));
        assert_eq!(second.data.as_ref().unwrap()["denial_state"]["denials"], 2);
        assert_eq!(
            second.data.as_ref().unwrap()["denial_state"]["bounded_recovery"],
            true
        );
        assert!(second
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("denied repeatedly"));
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
    fn bash_permission_metadata_includes_command_classification() {
        let tmp = tempdir().expect("tempdir");
        let mut context = ToolContext::new(tmp.path(), "session-1");
        context.permission_context.rules = crate::permissions::PermissionRules::new().ask("bash");
        let tool_call = ToolCall {
            id: "call_bash".to_string(),
            name: "bash".to_string(),
            arguments: json!({"command": "npm run dev -- --host 0.0.0.0"}),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &crate::tools::BashTool,
            &context,
            &DriftCheck::ok(),
        );

        let metadata = &evaluation.record.as_ref().unwrap().metadata;
        assert!(evaluation.requires_approval);
        assert_eq!(metadata["permission_family"], "shell");
        assert_eq!(
            metadata["permission_evidence"]["schema"],
            "permission_decision_evidence.v1"
        );
        assert_eq!(
            metadata["permission_evidence"]["request_kind"],
            "runtime_rule"
        );
        assert_eq!(
            metadata["permission_evidence"]["permission_family"],
            "shell"
        );
        assert_eq!(
            metadata["permission_evidence"]["requires"]["permission_rule"],
            true
        );
        assert_eq!(
            metadata["command_classification"]["command_category"],
            "dev_server"
        );
        assert_eq!(
            metadata["command_classification"]["command_kind"],
            "unknown"
        );
        assert_eq!(
            metadata["command_classification"]["requires_pty"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(
            metadata["command_classification"]["network_access"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(
            metadata["command_classification"]["external_path_access"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(
            metadata["command_classification"]["permission_rule_suggestions"][0]["scope"],
            "exact"
        );
        assert_eq!(
            metadata["permission_evidence"]["command_classification"]["parser_status"],
            "simple"
        );
        assert_eq!(
            metadata["permission_matcher_input"],
            "npm run dev -- --host 0.0.0.0"
        );
        assert_eq!(metadata["ui_render_kind"], "shell");
        assert_eq!(
            metadata["search_or_read"],
            serde_json::json!({"is_search": false, "is_read": false, "is_list": false})
        );
    }

    #[test]
    fn bash_permission_evidence_includes_mutation_subcommand_facts() {
        let tmp = tempdir().expect("tempdir");
        let mut context = ToolContext::new(tmp.path(), "session-1");
        context.permission_context.rules = crate::permissions::PermissionRules::new().ask("bash");
        let tool_call = ToolCall {
            id: "call_bash_mutation".to_string(),
            name: "bash".to_string(),
            arguments: json!({"command": "rg TODO src && sed -i '' 's/a/b/' src/lib.rs"}),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &crate::tools::BashTool,
            &context,
            &DriftCheck::ok(),
        );

        let evidence = &evaluation.record.as_ref().unwrap().metadata["permission_evidence"];
        assert_eq!(
            evidence["command_classification"]["parser_status"],
            "compound"
        );
        assert_eq!(
            evidence["command_classification"]["subcommands"][1]["category"],
            "file_mutation"
        );
        assert_eq!(
            evidence["command_classification"]["subcommands"][1]["mutation"],
            true
        );
        assert!(evidence["command_classification"]["mutation_indicators"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("sed_in_place")));
        assert_eq!(
            evidence["command_classification"]["command_plan"]["fail_closed"],
            true
        );
        assert!(
            evidence["command_classification"]["command_plan"]["fail_closed_reasons"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("mutation_subcommand"))
        );
        let stage_names = evidence["pipeline_stages"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|stage| stage["stage"].as_str())
            .collect::<Vec<_>>();
        assert!(stage_names.contains(&"rule_decision"));
        assert!(stage_names.contains(&"risk_assessment"));
        assert!(stage_names.contains(&"shell_fail_closed"));
        assert!(stage_names.contains(&"prompt_required"));
        assert!(stage_names.contains(&"recovery_guidance"));
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

    #[test]
    fn remote_trigger_run_records_remote_family_and_risk_facts() {
        let tmp = tempdir().expect("tempdir");
        let mut context = ToolContext::new(tmp.path(), "session-1");
        context.permission_context.mode = crate::permissions::PermissionMode::AutoAll;
        let tool_call = ToolCall {
            id: "call_remote_run".to_string(),
            name: "remote_trigger".to_string(),
            arguments: json!({"action": "run", "id": "session-123"}),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &NamedTool {
                name: "remote_trigger",
                requires_confirmation: false,
            },
            &context,
            &DriftCheck::ok(),
        );

        let metadata = &evaluation.record.as_ref().unwrap().metadata;
        assert!(evaluation.requires_approval);
        assert_eq!(metadata["permission_family"], "remote");
        assert_eq!(metadata["remote_classification"]["risk_level"], "high");
        assert_eq!(
            metadata["remote_classification"]["remote_effect"],
            "remote_execution"
        );
        assert!(evaluation
            .record
            .as_ref()
            .unwrap()
            .recovery_feedback
            .contains("/remote status"));
    }

    #[test]
    fn remote_dev_exec_records_command_preview() {
        let tmp = tempdir().expect("tempdir");
        let mut context = ToolContext::new(tmp.path(), "session-1");
        context.permission_context.mode = crate::permissions::PermissionMode::AutoAll;
        let tool_call = ToolCall {
            id: "call_remote_exec".to_string(),
            name: "remote_dev".to_string(),
            arguments: json!({
                "action": "exec",
                "id": "prod-shell",
                "command": "cargo test -q"
            }),
        };

        let evaluation = PermissionController::evaluate_tool_permission(
            "session-1",
            &tool_call,
            &NamedTool {
                name: "remote_dev",
                requires_confirmation: false,
            },
            &context,
            &DriftCheck::ok(),
        );

        let metadata = &evaluation.record.as_ref().unwrap().metadata;
        assert!(evaluation.requires_approval);
        assert_eq!(metadata["permission_family"], "remote");
        assert_eq!(
            metadata["remote_classification"]["remote_effect"],
            "remote_ssh_execution"
        );
        assert_eq!(
            metadata["remote_classification"]["command_preview"],
            "cargo test -q"
        );
    }
}

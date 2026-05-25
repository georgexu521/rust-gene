//! Structured recovery artifacts for failed or degraded runtime actions.
//!
//! The first version is intentionally small: it describes what failed, what the
//! runtime chose to do, whether retry is safe, and what should be shown to a
//! user or persisted as a learning signal.

use crate::engine::error_classifier::{ClassifiedError, ErrorCategory, RecoveryAction};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStatus {
    Planned,
    Applied,
    Succeeded,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub id: String,
    pub source: String,
    pub category: String,
    #[serde(default)]
    pub failure_type: String,
    #[serde(default)]
    pub recovery_kind: String,
    pub primary_error: String,
    pub action: String,
    pub retryable: bool,
    pub safe_retry: bool,
    #[serde(default)]
    pub allowed_alternatives: Vec<String>,
    #[serde(default)]
    pub retry_budget: Option<usize>,
    #[serde(default)]
    pub side_effect_uncertain: bool,
    #[serde(default)]
    pub requires_user_decision: bool,
    pub suggested_command: Option<String>,
    pub user_note: String,
    pub status: RecoveryStatus,
}

impl RecoveryPlan {
    pub fn from_classified(source: impl Into<String>, error: &ClassifiedError) -> Self {
        let suggested_command = suggested_command(&error.category, &error.action);
        let safe_retry = error.retryable
            && !matches!(
                error.category,
                ErrorCategory::Auth | ErrorCategory::Billing | ErrorCategory::ContentFiltered
            );
        Self {
            id: format!("recovery_{}", uuid::Uuid::new_v4().simple()),
            source: source.into(),
            category: error.category.to_string(),
            failure_type: error.category.to_string(),
            recovery_kind: error.action.to_string(),
            primary_error: error.message.clone(),
            action: error.action.to_string(),
            retryable: error.retryable,
            safe_retry,
            allowed_alternatives: classified_alternatives(&error.category, &error.action),
            retry_budget: if safe_retry { Some(1) } else { None },
            side_effect_uncertain: false,
            requires_user_decision: matches!(
                error.category,
                ErrorCategory::Auth | ErrorCategory::Billing | ErrorCategory::ContentFiltered
            ),
            suggested_command,
            user_note: user_note(&error.category, &error.action),
            status: RecoveryStatus::Planned,
        }
    }

    pub fn fallback_model(source: impl Into<String>, error: &str, fallback_model: &str) -> Self {
        Self {
            id: format!("recovery_{}", uuid::Uuid::new_v4().simple()),
            source: source.into(),
            category: "fallback_model".to_string(),
            failure_type: "model_unavailable".to_string(),
            recovery_kind: "fallback_model".to_string(),
            primary_error: truncate(error, 240),
            action: format!("switch to fallback model {}", fallback_model),
            retryable: true,
            safe_retry: true,
            allowed_alternatives: vec![
                "retry non-streaming".to_string(),
                "compact context".to_string(),
            ],
            retry_budget: Some(1),
            side_effect_uncertain: false,
            requires_user_decision: false,
            suggested_command: Some("/model".to_string()),
            user_note: format!(
                "Primary model failed; retrying this turn with fallback model {}.",
                fallback_model
            ),
            status: RecoveryStatus::Applied,
        }
    }

    pub fn streaming_fallback(source: impl Into<String>, error: &str) -> Self {
        Self {
            id: format!("recovery_{}", uuid::Uuid::new_v4().simple()),
            source: source.into(),
            category: "streaming_fallback".to_string(),
            failure_type: "streaming_transport".to_string(),
            recovery_kind: "non_streaming_retry".to_string(),
            primary_error: truncate(error, 240),
            action: "retry request without streaming".to_string(),
            retryable: true,
            safe_retry: true,
            allowed_alternatives: vec![
                "switch fallback model".to_string(),
                "compact context".to_string(),
            ],
            retry_budget: Some(1),
            side_effect_uncertain: false,
            requires_user_decision: false,
            suggested_command: Some("/retry".to_string()),
            user_note: "Streaming failed; retrying the same request through non-streaming mode."
                .to_string(),
            status: RecoveryStatus::Applied,
        }
    }

    pub fn tool_failure(tool_name: &str, error: &str, error_code: Option<&str>) -> Self {
        if is_hook_blocked_error(error) {
            return Self::hook_failure(
                "PreToolUse",
                "unknown",
                "pre_tool_hook",
                Some(tool_name),
                true,
                error,
            );
        }
        let remote_tool = is_remote_tool(tool_name);
        let recoverable = !matches!(
            error_code.unwrap_or("unknown"),
            "permission_denied" | "dangerous_blocked" | "cancelled"
        );
        let suggested_command = suggested_tool_command(tool_name, error, error_code);
        let profile = classified_failure_profile(tool_name, error, error_code);
        Self {
            id: format!("recovery_{}", uuid::Uuid::new_v4().simple()),
            source: "tool_execution".to_string(),
            category: error_code.unwrap_or("unknown").to_string(),
            failure_type: profile.failure_type,
            recovery_kind: profile.recovery_kind,
            primary_error: truncate(error, 240),
            action: suggested_tool_action(tool_name, error, error_code),
            retryable: recoverable,
            safe_retry: recoverable
                && !matches!(
                    error_code.unwrap_or("unknown"),
                    "execution_failed" | "unknown"
                )
                && !remote_tool,
            allowed_alternatives: profile.allowed_alternatives,
            retry_budget: profile.retry_budget,
            side_effect_uncertain: profile.side_effect_uncertain || remote_tool,
            requires_user_decision: profile.requires_user_decision,
            suggested_command,
            user_note: tool_user_note(tool_name, error, error_code),
            status: RecoveryStatus::Planned,
        }
    }

    pub fn hook_failure(
        event: &str,
        provider: &str,
        hook_name: &str,
        tool_name: Option<&str>,
        blocked: bool,
        detail: &str,
    ) -> Self {
        let category = if blocked {
            "hook_blocked"
        } else {
            "hook_failed"
        };
        let target = tool_name.unwrap_or("runtime");
        let action = if blocked {
            format!(
                "inspect or adjust hook '{}' before retrying {}",
                hook_name, target
            )
        } else {
            format!(
                "inspect hook '{}' failure before relying on {} lifecycle automation",
                hook_name, event
            )
        };
        Self {
            id: format!("recovery_{}", uuid::Uuid::new_v4().simple()),
            source: "hook_runtime".to_string(),
            category: category.to_string(),
            failure_type: category.to_string(),
            recovery_kind: if blocked {
                "inspect_hook_before_retry".to_string()
            } else {
                "repair_hook_or_continue_without_assumption".to_string()
            },
            primary_error: truncate(detail, 240),
            action,
            retryable: !blocked,
            safe_retry: false,
            allowed_alternatives: vec![
                "inspect hook configuration".to_string(),
                "choose a lower-risk tool path".to_string(),
            ],
            retry_budget: if blocked { None } else { Some(1) },
            side_effect_uncertain: true,
            requires_user_decision: blocked,
            suggested_command: Some("/hooks".to_string()),
            user_note: if blocked {
                format!(
                    "{} was blocked by hook '{}' from provider {}; do not treat the tool action as executed until the hook decision is reviewed.",
                    target, hook_name, provider
                )
            } else {
                format!(
                    "Hook '{}' from provider {} failed during {}; inspect `/hooks` before assuming lifecycle automation ran.",
                    hook_name, provider, event
                )
            },
            status: RecoveryStatus::Planned,
        }
    }

    pub fn with_status(mut self, status: RecoveryStatus) -> Self {
        self.status = status;
        self
    }

    pub fn trace_action(&self) -> String {
        format!(
            "{} [{} failure_type={} recovery_kind={} safe_retry={} suggested={}]",
            self.action,
            format!("{:?}", self.status).to_ascii_lowercase(),
            self.failure_type,
            self.recovery_kind,
            self.safe_retry,
            self.suggested_command.as_deref().unwrap_or("none")
        )
    }

    pub fn summary(&self) -> String {
        format!(
            "{}: {} -> {}",
            self.category,
            truncate(&self.primary_error, 80),
            self.action
        )
    }
}

struct FailureProfile {
    failure_type: String,
    recovery_kind: String,
    allowed_alternatives: Vec<String>,
    retry_budget: Option<usize>,
    side_effect_uncertain: bool,
    requires_user_decision: bool,
}

fn classified_failure_profile(
    tool_name: &str,
    error: &str,
    error_code: Option<&str>,
) -> FailureProfile {
    let lower = error.to_ascii_lowercase();
    let code = error_code.unwrap_or("unknown");

    let (failure_type, recovery_kind, alternatives, retry_budget, side_effect, requires_user) =
        if lower.contains("old_string")
            || lower.contains("old string")
            || lower.contains("string not found")
        {
            (
                "old_string_not_found",
                "refresh_target_and_retry_edit",
                vec!["read exact target range", "use patch with fresh context"],
                Some(1),
                false,
                false,
            )
        } else if lower.contains("occurrence") || lower.contains("multiple matches") {
            (
                "old_string_occurrence_mismatch",
                "narrow_edit_context",
                vec!["read narrower range", "use line-scoped replacement"],
                Some(1),
                false,
                false,
            )
        } else if lower.contains("stale") || lower.contains("changed since read") {
            (
                "stale_read_conflict",
                "refresh_read_before_edit",
                vec!["read file again", "recompute patch from latest content"],
                Some(1),
                false,
                false,
            )
        } else if lower.contains("checkpoint") && lower.contains("failed") {
            (
                "checkpoint_creation_failed",
                "stop_before_mutation",
                vec![
                    "inspect checkpoint store",
                    "retry after checkpoint succeeds",
                ],
                None,
                true,
                true,
            )
        } else if matches!(code, "permission_denied" | "dangerous_blocked")
            || lower.contains("permission")
            || lower.contains("denied")
        {
            (
                "permission_block",
                "ask_user_or_choose_safer_path",
                vec!["ask for permission", "switch to read-only inspection"],
                None,
                true,
                true,
            )
        } else if matches!(code, "timeout")
            || lower.contains("timed out")
            || lower.contains("timeout")
        {
            (
                "timeout",
                "retry_narrower",
                vec!["narrow command scope", "increase timeout if safe"],
                Some(1),
                is_remote_tool(tool_name),
                is_remote_tool(tool_name),
            )
        } else if lower.contains("command not found") || lower.contains("not found") {
            (
                if tool_name == "bash" {
                    "command_not_found"
                } else {
                    "target_not_found"
                },
                "verify_target_exists",
                vec!["search available target", "inspect project tooling"],
                Some(1),
                false,
                false,
            )
        } else if lower.contains("test result: failed")
            || lower.contains("test failed")
            || lower.contains("assertion failed")
        {
            (
                "test_failed",
                "debug_validation_failure",
                vec![
                    "inspect failing test output",
                    "patch before rerunning validation",
                ],
                None,
                false,
                false,
            )
        } else if matches!(code, "invalid_params") || lower.contains("invalid params") {
            (
                "invalid_params",
                "correct_arguments",
                vec!["inspect tool schema", "retry with corrected arguments"],
                Some(1),
                false,
                false,
            )
        } else if matches!(code, "unavailable") || lower.contains("unavailable") {
            (
                "unavailable",
                "check_tool_or_remote_status",
                vec!["check tool status", "use local fallback if available"],
                Some(1),
                is_remote_tool(tool_name),
                is_remote_tool(tool_name),
            )
        } else {
            (
                code,
                "inspect_failure_before_retry",
                vec!["read error detail", "choose alternate tool path"],
                None,
                is_remote_tool(tool_name),
                is_remote_tool(tool_name),
            )
        };

    FailureProfile {
        failure_type: failure_type.to_string(),
        recovery_kind: recovery_kind.to_string(),
        allowed_alternatives: alternatives
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        retry_budget,
        side_effect_uncertain: side_effect,
        requires_user_decision: requires_user,
    }
}

fn suggested_tool_command(
    tool_name: &str,
    error: &str,
    error_code: Option<&str>,
) -> Option<String> {
    let lower = error.to_ascii_lowercase();
    if is_remote_tool(tool_name) {
        return match error_code.unwrap_or("unknown") {
            "permission_denied" | "dangerous_blocked" => Some("/permissions explain".to_string()),
            _ => Some("/remote status".to_string()),
        };
    }
    match error_code.unwrap_or("unknown") {
        "permission_denied" | "dangerous_blocked" => Some("/permissions explain".to_string()),
        "timeout" => Some("/retry".to_string()),
        "not_found" => {
            if tool_name.contains("file") || lower.contains("file") {
                Some("/project search".to_string())
            } else {
                Some("/tools".to_string())
            }
        }
        "invalid_params" => Some("/tools".to_string()),
        _ => {
            if lower.contains("permission") {
                Some("/permissions explain".to_string())
            } else if lower.contains("timeout") {
                Some("/retry".to_string())
            } else {
                None
            }
        }
    }
}

fn classified_alternatives(category: &ErrorCategory, action: &RecoveryAction) -> Vec<String> {
    match category {
        ErrorCategory::ContextOverflow | ErrorCategory::PayloadTooLarge => {
            vec![
                "compact context".to_string(),
                "retry with fewer attachments".to_string(),
            ]
        }
        ErrorCategory::ProviderProtocol | ErrorCategory::RequestSchema => vec![
            "inspect latest trace".to_string(),
            "normalize request before retry".to_string(),
        ],
        ErrorCategory::Auth | ErrorCategory::Billing => vec![
            "resolve account or credential state".to_string(),
            "switch provider only after user decision".to_string(),
        ],
        _ if matches!(
            action,
            RecoveryAction::FallbackModel
                | RecoveryAction::Retry
                | RecoveryAction::RetryWithBackoff { .. }
        ) =>
        {
            vec![
                "retry once".to_string(),
                "switch fallback model".to_string(),
            ]
        }
        _ => vec!["inspect error detail".to_string()],
    }
}

fn suggested_tool_action(tool_name: &str, _error: &str, error_code: Option<&str>) -> String {
    if is_remote_tool(tool_name) {
        return match error_code.unwrap_or("unknown") {
            "invalid_params" => format!(
                "inspect {} remote arguments and retry with corrected parameters",
                tool_name
            ),
            "permission_denied" => {
                format!("review remote permission request before running {}", tool_name)
            }
            "timeout" => format!(
                "check remote status before retrying {} because remote side effects may already exist",
                tool_name
            ),
            "not_found" => format!(
                "verify the remote session/bridge target exists before retrying {}",
                tool_name
            ),
            _ => format!(
                "inspect bridge/remote status and retry {} only after confirming remote side effects",
                tool_name
            ),
        };
    }
    match error_code.unwrap_or("unknown") {
        "invalid_params" => format!(
            "inspect {} arguments and retry with corrected parameters",
            tool_name
        ),
        "permission_denied" => format!("review permission rule before running {}", tool_name),
        "not_found" => format!("verify target exists before retrying {}", tool_name),
        "timeout" => format!(
            "retry {} with a narrower scope or longer timeout",
            tool_name
        ),
        "dangerous_blocked" => format!("ask user before attempting dangerous {}", tool_name),
        "unavailable" => format!("check tool availability before retrying {}", tool_name),
        _ => format!("inspect {} failure and decide whether to retry", tool_name),
    }
}

fn tool_user_note(tool_name: &str, error: &str, error_code: Option<&str>) -> String {
    if is_remote_tool(tool_name) {
        return match error_code.unwrap_or("unknown") {
            "permission_denied" => format!(
                "{} was blocked by remote permission policy; do not treat the remote action as executed.",
                tool_name
            ),
            _ => format!(
                "{} failed in the bridge/remote path: {}. Inspect `/remote status`, bridge URL/auth/tenant, saved session/cursor state, and remote side effects before retrying.",
                tool_name,
                truncate(error, 120)
            ),
        };
    }
    match error_code.unwrap_or("unknown") {
        "invalid_params" => format!("{} failed because its arguments were invalid.", tool_name),
        "permission_denied" => format!("{} was blocked by permission policy.", tool_name),
        "not_found" => format!("{} could not find the requested resource.", tool_name),
        "timeout" => format!("{} timed out; a narrower retry may recover.", tool_name),
        "dangerous_blocked" => format!("{} was blocked as a dangerous action.", tool_name),
        _ => format!("{} failed: {}", tool_name, truncate(error, 120)),
    }
}

fn is_remote_tool(tool_name: &str) -> bool {
    matches!(tool_name, "remote_trigger" | "remote_dev")
}

fn is_hook_blocked_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("pre-tool hook") || lower.contains("blocked by hook")
}

fn suggested_command(category: &ErrorCategory, action: &RecoveryAction) -> Option<String> {
    match (category, action) {
        (ErrorCategory::Auth, _) => Some("/login".to_string()),
        (ErrorCategory::Billing, _) => Some("/status".to_string()),
        (ErrorCategory::ContextOverflow | ErrorCategory::PayloadTooLarge, _) => {
            Some("/compact".to_string())
        }
        (ErrorCategory::ProviderProtocol | ErrorCategory::RequestSchema, _) => {
            Some("/trace last".to_string())
        }
        (_, RecoveryAction::FallbackModel) => Some("/model".to_string()),
        (_, RecoveryAction::Retry | RecoveryAction::RetryWithBackoff { .. }) => {
            Some("/retry".to_string())
        }
        _ => None,
    }
}

fn user_note(category: &ErrorCategory, action: &RecoveryAction) -> String {
    match category {
        ErrorCategory::ContextOverflow | ErrorCategory::PayloadTooLarge => {
            "Context was too large; compacting and retrying is the safest recovery.".to_string()
        }
        ErrorCategory::RateLimited => {
            "Provider rate-limited the request; retry after backoff.".to_string()
        }
        ErrorCategory::Overloaded => {
            "Provider is overloaded; retry or fallback model may recover.".to_string()
        }
        ErrorCategory::Auth => {
            "Authentication failed; credentials need attention before retrying.".to_string()
        }
        ErrorCategory::Billing => {
            "Billing or quota blocked the request; retrying will not help until resolved."
                .to_string()
        }
        ErrorCategory::ProviderProtocol => {
            "Provider rejected the message/tool protocol; inspect the trace before retrying so the next request can be normalized instead of repeated blindly."
                .to_string()
        }
        ErrorCategory::RequestSchema => {
            "Provider rejected the request schema; inspect the trace and generated payload shape before retrying."
                .to_string()
        }
        _ => format!("Selected recovery action: {}", action),
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classified_context_error_suggests_compact() {
        let err = ClassifiedError::new(
            ErrorCategory::ContextOverflow,
            RecoveryAction::CompressAndRetry,
            "too many tokens".to_string(),
        );
        let plan = RecoveryPlan::from_classified("api", &err);
        assert_eq!(plan.suggested_command.as_deref(), Some("/compact"));
        assert!(plan.safe_retry);
    }

    #[test]
    fn billing_error_is_not_safe_retry() {
        let err = ClassifiedError::new(
            ErrorCategory::Billing,
            RecoveryAction::Abort,
            "quota".to_string(),
        );
        let plan = RecoveryPlan::from_classified("api", &err);
        assert!(!plan.retryable);
        assert!(!plan.safe_retry);
    }

    #[test]
    fn provider_protocol_error_suggests_trace_not_retry() {
        let err = ClassifiedError::new(
            ErrorCategory::ProviderProtocol,
            RecoveryAction::Abort,
            "tool result does not follow tool call".to_string(),
        );
        let plan = RecoveryPlan::from_classified("api", &err);

        assert_eq!(plan.suggested_command.as_deref(), Some("/trace last"));
        assert!(!plan.retryable);
        assert!(!plan.safe_retry);
        assert!(plan.user_note.contains("message/tool protocol"));
    }

    #[test]
    fn tool_timeout_suggests_retry() {
        let plan = RecoveryPlan::tool_failure("bash", "command timed out", Some("timeout"));
        assert_eq!(plan.suggested_command.as_deref(), Some("/retry"));
        assert!(plan.retryable);
        assert!(plan.safe_retry);
    }

    #[test]
    fn hook_blocked_tool_failure_suggests_hooks_and_disables_retry() {
        let plan = RecoveryPlan::tool_failure(
            "bash",
            "blocked by pre-tool hook: policy denied shell command",
            Some("dangerous_blocked"),
        );

        assert_eq!(plan.category, "hook_blocked");
        assert_eq!(plan.source, "hook_runtime");
        assert_eq!(plan.suggested_command.as_deref(), Some("/hooks"));
        assert!(!plan.retryable);
        assert!(!plan.safe_retry);
        assert!(plan
            .user_note
            .contains("do not treat the tool action as executed"));
    }

    #[test]
    fn hook_failure_suggests_hooks_without_safe_retry() {
        let plan = RecoveryPlan::hook_failure(
            "PostToolUse",
            "env",
            "env_post_tool_hook",
            Some("file_edit"),
            false,
            "exit status 1",
        );

        assert_eq!(plan.category, "hook_failed");
        assert_eq!(plan.suggested_command.as_deref(), Some("/hooks"));
        assert!(plan.retryable);
        assert!(!plan.safe_retry);
        assert!(plan.user_note.contains("inspect `/hooks`"));
    }

    #[test]
    fn remote_tool_failure_suggests_remote_status_and_disables_safe_retry() {
        let plan = RecoveryPlan::tool_failure(
            "remote_trigger",
            "Failed to run trigger: bridge unavailable",
            Some("unavailable"),
        );

        assert_eq!(plan.suggested_command.as_deref(), Some("/remote status"));
        assert!(plan.retryable);
        assert!(!plan.safe_retry);
        assert!(plan.user_note.contains("bridge/remote path"));
    }

    #[test]
    fn remote_permission_failure_stays_on_permission_explain() {
        let plan = RecoveryPlan::tool_failure(
            "remote_dev",
            "Permission denied: remote exec requires user confirmation",
            Some("permission_denied"),
        );

        assert_eq!(
            plan.suggested_command.as_deref(),
            Some("/permissions explain")
        );
        assert!(!plan.retryable);
        assert!(!plan.safe_retry);
    }
}

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
    pub primary_error: String,
    pub action: String,
    pub retryable: bool,
    pub safe_retry: bool,
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
            primary_error: error.message.clone(),
            action: error.action.to_string(),
            retryable: error.retryable,
            safe_retry,
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
            primary_error: truncate(error, 240),
            action: format!("switch to fallback model {}", fallback_model),
            retryable: true,
            safe_retry: true,
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
            primary_error: truncate(error, 240),
            action: "retry request without streaming".to_string(),
            retryable: true,
            safe_retry: true,
            suggested_command: Some("/retry".to_string()),
            user_note: "Streaming failed; retrying the same request through non-streaming mode."
                .to_string(),
            status: RecoveryStatus::Applied,
        }
    }

    pub fn tool_failure(tool_name: &str, error: &str, error_code: Option<&str>) -> Self {
        let remote_tool = is_remote_tool(tool_name);
        let recoverable = !matches!(
            error_code.unwrap_or("unknown"),
            "permission_denied" | "dangerous_blocked" | "cancelled"
        );
        let suggested_command = suggested_tool_command(tool_name, error, error_code);
        Self {
            id: format!("recovery_{}", uuid::Uuid::new_v4().simple()),
            source: "tool_execution".to_string(),
            category: error_code.unwrap_or("unknown").to_string(),
            primary_error: truncate(error, 240),
            action: suggested_tool_action(tool_name, error, error_code),
            retryable: recoverable,
            safe_retry: recoverable
                && !matches!(
                    error_code.unwrap_or("unknown"),
                    "execution_failed" | "unknown"
                )
                && !remote_tool,
            suggested_command,
            user_note: tool_user_note(tool_name, error, error_code),
            status: RecoveryStatus::Planned,
        }
    }

    pub fn with_status(mut self, status: RecoveryStatus) -> Self {
        self.status = status;
        self
    }

    pub fn trace_action(&self) -> String {
        format!(
            "{} [{} safe_retry={} suggested={}]",
            self.action,
            format!("{:?}", self.status).to_ascii_lowercase(),
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

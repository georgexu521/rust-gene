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

fn suggested_command(category: &ErrorCategory, action: &RecoveryAction) -> Option<String> {
    match (category, action) {
        (ErrorCategory::Auth, _) => Some("/login".to_string()),
        (ErrorCategory::Billing, _) => Some("/status".to_string()),
        (ErrorCategory::ContextOverflow | ErrorCategory::PayloadTooLarge, _) => {
            Some("/compact".to_string())
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
}

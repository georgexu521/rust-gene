//! Runtime verification proof semantics.
//!
//! Closeout should not infer "verified" from a friendly final answer. This
//! module gives the runtime a typed proof status that can be derived from task
//! state and evidence ledger records.

use crate::engine::task_context::VerificationStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationProofStatus {
    Verified,
    Failed,
    NotRun,
    NotApplicable,
    Blocked,
    UserDeferred,
    Unavailable,
}

impl VerificationProofStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::NotRun => "not_run",
            Self::NotApplicable => "not_applicable",
            Self::Blocked => "blocked",
            Self::UserDeferred => "user_deferred",
            Self::Unavailable => "unavailable",
        }
    }

    pub fn blocks_verified_closeout(self) -> bool {
        matches!(
            self,
            Self::Failed | Self::NotRun | Self::Blocked | Self::UserDeferred | Self::Unavailable
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationProof {
    pub status: VerificationProofStatus,
    pub summary: String,
    pub required_total: usize,
    pub required_passed: usize,
    pub required_failed: usize,
    pub required_missing: usize,
    pub validation_total: usize,
    pub validation_passed: usize,
    pub validation_failed: usize,
    pub recovered_failed: usize,
    pub evidence_items: usize,
    #[serde(default)]
    pub passed_commands: Vec<String>,
    #[serde(default)]
    pub failed_commands: Vec<String>,
    #[serde(default)]
    pub missing_required_commands: Vec<String>,
}

impl VerificationProof {
    pub fn new(status: VerificationProofStatus, summary: impl Into<String>) -> Self {
        Self {
            status,
            summary: summary.into(),
            required_total: 0,
            required_passed: 0,
            required_failed: 0,
            required_missing: 0,
            validation_total: 0,
            validation_passed: 0,
            validation_failed: 0,
            recovered_failed: 0,
            evidence_items: 0,
            passed_commands: Vec::new(),
            failed_commands: Vec::new(),
            missing_required_commands: Vec::new(),
        }
    }

    pub fn status_label(&self) -> &'static str {
        self.status.label()
    }

    pub fn validation_line(&self) -> String {
        format!(
            "verification proof: {} ({})",
            self.status.label(),
            self.summary
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VerificationProofRequest<'a> {
    pub required_commands: &'a [String],
    pub requires_validation: bool,
    pub task_verification_status: VerificationStatus,
}

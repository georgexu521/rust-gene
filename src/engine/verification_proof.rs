//! Runtime verification proof semantics.
//!
//! Closeout should not infer "verified" from a friendly final answer. This
//! module gives the runtime a typed proof status that can be derived from task
//! state and evidence ledger records.

use crate::engine::task_context::VerificationStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationProofKind {
    CommandPassed,
    StaticCheckPassed,
    RequiredValidationPassed,
    DiffReviewed,
    NoDiffAudit,
    KnownUnrelatedFailure,
    UserDeferred,
    ToolUnavailable,
    PermissionDenied,
    SubagentClaimOnly,
    ParentVerifiedSubagentResult,
    ManualInspectionOnly,
}

impl VerificationProofKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::CommandPassed => "command_passed",
            Self::StaticCheckPassed => "static_check_passed",
            Self::RequiredValidationPassed => "required_validation_passed",
            Self::DiffReviewed => "diff_reviewed",
            Self::NoDiffAudit => "no_diff_audit",
            Self::KnownUnrelatedFailure => "known_unrelated_failure",
            Self::UserDeferred => "user_deferred",
            Self::ToolUnavailable => "tool_unavailable",
            Self::PermissionDenied => "permission_denied",
            Self::SubagentClaimOnly => "subagent_claim_only",
            Self::ParentVerifiedSubagentResult => "parent_verified_subagent_result",
            Self::ManualInspectionOnly => "manual_inspection_only",
        }
    }

    pub fn derived_support(self, context: VerificationProofSupportContext) -> DerivedProofSupport {
        use VerificationProofTaskType as TaskType;
        match self {
            Self::CommandPassed | Self::RequiredValidationPassed => DerivedProofSupport::verified(
                "command evidence can support verified closeout when it matches task validation scope",
            ),
            Self::StaticCheckPassed => {
                if context.accepted_validation_family {
                    DerivedProofSupport::verified(
                        "static check is an accepted validation family for this task",
                    )
                } else {
                    DerivedProofSupport::partial(
                        "static check is useful evidence but not an accepted validation family",
                    )
                }
            }
            Self::DiffReviewed => DerivedProofSupport::partial(
                "diff review supports closeout evidence but does not prove changed behavior",
            ),
            Self::NoDiffAudit => {
                if matches!(context.task_type, TaskType::ReadOnlyAudit) {
                    DerivedProofSupport::verified("no-diff audit is valid for read-only audit scope")
                } else {
                    DerivedProofSupport::partial(
                        "no-diff audit does not verify a task that expected code changes",
                    )
                }
            }
            Self::KnownUnrelatedFailure => {
                if context.focused_validation_passed {
                    DerivedProofSupport::verified_with_risk(
                        "focused validation passed, but unrelated failure remains residual risk",
                    )
                } else {
                    DerivedProofSupport::partial(
                        "unrelatedness is not enough without focused passing validation",
                    )
                }
            }
            Self::UserDeferred => {
                DerivedProofSupport::not_verified(VerificationProofStatus::UserDeferred, "user deferred verification")
            }
            Self::ToolUnavailable => DerivedProofSupport::not_verified(
                VerificationProofStatus::Unavailable,
                "required verification tool was unavailable",
            ),
            Self::PermissionDenied => DerivedProofSupport::not_verified(
                VerificationProofStatus::Blocked,
                "permission denial blocks verified closeout",
            ),
            Self::SubagentClaimOnly => DerivedProofSupport::partial(
                "subagent claim must be verified by the parent runtime before closeout",
            ),
            Self::ParentVerifiedSubagentResult => {
                if context.parent_verified {
                    DerivedProofSupport::verified(
                        "parent runtime verified the subagent result in current task scope",
                    )
                } else {
                    DerivedProofSupport::partial("parent runtime has not verified the subagent result")
                }
            }
            Self::ManualInspectionOnly => {
                if matches!(context.task_type, TaskType::DirectAnswer | TaskType::ReadOnlyAudit) {
                    DerivedProofSupport::verified(
                        "manual inspection is sufficient for read-only explanation scope",
                    )
                } else {
                    DerivedProofSupport::partial(
                        "manual inspection alone is not enough for code-change verification",
                    )
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationProofTaskType {
    DirectAnswer,
    ReadOnlyAudit,
    CodeChange,
    BugFix,
    SubagentReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationProofSupportContext {
    pub task_type: VerificationProofTaskType,
    pub accepted_validation_family: bool,
    pub focused_validation_passed: bool,
    pub parent_verified: bool,
}

impl VerificationProofSupportContext {
    pub const fn code_change() -> Self {
        Self {
            task_type: VerificationProofTaskType::CodeChange,
            accepted_validation_family: false,
            focused_validation_passed: false,
            parent_verified: false,
        }
    }

    pub const fn read_only_audit() -> Self {
        Self {
            task_type: VerificationProofTaskType::ReadOnlyAudit,
            accepted_validation_family: false,
            focused_validation_passed: false,
            parent_verified: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivedProofSupport {
    pub supports_verified: bool,
    pub status: VerificationProofStatus,
    pub residual_risk: bool,
    pub reason: &'static str,
}

impl DerivedProofSupport {
    const fn verified(reason: &'static str) -> Self {
        Self {
            supports_verified: true,
            status: VerificationProofStatus::Verified,
            residual_risk: false,
            reason,
        }
    }

    const fn verified_with_risk(reason: &'static str) -> Self {
        Self {
            supports_verified: true,
            status: VerificationProofStatus::Verified,
            residual_risk: true,
            reason,
        }
    }

    const fn partial(reason: &'static str) -> Self {
        Self {
            supports_verified: false,
            status: VerificationProofStatus::Partial,
            residual_risk: true,
            reason,
        }
    }

    const fn not_verified(status: VerificationProofStatus, reason: &'static str) -> Self {
        Self {
            supports_verified: false,
            status,
            residual_risk: true,
            reason,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationProofStatus {
    Verified,
    Partial,
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
            Self::Partial => "partial",
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
            Self::Partial
                | Self::Failed
                | Self::NotRun
                | Self::Blocked
                | Self::UserDeferred
                | Self::Unavailable
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationProofSupportReport {
    pub status: VerificationProofStatus,
    pub supports_verified: bool,
    pub residual_risk: bool,
    pub summary: String,
    #[serde(default)]
    pub verified_kinds: Vec<VerificationProofKind>,
    #[serde(default)]
    pub partial_kinds: Vec<VerificationProofKind>,
    #[serde(default)]
    pub blocking_kinds: Vec<VerificationProofKind>,
}

impl Default for VerificationProofSupportReport {
    fn default() -> Self {
        Self {
            status: VerificationProofStatus::NotApplicable,
            supports_verified: false,
            residual_risk: false,
            summary: "proof support policy has not been evaluated".to_string(),
            verified_kinds: Vec::new(),
            partial_kinds: Vec::new(),
            blocking_kinds: Vec::new(),
        }
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
    #[serde(default)]
    pub proof_kinds: Vec<VerificationProofKind>,
    #[serde(default)]
    pub derived_support: VerificationProofSupportReport,
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
            proof_kinds: Vec::new(),
            derived_support: VerificationProofSupportReport::default(),
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

    pub fn proof_kind_summary(&self) -> String {
        proof_kind_labels(&self.proof_kinds)
    }

    pub fn support_line(&self) -> String {
        format!(
            "verification proof support: {} (supports_verified={} residual_risk={} kinds={}; {})",
            self.derived_support.status.label(),
            self.derived_support.supports_verified,
            self.derived_support.residual_risk,
            self.proof_kind_summary(),
            self.derived_support.summary
        )
    }

    pub fn apply_derived_support(&mut self, context: VerificationProofSupportContext) {
        self.derived_support = self.derive_support_report(context);
    }

    pub fn derive_support_report(
        &self,
        context: VerificationProofSupportContext,
    ) -> VerificationProofSupportReport {
        if self.status.blocks_verified_closeout() && self.status != VerificationProofStatus::Partial
        {
            return VerificationProofSupportReport {
                status: self.status,
                supports_verified: false,
                residual_risk: true,
                summary: format!(
                    "verification proof status {} blocks verified closeout before proof-kind policy",
                    self.status.label()
                ),
                verified_kinds: Vec::new(),
                partial_kinds: Vec::new(),
                blocking_kinds: self.proof_kinds.clone(),
            };
        }

        if self.proof_kinds.is_empty() {
            return match self.status {
                VerificationProofStatus::NotApplicable => VerificationProofSupportReport {
                    status: VerificationProofStatus::NotApplicable,
                    supports_verified: false,
                    residual_risk: false,
                    summary: "no proof kind required for this task scope".to_string(),
                    verified_kinds: Vec::new(),
                    partial_kinds: Vec::new(),
                    blocking_kinds: Vec::new(),
                },
                VerificationProofStatus::Verified => VerificationProofSupportReport {
                    status: VerificationProofStatus::Unavailable,
                    supports_verified: false,
                    residual_risk: true,
                    summary: "verified proof has no proof kinds for policy support".to_string(),
                    verified_kinds: Vec::new(),
                    partial_kinds: Vec::new(),
                    blocking_kinds: Vec::new(),
                },
                _ => VerificationProofSupportReport {
                    status: self.status,
                    supports_verified: false,
                    residual_risk: self.status.blocks_verified_closeout(),
                    summary: format!("no proof kinds recorded for {}", self.status.label()),
                    verified_kinds: Vec::new(),
                    partial_kinds: Vec::new(),
                    blocking_kinds: Vec::new(),
                },
            };
        }

        let mut verified_kinds = Vec::new();
        let mut partial_kinds = Vec::new();
        let mut blocking_kinds = Vec::new();
        let mut residual_risk = false;
        let mut blocking_status = VerificationProofStatus::Unavailable;

        for kind in &self.proof_kinds {
            let support = kind.derived_support(context);
            residual_risk |= support.residual_risk;
            if support.supports_verified {
                verified_kinds.push(*kind);
            } else if support.status == VerificationProofStatus::Partial {
                partial_kinds.push(*kind);
            } else {
                blocking_status = support.status;
                blocking_kinds.push(*kind);
            }
        }

        if !verified_kinds.is_empty() {
            let mut summary = format!("verified by {}", proof_kind_labels(&verified_kinds));
            if !partial_kinds.is_empty() {
                summary.push_str(&format!(
                    "; partial support from {}",
                    proof_kind_labels(&partial_kinds)
                ));
            }
            if !blocking_kinds.is_empty() {
                summary.push_str(&format!(
                    "; non-verifying evidence from {}",
                    proof_kind_labels(&blocking_kinds)
                ));
            }
            return VerificationProofSupportReport {
                status: VerificationProofStatus::Verified,
                supports_verified: true,
                residual_risk,
                summary,
                verified_kinds,
                partial_kinds,
                blocking_kinds,
            };
        }

        if !partial_kinds.is_empty() && blocking_kinds.is_empty() {
            return VerificationProofSupportReport {
                status: VerificationProofStatus::Partial,
                supports_verified: false,
                residual_risk: true,
                summary: format!(
                    "partial support only from {}",
                    proof_kind_labels(&partial_kinds)
                ),
                verified_kinds,
                partial_kinds,
                blocking_kinds,
            };
        }

        VerificationProofSupportReport {
            status: blocking_status,
            supports_verified: false,
            residual_risk: true,
            summary: format!(
                "no proof kind supports verified closeout; blocking={}; partial={}",
                proof_kind_labels(&blocking_kinds),
                proof_kind_labels(&partial_kinds)
            ),
            verified_kinds,
            partial_kinds,
            blocking_kinds,
        }
    }
}

fn proof_kind_labels(kinds: &[VerificationProofKind]) -> String {
    if kinds.is_empty() {
        return "none".to_string();
    }
    kinds
        .iter()
        .map(|kind| kind.label())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Clone, Copy)]
pub struct VerificationProofRequest<'a> {
    pub required_commands: &'a [String],
    pub requires_validation: bool,
    pub task_verification_status: VerificationStatus,
    pub support_context: VerificationProofSupportContext,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_kind_support_is_derived_from_task_context() {
        let audit = VerificationProofKind::NoDiffAudit
            .derived_support(VerificationProofSupportContext::read_only_audit());
        assert!(audit.supports_verified);
        assert_eq!(audit.status, VerificationProofStatus::Verified);

        let code_change = VerificationProofKind::NoDiffAudit
            .derived_support(VerificationProofSupportContext::code_change());
        assert!(!code_change.supports_verified);
        assert!(code_change.residual_risk);
    }

    #[test]
    fn subagent_claim_requires_parent_verification() {
        let claim = VerificationProofKind::SubagentClaimOnly
            .derived_support(VerificationProofSupportContext::code_change());
        assert!(!claim.supports_verified);

        let unverified_parent = VerificationProofKind::ParentVerifiedSubagentResult
            .derived_support(VerificationProofSupportContext::code_change());
        assert!(!unverified_parent.supports_verified);

        let verified_parent = VerificationProofKind::ParentVerifiedSubagentResult.derived_support(
            VerificationProofSupportContext {
                parent_verified: true,
                ..VerificationProofSupportContext::code_change()
            },
        );
        assert!(verified_parent.supports_verified);
        assert_eq!(verified_parent.status, VerificationProofStatus::Verified);
    }

    #[test]
    fn unrelated_failure_needs_focused_passing_validation() {
        let unresolved = VerificationProofKind::KnownUnrelatedFailure
            .derived_support(VerificationProofSupportContext::code_change());
        assert!(!unresolved.supports_verified);

        let resolved = VerificationProofKind::KnownUnrelatedFailure.derived_support(
            VerificationProofSupportContext {
                focused_validation_passed: true,
                ..VerificationProofSupportContext::code_change()
            },
        );
        assert!(resolved.supports_verified);
        assert!(resolved.residual_risk);
    }

    #[test]
    fn proof_support_rollup_requires_kind_policy_support() {
        let mut proof = VerificationProof::new(
            VerificationProofStatus::Verified,
            "validation passed with diff review only",
        );
        proof.proof_kinds = vec![VerificationProofKind::DiffReviewed];
        proof.apply_derived_support(VerificationProofSupportContext::code_change());

        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Partial
        );
        assert!(!proof.derived_support.supports_verified);
        assert!(proof.support_line().contains("supports_verified=false"));

        proof.proof_kinds = vec![VerificationProofKind::RequiredValidationPassed];
        proof.apply_derived_support(VerificationProofSupportContext::code_change());
        assert_eq!(
            proof.derived_support.status,
            VerificationProofStatus::Verified
        );
        assert!(proof.derived_support.supports_verified);
    }
}

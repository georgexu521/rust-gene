//! Code-change workflow coordination.
//!
//! This module owns the lightweight runtime state for programming turns:
//! risk-sensitive policy, stage validation records, and structured closeout
//! material. The model still provides engineering judgment; this module keeps
//! the workflow state stable and auditable.

use crate::engine::intent_router::{IntentRoute, RiskLevel};
use crate::engine::task_context::TaskContextBundle;
use crate::engine::workflow_contract::{
    AcceptanceReview, ProgrammingWorkflowJudgment, WeightFeedbackEvent, WeightFeedbackKind,
    WeightFeedbackSeverity,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod helpers;

pub use helpers::is_programming_workflow;
use helpers::{
    append_bullets, append_reason, preview, push_unique, route_allows_no_diff_closeout,
    runtime_validation_label_passed, select_validation_evidence, step_states_from_bundle,
    validation_evidence_summary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDepth {
    Minimal,
    Standard,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowVisibility {
    Quiet,
    Normal,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageValidationStatus {
    Passed,
    Partial,
    Failed,
    NotVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveWorkflowTrigger {
    RiskSignalHigh,
    RequiredValidation,
    FirstCodeChange,
    VerificationFailed,
    AcceptanceRejected,
    RepeatedNoCodeProgress,
}

impl AdaptiveWorkflowTrigger {
    pub fn label(self) -> &'static str {
        match self {
            Self::RiskSignalHigh => "risk_signal_high",
            Self::RequiredValidation => "required_validation",
            Self::FirstCodeChange => "first_code_change",
            Self::VerificationFailed => "verification_failed",
            Self::AcceptanceRejected => "acceptance_rejected",
            Self::RepeatedNoCodeProgress => "repeated_no_code_progress",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepRuntimeStatus {
    Pending,
    Active,
    Passed,
    Failed,
    Skipped,
}

impl PlanStepRuntimeStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStepRuntimeState {
    pub id: Option<String>,
    pub description: String,
    pub status: PlanStepRuntimeStatus,
    pub priority: String,
    pub last_evidence: Option<String>,
}

impl StageValidationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::NotVerified => "not_verified",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskSensitiveWorkflowPolicy {
    pub depth: WorkflowDepth,
    pub visibility: WorkflowVisibility,
    pub require_workflow_judgment: bool,
    pub require_stage_validation: bool,
    pub require_final_closeout: bool,
    pub reflection_blocks: bool,
    pub max_repair_attempts: usize,
    pub expose_weight_details: bool,
    pub reason: String,
}

impl RiskSensitiveWorkflowPolicy {
    pub fn from_route_and_judgment(
        route: &IntentRoute,
        judgment: Option<&ProgrammingWorkflowJudgment>,
    ) -> Self {
        let risk = judgment.map(|j| j.risk).unwrap_or(route.risk);
        let programming = is_programming_workflow(route.workflow);
        match risk {
            RiskLevel::High if programming => Self {
                depth: WorkflowDepth::Strict,
                visibility: WorkflowVisibility::Debug,
                require_workflow_judgment: true,
                require_stage_validation: true,
                require_final_closeout: true,
                reflection_blocks: true,
                max_repair_attempts: 3,
                expose_weight_details: true,
                reason: "high-risk programming workflow requires strict validation".to_string(),
            },
            RiskLevel::Medium if programming => Self {
                depth: WorkflowDepth::Minimal,
                visibility: WorkflowVisibility::Quiet,
                require_workflow_judgment: false,
                require_stage_validation: false,
                require_final_closeout: true,
                reflection_blocks: false,
                max_repair_attempts: 1,
                expose_weight_details: false,
                reason: "medium-risk programming workflow starts lightweight; strict checks are trigger-activated".to_string(),
            },
            _ if programming => Self {
                depth: WorkflowDepth::Minimal,
                visibility: WorkflowVisibility::Quiet,
                require_workflow_judgment: false,
                require_stage_validation: false,
                require_final_closeout: true,
                reflection_blocks: false,
                max_repair_attempts: 1,
                expose_weight_details: false,
                reason: "low-risk programming workflow stays fast with minimal checks".to_string(),
            },
            _ => Self {
                depth: WorkflowDepth::Minimal,
                visibility: WorkflowVisibility::Quiet,
                require_workflow_judgment: false,
                require_stage_validation: false,
                require_final_closeout: false,
                reflection_blocks: false,
                max_repair_attempts: 0,
                expose_weight_details: false,
                reason: "non-programming workflow does not need code-change gates".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageValidationRecord {
    pub step_id: Option<String>,
    pub step_description: Option<String>,
    pub status: StageValidationStatus,
    pub changed_files: Vec<String>,
    pub evidence: Vec<String>,
    pub feedback: Option<WeightFeedbackEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCloseout {
    pub status: StageValidationStatus,
    pub risk: RiskLevel,
    pub changed_files: Vec<String>,
    pub validation: Vec<String>,
    pub acceptance: Vec<String>,
    pub residual_risks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseoutVisibility {
    Hidden,
    Concise,
    Full,
}

impl WorkflowCloseout {
    pub fn evidence_summary(&self) -> String {
        let passed = self
            .validation
            .iter()
            .filter(|item| item.contains(": passed"))
            .count();
        let failed = self
            .validation
            .iter()
            .filter(|item| item.contains(": failed"))
            .count();
        let partial = self
            .validation
            .iter()
            .filter(|item| item.contains(": partial"))
            .count();
        let not_verified = self
            .validation
            .iter()
            .filter(|item| item.contains(": not_verified"))
            .count();
        let accepted = self
            .acceptance
            .iter()
            .filter(|item| item.contains("accepted=true"))
            .count();
        let rejected = self
            .acceptance
            .iter()
            .filter(|item| item.contains("accepted=false"))
            .count();
        let pending_acceptance = self
            .acceptance
            .iter()
            .filter(|item| item.starts_with("pending:"))
            .count();
        format!(
            "changed_files={} validation_passed={} validation_failed={} validation_partial={} validation_not_verified={} acceptance_passed={} acceptance_rejected={} acceptance_pending={}",
            self.changed_files.len(),
            passed,
            failed,
            partial,
            not_verified,
            accepted,
            rejected,
            pending_acceptance
        )
    }

    pub fn format_for_final_response(&self) -> String {
        let mut out = String::from("\n\nCloseout:\n");
        out.push_str(&format!("- Status: {}\n", self.status.label()));
        out.push_str(&format!("- Evidence: {}\n", self.evidence_summary()));
        out.push_str(&format!(
            "- Changed: {}\n",
            if self.changed_files.is_empty() {
                "none".to_string()
            } else {
                self.changed_files.join(", ")
            }
        ));
        out.push_str("- Verified:\n");
        append_bullets(&mut out, &self.validation);
        out.push_str("- Acceptance:\n");
        append_bullets(&mut out, &self.acceptance);
        out.push_str("- Risk:\n");
        append_bullets(&mut out, &self.residual_risks);
        out
    }

    pub fn default_visibility(&self) -> CloseoutVisibility {
        if self.risk == RiskLevel::High {
            return CloseoutVisibility::Full;
        }
        if matches!(
            self.status,
            StageValidationStatus::Failed | StageValidationStatus::Partial
        ) {
            return CloseoutVisibility::Full;
        }
        let has_real_risk = self
            .residual_risks
            .iter()
            .any(|item| item != "none recorded");
        let has_pending_or_rejected_acceptance = self
            .acceptance
            .iter()
            .any(|item| item.starts_with("pending:") || item.contains("accepted=false"));

        match self.status {
            StageValidationStatus::Passed
                if !has_real_risk && !has_pending_or_rejected_acceptance =>
            {
                CloseoutVisibility::Concise
            }
            StageValidationStatus::NotVerified if !has_pending_or_rejected_acceptance => {
                CloseoutVisibility::Concise
            }
            _ => CloseoutVisibility::Full,
        }
    }

    pub fn visibility_from_env(&self) -> CloseoutVisibility {
        match std::env::var("PRIORITY_AGENT_CLOSEOUT_VISIBILITY")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "hidden" | "off" | "none" => CloseoutVisibility::Hidden,
            "concise" | "quiet" | "summary" => CloseoutVisibility::Concise,
            "full" | "debug" | "verbose" => CloseoutVisibility::Full,
            _ => self.default_visibility(),
        }
    }

    pub fn format_concise_for_final_response(&self) -> String {
        let changed = if self.changed_files.is_empty() {
            "no changed files recorded".to_string()
        } else {
            format!("changed {}", self.changed_files.join(", "))
        };
        let validation = self
            .validation
            .iter()
            .find(|item| item.contains(": passed"))
            .or_else(|| self.validation.first())
            .map(|item| item.as_str())
            .unwrap_or("validation not recorded");
        let risks = self
            .residual_risks
            .iter()
            .filter(|item| item.as_str() != "none recorded")
            .cloned()
            .collect::<Vec<_>>();
        match self.status {
            StageValidationStatus::Passed if risks.is_empty() => {
                format!("\n\nDone. {}. Verified: {}.\n", changed, validation)
            }
            StageValidationStatus::NotVerified => {
                let risk_text = if risks.is_empty() {
                    "verification was not recorded".to_string()
                } else {
                    risks.join("; ")
                };
                format!(
                    "\n\nDone with caveats. {}. Not verified: {}. Risk: {}.\n",
                    changed, validation, risk_text
                )
            }
            _ => {
                let risk_text = if risks.is_empty() {
                    "none recorded".to_string()
                } else {
                    risks.join("; ")
                };
                format!(
                    "\n\nDone with caveats. {}. Verified: {}. Risk: {}.\n",
                    changed, validation, risk_text
                )
            }
        }
    }

    pub fn format_for_user_response(&self) -> String {
        match self.visibility_from_env() {
            CloseoutVisibility::Hidden => String::new(),
            CloseoutVisibility::Concise => self.format_concise_for_final_response(),
            CloseoutVisibility::Full => self.format_for_final_response(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodeChangeWorkflowRunner {
    pub task_id: String,
    pub policy: RiskSensitiveWorkflowPolicy,
    programming_workflow: bool,
    changed_files: Vec<String>,
    step_states: Vec<PlanStepRuntimeState>,
    validations: Vec<StageValidationRecord>,
    acceptance_reviews: Vec<AcceptanceReview>,
    residual_risks: Vec<String>,
    adaptive_triggers: Vec<AdaptiveWorkflowTrigger>,
}

impl CodeChangeWorkflowRunner {
    pub fn new(bundle: &TaskContextBundle) -> Self {
        let policy = RiskSensitiveWorkflowPolicy::from_route_and_judgment(
            &bundle.route,
            bundle.workflow_judgment.as_ref(),
        );
        Self {
            task_id: bundle.task_id.clone(),
            policy,
            programming_workflow: is_programming_workflow(bundle.route.workflow),
            changed_files: Vec::new(),
            step_states: step_states_from_bundle(bundle),
            validations: Vec::new(),
            acceptance_reviews: Vec::new(),
            residual_risks: Vec::new(),
            adaptive_triggers: Vec::new(),
        }
    }

    pub fn refresh_policy(&mut self, bundle: &TaskContextBundle) {
        self.policy = RiskSensitiveWorkflowPolicy::from_route_and_judgment(
            &bundle.route,
            bundle.workflow_judgment.as_ref(),
        );
        self.programming_workflow = is_programming_workflow(bundle.route.workflow);
        self.step_states = step_states_from_bundle(bundle);
        let triggers = self.adaptive_triggers.clone();
        for trigger in triggers {
            self.apply_trigger_to_policy(trigger);
        }
    }

    pub fn activate_trigger(&mut self, trigger: AdaptiveWorkflowTrigger) -> bool {
        if self.adaptive_triggers.contains(&trigger) {
            return false;
        }
        self.adaptive_triggers.push(trigger);
        self.apply_trigger_to_policy(trigger);
        true
    }

    pub fn adaptive_trigger_labels(&self) -> Vec<&'static str> {
        self.adaptive_triggers
            .iter()
            .map(|trigger| trigger.label())
            .collect()
    }

    pub fn should_run_acceptance_review(
        &self,
        verify_passed: bool,
        code_review_passed: bool,
        has_required_validation: bool,
        acceptance_checks: usize,
    ) -> bool {
        if acceptance_checks == 0 {
            return false;
        }
        matches!(self.policy.depth, WorkflowDepth::Strict)
            || has_required_validation
            || !verify_passed
            || !code_review_passed
            || self
                .adaptive_triggers
                .iter()
                .any(|trigger| matches!(trigger, AdaptiveWorkflowTrigger::AcceptanceRejected))
    }

    pub fn should_request_workflow_judgment(&self) -> bool {
        self.policy.require_workflow_judgment
    }

    pub fn should_block_on_reflection(&self) -> bool {
        self.policy.reflection_blocks
    }

    pub fn max_repair_attempts(&self) -> usize {
        self.policy.max_repair_attempts
    }

    fn apply_trigger_to_policy(&mut self, trigger: AdaptiveWorkflowTrigger) {
        match trigger {
            AdaptiveWorkflowTrigger::RiskSignalHigh => {
                self.policy.require_workflow_judgment = true;
                if self.programming_workflow {
                    self.policy.require_stage_validation = true;
                    self.policy.require_final_closeout = true;
                    self.policy.max_repair_attempts = self.policy.max_repair_attempts.max(2);
                }
                self.policy.visibility = WorkflowVisibility::Normal;
                let reason = if self.programming_workflow {
                    "high runtime risk signal activated workflow judgment and validation"
                } else {
                    "high runtime risk signal activated workflow judgment without code-change validation"
                };
                self.policy.reason = append_reason(&self.policy.reason, reason);
            }
            AdaptiveWorkflowTrigger::RequiredValidation => {
                self.policy.require_workflow_judgment = true;
                self.policy.require_stage_validation = true;
                self.policy.require_final_closeout = true;
                self.policy.max_repair_attempts = self.policy.max_repair_attempts.max(2);
                self.policy.visibility = WorkflowVisibility::Normal;
                self.policy.reason = append_reason(
                    &self.policy.reason,
                    "required validation command activated workflow judgment and stage validation",
                );
            }
            AdaptiveWorkflowTrigger::FirstCodeChange => {
                self.policy.require_stage_validation = true;
                self.policy.require_final_closeout = true;
                self.policy.max_repair_attempts = self.policy.max_repair_attempts.max(1);
                self.policy.reason = append_reason(
                    &self.policy.reason,
                    "code changes activated validation closeout",
                );
            }
            AdaptiveWorkflowTrigger::VerificationFailed => {
                self.policy.depth = WorkflowDepth::Strict;
                self.policy.visibility = WorkflowVisibility::Debug;
                self.policy.require_workflow_judgment = true;
                self.policy.require_stage_validation = true;
                self.policy.require_final_closeout = true;
                self.policy.max_repair_attempts = self.policy.max_repair_attempts.max(3);
                self.policy.expose_weight_details = true;
                self.policy.reason = append_reason(
                    &self.policy.reason,
                    "failed verification activated strict repair",
                );
            }
            AdaptiveWorkflowTrigger::AcceptanceRejected => {
                self.policy.depth = WorkflowDepth::Strict;
                self.policy.visibility = WorkflowVisibility::Debug;
                self.policy.require_stage_validation = true;
                self.policy.require_final_closeout = true;
                self.policy.max_repair_attempts = self.policy.max_repair_attempts.max(3);
                self.policy.expose_weight_details = true;
                self.policy.reason = append_reason(
                    &self.policy.reason,
                    "acceptance rejection activated strict repair",
                );
            }
            AdaptiveWorkflowTrigger::RepeatedNoCodeProgress => {
                self.policy.require_stage_validation = true;
                self.policy.require_final_closeout = true;
                self.policy.max_repair_attempts = self.policy.max_repair_attempts.max(2);
                self.policy.visibility = WorkflowVisibility::Normal;
                self.policy.reason = append_reason(
                    &self.policy.reason,
                    "repeated no-edit progress activated focused repair",
                );
            }
        }
    }

    pub fn record_stage_validation(
        &mut self,
        bundle: &TaskContextBundle,
        changed_files: &[PathBuf],
        verify_passed: bool,
        evidence: &[String],
    ) -> StageValidationRecord {
        let active = bundle
            .workflow_judgment
            .as_ref()
            .and_then(|judgment| judgment.top_plan_step());
        if let Some(active) = active.as_ref() {
            self.mark_active_step(active.id.as_deref(), &active.description);
        } else if self.step_states.is_empty() {
            self.step_states.push(PlanStepRuntimeState {
                id: None,
                description: "file-change validation".to_string(),
                status: PlanStepRuntimeStatus::Active,
                priority: "implicit".to_string(),
                last_evidence: None,
            });
        }
        let status = if verify_passed && !evidence.is_empty() {
            StageValidationStatus::Passed
        } else if evidence.is_empty() {
            StageValidationStatus::NotVerified
        } else {
            StageValidationStatus::Failed
        };
        let changed = changed_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        for path in &changed {
            push_unique(&mut self.changed_files, path.clone());
        }
        let feedback = (!verify_passed).then_some(WeightFeedbackEvent {
            kind: WeightFeedbackKind::TestFailure,
            severity: if matches!(self.policy.depth, WorkflowDepth::Strict) {
                WeightFeedbackSeverity::High
            } else {
                WeightFeedbackSeverity::Medium
            },
            confidence: 0.85,
            reason: Some("stage validation did not pass".to_string()),
        });
        let record = StageValidationRecord {
            step_id: active.as_ref().and_then(|step| step.id.clone()),
            step_description: active
                .map(|step| step.description)
                .or_else(|| Some("file-change validation".to_string())),
            status,
            changed_files: changed,
            evidence: evidence.iter().map(|item| preview(item, 160)).collect(),
            feedback,
        };
        self.mark_stage_result(&record);
        self.validations.push(record.clone());
        record
    }

    pub fn record_acceptance_review(&mut self, review: AcceptanceReview) {
        if review.accepted && review.unresolved_count() == 0 {
            self.residual_risks.clear();
            for step in &mut self.step_states {
                if matches!(
                    step.status,
                    PlanStepRuntimeStatus::Pending | PlanStepRuntimeStatus::Active
                ) {
                    step.status = PlanStepRuntimeStatus::Passed;
                    step.last_evidence =
                        Some("clean acceptance review completed the remaining plan".to_string());
                }
            }
        }
        for item in &review.unresolved_items {
            push_unique(&mut self.residual_risks, item.clone());
        }
        for item in &review.residual_risks {
            push_unique(&mut self.residual_risks, item.clone());
        }
        self.acceptance_reviews.push(review);
    }

    pub fn build_closeout(&self, bundle: &TaskContextBundle) -> Option<WorkflowCloseout> {
        self.build_closeout_with_runtime_validation(bundle, None)
    }

    pub fn build_closeout_with_runtime_validation(
        &self,
        bundle: &TaskContextBundle,
        runtime_validation_label: Option<&str>,
    ) -> Option<WorkflowCloseout> {
        if !self.policy.require_final_closeout {
            return None;
        }

        let no_diff_runtime_verified =
            self.no_diff_runtime_verified(bundle, runtime_validation_label);
        let status = self.closeout_status(bundle, runtime_validation_label);
        let mut validation = self
            .step_states
            .iter()
            .filter(|step| step.status != PlanStepRuntimeStatus::Pending)
            .map(|step| {
                let base = format!("{}: {}", step.description, step.status.label());
                step.last_evidence
                    .as_deref()
                    .and_then(validation_evidence_summary)
                    .map(|evidence| format!("{base} ({evidence})"))
                    .unwrap_or(base)
            })
            .collect::<Vec<_>>();
        if validation.is_empty() {
            if no_diff_runtime_verified {
                validation.push(format!(
                    "required validation: passed ({})",
                    runtime_validation_label.unwrap_or("passed")
                ));
            } else if self.policy.require_stage_validation {
                validation.push(
                    "No required file-change validation was recorded for this workflow".to_string(),
                );
            } else {
                validation.push("No file-change validation was required or recorded".to_string());
            }
        }
        if !self.adaptive_triggers.is_empty() {
            validation.push(format!(
                "Adaptive triggers: {}",
                self.adaptive_trigger_labels().join(", ")
            ));
        }

        let mut acceptance = Vec::new();
        for review in &self.acceptance_reviews {
            acceptance.push(format!(
                "accepted={} confidence={:?} unresolved={}",
                review.accepted,
                review.confidence,
                review.unresolved_count()
            ));
        }
        if acceptance.is_empty() && no_diff_runtime_verified && !bundle.acceptance_checks.is_empty()
        {
            acceptance.push(
                "accepted=true confidence=High unresolved=0 (required validation passed; code diff optional for audit/regression task)"
                    .to_string(),
            );
        }
        if acceptance.is_empty() {
            for check in &bundle.acceptance_checks {
                acceptance.push(format!("pending: {}", check));
            }
        }
        if acceptance.is_empty() {
            acceptance.push("No explicit acceptance criteria were recorded".to_string());
        }

        let mut residual = if self.has_clean_accepted_review() {
            self.latest_acceptance_review()
                .map(|review| review.residual_risks.clone())
                .unwrap_or_default()
        } else {
            self.residual_risks.clone()
        };
        if is_programming_workflow(bundle.route.workflow)
            && self.changed_files.is_empty()
            && !no_diff_runtime_verified
        {
            push_unique(
                &mut residual,
                "No changed files were recorded for this code-change workflow".to_string(),
            );
        }
        if self.policy.require_stage_validation
            && self.validations.is_empty()
            && !no_diff_runtime_verified
        {
            push_unique(
                &mut residual,
                "Required validation was not run or not recorded".to_string(),
            );
        }
        if !bundle.acceptance_checks.is_empty()
            && self.acceptance_reviews.is_empty()
            && !no_diff_runtime_verified
        {
            push_unique(
                &mut residual,
                "Acceptance criteria were generated but not reviewed".to_string(),
            );
        }
        if matches!(
            status,
            StageValidationStatus::Failed
                | StageValidationStatus::Partial
                | StageValidationStatus::NotVerified
        ) {
            push_unique(
                &mut residual,
                "Workflow finished with unresolved validation or acceptance risk".to_string(),
            );
        }
        if residual.is_empty() {
            residual.push("none recorded".to_string());
        }

        Some(WorkflowCloseout {
            status,
            risk: bundle.route.risk,
            changed_files: self.changed_files.clone(),
            validation,
            acceptance,
            residual_risks: residual,
        })
    }

    fn closeout_status(
        &self,
        bundle: &TaskContextBundle,
        runtime_validation_label: Option<&str>,
    ) -> StageValidationStatus {
        let latest_validation_status = self.validations.last().map(|record| record.status);
        let has_failed_validation = matches!(
            latest_validation_status,
            Some(StageValidationStatus::Failed)
        );
        let has_unverified_validation = matches!(
            latest_validation_status,
            Some(StageValidationStatus::NotVerified)
        );
        let has_partial_validation = matches!(
            latest_validation_status,
            Some(StageValidationStatus::Partial)
        );
        let latest_acceptance = self.latest_acceptance_review();
        let has_rejected_acceptance = latest_acceptance
            .map(|review| !review.accepted)
            .unwrap_or(false);
        let has_unresolved_acceptance = latest_acceptance
            .map(|review| review.unresolved_count() > 0)
            .unwrap_or(false);

        if has_failed_validation || has_rejected_acceptance {
            return StageValidationStatus::Failed;
        }

        if has_unverified_validation {
            return StageValidationStatus::NotVerified;
        }

        let no_diff_runtime_verified =
            self.no_diff_runtime_verified(bundle, runtime_validation_label);
        if self.policy.require_stage_validation
            && self.validations.is_empty()
            && !no_diff_runtime_verified
        {
            return StageValidationStatus::NotVerified;
        }

        if !bundle.acceptance_checks.is_empty()
            && self.acceptance_reviews.is_empty()
            && !no_diff_runtime_verified
        {
            return StageValidationStatus::NotVerified;
        }

        if is_programming_workflow(bundle.route.workflow)
            && self.changed_files.is_empty()
            && !self.has_clean_accepted_review()
            && !no_diff_runtime_verified
        {
            return StageValidationStatus::NotVerified;
        }

        let all_recorded_validations_passed = !self.validations.is_empty()
            && self
                .validations
                .iter()
                .all(|record| matches!(record.status, StageValidationStatus::Passed));
        if all_recorded_validations_passed
            && self.has_clean_accepted_review()
            && (!is_programming_workflow(bundle.route.workflow) || !self.changed_files.is_empty())
        {
            return StageValidationStatus::Passed;
        }

        if no_diff_runtime_verified && !has_partial_validation && !has_unresolved_acceptance {
            return StageValidationStatus::Passed;
        }

        if has_partial_validation
            || has_unresolved_acceptance
            || self.step_states.iter().any(|step| {
                matches!(
                    step.status,
                    PlanStepRuntimeStatus::Pending | PlanStepRuntimeStatus::Active
                )
            })
        {
            StageValidationStatus::Partial
        } else {
            StageValidationStatus::Passed
        }
    }

    fn no_diff_runtime_verified(
        &self,
        bundle: &TaskContextBundle,
        runtime_validation_label: Option<&str>,
    ) -> bool {
        self.changed_files.is_empty()
            && is_programming_workflow(bundle.route.workflow)
            && route_allows_no_diff_closeout(&bundle.route.reason)
            && runtime_validation_label_passed(runtime_validation_label)
    }

    fn has_clean_accepted_review(&self) -> bool {
        self.latest_acceptance_review()
            .map(|review| review.accepted && review.unresolved_count() == 0)
            .unwrap_or(false)
    }

    fn latest_acceptance_review(&self) -> Option<&AcceptanceReview> {
        self.acceptance_reviews.last()
    }

    pub fn step_states(&self) -> &[PlanStepRuntimeState] {
        &self.step_states
    }

    fn mark_active_step(&mut self, id: Option<&str>, description: &str) {
        let mut found = false;
        for step in &mut self.step_states {
            let same = match (id, step.id.as_deref()) {
                (Some(id), Some(step_id)) => id == step_id,
                _ => step.description == description,
            };
            if same && matches!(step.status, PlanStepRuntimeStatus::Pending) {
                step.status = PlanStepRuntimeStatus::Active;
                found = true;
            } else if step.status == PlanStepRuntimeStatus::Active {
                step.status = PlanStepRuntimeStatus::Pending;
            }
        }
        if !found && self.step_states.is_empty() {
            self.step_states.push(PlanStepRuntimeState {
                id: id.map(ToString::to_string),
                description: description.to_string(),
                status: PlanStepRuntimeStatus::Active,
                priority: "unknown".to_string(),
                last_evidence: None,
            });
        }
    }

    fn mark_stage_result(&mut self, record: &StageValidationRecord) {
        let runtime_status = match record.status {
            StageValidationStatus::Passed => PlanStepRuntimeStatus::Passed,
            StageValidationStatus::Failed => PlanStepRuntimeStatus::Failed,
            StageValidationStatus::Partial => PlanStepRuntimeStatus::Active,
            StageValidationStatus::NotVerified => PlanStepRuntimeStatus::Active,
        };
        let evidence = select_validation_evidence(record);
        for step in &mut self.step_states {
            let same = match (record.step_id.as_deref(), step.id.as_deref()) {
                (Some(id), Some(step_id)) => id == step_id,
                _ => record
                    .step_description
                    .as_ref()
                    .map(|description| description == &step.description)
                    .unwrap_or(false),
            };
            if same {
                step.status = runtime_status;
                step.last_evidence = evidence.clone();
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests;

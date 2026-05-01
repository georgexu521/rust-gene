//! Code-change workflow coordination.
//!
//! This module owns the lightweight runtime state for programming turns:
//! risk-sensitive policy, stage validation records, and structured closeout
//! material. The model still provides engineering judgment; this module keeps
//! the workflow state stable and auditable.

use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::engine::task_context::TaskContextBundle;
use crate::engine::workflow_contract::{
    AcceptanceReview, ProgrammingWorkflowJudgment, WeightFeedbackEvent, WeightFeedbackKind,
    WeightFeedbackSeverity,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
                depth: WorkflowDepth::Standard,
                visibility: WorkflowVisibility::Normal,
                require_workflow_judgment: true,
                require_stage_validation: true,
                require_final_closeout: true,
                reflection_blocks: false,
                max_repair_attempts: 2,
                expose_weight_details: false,
                reason: "medium-risk programming workflow uses lightweight validation".to_string(),
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
    pub changed_files: Vec<String>,
    pub validation: Vec<String>,
    pub acceptance: Vec<String>,
    pub residual_risks: Vec<String>,
}

impl WorkflowCloseout {
    pub fn format_for_final_response(&self) -> String {
        let mut out = String::from("\n\nCloseout:\n");
        out.push_str(&format!("- Status: {}\n", self.status.label()));
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
}

#[derive(Debug, Clone)]
pub struct CodeChangeWorkflowRunner {
    pub task_id: String,
    pub policy: RiskSensitiveWorkflowPolicy,
    changed_files: Vec<String>,
    step_states: Vec<PlanStepRuntimeState>,
    validations: Vec<StageValidationRecord>,
    acceptance_reviews: Vec<AcceptanceReview>,
    residual_risks: Vec<String>,
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
            changed_files: Vec::new(),
            step_states: step_states_from_bundle(bundle),
            validations: Vec::new(),
            acceptance_reviews: Vec::new(),
            residual_risks: Vec::new(),
        }
    }

    pub fn refresh_policy(&mut self, bundle: &TaskContextBundle) {
        self.policy = RiskSensitiveWorkflowPolicy::from_route_and_judgment(
            &bundle.route,
            bundle.workflow_judgment.as_ref(),
        );
        self.step_states = step_states_from_bundle(bundle);
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
        let status = if verify_passed {
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
        for item in &review.unresolved_items {
            push_unique(&mut self.residual_risks, item.clone());
        }
        for item in &review.residual_risks {
            push_unique(&mut self.residual_risks, item.clone());
        }
        self.acceptance_reviews.push(review);
    }

    pub fn build_closeout(&self, bundle: &TaskContextBundle) -> Option<WorkflowCloseout> {
        if !self.policy.require_final_closeout {
            return None;
        }

        let status = self.closeout_status(bundle);
        let mut validation = self
            .step_states
            .iter()
            .filter(|step| step.status != PlanStepRuntimeStatus::Pending)
            .map(|step| format!("{}: {}", step.description, step.status.label()))
            .collect::<Vec<_>>();
        if validation.is_empty() {
            if self.policy.require_stage_validation {
                validation.push(
                    "No required file-change validation was recorded for this workflow".to_string(),
                );
            } else {
                validation.push("No file-change validation was required or recorded".to_string());
            }
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
        if is_programming_workflow(bundle.route.workflow) && self.changed_files.is_empty() {
            push_unique(
                &mut residual,
                "No changed files were recorded for this code-change workflow".to_string(),
            );
        }
        if self.policy.require_stage_validation && self.validations.is_empty() {
            push_unique(
                &mut residual,
                "Required validation was not run or not recorded".to_string(),
            );
        }
        if !bundle.acceptance_checks.is_empty() && self.acceptance_reviews.is_empty() {
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
            changed_files: self.changed_files.clone(),
            validation,
            acceptance,
            residual_risks: residual,
        })
    }

    fn closeout_status(&self, bundle: &TaskContextBundle) -> StageValidationStatus {
        let has_failed_validation = self
            .validations
            .iter()
            .any(|record| matches!(record.status, StageValidationStatus::Failed));
        let has_unverified_validation = self
            .validations
            .iter()
            .any(|record| matches!(record.status, StageValidationStatus::NotVerified));
        let has_partial_validation = self
            .validations
            .iter()
            .any(|record| matches!(record.status, StageValidationStatus::Partial));
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

        if self.policy.require_stage_validation && self.validations.is_empty() {
            return StageValidationStatus::NotVerified;
        }

        if !bundle.acceptance_checks.is_empty() && self.acceptance_reviews.is_empty() {
            return StageValidationStatus::NotVerified;
        }

        if is_programming_workflow(bundle.route.workflow)
            && self.changed_files.is_empty()
            && !self.has_clean_accepted_review()
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
        let evidence = record.evidence.first().cloned();
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

pub fn is_programming_workflow(workflow: WorkflowKind) -> bool {
    matches!(workflow, WorkflowKind::CodeChange | WorkflowKind::BugFix)
}

fn append_bullets(out: &mut String, items: &[String]) {
    if items.is_empty() {
        out.push_str("  - none\n");
    } else {
        for item in items {
            out.push_str(&format!("  - {}\n", item));
        }
    }
}

fn step_states_from_bundle(bundle: &TaskContextBundle) -> Vec<PlanStepRuntimeState> {
    bundle
        .workflow_judgment
        .as_ref()
        .map(|judgment| {
            judgment
                .sorted_plan()
                .into_iter()
                .map(|step| PlanStepRuntimeState {
                    id: step.id,
                    description: step.description,
                    status: PlanStepRuntimeStatus::Pending,
                    priority: format!("{:?}", step.priority),
                    last_evidence: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.contains(&value) {
        items.push(value);
    }
}

fn preview(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

#[allow(dead_code)]
fn path_label(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{
        IntentKind, ReasoningPolicy, RetrievalPolicy, WorkflowKind,
    };
    use crate::engine::workflow_contract::{AcceptanceConfidence, AcceptanceNextAction};

    fn code_change_route(risk: RiskLevel) -> IntentRoute {
        IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.90,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk,
            recommended_tools: Vec::new(),
            reason: "test route".into(),
        }
    }

    #[test]
    fn policy_is_strict_for_high_risk_code() {
        let route = IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.95,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::High,
            risk: RiskLevel::High,
            recommended_tools: Vec::new(),
            reason: "test route".into(),
        };
        let policy = RiskSensitiveWorkflowPolicy::from_route_and_judgment(&route, None);

        assert_eq!(policy.depth, WorkflowDepth::Strict);
        assert!(policy.require_stage_validation);
        assert!(policy.reflection_blocks);
        assert_eq!(policy.max_repair_attempts, 3);
    }

    #[test]
    fn runner_builds_failed_closeout_from_stage_validation() {
        let route = code_change_route(RiskLevel::Medium);
        let bundle = TaskContextBundle::new("修改 CLI 状态栏", ".", route, None);
        let mut runner = CodeChangeWorkflowRunner::new(&bundle);
        runner.record_stage_validation(
            &bundle,
            &[PathBuf::from("src/main.rs")],
            false,
            &["cargo check failed".to_string()],
        );

        let closeout = runner.build_closeout(&bundle).unwrap();

        assert_eq!(closeout.status, StageValidationStatus::Failed);
        assert!(closeout
            .changed_files
            .iter()
            .any(|path| path == "src/main.rs"));
        assert!(closeout.format_for_final_response().contains("Closeout:"));
        assert!(runner
            .step_states()
            .iter()
            .any(|step| step.status == PlanStepRuntimeStatus::Failed));
    }

    #[test]
    fn closeout_is_not_verified_without_required_validation_or_acceptance() {
        let route = code_change_route(RiskLevel::Medium);
        let mut bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
        bundle.add_acceptance_check("memory_save respects quality gates");
        let runner = CodeChangeWorkflowRunner::new(&bundle);

        let closeout = runner.build_closeout(&bundle).unwrap();

        assert_eq!(closeout.status, StageValidationStatus::NotVerified);
        assert!(closeout
            .validation
            .iter()
            .any(|item| item.contains("No required file-change validation")));
        assert!(closeout
            .acceptance
            .iter()
            .any(|item| item.contains("pending: memory_save respects quality gates")));
        assert!(closeout
            .residual_risks
            .iter()
            .any(|item| item.contains("No changed files")));
    }

    #[test]
    fn closeout_passes_only_with_change_validation_and_clean_acceptance() {
        let route = code_change_route(RiskLevel::Medium);
        let mut bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
        bundle.add_acceptance_check("memory_save respects quality gates");
        let mut runner = CodeChangeWorkflowRunner::new(&bundle);

        runner.record_stage_validation(
            &bundle,
            &[PathBuf::from("src/tools/memory_tool/mod.rs")],
            true,
            &["cargo test -q memory -- --test-threads=1 passed".to_string()],
        );
        runner.record_acceptance_review(AcceptanceReview {
            accepted: true,
            confidence: AcceptanceConfidence::High,
            criteria: Vec::new(),
            unresolved_items: Vec::new(),
            residual_risks: Vec::new(),
            next_action: AcceptanceNextAction::Finish,
        });

        let closeout = runner.build_closeout(&bundle).unwrap();

        assert_eq!(closeout.status, StageValidationStatus::Passed);
        assert_eq!(
            closeout.changed_files,
            vec!["src/tools/memory_tool/mod.rs".to_string()]
        );
        assert!(closeout
            .residual_risks
            .iter()
            .any(|item| item == "none recorded"));
    }

    #[test]
    fn closeout_uses_latest_acceptance_review_for_current_status() {
        let route = code_change_route(RiskLevel::Medium);
        let mut bundle = TaskContextBundle::new("修复 memory_save 质量门控", ".", route, None);
        bundle.add_acceptance_check("memory_save respects quality gates");
        let mut runner = CodeChangeWorkflowRunner::new(&bundle);

        runner.record_stage_validation(
            &bundle,
            &[PathBuf::from("src/tools/memory_tool/mod.rs")],
            true,
            &["cargo test -q memory -- --test-threads=1 passed".to_string()],
        );
        runner.record_acceptance_review(AcceptanceReview {
            accepted: false,
            confidence: AcceptanceConfidence::Medium,
            criteria: Vec::new(),
            unresolved_items: vec!["initial review missed runtime save outcome".to_string()],
            residual_risks: vec!["format_memory_write_outcome not verified".to_string()],
            next_action: AcceptanceNextAction::ContinueRepair,
        });
        runner.record_acceptance_review(AcceptanceReview {
            accepted: true,
            confidence: AcceptanceConfidence::High,
            criteria: Vec::new(),
            unresolved_items: Vec::new(),
            residual_risks: Vec::new(),
            next_action: AcceptanceNextAction::Finish,
        });

        let closeout = runner.build_closeout(&bundle).unwrap();

        assert_eq!(closeout.status, StageValidationStatus::Passed);
        assert!(closeout
            .acceptance
            .iter()
            .any(|item| item.contains("accepted=false")));
        assert!(closeout
            .acceptance
            .iter()
            .any(|item| item.contains("accepted=true")));
        assert_eq!(closeout.residual_risks, vec!["none recorded".to_string()]);
    }
}

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
                max_repair_attempts: 1,
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

        let status = self.closeout_status();
        let mut validation = self
            .step_states
            .iter()
            .filter(|step| step.status != PlanStepRuntimeStatus::Pending)
            .map(|step| format!("{}: {}", step.description, step.status.label()))
            .collect::<Vec<_>>();
        if validation.is_empty() {
            validation.push("No file-change validation was required or recorded".to_string());
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

        let mut residual = self.residual_risks.clone();
        if matches!(
            status,
            StageValidationStatus::Failed | StageValidationStatus::Partial
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

    fn closeout_status(&self) -> StageValidationStatus {
        if self.validations.iter().any(|record| {
            matches!(
                record.status,
                StageValidationStatus::Failed | StageValidationStatus::NotVerified
            )
        }) || self
            .acceptance_reviews
            .iter()
            .any(|review| !review.accepted)
        {
            StageValidationStatus::Failed
        } else if self
            .acceptance_reviews
            .iter()
            .any(|review| review.unresolved_count() > 0)
        {
            StageValidationStatus::Partial
        } else {
            StageValidationStatus::Passed
        }
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
        assert_eq!(policy.max_repair_attempts, 1);
    }

    #[test]
    fn runner_builds_failed_closeout_from_stage_validation() {
        let route = IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.90,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk: RiskLevel::Medium,
            recommended_tools: Vec::new(),
            reason: "test route".into(),
        };
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
}

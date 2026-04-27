//! Model-led programming workflow contracts.
//!
//! This module defines the structured prompt contract used to ask the model to
//! judge programming-task completeness, risk, priority, guided reasoning needs,
//! and acceptance criteria. The software supplies the structure and records the
//! result; the model supplies the judgment.

use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::services::api::{ChatRequest, LlmProvider, Message};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityLabel {
    P0,
    P1,
    P2,
    P3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightSource {
    Factors,
    ModelImportance,
    LegacyWeight,
    PriorityLabelFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightOverrideStatus {
    None,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WeightFactors {
    pub dependency: f32,
    pub user_value: f32,
    pub risk_reduction: f32,
    pub uncertainty_reduction: f32,
    pub blocking: f32,
    pub cost: f32,
}

impl WeightFactors {
    pub fn from_priority(priority: PriorityLabel) -> Self {
        let base = priority_fallback_score(priority);
        Self {
            dependency: base,
            user_value: base,
            risk_reduction: base * 0.75,
            uncertainty_reduction: base * 0.75,
            blocking: base,
            cost: (1.0 - base).clamp(0.10, 0.75),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightOverride {
    pub adjusted_importance_score: f32,
    pub reason: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightComputation {
    pub formula_importance_score: f32,
    pub adjusted_importance_score: f32,
    pub weight_share: f32,
    pub priority: PriorityLabel,
    pub source: WeightSource,
    pub override_status: WeightOverrideStatus,
    pub override_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightFeedbackKind {
    TestFailure,
    ToolFailure,
    AcceptanceGap,
    GoalDrift,
    UserCorrection,
    StepCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightFeedbackSeverity {
    Low,
    Medium,
    High,
}

impl WeightFeedbackSeverity {
    pub fn multiplier(self) -> f32 {
        match self {
            Self::Low => 0.25,
            Self::Medium => 0.50,
            Self::High => 0.80,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightFeedbackEvent {
    pub kind: WeightFeedbackKind,
    pub severity: WeightFeedbackSeverity,
    pub confidence: f32,
    #[serde(default)]
    pub reason: Option<String>,
}

impl PriorityLabel {
    pub fn sort_rank(self) -> u8 {
        match self {
            Self::P0 => 0,
            Self::P1 => 1,
            Self::P2 => 2,
            Self::P3 => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidedReasoningTrigger {
    AmbiguousRequirement,
    CompetingApproaches,
    HighRiskArea,
    UnfamiliarCodePath,
    ToolFailure,
    TestFailure,
    UnexpectedDiff,
    RepeatedRepair,
    GoalDrift,
    ContextConflict,
    BroadProductRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceStatus {
    Pending,
    Passed,
    Failed,
    NotVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceNextAction {
    Finish,
    ContinueRepair,
    AskUser,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebuggingNextAction {
    InspectMore,
    Repair,
    AskUser,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlanStep {
    #[serde(default)]
    pub id: Option<String>,
    pub description: String,
    pub priority: PriorityLabel,
    #[serde(default)]
    pub weight: Option<f32>,
    #[serde(default)]
    pub importance_score: Option<f32>,
    #[serde(default)]
    pub weight_share: Option<f32>,
    #[serde(default)]
    pub factors: Option<WeightFactors>,
    #[serde(default, rename = "override")]
    pub override_adjustment: Option<WeightOverride>,
    #[serde(default)]
    pub computation: Option<WeightComputation>,
    pub reason: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

impl WorkflowPlanStep {
    pub fn normalized_weight(&self) -> f32 {
        self.computation
            .as_ref()
            .map(|computation| computation.adjusted_importance_score)
            .or(self.importance_score)
            .or(self.weight)
            .unwrap_or_else(|| priority_fallback_score(self.priority))
            .clamp(0.05, 1.0)
    }

    pub fn computed_weight_share(&self) -> f32 {
        self.computation
            .as_ref()
            .map(|computation| computation.weight_share)
            .or(self.weight_share)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    }

    pub fn weight_source(&self) -> Option<WeightSource> {
        self.computation
            .as_ref()
            .map(|computation| computation.source)
    }
}

pub fn compute_importance(factors: WeightFactors) -> f32 {
    let factors = sanitize_factors(factors);
    let positive = factors.dependency * 0.25
        + factors.user_value * 0.25
        + factors.risk_reduction * 0.20
        + factors.uncertainty_reduction * 0.15
        + factors.blocking * 0.15;
    let cost_penalty = factors.cost * 0.10;
    (positive - cost_penalty).clamp(0.05, 1.0)
}

pub fn priority_from_importance(score: f32) -> PriorityLabel {
    let score = sanitize_score(score).unwrap_or(0.25);
    if score >= 0.80 {
        PriorityLabel::P0
    } else if score >= 0.60 {
        PriorityLabel::P1
    } else if score >= 0.40 {
        PriorityLabel::P2
    } else {
        PriorityLabel::P3
    }
}

pub fn priority_fallback_score(priority: PriorityLabel) -> f32 {
    match priority {
        PriorityLabel::P0 => 0.90,
        PriorityLabel::P1 => 0.70,
        PriorityLabel::P2 => 0.50,
        PriorityLabel::P3 => 0.25,
    }
}

pub fn normalize_weight_shares(steps: &mut [WorkflowPlanStep]) {
    normalize_open_weight_shares(steps, &[]);
}

pub fn normalize_open_weight_shares(steps: &mut [WorkflowPlanStep], completed_step_ids: &[String]) {
    let is_completed = |step: &WorkflowPlanStep| {
        step.id
            .as_deref()
            .map(|id| completed_step_ids.iter().any(|completed| completed == id))
            .unwrap_or(false)
    };
    let sum = steps
        .iter()
        .filter(|step| !is_completed(step))
        .map(WorkflowPlanStep::normalized_weight)
        .sum::<f32>();

    let open_count = steps.iter().filter(|step| !is_completed(step)).count();
    let fallback_share = if open_count == 0 {
        0.0
    } else {
        1.0 / open_count as f32
    };

    for step in steps {
        let share = if is_completed(step) {
            0.0
        } else if sum > 0.0 {
            step.normalized_weight() / sum
        } else {
            fallback_share
        }
        .clamp(0.0, 1.0);
        step.weight_share = Some(share);
        if let Some(computation) = step.computation.as_mut() {
            computation.weight_share = share;
        }
    }
}

pub fn should_record_reweight(
    old_steps: &[WorkflowPlanStep],
    new_steps: &[WorkflowPlanStep],
) -> bool {
    if plan_step_key(top_sorted_step(old_steps)) != plan_step_key(top_sorted_step(new_steps)) {
        return true;
    }

    new_steps.iter().any(|new_step| {
        old_steps
            .iter()
            .find(|old_step| same_plan_step(old_step, new_step))
            .map(|old_step| {
                (old_step.normalized_weight() - new_step.normalized_weight()).abs() >= 0.15
                    || (old_step.computed_weight_share() - new_step.computed_weight_share()).abs()
                        >= 0.15
            })
            .unwrap_or(true)
    })
}

pub fn compute_step_weight(step: &WorkflowPlanStep) -> WeightComputation {
    let (formula_score, source) = if let Some(factors) = step.factors {
        (compute_importance(factors), WeightSource::Factors)
    } else if let Some(score) = sanitize_score_opt(step.importance_score) {
        (score.clamp(0.05, 1.0), WeightSource::ModelImportance)
    } else if let Some(score) = sanitize_score_opt(step.weight) {
        (score.clamp(0.05, 1.0), WeightSource::LegacyWeight)
    } else {
        (
            priority_fallback_score(step.priority),
            WeightSource::PriorityLabelFallback,
        )
    };

    let (adjusted_score, override_status, override_reason) =
        apply_weight_override(formula_score, step.override_adjustment.as_ref());
    let priority = priority_from_importance(adjusted_score);
    WeightComputation {
        formula_importance_score: formula_score,
        adjusted_importance_score: adjusted_score,
        weight_share: 0.0,
        priority,
        source,
        override_status,
        override_reason,
    }
}

pub fn recompute_step_weight(step: &mut WorkflowPlanStep) {
    if let Some(factors) = step.factors.as_mut() {
        *factors = sanitize_factors(*factors);
    }
    let computation = compute_step_weight(step);
    step.priority = computation.priority;
    step.importance_score = Some(computation.adjusted_importance_score);
    step.computation = Some(computation);
}

pub fn apply_weight_feedback(step: &mut WorkflowPlanStep, feedback: &WeightFeedbackEvent) {
    let confidence = sanitize_score(feedback.confidence).unwrap_or(0.0);
    let strength = (feedback.severity.multiplier() * confidence).clamp(0.0, 1.0);
    let mut factors = step
        .factors
        .unwrap_or_else(|| WeightFactors::from_priority(step.priority));

    match feedback.kind {
        WeightFeedbackKind::TestFailure => {
            factors.risk_reduction += 0.25 * strength;
            factors.uncertainty_reduction += 0.10 * strength;
            factors.blocking += 0.20 * strength;
        }
        WeightFeedbackKind::ToolFailure => {
            factors.uncertainty_reduction += 0.25 * strength;
            factors.blocking += 0.15 * strength;
            factors.cost += 0.20 * strength;
        }
        WeightFeedbackKind::AcceptanceGap => {
            factors.user_value += 0.20 * strength;
            factors.risk_reduction += 0.25 * strength;
            factors.blocking += 0.20 * strength;
        }
        WeightFeedbackKind::GoalDrift => {
            factors.risk_reduction += 0.20 * strength;
            factors.uncertainty_reduction += 0.30 * strength;
            factors.blocking += 0.20 * strength;
        }
        WeightFeedbackKind::UserCorrection => {
            factors.user_value += 0.28 * strength;
            factors.dependency += 0.10 * strength;
            factors.blocking += 0.14 * strength;
        }
        WeightFeedbackKind::StepCompleted => {
            factors.dependency -= 0.25 * strength;
            factors.user_value -= 0.20 * strength;
            factors.risk_reduction -= 0.25 * strength;
            factors.uncertainty_reduction -= 0.20 * strength;
            factors.blocking -= 0.30 * strength;
        }
    }

    step.factors = Some(sanitize_factors(factors));
    recompute_step_weight(step);
}

fn apply_weight_override(
    formula_score: f32,
    override_adjustment: Option<&WeightOverride>,
) -> (f32, WeightOverrideStatus, Option<String>) {
    let Some(override_adjustment) = override_adjustment else {
        return (formula_score, WeightOverrideStatus::None, None);
    };
    let Some(adjusted) = sanitize_score(override_adjustment.adjusted_importance_score) else {
        return (
            formula_score,
            WeightOverrideStatus::Rejected,
            Some("override adjusted_importance_score was invalid".to_string()),
        );
    };
    let confidence = sanitize_score(override_adjustment.confidence).unwrap_or(0.0);
    let delta = (adjusted - formula_score).abs();
    if delta > 0.25 && (confidence < 0.70 || override_adjustment.reason.trim().is_empty()) {
        return (
            formula_score,
            WeightOverrideStatus::Rejected,
            Some("override delta too large without enough confidence or reason".to_string()),
        );
    }
    (
        adjusted.clamp(0.05, 1.0),
        WeightOverrideStatus::Accepted,
        Some(override_adjustment.reason.clone()),
    )
}

fn sanitize_factors(factors: WeightFactors) -> WeightFactors {
    WeightFactors {
        dependency: sanitize_score(factors.dependency).unwrap_or(0.0),
        user_value: sanitize_score(factors.user_value).unwrap_or(0.0),
        risk_reduction: sanitize_score(factors.risk_reduction).unwrap_or(0.0),
        uncertainty_reduction: sanitize_score(factors.uncertainty_reduction).unwrap_or(0.0),
        blocking: sanitize_score(factors.blocking).unwrap_or(0.0),
        cost: sanitize_score(factors.cost).unwrap_or(0.0),
    }
}

fn sanitize_score_opt(value: Option<f32>) -> Option<f32> {
    value.and_then(sanitize_score)
}

fn sanitize_score(value: f32) -> Option<f32> {
    value.is_finite().then_some(value.clamp(0.0, 1.0))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub criterion: String,
    pub status: AcceptanceStatus,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceContract {
    pub original_user_goal: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub unresolved_items: Vec<String>,
    #[serde(default)]
    pub residual_risks: Vec<String>,
}

impl AcceptanceContract {
    pub fn pending(
        goal: impl Into<String>,
        criteria: Vec<String>,
        assumptions: Vec<String>,
    ) -> Self {
        Self {
            original_user_goal: goal.into(),
            assumptions,
            criteria: criteria
                .into_iter()
                .filter(|criterion| !criterion.trim().is_empty())
                .map(|criterion| AcceptanceCriterion {
                    criterion,
                    status: AcceptanceStatus::Pending,
                    evidence: None,
                })
                .collect(),
            unresolved_items: Vec::new(),
            residual_risks: Vec::new(),
        }
    }

    pub fn incomplete_count(&self) -> usize {
        self.criteria
            .iter()
            .filter(|criterion| !matches!(criterion.status, AcceptanceStatus::Passed))
            .count()
            + self.unresolved_items.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgrammingWorkflowJudgment {
    pub task_type: String,
    pub complexity: TaskComplexity,
    pub risk: RiskLevel,
    pub requirement_complete_enough: bool,
    pub needs_user_questions: bool,
    pub question_reason: Option<String>,
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub guided_reasoning_required: bool,
    #[serde(default)]
    pub guided_reasoning_triggers: Vec<GuidedReasoningTrigger>,
    #[serde(default)]
    pub plan: Vec<WorkflowPlanStep>,
    pub acceptance: AcceptanceContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceReview {
    pub accepted: bool,
    pub confidence: AcceptanceConfidence,
    #[serde(default)]
    pub criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub unresolved_items: Vec<String>,
    #[serde(default)]
    pub residual_risks: Vec<String>,
    pub next_action: AcceptanceNextAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidedDebuggingAnalysis {
    pub blocker: bool,
    pub symptom: String,
    #[serde(default)]
    pub likely_causes: Vec<String>,
    #[serde(default)]
    pub evidence_to_collect: Vec<String>,
    pub smallest_safe_action: String,
    pub ask_user: bool,
    #[serde(default)]
    pub questions: Vec<String>,
    pub next_action: DebuggingNextAction,
}

impl GuidedDebuggingAnalysis {
    pub fn format_for_prompt(&self) -> String {
        let mut out = format!(
            "Guided debugging analysis: blocker={} next_action={:?}\nSymptom: {}\nSmallest safe action: {}\n",
            self.blocker, self.next_action, self.symptom, self.smallest_safe_action
        );
        if !self.likely_causes.is_empty() {
            out.push_str("Likely causes:\n");
            for cause in &self.likely_causes {
                out.push_str(&format!("- {}\n", cause));
            }
        }
        if !self.evidence_to_collect.is_empty() {
            out.push_str("Evidence to collect:\n");
            for item in &self.evidence_to_collect {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if self.ask_user && !self.questions.is_empty() {
            out.push_str("Questions for user if blocked:\n");
            for question in &self.questions {
                out.push_str(&format!("- {}\n", question));
            }
        }
        out
    }
}

impl AcceptanceReview {
    pub fn unresolved_count(&self) -> usize {
        self.criteria
            .iter()
            .filter(|criterion| !matches!(criterion.status, AcceptanceStatus::Passed))
            .count()
            + self.unresolved_items.len()
    }

    pub fn format_for_prompt(&self) -> String {
        let mut out = format!(
            "Acceptance review: accepted={} confidence={:?} next_action={:?}\n",
            self.accepted, self.confidence, self.next_action
        );
        if !self.criteria.is_empty() {
            out.push_str("Criteria:\n");
            for criterion in &self.criteria {
                out.push_str(&format!(
                    "- [{:?}] {}{}{}\n",
                    criterion.status,
                    criterion.criterion,
                    if criterion.evidence.is_some() {
                        " -- "
                    } else {
                        ""
                    },
                    criterion.evidence.as_deref().unwrap_or("")
                ));
            }
        }
        if !self.unresolved_items.is_empty() {
            out.push_str("Unresolved items:\n");
            for item in &self.unresolved_items {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if !self.residual_risks.is_empty() {
            out.push_str("Residual risks:\n");
            for risk in &self.residual_risks {
                out.push_str(&format!("- {}\n", risk));
            }
        }
        out
    }
}

impl ProgrammingWorkflowJudgment {
    pub fn sorted_plan(&self) -> Vec<WorkflowPlanStep> {
        let mut steps = self.plan.clone();
        steps.sort_by(|a, b| compare_plan_steps(a, b));
        steps
    }

    pub fn weighted_plan_summary(&self) -> Vec<serde_json::Value> {
        self.sorted_plan()
            .into_iter()
            .map(|step| {
                serde_json::json!({
                    "id": step.id,
                    "description": step.description,
                    "priority": format!("{:?}", step.priority),
                    "importance_score": step.normalized_weight(),
                    "weight_share": step.computed_weight_share(),
                    "source": step.weight_source().map(|source| format!("{:?}", source)),
                    "reason": step.reason,
                })
            })
            .collect()
    }

    pub fn top_plan_step(&self) -> Option<WorkflowPlanStep> {
        self.sorted_plan().into_iter().next()
    }

    pub fn completed_plan_progress(&self) -> (usize, usize) {
        (self.plan.len(), self.plan.len())
    }
}

fn compare_plan_steps(a: &WorkflowPlanStep, b: &WorkflowPlanStep) -> std::cmp::Ordering {
    a.priority
        .sort_rank()
        .cmp(&b.priority.sort_rank())
        .then_with(|| {
            b.normalized_weight()
                .partial_cmp(&a.normalized_weight())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            b.factors
                .as_ref()
                .map(|factors| factors.blocking)
                .unwrap_or_default()
                .partial_cmp(
                    &a.factors
                        .as_ref()
                        .map(|factors| factors.blocking)
                        .unwrap_or_default(),
                )
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            b.factors
                .as_ref()
                .map(|factors| factors.dependency)
                .unwrap_or_default()
                .partial_cmp(
                    &a.factors
                        .as_ref()
                        .map(|factors| factors.dependency)
                        .unwrap_or_default(),
                )
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            a.factors
                .as_ref()
                .map(|factors| factors.cost)
                .unwrap_or(1.0)
                .partial_cmp(
                    &b.factors
                        .as_ref()
                        .map(|factors| factors.cost)
                        .unwrap_or(1.0),
                )
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn top_sorted_step(steps: &[WorkflowPlanStep]) -> Option<WorkflowPlanStep> {
    let mut sorted = steps.to_vec();
    sorted.sort_by(|a, b| compare_plan_steps(a, b));
    sorted.into_iter().next()
}

fn plan_step_key(step: Option<WorkflowPlanStep>) -> Option<String> {
    step.map(|step| {
        step.id
            .unwrap_or_else(|| format!("description:{}", step.description))
    })
}

fn same_plan_step(a: &WorkflowPlanStep, b: &WorkflowPlanStep) -> bool {
    match (a.id.as_deref(), b.id.as_deref()) {
        (Some(a), Some(b)) => a == b,
        _ => a.description == b.description,
    }
}

impl ProgrammingWorkflowJudgment {
    pub fn acceptance_checks(&self) -> Vec<String> {
        self.acceptance
            .criteria
            .iter()
            .map(|criterion| criterion.criterion.clone())
            .collect()
    }

    pub fn risk_notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        notes.push(format!("model-judged risk: {:?}", self.risk));
        if self.guided_reasoning_required {
            notes.push(format!(
                "guided reasoning triggers: {:?}",
                self.guided_reasoning_triggers
            ));
        }
        notes.extend(self.acceptance.residual_risks.clone());
        notes
    }

    pub fn to_turn_context(&self) -> String {
        let mut out = String::from("Model-led programming workflow judgment for this turn:\n");
        out.push_str(&format!(
            "- task_type: {}\n- complexity: {:?}\n- risk: {:?}\n",
            self.task_type, self.complexity, self.risk
        ));
        out.push_str(&format!(
            "- requirement_complete_enough: {}\n- needs_user_questions: {}\n",
            self.requirement_complete_enough, self.needs_user_questions
        ));
        if let Some(reason) = &self.question_reason {
            out.push_str(&format!("- question_reason: {}\n", reason));
        }
        if !self.questions.is_empty() {
            out.push_str("- questions_to_ask_before_execution:\n");
            for question in self.questions.iter().take(5) {
                out.push_str(&format!("  - {}\n", question));
            }
        }
        if !self.assumptions.is_empty() {
            out.push_str("- assumptions_if_proceeding:\n");
            for assumption in self.assumptions.iter().take(6) {
                out.push_str(&format!("  - {}\n", assumption));
            }
        }
        if !self.plan.is_empty() {
            out.push_str("- prioritized_plan:\n");
            for step in self.sorted_plan().iter().take(8) {
                out.push_str(&format!(
                    "  - [{:?} importance={:.2} share={:.2}] {} -- {}\n",
                    step.priority,
                    step.normalized_weight(),
                    step.computed_weight_share(),
                    step.description,
                    step.reason
                ));
            }
        }
        if !self.acceptance.criteria.is_empty() {
            out.push_str("- acceptance_criteria:\n");
            for criterion in self.acceptance.criteria.iter().take(8) {
                out.push_str(&format!("  - {}\n", criterion.criterion));
            }
        }
        out.push_str(
            "Use this as operating context. Ask the listed questions if they block correctness; otherwise proceed under the assumptions and verify against the acceptance criteria before final response.",
        );
        out
    }
}

#[derive(Debug, Clone)]
pub struct WorkflowContractPrompt {
    pub user_request: String,
    pub route: IntentRoute,
    pub working_dir: String,
}

impl WorkflowContractPrompt {
    pub fn new(
        user_request: impl Into<String>,
        route: IntentRoute,
        working_dir: impl Into<String>,
    ) -> Self {
        Self {
            user_request: user_request.into(),
            route,
            working_dir: working_dir.into(),
        }
    }

    pub fn should_ask_model(&self) -> bool {
        matches!(
            self.route.workflow,
            WorkflowKind::CodeChange
                | WorkflowKind::BugFix
                | WorkflowKind::Planning
                | WorkflowKind::Delegation
        )
    }

    pub fn render(&self) -> String {
        format!(
            r#"You are producing a model-led programming workflow judgment for Priority Agent.

Important principle:
- The software provides structure.
- You provide judgment.
- Do not assume the user must fill in numeric weights.
- Weight/priority is only a way to decide which plan step matters more.
- Use guided reasoning only when the task is complex, ambiguous, risky, or failing.
- Keep the output compact and operational.

User request:
{user_request}

Working directory:
{working_dir}

Advisory route from the runtime:
- intent: {intent:?}
- workflow: {workflow:?}
- retrieval: {retrieval:?}
- reasoning: {reasoning:?}
- risk: {risk:?}
- reason: {reason}

Return only valid JSON with this shape:
{{
  "task_type": "bug_fix | feature | refactor | website | investigation | review | other",
  "complexity": "low | medium | high",
  "risk": "low | medium | high",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": [],
  "guided_reasoning_required": false,
  "guided_reasoning_triggers": [],
  "plan": [
    {{
      "id": "stable-step-id",
      "description": "short action",
      "priority": "p0 | p1 | p2 | p3",
      "importance_score": 0.86,
      "weight_share": 0.0,
      "factors": {{
        "dependency": 0.0,
        "user_value": 0.0,
        "risk_reduction": 0.0,
        "uncertainty_reduction": 0.0,
        "blocking": 0.0,
        "cost": 0.0
      }},
      "override": null,
      "reason": "why this comes before other work",
      "acceptance_criteria": ["concrete check"]
    }}
  ],
  "acceptance": {{
    "original_user_goal": "restated user goal",
    "assumptions": [],
    "criteria": [
      {{
        "criterion": "what must be true before closeout",
        "status": "pending",
        "evidence": null
      }}
    ],
    "unresolved_items": [],
    "residual_risks": []
  }}
}}

Guidance:
- Ask user questions only when missing information affects architecture, data, permissions, deployment, UX, or acceptance criteria.
- If a conservative default is safe, proceed and record the assumption.
- For simple tasks, keep the plan short.
- For complex or high-risk tasks, include acceptance criteria and guided reasoning triggers.
- For low-risk tasks, priority and reason are enough.
- For medium-risk tasks, include priority, reason, and acceptance criteria.
- For high-risk tasks, include factors. The runtime will compute stable importance_score and weight_share from those factors.
- Do not ask the user to provide weights. You judge the factors; the software normalizes them.
"#,
            user_request = self.user_request,
            working_dir = self.working_dir,
            intent = self.route.intent,
            workflow = self.route.workflow,
            retrieval = self.route.retrieval,
            reasoning = self.route.reasoning,
            risk = self.route.risk,
            reason = self.route.reason
        )
    }
}

#[derive(Debug, Clone)]
pub struct AcceptanceReviewPrompt {
    pub contract: AcceptanceContract,
    pub changed_files: Vec<String>,
    pub verification_passed: bool,
    pub evidence: Vec<String>,
}

impl AcceptanceReviewPrompt {
    pub fn new(
        contract: AcceptanceContract,
        changed_files: Vec<String>,
        verification_passed: bool,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            contract,
            changed_files,
            verification_passed,
            evidence,
        }
    }

    pub fn render(&self) -> String {
        format!(
            r#"You are performing a model-led acceptance review for a programming task.

Judge whether the implementation satisfies the original user goal and acceptance criteria.
Do not pass criteria just because the intent was good. Use the evidence.
If evidence is missing, mark that criterion as not_verified.
If the task should continue, choose continue_repair. If a human choice is needed, choose ask_user.

Original goal:
{goal}

Assumptions:
{assumptions}

Original acceptance criteria:
{criteria}

Changed files:
{changed_files}

Verification passed:
{verification_passed}

Evidence:
{evidence}

Return only valid JSON with this shape:
{{
  "accepted": true,
  "confidence": "low | medium | high",
  "criteria": [
    {{
      "criterion": "criterion text",
      "status": "passed | failed | not_verified | pending",
      "evidence": "short evidence or null"
    }}
  ],
  "unresolved_items": [],
  "residual_risks": [],
  "next_action": "finish | continue_repair | ask_user | stop"
}}
"#,
            goal = self.contract.original_user_goal,
            assumptions = bullet_list(&self.contract.assumptions),
            criteria = bullet_list(
                &self
                    .contract
                    .criteria
                    .iter()
                    .map(|criterion| criterion.criterion.clone())
                    .collect::<Vec<_>>()
            ),
            changed_files = bullet_list(&self.changed_files),
            verification_passed = self.verification_passed,
            evidence = bullet_list(&self.evidence),
        )
    }
}

#[derive(Debug, Clone)]
pub struct GuidedDebuggingPrompt {
    pub user_request: String,
    pub workflow_context: Option<String>,
    pub failed_tools: Vec<String>,
    pub evidence: Vec<String>,
}

impl GuidedDebuggingPrompt {
    pub fn new(
        user_request: impl Into<String>,
        workflow_context: Option<String>,
        failed_tools: Vec<String>,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            user_request: user_request.into(),
            workflow_context,
            failed_tools,
            evidence,
        }
    }

    pub fn render(&self) -> String {
        format!(
            r#"You are performing guided debugging for a programming-agent workflow.

The agent hit a failure. Do not guess. Decide whether this is a blocker, what evidence resolves it fastest, and what the next safest action is.
Ask the user only when the next step requires a human product/permission/architecture decision.

User request:
{user_request}

Workflow context:
{workflow_context}

Failed tools:
{failed_tools}

Evidence:
{evidence}

Return only valid JSON with this shape:
{{
  "blocker": false,
  "symptom": "exact failure in one sentence",
  "likely_causes": ["cause"],
  "evidence_to_collect": ["focused check"],
  "smallest_safe_action": "next action",
  "ask_user": false,
  "questions": [],
  "next_action": "inspect_more | repair | ask_user | stop"
}}
"#,
            user_request = self.user_request,
            workflow_context = self
                .workflow_context
                .as_deref()
                .unwrap_or("No structured workflow context was available."),
            failed_tools = bullet_list(&self.failed_tools),
            evidence = bullet_list(&self.evidence),
        )
    }
}

pub struct WorkflowContractAnalyzer<'a> {
    provider: &'a dyn LlmProvider,
    model: String,
}

impl<'a> WorkflowContractAnalyzer<'a> {
    pub fn new(provider: &'a dyn LlmProvider, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }

    pub async fn analyze(
        &self,
        prompt: WorkflowContractPrompt,
    ) -> anyhow::Result<ProgrammingWorkflowJudgment> {
        let request = ChatRequest::new(self.model.clone())
            .with_temperature(0.1)
            .with_messages(vec![
                Message::system("Return only valid JSON. Do not include markdown fences."),
                Message::user(prompt.render()),
            ]);
        let response = self.provider.chat(request).await?;
        parse_workflow_judgment(&response.content)
    }

    pub async fn review_acceptance(
        &self,
        prompt: AcceptanceReviewPrompt,
    ) -> anyhow::Result<AcceptanceReview> {
        let request = ChatRequest::new(self.model.clone())
            .with_temperature(0.1)
            .with_messages(vec![
                Message::system("Return only valid JSON. Do not include markdown fences."),
                Message::user(prompt.render()),
            ]);
        let response = self.provider.chat(request).await?;
        parse_acceptance_review(&response.content)
    }

    pub async fn analyze_debugging(
        &self,
        prompt: GuidedDebuggingPrompt,
    ) -> anyhow::Result<GuidedDebuggingAnalysis> {
        let request = ChatRequest::new(self.model.clone())
            .with_temperature(0.1)
            .with_messages(vec![
                Message::system("Return only valid JSON. Do not include markdown fences."),
                Message::user(prompt.render()),
            ]);
        let response = self.provider.chat(request).await?;
        parse_guided_debugging_analysis(&response.content)
    }
}

pub fn parse_workflow_judgment(content: &str) -> anyhow::Result<ProgrammingWorkflowJudgment> {
    let json = extract_json_object(content)
        .ok_or_else(|| anyhow::anyhow!("workflow judgment response did not contain JSON"))?;
    let mut judgment: ProgrammingWorkflowJudgment = serde_json::from_str(json)?;
    normalize_judgment(&mut judgment);
    Ok(judgment)
}

pub fn parse_acceptance_review(content: &str) -> anyhow::Result<AcceptanceReview> {
    let json = extract_json_object(content)
        .ok_or_else(|| anyhow::anyhow!("acceptance review response did not contain JSON"))?;
    Ok(serde_json::from_str(json)?)
}

pub fn parse_guided_debugging_analysis(content: &str) -> anyhow::Result<GuidedDebuggingAnalysis> {
    let json = extract_json_object(content)
        .ok_or_else(|| anyhow::anyhow!("guided debugging response did not contain JSON"))?;
    Ok(serde_json::from_str(json)?)
}

fn normalize_judgment(judgment: &mut ProgrammingWorkflowJudgment) {
    for (index, step) in judgment.plan.iter_mut().enumerate() {
        if step.id.as_deref().unwrap_or_default().trim().is_empty() {
            step.id = Some(format!("step-{}", index + 1));
        }
        step.weight = sanitize_score_opt(step.weight);
        step.importance_score = sanitize_score_opt(step.importance_score);
        step.weight_share = sanitize_score_opt(step.weight_share);
        if let Some(factors) = step.factors.as_mut() {
            *factors = sanitize_factors(*factors);
        }
        if let Some(override_adjustment) = step.override_adjustment.as_mut() {
            override_adjustment.adjusted_importance_score =
                sanitize_score(override_adjustment.adjusted_importance_score).unwrap_or(0.0);
            override_adjustment.confidence =
                sanitize_score(override_adjustment.confidence).unwrap_or(0.0);
        }
        recompute_step_weight(step);
    }
    normalize_weight_shares(&mut judgment.plan);
    if judgment.acceptance.original_user_goal.trim().is_empty() {
        judgment.acceptance.original_user_goal = judgment.task_type.clone();
    }
    if judgment.acceptance.criteria.is_empty() {
        judgment.acceptance.criteria = judgment
            .plan
            .iter()
            .flat_map(|step| step.acceptance_criteria.clone())
            .filter(|criterion| !criterion.trim().is_empty())
            .map(|criterion| AcceptanceCriterion {
                criterion,
                status: AcceptanceStatus::Pending,
                evidence: None,
            })
            .collect();
    }
}

fn extract_json_object(content: &str) -> Option<&str> {
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }

    let start = content.find('{')?;
    let end = content.rfind('}')?;
    (end > start).then_some(&content[start..=end])
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        return "- none".to_string();
    }
    items
        .iter()
        .map(|item| format!("- {}", item))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn prompt_emphasizes_model_led_judgment() {
        let route = IntentRouter::new().route("帮我做一个网站");
        let prompt = WorkflowContractPrompt::new("帮我做一个网站", route, ".").render();

        assert!(prompt.contains("You provide judgment"));
        assert!(prompt.contains("Do not assume the user must fill in numeric weights"));
        assert!(prompt.contains("Return only valid JSON"));
    }

    #[test]
    fn code_change_routes_need_model_judgment() {
        let route = IntentRouter::new().route("实现一个新网站");
        let prompt = WorkflowContractPrompt::new("实现一个新网站", route, ".");

        assert!(prompt.should_ask_model());
    }

    #[test]
    fn direct_routes_can_skip_model_judgment() {
        let route = IntentRouter::new().route("你好");
        let prompt = WorkflowContractPrompt::new("你好", route, ".");

        assert!(!prompt.should_ask_model());
    }

    #[test]
    fn parse_judgment_from_fenced_text() {
        let content = r#"```json
{
  "task_type": "feature",
  "complexity": "medium",
  "risk": "medium",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": ["Use existing patterns"],
  "guided_reasoning_required": false,
  "guided_reasoning_triggers": [],
  "plan": [
    {
      "description": "Inspect existing code",
      "priority": "p0",
      "weight": 1.2,
      "reason": "Need context before editing",
      "acceptance_criteria": ["Relevant files read"]
    }
  ],
  "acceptance": {
    "original_user_goal": "Add feature",
    "assumptions": [],
    "criteria": [],
    "unresolved_items": [],
    "residual_risks": []
  }
}
```"#;

        let judgment = parse_workflow_judgment(content).unwrap();

        assert_eq!(judgment.plan[0].weight, Some(1.0));
        assert_eq!(judgment.plan[0].importance_score, Some(1.0));
        assert_eq!(judgment.plan[0].computed_weight_share(), 1.0);
        assert_eq!(judgment.acceptance.criteria.len(), 1);
        assert_eq!(judgment.sorted_plan()[0].priority, PriorityLabel::P0);
    }

    #[test]
    fn computes_factor_based_importance_and_weight_share() {
        let content = r#"{
  "task_type": "feature",
  "complexity": "high",
  "risk": "high",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": [],
  "guided_reasoning_required": true,
  "guided_reasoning_triggers": ["high_risk_area"],
  "plan": [
    {
      "id": "schema",
      "description": "Define data model",
      "priority": "p2",
      "factors": {
        "dependency": 0.95,
        "user_value": 0.80,
        "risk_reduction": 0.70,
        "uncertainty_reduction": 0.75,
        "blocking": 0.90,
        "cost": 0.30
      },
      "reason": "Everything depends on schema",
      "acceptance_criteria": ["Schema supports tags"]
    },
    {
      "id": "ui",
      "description": "Polish UI",
      "priority": "p2",
      "factors": {
        "dependency": 0.20,
        "user_value": 0.60,
        "risk_reduction": 0.20,
        "uncertainty_reduction": 0.20,
        "blocking": 0.10,
        "cost": 0.40
      },
      "reason": "Important but not blocking",
      "acceptance_criteria": ["UI remains usable"]
    }
  ],
  "acceptance": {
    "original_user_goal": "Build app",
    "assumptions": [],
    "criteria": [],
    "unresolved_items": [],
    "residual_risks": []
  }
}"#;

        let judgment = parse_workflow_judgment(content).unwrap();
        let sorted = judgment.sorted_plan();

        assert_eq!(sorted[0].id.as_deref(), Some("schema"));
        assert_eq!(sorted[0].weight_source(), Some(WeightSource::Factors));
        assert!(sorted[0].normalized_weight() > sorted[1].normalized_weight());
        let total_share = judgment
            .plan
            .iter()
            .map(WorkflowPlanStep::computed_weight_share)
            .sum::<f32>();
        assert!((total_share - 1.0).abs() < 0.001);
    }

    #[test]
    fn rejects_large_low_confidence_override() {
        let mut step = WorkflowPlanStep {
            id: Some("ui".into()),
            description: "Polish UI".into(),
            priority: PriorityLabel::P2,
            weight: None,
            importance_score: Some(0.40),
            weight_share: None,
            factors: None,
            override_adjustment: Some(WeightOverride {
                adjusted_importance_score: 0.90,
                reason: "looks better".into(),
                confidence: 0.40,
            }),
            computation: None,
            reason: "visual task".into(),
            acceptance_criteria: Vec::new(),
        };

        step.computation = Some(compute_step_weight(&step));

        let computation = step.computation.unwrap();
        assert_eq!(computation.override_status, WeightOverrideStatus::Rejected);
        assert!((computation.adjusted_importance_score - 0.40).abs() < 0.001);
    }

    #[test]
    fn feedback_reweights_acceptance_gaps_upward() {
        let mut step = WorkflowPlanStep {
            id: Some("validation".into()),
            description: "Validate persistence".into(),
            priority: PriorityLabel::P2,
            weight: None,
            importance_score: None,
            weight_share: None,
            factors: Some(WeightFactors {
                dependency: 0.40,
                user_value: 0.45,
                risk_reduction: 0.40,
                uncertainty_reduction: 0.35,
                blocking: 0.30,
                cost: 0.25,
            }),
            override_adjustment: None,
            computation: None,
            reason: "Need evidence".into(),
            acceptance_criteria: Vec::new(),
        };
        recompute_step_weight(&mut step);
        let before = step.normalized_weight();

        apply_weight_feedback(
            &mut step,
            &WeightFeedbackEvent {
                kind: WeightFeedbackKind::AcceptanceGap,
                severity: WeightFeedbackSeverity::High,
                confidence: 0.90,
                reason: Some("acceptance review found missing persistence proof".into()),
            },
        );

        assert!(step.normalized_weight() > before + 0.05);
        assert!(matches!(
            step.priority,
            PriorityLabel::P0 | PriorityLabel::P1 | PriorityLabel::P2
        ));
    }

    #[test]
    fn completed_step_feedback_lowers_remaining_importance() {
        let mut step = WorkflowPlanStep {
            id: Some("inspect".into()),
            description: "Inspect entry points".into(),
            priority: PriorityLabel::P0,
            weight: None,
            importance_score: None,
            weight_share: None,
            factors: Some(WeightFactors {
                dependency: 0.95,
                user_value: 0.70,
                risk_reduction: 0.80,
                uncertainty_reduction: 0.90,
                blocking: 0.90,
                cost: 0.30,
            }),
            override_adjustment: None,
            computation: None,
            reason: "Entry points block the refactor".into(),
            acceptance_criteria: Vec::new(),
        };
        recompute_step_weight(&mut step);
        let before = step.normalized_weight();

        apply_weight_feedback(
            &mut step,
            &WeightFeedbackEvent {
                kind: WeightFeedbackKind::StepCompleted,
                severity: WeightFeedbackSeverity::High,
                confidence: 1.0,
                reason: Some("entry points inspected".into()),
            },
        );

        assert!(step.normalized_weight() < before);
    }

    #[test]
    fn normalizes_only_open_steps_when_completed_ids_are_known() {
        let mut steps = vec![
            WorkflowPlanStep {
                id: Some("done".into()),
                description: "Completed work".into(),
                priority: PriorityLabel::P0,
                weight: Some(0.90),
                importance_score: None,
                weight_share: None,
                factors: None,
                override_adjustment: None,
                computation: None,
                reason: "Already complete".into(),
                acceptance_criteria: Vec::new(),
            },
            WorkflowPlanStep {
                id: Some("open".into()),
                description: "Open work".into(),
                priority: PriorityLabel::P1,
                weight: Some(0.70),
                importance_score: None,
                weight_share: None,
                factors: None,
                override_adjustment: None,
                computation: None,
                reason: "Still needed".into(),
                acceptance_criteria: Vec::new(),
            },
        ];
        for step in &mut steps {
            recompute_step_weight(step);
        }

        normalize_open_weight_shares(&mut steps, &[String::from("done")]);

        assert_eq!(steps[0].computed_weight_share(), 0.0);
        assert_eq!(steps[1].computed_weight_share(), 1.0);
    }

    #[test]
    fn detects_meaningful_reweight_changes() {
        let old_steps = vec![WorkflowPlanStep {
            id: Some("repair".into()),
            description: "Repair persistence".into(),
            priority: PriorityLabel::P2,
            weight: Some(0.45),
            importance_score: Some(0.45),
            weight_share: Some(1.0),
            factors: None,
            override_adjustment: None,
            computation: Some(WeightComputation {
                formula_importance_score: 0.45,
                adjusted_importance_score: 0.45,
                weight_share: 1.0,
                priority: PriorityLabel::P2,
                source: WeightSource::ModelImportance,
                override_status: WeightOverrideStatus::None,
                override_reason: None,
            }),
            reason: "Fix issue".into(),
            acceptance_criteria: Vec::new(),
        }];
        let mut new_steps = old_steps.clone();
        new_steps[0].importance_score = Some(0.75);
        new_steps[0].computation = Some(WeightComputation {
            formula_importance_score: 0.75,
            adjusted_importance_score: 0.75,
            weight_share: 1.0,
            priority: PriorityLabel::P1,
            source: WeightSource::ModelImportance,
            override_status: WeightOverrideStatus::None,
            override_reason: None,
        });
        new_steps[0].priority = PriorityLabel::P1;

        assert!(should_record_reweight(&old_steps, &new_steps));
    }

    #[test]
    fn acceptance_contract_counts_incomplete_items() {
        let contract = AcceptanceContract::pending(
            "Build app",
            vec!["Main flow works".into()],
            vec!["Local storage".into()],
        );

        assert_eq!(contract.incomplete_count(), 1);
    }

    #[test]
    fn parse_acceptance_review_from_fenced_text() {
        let content = r#"```json
{
  "accepted": false,
  "confidence": "medium",
  "criteria": [
    {
      "criterion": "Tests pass",
      "status": "not_verified",
      "evidence": "No test command was run"
    }
  ],
  "unresolved_items": ["Run focused tests"],
  "residual_risks": ["Manual browser flow not checked"],
  "next_action": "continue_repair"
}
```"#;

        let review = parse_acceptance_review(content).unwrap();

        assert!(!review.accepted);
        assert_eq!(review.unresolved_count(), 2);
        assert_eq!(review.next_action, AcceptanceNextAction::ContinueRepair);
    }

    #[test]
    fn parse_guided_debugging_analysis_from_json() {
        let content = r#"{
  "blocker": true,
  "symptom": "cargo test failed with a type error",
  "likely_causes": ["new enum variant not matched"],
  "evidence_to_collect": ["run cargo check"],
  "smallest_safe_action": "add the missing match arm",
  "ask_user": false,
  "questions": [],
  "next_action": "repair"
}"#;

        let analysis = parse_guided_debugging_analysis(content).unwrap();

        assert!(analysis.blocker);
        assert_eq!(analysis.next_action, DebuggingNextAction::Repair);
        assert!(analysis
            .format_for_prompt()
            .contains("Smallest safe action"));
    }
}

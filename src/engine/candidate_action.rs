//! Gated candidate-action scoring.
//!
//! This module intentionally does not force every turn through candidate JSON.
//! It provides the small runtime contract used when the agent is stuck,
//! repeatedly revised, or about to take a risky low-value action.
//! Candidate order and model-provided scores remain authoritative for semantic
//! selection; runtime scores are advisory calibration evidence.

use crate::engine::action_decision::{ActionDecision, ActionDecisionInput, ActionScores};
use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateActionMode {
    Off,
    Shadow,
    Gated,
}

impl CandidateActionMode {
    pub fn from_env() -> Self {
        let explicit = std::env::var("PRIORITY_AGENT_CANDIDATE_ACTIONS").ok();
        match explicit
            .as_deref()
            .unwrap_or_else(|| {
                if model_led_weighting_enabled() {
                    "shadow"
                } else {
                    "off"
                }
            })
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "shadow" => Self::Shadow,
            "gated" | "on" | "1" | "true" => Self::Gated,
            _ => Self::Off,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Shadow => "shadow",
            Self::Gated => "gated",
        }
    }
}

pub fn model_led_weighting_enabled() -> bool {
    !matches!(
        std::env::var("PRIORITY_AGENT_MODEL_LED_WEIGHTING")
            .unwrap_or_else(|_| "1".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "no" | "off"
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateActionSet {
    #[serde(default)]
    pub candidate_actions: Vec<CandidateAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateAction {
    pub id: String,
    pub action_type: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
    pub reason: String,
    #[serde(default)]
    pub expected_observation: Option<String>,
    #[serde(default)]
    pub model_scores: Option<CandidateModelScores>,
    #[serde(default)]
    pub model_factors: Option<CandidateModelFactors>,
    #[serde(default)]
    pub evidence: Vec<CandidateActionEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateModelScores {
    pub value: u8,
    pub risk: u8,
    pub uncertainty_reduction: u8,
    pub cost: u8,
    pub reversibility: u8,
    pub scope_fit: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateModelFactors {
    #[serde(default)]
    pub goal_importance: u8,
    #[serde(default)]
    pub evidence_strength: u8,
    #[serde(default)]
    pub uncertainty_reduction: u8,
    #[serde(default)]
    pub risk: u8,
    #[serde(default)]
    pub cost: u8,
    #[serde(default)]
    pub reversibility: u8,
    #[serde(default)]
    pub scope_fit: u8,
    #[serde(default)]
    pub validation_need: u8,
    #[serde(default)]
    pub memory_relevance: u8,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateActionEvidence {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub relevance: String,
    #[serde(default)]
    pub quote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateActionRanking {
    pub mode: CandidateActionMode,
    pub candidate_count: usize,
    pub selected_id: Option<String>,
    pub selected_tool: Option<String>,
    pub selected_score: Option<i16>,
    #[serde(default)]
    pub selected_runtime_score: Option<i16>,
    #[serde(default)]
    pub selected_model_score: Option<i16>,
    #[serde(default)]
    pub runtime_model_score_delta: Option<i16>,
    #[serde(default)]
    pub runtime_selected_differs_from_model_order: bool,
    #[serde(default)]
    pub calibration_reason: String,
    #[serde(default)]
    pub selected_factor_score: Option<i16>,
    #[serde(default)]
    pub model_factor_coverage: usize,
    #[serde(default)]
    pub memory_evidence_items: usize,
    #[serde(default)]
    pub selected_factor_rationale: Option<String>,
    pub rejected: Vec<CandidateActionRejection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateActionRejection {
    pub id: String,
    pub tool: String,
    pub reason: String,
}

type AcceptedCandidate<'a> = (
    usize,
    &'a CandidateAction,
    ActionDecision,
    Option<i16>,
    Option<i16>,
);

pub fn parse_candidate_actions(content: &str) -> Result<CandidateActionSet, String> {
    let raw = extract_json_object(content).ok_or_else(|| "candidate JSON not found".to_string())?;
    serde_json::from_str::<CandidateActionSet>(raw)
        .map_err(|err| format!("invalid candidate action JSON: {err}"))
}

pub fn rank_candidate_actions(
    candidates: &CandidateActionSet,
    input: ActionDecisionInput,
    exposed_tools: &HashSet<String>,
    mode: CandidateActionMode,
) -> CandidateActionRanking {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();

    for (index, candidate) in candidates.candidate_actions.iter().enumerate() {
        if candidate.id.trim().is_empty() {
            rejected.push(rejection(candidate, "candidate id is empty"));
            continue;
        }
        if candidate.tool.trim().is_empty() {
            rejected.push(rejection(candidate, "candidate tool is empty"));
            continue;
        }
        if !candidate.action_type.eq_ignore_ascii_case("tool_call") {
            rejected.push(rejection(
                candidate,
                "candidate action_type must be tool_call",
            ));
            continue;
        }
        if !exposed_tools.contains(&candidate.tool) {
            rejected.push(rejection(candidate, "candidate tool is not exposed"));
            continue;
        }

        let tool_call = ToolCall {
            id: candidate.id.clone(),
            name: candidate.tool.clone(),
            arguments: candidate.arguments.clone(),
        };
        let decision = ActionDecision::for_tool_call(&tool_call, input);
        let factor_score = candidate.model_factors.as_ref().map(model_factor_score);
        let model_score = candidate
            .model_scores
            .as_ref()
            .map(model_action_score)
            .or(factor_score);
        accepted.push((index, candidate, decision, model_score, factor_score));
    }

    let runtime_selected_id = accepted
        .iter()
        .max_by(|left, right| runtime_score_order(left, right))
        .map(|(_, candidate, _, _, _)| candidate.id.clone());

    accepted.sort_by(model_authority_order);

    let selected = accepted.first();
    let selected_id = selected.map(|(_, candidate, _, _, _)| candidate.id.clone());
    let selected_runtime_score =
        selected.map(|(_, _, decision, _, _)| decision.scores.action_score);
    let selected_model_score = selected.and_then(|(_, _, _, model_score, _)| *model_score);
    let selected_factor_score = selected.and_then(|(_, _, _, _, factor_score)| *factor_score);
    let selected_factor_rationale = selected
        .and_then(|(_, candidate, _, _, _)| candidate.model_factors.as_ref())
        .map(|factors| factors.rationale.clone());
    let model_factor_coverage = accepted
        .iter()
        .filter(|(_, candidate, _, _, _)| candidate.model_factors.is_some())
        .count();
    let memory_evidence_items = candidates
        .candidate_actions
        .iter()
        .flat_map(|candidate| candidate.evidence.iter())
        .filter(|evidence| evidence.source.to_ascii_lowercase().contains("memory"))
        .count();
    let runtime_model_score_delta = selected_runtime_score
        .zip(selected_model_score)
        .map(|(runtime, model)| runtime - model);
    let runtime_selected_differs_from_model_order = selected_id.is_some()
        && runtime_selected_id.is_some()
        && selected_id != runtime_selected_id;
    let calibration_reason = if selected_model_score.is_some() {
        if runtime_selected_differs_from_model_order {
            "model score selected the candidate; runtime score recorded a different advisory preference"
                .to_string()
        } else {
            "model score selected the candidate; runtime score agreed as advisory calibration"
                .to_string()
        }
    } else if runtime_selected_differs_from_model_order {
        "model scores unavailable; preserved model candidate order while runtime score stayed advisory"
            .to_string()
    } else {
        "model scores unavailable; model candidate order and runtime advisory score agreed"
            .to_string()
    };

    CandidateActionRanking {
        mode,
        candidate_count: candidates.candidate_actions.len(),
        selected_id,
        selected_tool: selected.map(|(_, candidate, _, _, _)| candidate.tool.clone()),
        selected_score: selected_runtime_score,
        selected_runtime_score,
        selected_model_score,
        runtime_model_score_delta,
        runtime_selected_differs_from_model_order,
        calibration_reason,
        selected_factor_score,
        model_factor_coverage,
        memory_evidence_items,
        selected_factor_rationale,
        rejected,
    }
}

fn model_authority_order(left: &AcceptedCandidate<'_>, right: &AcceptedCandidate<'_>) -> Ordering {
    match (left.3, right.3) {
        (Some(left_score), Some(right_score)) => right_score
            .cmp(&left_score)
            .then_with(|| left.0.cmp(&right.0)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.0.cmp(&right.0),
    }
}

fn runtime_score_order(left: &AcceptedCandidate<'_>, right: &AcceptedCandidate<'_>) -> Ordering {
    left.2
        .scores
        .action_score
        .cmp(&right.2.scores.action_score)
        .then_with(|| left.2.scores.scope_fit.cmp(&right.2.scores.scope_fit))
        .then_with(|| right.2.scores.risk.cmp(&left.2.scores.risk))
        .then_with(|| right.0.cmp(&left.0))
}

impl From<CandidateModelScores> for ActionScores {
    fn from(value: CandidateModelScores) -> Self {
        Self {
            value: value.value.min(10),
            risk: value.risk.min(10),
            uncertainty_reduction: value.uncertainty_reduction.min(10),
            cost: value.cost.min(10),
            reversibility: value.reversibility.min(10),
            scope_fit: value.scope_fit.min(10),
            action_score: 0,
        }
    }
}

fn rejection(candidate: &CandidateAction, reason: &str) -> CandidateActionRejection {
    CandidateActionRejection {
        id: candidate.id.clone(),
        tool: candidate.tool.clone(),
        reason: reason.to_string(),
    }
}

fn model_action_score(scores: &CandidateModelScores) -> i16 {
    i16::from(scores.value)
        + i16::from(scores.uncertainty_reduction) * 2
        + i16::from(scores.scope_fit)
        + i16::from(scores.reversibility)
        - i16::from(scores.risk)
        - i16::from(scores.cost)
}

fn model_factor_score(factors: &CandidateModelFactors) -> i16 {
    i16::from(factors.goal_importance.min(10)) * 2
        + i16::from(factors.evidence_strength.min(10))
        + i16::from(factors.uncertainty_reduction.min(10)) * 2
        + i16::from(factors.scope_fit.min(10)) * 2
        + i16::from(factors.validation_need.min(10))
        + i16::from(factors.memory_relevance.min(10))
        + i16::from(factors.reversibility.min(10))
        - i16::from(factors.risk.min(10)) * 2
        - i16::from(factors.cost.min(10))
}

fn extract_json_object(content: &str) -> Option<&str> {
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (start < end).then_some(&trimmed[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{RiskLevel, WorkflowKind};
    use crate::engine::task_context::AgentTaskStage;

    fn input(stage: AgentTaskStage) -> ActionDecisionInput {
        ActionDecisionInput {
            task_stage: stage,
            route_workflow: Some(WorkflowKind::CodeChange),
            route_risk: Some(RiskLevel::Medium),
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        }
    }

    #[test]
    fn parses_candidate_action_json_from_text() {
        let parsed = parse_candidate_actions(
            r#"Candidate set:
            {"candidate_actions":[{"id":"read","action_type":"tool_call","tool":"file_read","arguments":{"path":"src/lib.rs"},"reason":"inspect first"}]}"#,
        )
        .unwrap();

        assert_eq!(parsed.candidate_actions.len(), 1);
        assert_eq!(parsed.candidate_actions[0].tool, "file_read");
    }

    #[test]
    fn parses_partial_model_factors_without_rejecting_candidate_set() {
        let parsed = parse_candidate_actions(
            r#"{"candidate_actions":[{"id":"validate","action_type":"tool_call","tool":"bash","arguments":{"command":"cargo test -q"},"reason":"check change","model_factors":{"goal_importance":8,"rationale":"validation matters"},"evidence":[{"source":"memory"}]}]}"#,
        )
        .unwrap();

        let factors = parsed.candidate_actions[0].model_factors.as_ref().unwrap();
        assert_eq!(factors.goal_importance, 8);
        assert_eq!(factors.scope_fit, 0);
        assert_eq!(factors.rationale, "validation matters");
        assert_eq!(parsed.candidate_actions[0].evidence[0].source, "memory");
        assert_eq!(parsed.candidate_actions[0].evidence[0].relevance, "");
    }

    #[test]
    fn preserves_model_tool_order_when_scores_are_absent() {
        let candidates = CandidateActionSet {
            candidate_actions: vec![
                CandidateAction {
                    id: "edit".to_string(),
                    action_type: "tool_call".to_string(),
                    tool: "file_edit".to_string(),
                    arguments: serde_json::json!({"path":"src/lib.rs"}),
                    reason: "edit now".to_string(),
                    expected_observation: None,
                    model_scores: None,
                    model_factors: None,
                    evidence: Vec::new(),
                },
                CandidateAction {
                    id: "read".to_string(),
                    action_type: "tool_call".to_string(),
                    tool: "file_read".to_string(),
                    arguments: serde_json::json!({"path":"src/lib.rs"}),
                    reason: "inspect first".to_string(),
                    expected_observation: None,
                    model_scores: None,
                    model_factors: None,
                    evidence: Vec::new(),
                },
            ],
        };
        let exposed = HashSet::from(["file_edit".to_string(), "file_read".to_string()]);

        let ranking = rank_candidate_actions(
            &candidates,
            input(AgentTaskStage::Understand),
            &exposed,
            CandidateActionMode::Shadow,
        );

        assert_eq!(ranking.candidate_count, 2);
        assert_eq!(ranking.selected_id.as_deref(), Some("edit"));
        assert!(ranking.runtime_selected_differs_from_model_order);
        assert!(ranking
            .calibration_reason
            .contains("preserved model candidate order"));
        assert!(ranking.rejected.is_empty());
    }

    #[test]
    fn model_scores_drive_selection_before_runtime_scores() {
        let candidates = CandidateActionSet {
            candidate_actions: vec![
                CandidateAction {
                    id: "edit".to_string(),
                    action_type: "tool_call".to_string(),
                    tool: "file_edit".to_string(),
                    arguments: serde_json::json!({"path":"src/lib.rs"}),
                    reason: "edit now".to_string(),
                    expected_observation: None,
                    model_scores: Some(CandidateModelScores {
                        value: 3,
                        risk: 7,
                        uncertainty_reduction: 2,
                        cost: 5,
                        reversibility: 5,
                        scope_fit: 3,
                    }),
                    model_factors: None,
                    evidence: Vec::new(),
                },
                CandidateAction {
                    id: "read".to_string(),
                    action_type: "tool_call".to_string(),
                    tool: "file_read".to_string(),
                    arguments: serde_json::json!({"path":"src/lib.rs"}),
                    reason: "inspect first".to_string(),
                    expected_observation: None,
                    model_scores: Some(CandidateModelScores {
                        value: 8,
                        risk: 1,
                        uncertainty_reduction: 8,
                        cost: 2,
                        reversibility: 10,
                        scope_fit: 9,
                    }),
                    model_factors: None,
                    evidence: Vec::new(),
                },
            ],
        };
        let exposed = HashSet::from(["file_edit".to_string(), "file_read".to_string()]);

        let ranking = rank_candidate_actions(
            &candidates,
            input(AgentTaskStage::Understand),
            &exposed,
            CandidateActionMode::Shadow,
        );

        assert_eq!(ranking.selected_id.as_deref(), Some("read"));
        assert!(ranking.selected_model_score.is_some());
        assert!(ranking.calibration_reason.contains("model score selected"));
        assert!(ranking.rejected.is_empty());
    }

    #[test]
    fn rejects_unexposed_candidate_tools() {
        let candidates = CandidateActionSet {
            candidate_actions: vec![CandidateAction {
                id: "shell".to_string(),
                action_type: "tool_call".to_string(),
                tool: "bash".to_string(),
                arguments: serde_json::json!({"command":"rm -rf target"}),
                reason: "bad idea".to_string(),
                expected_observation: None,
                model_scores: None,
                model_factors: None,
                evidence: Vec::new(),
            }],
        };
        let exposed = HashSet::from(["file_read".to_string()]);

        let ranking = rank_candidate_actions(
            &candidates,
            input(AgentTaskStage::Understand),
            &exposed,
            CandidateActionMode::Shadow,
        );

        assert_eq!(ranking.selected_id, None);
        assert_eq!(ranking.rejected[0].reason, "candidate tool is not exposed");
    }

    #[test]
    fn model_factors_produce_shadow_scores_and_memory_evidence_counts() {
        let candidates = CandidateActionSet {
            candidate_actions: vec![CandidateAction {
                id: "validate".to_string(),
                action_type: "tool_call".to_string(),
                tool: "bash".to_string(),
                arguments: serde_json::json!({"command":"corepack pnpm --dir apps/desktop test:ui-smoke"}),
                reason: "validate UI change with remembered smoke test".to_string(),
                expected_observation: Some("smoke test result".to_string()),
                model_scores: None,
                model_factors: Some(CandidateModelFactors {
                    goal_importance: 8,
                    evidence_strength: 8,
                    uncertainty_reduction: 7,
                    risk: 2,
                    cost: 4,
                    reversibility: 9,
                    scope_fit: 9,
                    validation_need: 10,
                    memory_relevance: 8,
                    rationale: "memory says UI changes need the desktop smoke test".to_string(),
                }),
                evidence: vec![CandidateActionEvidence {
                    source: "memory".to_string(),
                    relevance: "prior UI validation convention".to_string(),
                    quote: Some("run desktop smoke after UI changes".to_string()),
                }],
            }],
        };
        let exposed = HashSet::from(["bash".to_string()]);

        let ranking = rank_candidate_actions(
            &candidates,
            input(AgentTaskStage::Validate),
            &exposed,
            CandidateActionMode::Shadow,
        );

        assert_eq!(ranking.selected_id.as_deref(), Some("validate"));
        assert!(ranking.selected_model_score.is_some());
        assert_eq!(ranking.selected_factor_score, ranking.selected_model_score);
        assert_eq!(ranking.model_factor_coverage, 1);
        assert_eq!(ranking.memory_evidence_items, 1);
        assert!(ranking
            .selected_factor_rationale
            .as_deref()
            .unwrap_or_default()
            .contains("desktop smoke"));
    }
}

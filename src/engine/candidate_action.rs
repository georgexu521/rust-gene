//! Gated candidate-action scoring.
//!
//! This module intentionally does not force every turn through candidate JSON.
//! It provides the small runtime contract used when the agent is stuck,
//! repeatedly revised, or about to take a risky low-value action.

use crate::engine::action_decision::{ActionDecision, ActionDecisionInput, ActionScores};
use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
        match std::env::var("PRIORITY_AGENT_CANDIDATE_ACTIONS")
            .unwrap_or_else(|_| "off".to_string())
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
    pub rejected: Vec<CandidateActionRejection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateActionRejection {
    pub id: String,
    pub tool: String,
    pub reason: String,
}

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

    for candidate in &candidates.candidate_actions {
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
        let model_score = candidate.model_scores.as_ref().map(model_action_score);
        accepted.push((candidate, decision, model_score));
    }

    let model_selected_id = accepted
        .iter()
        .filter_map(|(candidate, _, model_score)| model_score.map(|score| (*candidate, score)))
        .max_by(
            |(left_candidate, left_score), (right_candidate, right_score)| {
                left_score
                    .cmp(right_score)
                    .then_with(|| right_candidate.id.cmp(&left_candidate.id))
            },
        )
        .map(|(candidate, _)| candidate.id.clone());

    accepted.sort_by(|(_, left, _), (_, right, _)| {
        right
            .scores
            .action_score
            .cmp(&left.scores.action_score)
            .then_with(|| right.scores.scope_fit.cmp(&left.scores.scope_fit))
            .then_with(|| left.scores.risk.cmp(&right.scores.risk))
    });

    let selected = accepted.first();
    let selected_id = selected.map(|(candidate, _, _)| candidate.id.clone());
    let selected_runtime_score = selected.map(|(_, decision, _)| decision.scores.action_score);
    let selected_model_score = selected.and_then(|(_, _, model_score)| *model_score);
    let runtime_model_score_delta = selected_runtime_score
        .zip(selected_model_score)
        .map(|(runtime, model)| runtime - model);
    let runtime_selected_differs_from_model_order =
        selected_id.is_some() && model_selected_id.is_some() && selected_id != model_selected_id;
    let calibration_reason = if selected_model_score.is_none() {
        "model candidate scores unavailable; runtime score only".to_string()
    } else if runtime_selected_differs_from_model_order {
        "runtime ranking selected a different candidate than model score order".to_string()
    } else {
        "runtime ranking agreed with model score order for the selected candidate".to_string()
    };

    CandidateActionRanking {
        mode,
        candidate_count: candidates.candidate_actions.len(),
        selected_id,
        selected_tool: selected.map(|(candidate, _, _)| candidate.tool.clone()),
        selected_score: selected_runtime_score,
        selected_runtime_score,
        selected_model_score,
        runtime_model_score_delta,
        runtime_selected_differs_from_model_order,
        calibration_reason,
        rejected,
    }
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
    fn ranks_exposed_candidates_by_runtime_score() {
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
                },
                CandidateAction {
                    id: "read".to_string(),
                    action_type: "tool_call".to_string(),
                    tool: "file_read".to_string(),
                    arguments: serde_json::json!({"path":"src/lib.rs"}),
                    reason: "inspect first".to_string(),
                    expected_observation: None,
                    model_scores: None,
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
        assert_eq!(ranking.selected_id.as_deref(), Some("read"));
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
}

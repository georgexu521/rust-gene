//! Minimal plans for light agent turns.
//!
//! These plans are runtime diagnostics and context hints. They do not replace
//! model reasoning and they intentionally stay much smaller than workflow
//! contracts.

use crate::engine::intent_router::{IntentRoute, RetrievalPolicy, RiskLevel, WorkflowKind};
use crate::engine::task_context::AgentTaskMode;
use crate::engine::task_mode_score::TaskModeScore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LightweightPlan {
    pub objective: String,
    pub steps: Vec<LightweightPlanStep>,
    pub verification_required: bool,
    pub heavy_contract_avoided: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LightweightPlanStep {
    pub label: String,
    pub action: String,
    pub expected_observation: String,
}

impl LightweightPlan {
    pub fn format_for_context_zone(&self) -> String {
        let steps = self
            .steps
            .iter()
            .map(|step| {
                format!(
                    "{}: {} -> {}",
                    step.label, step.action, step.expected_observation
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!(
            "{}; verification_required={}; steps={}",
            self.objective, self.verification_required, steps
        )
    }
}

pub struct LightweightPlanner;

impl LightweightPlanner {
    pub fn plan(
        prompt: &str,
        route: &IntentRoute,
        score: &TaskModeScore,
    ) -> Option<LightweightPlan> {
        if score.mode != AgentTaskMode::Light {
            return None;
        }

        let mut steps = vec![LightweightPlanStep {
            label: "scope".to_string(),
            action: "keep the turn bounded to the user's explicit request".to_string(),
            expected_observation: "clear boundary for what needs inspection or action".to_string(),
        }];

        if route.recommended_tools.is_empty() {
            steps.push(LightweightPlanStep {
                label: "answer".to_string(),
                action: "answer directly without expanding into a workflow".to_string(),
                expected_observation: "concise response".to_string(),
            });
        } else {
            steps.push(LightweightPlanStep {
                label: "observe".to_string(),
                action: format!(
                    "use only the narrow recommended tools: {}",
                    route.recommended_tools.join(", ")
                ),
                expected_observation: observation_for_route(route),
            });
            steps.push(LightweightPlanStep {
                label: "respond".to_string(),
                action: "summarize the observed result and any bounded follow-up".to_string(),
                expected_observation: "answer grounded in the small observation set".to_string(),
            });
        }

        Some(LightweightPlan {
            objective: preview(prompt, 140),
            steps,
            verification_required: verification_required(route),
            heavy_contract_avoided: true,
            reason: format!(
                "light mode from task score; route={:?}/{:?} confidence={}",
                route.intent, route.workflow, score.confidence
            ),
        })
    }
}

fn observation_for_route(route: &IntentRoute) -> String {
    match route.workflow {
        WorkflowKind::Direct if route.retrieval == RetrievalPolicy::Light => {
            "small local observation or terminal result".to_string()
        }
        WorkflowKind::Research => "bounded research evidence".to_string(),
        WorkflowKind::Planning => "enough project context to write a short plan".to_string(),
        WorkflowKind::Delegation => "delegation target and expected output".to_string(),
        WorkflowKind::CodeChange | WorkflowKind::BugFix => {
            "targeted project evidence before any mutation".to_string()
        }
        WorkflowKind::Direct => "direct observation".to_string(),
    }
}

fn verification_required(route: &IntentRoute) -> bool {
    route.risk != RiskLevel::Low
        || route.recommended_tools.iter().any(|tool| {
            matches!(
                tool.as_str(),
                "bash" | "file_edit" | "file_write" | "file_patch"
            )
        })
}

fn preview(text: &str, max_chars: usize) -> String {
    let mut out = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out.replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn planner_creates_bounded_steps_for_local_inspection() {
        let prompt = "请帮我看看桌面有没有 gex 文件夹";
        let route = IntentRouter::new().route(prompt);
        let score = TaskModeScore::from_route(prompt, &route);
        let plan = LightweightPlanner::plan(prompt, &route, &score).expect("light plan");

        assert!(plan.heavy_contract_avoided);
        assert_eq!(plan.steps.len(), 3);
        assert!(plan.steps[1].action.contains("glob"));
        assert!(!plan.verification_required);
    }

    #[test]
    fn planner_skips_full_code_changes() {
        let prompt = "帮我做一个贪吃蛇游戏吧，用 python 做吧";
        let route = IntentRouter::new().route(prompt);
        let score = TaskModeScore::from_route(prompt, &route);

        assert!(LightweightPlanner::plan(prompt, &route, &score).is_none());
    }
}

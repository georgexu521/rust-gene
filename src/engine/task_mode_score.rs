//! Runtime-owned task mode scoring.
//!
//! This keeps direct/light/full/high-risk routing explainable without asking the
//! model to carry a heavyweight workflow contract for every turn.

use crate::engine::intent_router::{
    IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
};
use crate::engine::task_context::AgentTaskMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskModeScore {
    pub mode: AgentTaskMode,
    pub confidence: u8,
    pub complexity: u8,
    pub risk: u8,
    pub uncertainty: u8,
    pub tool_need: u8,
    pub user_impact: u8,
    pub reason: String,
}

impl Default for TaskModeScore {
    fn default() -> Self {
        Self {
            mode: AgentTaskMode::Direct,
            confidence: 60,
            complexity: 1,
            risk: 1,
            uncertainty: 4,
            tool_need: 0,
            user_impact: 1,
            reason: "default direct task mode score".to_string(),
        }
    }
}

impl TaskModeScore {
    pub fn from_route(prompt: &str, route: &IntentRoute) -> Self {
        let complexity = complexity_score(route);
        let risk = risk_score(route);
        let uncertainty = uncertainty_score(route);
        let tool_need = tool_need_score(route);
        let user_impact = user_impact_score(route, prompt);
        let mode = mode_from_scores(route, complexity, risk, uncertainty, tool_need, user_impact);
        let confidence = route_confidence(route, mode, uncertainty);
        let reason = format!(
            "mode={:?} complexity={} risk={} uncertainty={} tool_need={} user_impact={} route_confidence={:.2}",
            mode, complexity, risk, uncertainty, tool_need, user_impact, route.confidence
        );

        Self {
            mode,
            confidence,
            complexity,
            risk,
            uncertainty,
            tool_need,
            user_impact,
            reason,
        }
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "{:?} confidence={} complexity={} risk={} uncertainty={} tools={} impact={}",
            self.mode,
            self.confidence,
            self.complexity,
            self.risk,
            self.uncertainty,
            self.tool_need,
            self.user_impact
        )
    }
}

fn complexity_score(route: &IntentRoute) -> u8 {
    let base = match route.workflow {
        WorkflowKind::Direct => {
            if route.recommended_tools.is_empty() {
                1
            } else {
                3
            }
        }
        WorkflowKind::Research => 4,
        WorkflowKind::Planning | WorkflowKind::Delegation => 5,
        WorkflowKind::CodeChange => 7,
        WorkflowKind::BugFix => 8,
    };
    if route.reasoning == ReasoningPolicy::High {
        (base + 1).min(10)
    } else {
        base
    }
}

fn risk_score(route: &IntentRoute) -> u8 {
    match route.risk {
        RiskLevel::Low => 1,
        RiskLevel::Medium => 5,
        RiskLevel::High => 9,
    }
}

fn uncertainty_score(route: &IntentRoute) -> u8 {
    let confidence_gap = ((1.0 - route.confidence.clamp(0.0, 1.0)) * 10.0).round() as u8;
    let reasoning_bump = match route.reasoning {
        ReasoningPolicy::Low => 0,
        ReasoningPolicy::Medium => 1,
        ReasoningPolicy::High => 2,
    };
    let retrieval_bump = match route.retrieval {
        RetrievalPolicy::None => 0,
        RetrievalPolicy::Light => 1,
        RetrievalPolicy::Project | RetrievalPolicy::Memory => 2,
        RetrievalPolicy::Web | RetrievalPolicy::Full => 3,
    };
    confidence_gap
        .saturating_add(reasoning_bump)
        .saturating_add(retrieval_bump)
        .min(10)
}

fn tool_need_score(route: &IntentRoute) -> u8 {
    if route.recommended_tools.is_empty() {
        return 0;
    }
    let base = (route.recommended_tools.len() as u8)
        .saturating_mul(2)
        .min(8);
    match route.workflow {
        WorkflowKind::CodeChange | WorkflowKind::BugFix => base.max(7),
        WorkflowKind::Direct => base.min(5),
        WorkflowKind::Research | WorkflowKind::Planning | WorkflowKind::Delegation => base.max(4),
    }
}

fn user_impact_score(route: &IntentRoute, prompt: &str) -> u8 {
    let lower = prompt.to_ascii_lowercase();
    let mutation_hint = contains_any(
        &lower,
        &["delete", "remove", "write", "edit", "fix", "push"],
    ) || prompt.contains("删除")
        || prompt.contains("修改")
        || prompt.contains("修复")
        || prompt.contains("推送");
    let base = match route.workflow {
        WorkflowKind::Direct => {
            if mutation_hint {
                5
            } else {
                2
            }
        }
        WorkflowKind::Research | WorkflowKind::Planning => 4,
        WorkflowKind::Delegation => 5,
        WorkflowKind::CodeChange | WorkflowKind::BugFix => 7,
    };
    if route.risk == RiskLevel::High {
        (base + 2).min(10)
    } else {
        base
    }
}

fn mode_from_scores(
    route: &IntentRoute,
    complexity: u8,
    risk: u8,
    uncertainty: u8,
    tool_need: u8,
    user_impact: u8,
) -> AgentTaskMode {
    if route.risk == RiskLevel::High || risk >= 9 {
        return AgentTaskMode::HighRisk;
    }
    if route.workflow == WorkflowKind::Direct && route.recommended_tools.is_empty() {
        return AgentTaskMode::Direct;
    }
    if route.workflow == WorkflowKind::Direct {
        return AgentTaskMode::Light;
    }
    let load = complexity
        .saturating_add(tool_need)
        .saturating_add(user_impact)
        .saturating_add(uncertainty / 2);
    if matches!(
        route.workflow,
        WorkflowKind::CodeChange | WorkflowKind::BugFix
    ) || load >= 16
    {
        AgentTaskMode::Full
    } else {
        AgentTaskMode::Light
    }
}

fn route_confidence(route: &IntentRoute, mode: AgentTaskMode, uncertainty: u8) -> u8 {
    let base = (route.confidence.clamp(0.0, 1.0) * 100.0).round() as i16;
    let mode_penalty = match mode {
        AgentTaskMode::Direct => 0,
        AgentTaskMode::Light => 4,
        AgentTaskMode::Full => 6,
        AgentTaskMode::HighRisk => 8,
    };
    (base - mode_penalty - i16::from(uncertainty / 2)).clamp(1, 100) as u8
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn direct_prompt_scores_as_direct_without_tool_need() {
        let route = IntentRouter::new().route("简单回答：2+2 等于几？");
        let score = TaskModeScore::from_route("简单回答：2+2 等于几？", &route);

        assert_eq!(score.mode, AgentTaskMode::Direct);
        assert_eq!(score.tool_need, 0);
        assert!(score.confidence >= 50);
    }

    #[test]
    fn local_inspection_scores_as_light() {
        let prompt = "请帮我看看桌面有没有 gex 文件夹";
        let route = IntentRouter::new().route(prompt);
        let score = TaskModeScore::from_route(prompt, &route);

        assert_eq!(score.mode, AgentTaskMode::Light);
        assert!(score.tool_need > 0);
        assert!(score.complexity < 6);
    }

    #[test]
    fn code_change_scores_as_full() {
        let prompt = "帮我做一个贪吃蛇游戏吧，用 python 做吧";
        let route = IntentRouter::new().route(prompt);
        let score = TaskModeScore::from_route(prompt, &route);

        assert_eq!(score.mode, AgentTaskMode::Full);
        assert!(score.tool_need >= 7);
        assert!(score.user_impact >= 7);
    }
}

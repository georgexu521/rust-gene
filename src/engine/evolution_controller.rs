//! Evolution controller (MAINTENANCE-ONLY)
//!
//! Gates whether the agent can auto-evolve prompts, tools, workflows, or core code.
//!
//! 🟡 维护状态：gated 功能，cooldown 5 轮。当前不活跃，保留用于 future
//! auto-evolution 实验。不调整系数，不添加新 triger 类型。
//! 参见 docs/WEIGHTING_SYSTEM_AUDIT_2026-06-08.md 第 3 节。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionTarget {
    Memory,
    Skill,
    PromptSection,
    WorkflowPolicy,
    ToolDescription,
    CoreCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionAction {
    AutoAccept,
    Propose,
    Monitor,
    Reject,
    RequireHumanReview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EvolutionTriggerFactors {
    pub repeated_failure: f32,
    pub reuse_frequency: f32,
    pub user_correction_frequency: f32,
    pub task_impact: f32,
    pub optimization_potential: f32,
    pub evolution_cost: f32,
    pub risk: f32,
}

impl EvolutionTriggerFactors {
    pub fn clamped(self) -> Self {
        Self {
            repeated_failure: self.repeated_failure.clamp(0.0, 1.0),
            reuse_frequency: self.reuse_frequency.clamp(0.0, 1.0),
            user_correction_frequency: self.user_correction_frequency.clamp(0.0, 1.0),
            task_impact: self.task_impact.clamp(0.0, 1.0),
            optimization_potential: self.optimization_potential.clamp(0.0, 1.0),
            evolution_cost: self.evolution_cost.clamp(0.0, 1.0),
            risk: self.risk.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionGateDecision {
    pub target: EvolutionTarget,
    pub score: f32,
    pub action: EvolutionAction,
    pub auto_apply_allowed: bool,
    pub cooldown_active: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionController {
    cooldown_turns: u64,
    last_update_turn: HashMap<EvolutionTarget, u64>,
}

impl Default for EvolutionController {
    fn default() -> Self {
        Self::new()
    }
}

impl EvolutionController {
    pub fn new() -> Self {
        Self {
            cooldown_turns: 5,
            last_update_turn: HashMap::new(),
        }
    }

    pub fn with_cooldown_turns(mut self, cooldown_turns: u64) -> Self {
        self.cooldown_turns = cooldown_turns;
        self
    }

    pub fn with_last_updates(mut self, last_update_turn: HashMap<EvolutionTarget, u64>) -> Self {
        self.last_update_turn = last_update_turn;
        self
    }

    pub fn last_update_turns(&self) -> &HashMap<EvolutionTarget, u64> {
        &self.last_update_turn
    }

    pub fn mark_updated(&mut self, target: EvolutionTarget, turn_index: u64) {
        self.last_update_turn.insert(target, turn_index);
    }

    pub fn gate(
        &self,
        target: EvolutionTarget,
        factors: EvolutionTriggerFactors,
        current_turn: u64,
    ) -> EvolutionGateDecision {
        let factors = factors.clamped();
        let score = score_evolution_trigger(factors);
        let cooldown_active = self
            .last_update_turn
            .get(&target)
            .map(|last| current_turn.saturating_sub(*last) < self.cooldown_turns)
            .unwrap_or(false);
        let mut reasons = Vec::new();
        if cooldown_active {
            reasons.push(format!(
                "cooldown active for {:?}; wait {} turn(s)",
                target, self.cooldown_turns
            ));
        }
        if factors.risk >= 0.70 {
            reasons.push(format!("risk {:.2} requires review", factors.risk));
        }
        if factors.evolution_cost >= 0.80 {
            reasons.push(format!(
                "evolution cost {:.2} is too high for automatic action",
                factors.evolution_cost
            ));
        }

        let high_risk_target = matches!(
            target,
            EvolutionTarget::PromptSection
                | EvolutionTarget::WorkflowPolicy
                | EvolutionTarget::ToolDescription
                | EvolutionTarget::CoreCode
        );
        let auto_apply_allowed = target == EvolutionTarget::Memory
            && score >= 0.70
            && factors.risk < 0.45
            && !cooldown_active;
        let action = if cooldown_active {
            EvolutionAction::Monitor
        } else if high_risk_target || factors.risk >= 0.70 {
            if score >= 0.50 {
                EvolutionAction::RequireHumanReview
            } else {
                EvolutionAction::Monitor
            }
        } else if auto_apply_allowed {
            EvolutionAction::AutoAccept
        } else if score >= 0.70 {
            EvolutionAction::Propose
        } else if score >= 0.50 {
            EvolutionAction::Monitor
        } else {
            EvolutionAction::Reject
        };

        if action == EvolutionAction::Reject {
            reasons.push(format!(
                "trigger score {:.2} below monitor threshold",
                score
            ));
        }

        EvolutionGateDecision {
            target,
            score,
            action,
            auto_apply_allowed,
            cooldown_active,
            reasons,
        }
    }
}

pub fn score_evolution_trigger(factors: EvolutionTriggerFactors) -> f32 {
    let factors = factors.clamped();
    (factors.repeated_failure * 0.30
        + factors.reuse_frequency * 0.25
        + factors.user_correction_frequency * 0.20
        + factors.task_impact * 0.15
        + factors.optimization_potential * 0.10
        - factors.evolution_cost * 0.20
        - factors.risk * 0.20)
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strong_low_risk_factors() -> EvolutionTriggerFactors {
        EvolutionTriggerFactors {
            repeated_failure: 0.9,
            reuse_frequency: 0.9,
            user_correction_frequency: 0.6,
            task_impact: 0.8,
            optimization_potential: 0.9,
            evolution_cost: 0.1,
            risk: 0.1,
        }
    }

    #[test]
    fn memory_can_auto_accept_low_risk_high_score() {
        let controller = EvolutionController::new();
        let decision = controller.gate(EvolutionTarget::Memory, strong_low_risk_factors(), 10);
        assert_eq!(decision.action, EvolutionAction::AutoAccept);
        assert!(decision.auto_apply_allowed);
    }

    #[test]
    fn prompt_changes_require_review_even_when_score_is_high() {
        let controller = EvolutionController::new();
        let decision = controller.gate(
            EvolutionTarget::PromptSection,
            strong_low_risk_factors(),
            10,
        );
        assert_eq!(decision.action, EvolutionAction::RequireHumanReview);
        assert!(!decision.auto_apply_allowed);
    }

    #[test]
    fn cooldown_prevents_repeated_updates() {
        let mut controller = EvolutionController::new().with_cooldown_turns(5);
        controller.mark_updated(EvolutionTarget::Skill, 10);
        let decision = controller.gate(EvolutionTarget::Skill, strong_low_risk_factors(), 12);
        assert_eq!(decision.action, EvolutionAction::Monitor);
        assert!(decision.cooldown_active);
    }
}

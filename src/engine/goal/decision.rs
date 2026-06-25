//! Deterministic goal decision engine.
//!
//! Takes turn-level closeout/verification/permission evidence and decides whether
//! the goal should continue, complete, pause, block, fail, or ask the user.
//! The engine owns no semantic judgment — the LLM decides approach and repair,
//! while the engine screens evidence against hard rules.

use super::model::{GoalBudget, GoalDecision, GoalRun, ScoredEvalConfig};
use crate::engine::trace::TurnTrace;

#[derive(Debug, Clone)]
pub struct GoalDecisionInput {
    pub closeout_status: Option<String>,
    pub terminal_status: Option<String>,
    pub verification_proof_status: Option<String>,
    pub changed_files: usize,
    pub validation_items: usize,
    pub tool_records: usize,
    pub acceptance_items: usize,
    pub residual_risks: usize,
    pub blocker_detected: bool,
    pub blocker_ask_user: bool,
    pub permission_denied: bool,
    pub requires_user_decision: bool,
    pub current_turn: u32,
    pub repeated_blocker_count: u32,
    pub budget: GoalBudget,
    pub require_verified_closeout: bool,
    pub current_score: Option<f64>,
    pub previous_score: Option<f64>,
    pub score_no_improvement_count: u32,
    pub scored_eval: Option<ScoredEvalConfig>,
    pub claim_gate_downgrade: bool,
}

pub struct GoalDecisionEngine;

impl GoalDecisionEngine {
    pub fn decide(input: &GoalDecisionInput) -> GoalDecision {
        if let Some(ref status) = input.terminal_status {
            if status == "stopped_by_user" {
                return GoalDecision::Pause;
            }
            if status == "failed" {
                return GoalDecision::Failed;
            }
        }

        if input.verification_proof_status.as_deref() == Some("failed") {
            return GoalDecision::Failed;
        }

        if input.repeated_blocker_count >= input.budget.max_repeated_blockers {
            return GoalDecision::Blocked;
        }

        if input.terminal_status.as_deref() == Some("blocked") {
            return GoalDecision::Blocked;
        }

        if input.turn_count_exhausted() {
            return GoalDecision::Blocked;
        }

        if input.score_exhausted() {
            return GoalDecision::Blocked;
        }

        if input.terminal_status.as_deref() == Some("needs_user")
            || input.permission_denied
            || input.blocker_ask_user
            || input.requires_user_decision
        {
            return GoalDecision::NeedsUser;
        }

        if input.claim_gate_downgrade {
            return GoalDecision::Continue;
        }

        if Self::stop_rules_satisfied(input) {
            return GoalDecision::Complete;
        }

        GoalDecision::Continue
    }

    fn stop_rules_satisfied(input: &GoalDecisionInput) -> bool {
        let closeout_ok = matches!(input.closeout_status.as_deref(), Some("passed"));

        let terminal_ok = matches!(input.terminal_status.as_deref(), Some("completed"));

        if !closeout_ok && !terminal_ok {
            return false;
        }

        if input.require_verified_closeout {
            let proof_ok = matches!(
                input.verification_proof_status.as_deref(),
                Some("verified") | Some("not_applicable")
            );
            if !proof_ok {
                return false;
            }
        }

        true
    }
}

impl GoalDecisionInput {
    pub fn from_trace_and_run(
        trace: &TurnTrace,
        run: &GoalRun,
        repeated_blocker_count: u32,
        previous_score: Option<f64>,
        score_no_improvement_count: u32,
    ) -> Self {
        let mut input = Self::default_from_run(run, repeated_blocker_count);
        input.previous_score = previous_score;
        input.score_no_improvement_count = score_no_improvement_count;
        input.scored_eval = run.stop_rules.scored_eval.clone();

        for event in trace.events.iter() {
            match event {
                crate::engine::trace::TraceEvent::FinalCloseoutPrepared {
                    status,
                    terminal_status,
                    verification_proof_status,
                    changed_files,
                    validation_items,
                    tool_records,
                    acceptance_items,
                    residual_risks,
                    ..
                } => {
                    input.closeout_status = Some(status.clone());
                    input.terminal_status = terminal_status.clone();
                    input.verification_proof_status = verification_proof_status.clone();
                    input.changed_files = *changed_files;
                    input.validation_items = *validation_items;
                    input.tool_records = *tool_records;
                    input.acceptance_items = *acceptance_items;
                    input.residual_risks = *residual_risks;
                }
                crate::engine::trace::TraceEvent::GuidedDebuggingCompleted {
                    blocker,
                    ask_user,
                    ..
                } => {
                    input.blocker_detected = *blocker;
                    input.blocker_ask_user = *ask_user;
                }
                crate::engine::trace::TraceEvent::PermissionResolved { approved, .. } => {
                    if !approved {
                        input.permission_denied = true;
                    }
                }
                crate::engine::trace::TraceEvent::RecoveryPlan {
                    requires_user_decision,
                    ..
                } => {
                    if *requires_user_decision {
                        input.requires_user_decision = true;
                    }
                }
                crate::engine::trace::TraceEvent::FinalAnswerClaimGate { decision, .. }
                    if decision == "downgrade" =>
                {
                    input.claim_gate_downgrade = true;
                }
                crate::engine::trace::TraceEvent::FinalAnswerClaimGate { .. } => {}
                _ => {}
            }
        }

        input
    }

    fn default_from_run(run: &GoalRun, repeated_blocker_count: u32) -> Self {
        Self {
            closeout_status: None,
            terminal_status: None,
            verification_proof_status: None,
            changed_files: 0,
            validation_items: 0,
            tool_records: 0,
            acceptance_items: 0,
            residual_risks: 0,
            blocker_detected: false,
            blocker_ask_user: false,
            permission_denied: false,
            requires_user_decision: false,
            current_turn: run.turn_count,
            repeated_blocker_count,
            budget: run.budget.clone(),
            require_verified_closeout: run.stop_rules.require_verified_closeout,
            current_score: None,
            previous_score: None,
            score_no_improvement_count: 0,
            scored_eval: None,
            claim_gate_downgrade: false,
        }
    }

    fn turn_count_exhausted(&self) -> bool {
        self.current_turn >= self.budget.max_turns
    }

    fn score_exhausted(&self) -> bool {
        let Some(ref eval) = self.scored_eval else {
            return false;
        };
        if eval.max_attempts == 0 {
            return false;
        }
        if self.score_no_improvement_count >= eval.max_attempts {
            return true;
        }
        if let (Some(current), Some(_previous)) = (self.current_score, self.previous_score) {
            if current >= eval.target_threshold {
                return false;
            }
        }
        self.current_turn >= eval.max_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::goal::model::{GoalBudget, ScoredEvalConfig};

    fn test_budget() -> GoalBudget {
        GoalBudget {
            max_turns: 10,
            max_minutes: 30,
            max_tokens: None,
            max_repeated_blockers: 3,
        }
    }

    fn input_with_closeout(status: &str, proof: &str) -> GoalDecisionInput {
        GoalDecisionInput {
            closeout_status: Some(status.to_string()),
            terminal_status: None,
            verification_proof_status: Some(proof.to_string()),
            changed_files: 3,
            validation_items: 2,
            tool_records: 5,
            acceptance_items: 2,
            residual_risks: 0,
            blocker_detected: false,
            blocker_ask_user: false,
            permission_denied: false,
            requires_user_decision: false,
            current_turn: 1,
            repeated_blocker_count: 0,
            budget: test_budget(),
            require_verified_closeout: true,
            current_score: None,
            previous_score: None,
            score_no_improvement_count: 0,
            scored_eval: None,
            claim_gate_downgrade: false,
        }
    }

    #[test]
    fn completes_on_passed_closeout_with_verified_proof() {
        let input = input_with_closeout("passed", "verified");
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Complete);
    }

    #[test]
    fn completes_on_passed_closeout_with_not_applicable_proof() {
        let input = input_with_closeout("passed", "not_applicable");
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Complete);
    }

    #[test]
    fn blocks_completion_when_partial_closeout() {
        let input = input_with_closeout("partial", "verified");
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Continue);
    }

    #[test]
    fn blocks_completion_when_not_verified_closeout() {
        let input = input_with_closeout("not_verified", "partial");
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Continue);
    }

    #[test]
    fn blocks_completion_when_proof_is_partial() {
        let mut input = input_with_closeout("passed", "partial");
        input.require_verified_closeout = true;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Continue);
    }

    #[test]
    fn blocks_completion_when_proof_is_failed() {
        let input = input_with_closeout("passed", "failed");
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Failed);
    }

    #[test]
    fn continues_when_no_closeout_event() {
        let input = GoalDecisionInput {
            closeout_status: None,
            terminal_status: None,
            verification_proof_status: None,
            changed_files: 2,
            validation_items: 1,
            tool_records: 3,
            acceptance_items: 0,
            residual_risks: 0,
            blocker_detected: false,
            blocker_ask_user: false,
            permission_denied: false,
            requires_user_decision: false,
            current_turn: 1,
            repeated_blocker_count: 0,
            budget: test_budget(),
            require_verified_closeout: true,
            current_score: None,
            previous_score: None,
            score_no_improvement_count: 0,
            scored_eval: None,
            claim_gate_downgrade: false,
        };
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Continue);
    }

    #[test]
    fn pauses_when_stopped_by_user() {
        let mut input = input_with_closeout("passed", "verified");
        input.terminal_status = Some("stopped_by_user".to_string());
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Pause);
    }

    #[test]
    fn fails_when_terminal_is_failed() {
        let mut input = input_with_closeout("failed", "failed");
        input.terminal_status = Some("failed".to_string());
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Failed);
    }

    #[test]
    fn blocks_on_repeated_blocker_threshold() {
        let input = GoalDecisionInput {
            closeout_status: Some("partial".to_string()),
            terminal_status: None,
            verification_proof_status: Some("partial".to_string()),
            changed_files: 0,
            validation_items: 0,
            tool_records: 2,
            acceptance_items: 0,
            residual_risks: 0,
            blocker_detected: true,
            blocker_ask_user: false,
            permission_denied: false,
            requires_user_decision: false,
            current_turn: 3,
            repeated_blocker_count: 3,
            budget: test_budget(),
            require_verified_closeout: true,
            current_score: None,
            previous_score: None,
            score_no_improvement_count: 0,
            scored_eval: None,
            claim_gate_downgrade: false,
        };
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Blocked);
    }

    #[test]
    fn blocks_when_turn_budget_exhausted() {
        let input = GoalDecisionInput {
            closeout_status: Some("partial".to_string()),
            terminal_status: None,
            verification_proof_status: Some("partial".to_string()),
            changed_files: 1,
            validation_items: 1,
            tool_records: 1,
            acceptance_items: 0,
            residual_risks: 0,
            blocker_detected: false,
            blocker_ask_user: false,
            permission_denied: false,
            requires_user_decision: false,
            current_turn: 10,
            repeated_blocker_count: 0,
            budget: test_budget(),
            require_verified_closeout: true,
            current_score: None,
            previous_score: None,
            score_no_improvement_count: 0,
            scored_eval: None,
            claim_gate_downgrade: false,
        };
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Blocked);
    }

    #[test]
    fn needs_user_when_permission_denied() {
        let mut input = input_with_closeout("partial", "partial");
        input.permission_denied = true;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::NeedsUser);
    }

    #[test]
    fn needs_user_when_terminal_is_needs_user() {
        let mut input = input_with_closeout("partial", "partial");
        input.terminal_status = Some("needs_user".to_string());
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::NeedsUser);
    }

    #[test]
    fn needs_user_when_blocker_asks_user() {
        let mut input = input_with_closeout("partial", "partial");
        input.blocker_ask_user = true;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::NeedsUser);
    }

    #[test]
    fn needs_user_when_recovery_requires_user_decision() {
        let mut input = input_with_closeout("partial", "partial");
        input.requires_user_decision = true;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::NeedsUser);
    }

    #[test]
    fn completes_on_terminal_completed_with_passed() {
        let input = GoalDecisionInput {
            closeout_status: None,
            terminal_status: Some("completed".to_string()),
            verification_proof_status: Some("verified".to_string()),
            changed_files: 5,
            validation_items: 3,
            tool_records: 10,
            acceptance_items: 3,
            residual_risks: 0,
            blocker_detected: false,
            blocker_ask_user: false,
            permission_denied: false,
            requires_user_decision: false,
            current_turn: 2,
            repeated_blocker_count: 0,
            budget: test_budget(),
            require_verified_closeout: true,
            current_score: None,
            previous_score: None,
            score_no_improvement_count: 0,
            scored_eval: None,
            claim_gate_downgrade: false,
        };
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Complete);
    }

    #[test]
    fn continues_under_repeated_blocker_threshold() {
        let input = GoalDecisionInput {
            closeout_status: Some("partial".to_string()),
            terminal_status: None,
            verification_proof_status: Some("partial".to_string()),
            changed_files: 0,
            validation_items: 0,
            tool_records: 2,
            acceptance_items: 0,
            residual_risks: 0,
            blocker_detected: true,
            blocker_ask_user: false,
            permission_denied: false,
            requires_user_decision: false,
            current_turn: 2,
            repeated_blocker_count: 2,
            budget: test_budget(),
            require_verified_closeout: true,
            current_score: None,
            previous_score: None,
            score_no_improvement_count: 0,
            scored_eval: None,
            claim_gate_downgrade: false,
        };
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Continue);
    }

    #[test]
    fn completes_when_verified_closeout_not_required_and_passed() {
        let mut input = input_with_closeout("passed", "not_run");
        input.require_verified_closeout = false;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Complete);
    }

    #[test]
    fn blocks_when_score_no_improvement_exhausted() {
        let mut input = input_with_closeout("passed", "verified");
        input.scored_eval = Some(ScoredEvalConfig {
            max_attempts: 3,
            ..Default::default()
        });
        input.score_no_improvement_count = 3;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Blocked);
    }

    #[test]
    fn continues_when_score_improves() {
        let mut input = input_with_closeout("partial", "partial");
        input.scored_eval = Some(ScoredEvalConfig {
            max_attempts: 5,
            target_threshold: 0.9,
            ..Default::default()
        });
        input.current_score = Some(0.7);
        input.previous_score = Some(0.5);
        input.score_no_improvement_count = 0;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Continue);
    }

    #[test]
    fn blocks_when_score_target_not_met_and_attempts_exhausted() {
        let mut input = input_with_closeout("partial", "partial");
        input.scored_eval = Some(ScoredEvalConfig {
            max_attempts: 3,
            target_threshold: 0.9,
            ..Default::default()
        });
        input.current_score = Some(0.3);
        input.previous_score = Some(0.3);
        input.current_turn = 3;
        input.score_no_improvement_count = 3;
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Blocked);
    }

    #[test]
    fn blocks_completion_when_claim_gate_downgraded() {
        let mut input = input_with_closeout("passed", "verified");
        input.claim_gate_downgrade = true;
        // Even with passed closeout and verified proof, claim gate downgrade
        // must prevent GoalDecision::Complete.
        assert_eq!(GoalDecisionEngine::decide(&input), GoalDecision::Continue);
    }
}

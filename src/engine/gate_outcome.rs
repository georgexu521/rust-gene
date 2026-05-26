//! Derived gate-outcome records for runtime-spine reports.
//!
//! This module intentionally derives from existing trace events first. It gives
//! eval/reporting code a stable shape without forcing a trace schema migration.

use crate::engine::scenario_matrix::{FailureOwner, RuntimeSpineGateOutcomeClass};
use crate::engine::trace::{TraceEvent, TurnTrace};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    ActionReview,
    Permission,
    Checkpoint,
    Closeout,
}

impl GateKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ActionReview => "action_review",
            Self::Permission => "permission",
            Self::Checkpoint => "checkpoint",
            Self::Closeout => "closeout",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateOutcomeRecord {
    pub gate: GateKind,
    pub decision: String,
    pub outcome: RuntimeSpineGateOutcomeClass,
    pub reason: String,
    pub tool: Option<String>,
    pub route: Option<String>,
    pub stage: Option<String>,
    pub risk: Option<String>,
    pub recovered_after_gate: Option<bool>,
    pub final_status: Option<String>,
    pub failure_owner: Option<FailureOwner>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateOutcomeSummary {
    pub total: usize,
    pub protective_blocks: usize,
    pub recoverable_friction: usize,
    pub unrecovered_blocks: usize,
    pub suspected_false_positives: usize,
    pub policy_correct_but_ux_costly: usize,
    pub harmless_passes: usize,
}

pub fn derive_gate_outcomes(trace: &TurnTrace) -> Vec<GateOutcomeRecord> {
    let final_status = latest_completion_status(trace);
    let route = latest_route_workflow(trace);
    let risk = latest_route_risk(trace);
    let recovered = final_status
        .as_deref()
        .map(|status| matches!(status, "completed" | "passed"));

    trace
        .events
        .iter()
        .filter_map(|event| match event {
            TraceEvent::ActionReviewed {
                tool,
                decision,
                reason,
                permission,
                scope_allowed,
                budget_allowed,
                checkpoint,
                recovery,
                ..
            } => {
                let outcome = classify_action_review(
                    decision,
                    reason,
                    permission.as_deref(),
                    *scope_allowed,
                    *budget_allowed,
                    checkpoint,
                    recovery,
                    final_status.as_deref(),
                );
                Some(GateOutcomeRecord {
                    gate: GateKind::ActionReview,
                    decision: decision.clone(),
                    outcome,
                    reason: reason.clone(),
                    tool: Some(tool.clone()),
                    route: route.clone(),
                    stage: None,
                    risk: risk.clone(),
                    recovered_after_gate: recovered,
                    final_status: final_status.clone(),
                    failure_owner: failure_owner_for_outcome(outcome),
                })
            }
            TraceEvent::PermissionResolved {
                tool,
                approved,
                decision,
                ..
            } => {
                let outcome = if *approved {
                    RuntimeSpineGateOutcomeClass::HarmlessPass
                } else if recovered.unwrap_or(false) {
                    RuntimeSpineGateOutcomeClass::RecoverableFriction
                } else {
                    RuntimeSpineGateOutcomeClass::UnrecoveredBlock
                };
                Some(GateOutcomeRecord {
                    gate: GateKind::Permission,
                    decision: decision
                        .clone()
                        .unwrap_or_else(|| if *approved { "approved" } else { "denied" }.into()),
                    outcome,
                    reason: if *approved {
                        "permission approved".to_string()
                    } else {
                        "permission denied".to_string()
                    },
                    tool: Some(tool.clone()),
                    route: route.clone(),
                    stage: None,
                    risk: risk.clone(),
                    recovered_after_gate: recovered,
                    final_status: final_status.clone(),
                    failure_owner: failure_owner_for_outcome(outcome),
                })
            }
            TraceEvent::FinalCloseoutPrepared {
                status,
                verification_proof_status,
                failure_type,
                ..
            } => {
                let outcome = if matches!(status.as_str(), "passed" | "completed")
                    && matches!(
                        verification_proof_status.as_deref(),
                        Some("verified" | "not_applicable") | None
                    ) {
                    RuntimeSpineGateOutcomeClass::HarmlessPass
                } else if failure_type.is_some() {
                    RuntimeSpineGateOutcomeClass::ProtectiveBlock
                } else {
                    RuntimeSpineGateOutcomeClass::UnrecoveredBlock
                };
                Some(GateOutcomeRecord {
                    gate: GateKind::Closeout,
                    decision: status.clone(),
                    outcome,
                    reason: verification_proof_status
                        .clone()
                        .unwrap_or_else(|| "no verification proof status".to_string()),
                    tool: None,
                    route: route.clone(),
                    stage: None,
                    risk: risk.clone(),
                    recovered_after_gate: recovered,
                    final_status: final_status.clone(),
                    failure_owner: failure_owner_for_outcome(outcome),
                })
            }
            _ => None,
        })
        .collect()
}

pub fn summarize_gate_outcomes(records: &[GateOutcomeRecord]) -> GateOutcomeSummary {
    GateOutcomeSummary {
        total: records.len(),
        protective_blocks: count(records, RuntimeSpineGateOutcomeClass::ProtectiveBlock),
        recoverable_friction: count(records, RuntimeSpineGateOutcomeClass::RecoverableFriction),
        unrecovered_blocks: count(records, RuntimeSpineGateOutcomeClass::UnrecoveredBlock),
        suspected_false_positives: count(
            records,
            RuntimeSpineGateOutcomeClass::SuspectedFalsePositive,
        ),
        policy_correct_but_ux_costly: count(
            records,
            RuntimeSpineGateOutcomeClass::PolicyCorrectButUxCostly,
        ),
        harmless_passes: count(records, RuntimeSpineGateOutcomeClass::HarmlessPass),
    }
}

fn count(records: &[GateOutcomeRecord], outcome: RuntimeSpineGateOutcomeClass) -> usize {
    records
        .iter()
        .filter(|record| record.outcome == outcome)
        .count()
}

fn classify_action_review(
    decision: &str,
    reason: &str,
    permission: Option<&str>,
    scope_allowed: bool,
    budget_allowed: bool,
    checkpoint: &str,
    recovery: &str,
    final_status: Option<&str>,
) -> RuntimeSpineGateOutcomeClass {
    let decision = decision.to_ascii_lowercase();
    let reason = reason.to_ascii_lowercase();
    let checkpoint = checkpoint.to_ascii_lowercase();
    let recovery = recovery.to_ascii_lowercase();

    if matches!(decision.as_str(), "allow" | "allowed") {
        return RuntimeSpineGateOutcomeClass::HarmlessPass;
    }

    if !scope_allowed
        || !budget_allowed
        || checkpoint.contains("required")
        || permission
            .map(|value| value.eq_ignore_ascii_case("ask"))
            .unwrap_or(false)
        || reason.contains("destructive")
        || reason.contains("scope")
        || reason.contains("checkpoint")
    {
        if matches!(final_status, Some("completed" | "passed")) {
            return RuntimeSpineGateOutcomeClass::RecoverableFriction;
        }
        return RuntimeSpineGateOutcomeClass::ProtectiveBlock;
    }

    if recovery.contains("alternative") || matches!(final_status, Some("completed" | "passed")) {
        RuntimeSpineGateOutcomeClass::RecoverableFriction
    } else {
        RuntimeSpineGateOutcomeClass::UnrecoveredBlock
    }
}

fn latest_completion_status(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::CompletionContractEvaluated { status, .. } => Some(status.clone()),
        _ => None,
    })
}

fn latest_route_workflow(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::IntentRouted { workflow, .. } => Some(workflow.clone()),
        TraceEvent::AgentLoopStepEvaluated { route_workflow, .. } => Some(route_workflow.clone()),
        _ => None,
    })
}

fn latest_route_risk(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::IntentRouted { risk, .. } => Some(risk.clone()),
        TraceEvent::AgentLoopStepEvaluated { route_risk, .. } => Some(route_risk.clone()),
        _ => None,
    })
}

fn failure_owner_for_outcome(outcome: RuntimeSpineGateOutcomeClass) -> Option<FailureOwner> {
    match outcome {
        RuntimeSpineGateOutcomeClass::ProtectiveBlock
        | RuntimeSpineGateOutcomeClass::RecoverableFriction
        | RuntimeSpineGateOutcomeClass::HarmlessPass => None,
        RuntimeSpineGateOutcomeClass::UnrecoveredBlock => Some(FailureOwner::ActionReview),
        RuntimeSpineGateOutcomeClass::SuspectedFalsePositive => Some(FailureOwner::ActionReview),
        RuntimeSpineGateOutcomeClass::PolicyCorrectButUxCostly => Some(FailureOwner::ActionReview),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TraceCollector, TraceEvent, TurnTrace};

    #[test]
    fn derives_harmless_pass_from_allowed_action_review() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "test"));
        trace.record(TraceEvent::IntentRouted {
            agent_mode: None,
            intent: "CodeChange".to_string(),
            workflow: "CodeChange".to_string(),
            retrieval: "Project".to_string(),
            confidence: 0.8,
            risk: "Medium".to_string(),
            reason: "test".to_string(),
        });
        trace.record(TraceEvent::ActionReviewed {
            tool: "file_read".to_string(),
            call_id: "call_read".to_string(),
            decision: "allow".to_string(),
            reason: "safe_to_execute".to_string(),
            permission: Some("allow".to_string()),
            scope_allowed: true,
            budget_allowed: true,
            checkpoint: "not_required".to_string(),
            network: "none".to_string(),
            external_effect: "none".to_string(),
            recovery: "use the observation after execution".to_string(),
        });
        trace.record(TraceEvent::CompletionContractEvaluated {
            mode: "light".to_string(),
            workflow: "direct".to_string(),
            status: "completed".to_string(),
            terminal_status: "completed".to_string(),
            requires_validation: false,
            verification_status: "not_required".to_string(),
            verification_proof_status: "not_applicable".to_string(),
            changed_files: 0,
            reason: "direct task completed".to_string(),
        });

        let records = derive_gate_outcomes(&trace.snapshot());
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].outcome,
            RuntimeSpineGateOutcomeClass::HarmlessPass
        );
        assert_eq!(records[0].route.as_deref(), Some("CodeChange"));
        assert_eq!(summarize_gate_outcomes(&records).harmless_passes, 1);
    }

    #[test]
    fn denied_action_that_later_completes_is_recoverable_friction() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "test"));
        trace.record(TraceEvent::ActionReviewed {
            tool: "bash".to_string(),
            call_id: "call_bash".to_string(),
            decision: "revise".to_string(),
            reason: "checkpoint_required".to_string(),
            permission: Some("ask".to_string()),
            scope_allowed: true,
            budget_allowed: true,
            checkpoint: "required_missing".to_string(),
            network: "none".to_string(),
            external_effect: "local_workspace_mutation".to_string(),
            recovery: "use file_patch alternative".to_string(),
        });
        trace.record(TraceEvent::CompletionContractEvaluated {
            mode: "full".to_string(),
            workflow: "code_change".to_string(),
            status: "completed".to_string(),
            terminal_status: "completed".to_string(),
            requires_validation: true,
            verification_status: "verified".to_string(),
            verification_proof_status: "verified".to_string(),
            changed_files: 1,
            reason: "required validation proof is verified".to_string(),
        });

        let records = derive_gate_outcomes(&trace.snapshot());
        assert_eq!(
            records[0].outcome,
            RuntimeSpineGateOutcomeClass::RecoverableFriction
        );
        assert_eq!(records[0].recovered_after_gate, Some(true));
    }
}

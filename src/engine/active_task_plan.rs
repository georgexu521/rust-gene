//! Unified active task/progress panel derived from goal and trace state.

use crate::engine::session_goal::SessionGoal;
use crate::engine::trace::{latest_memory_proposal_summary, TraceEvent, TurnTrace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveTaskPlan {
    pub objective: String,
    pub plan_progress: String,
    pub active_step: String,
    pub verification: String,
    pub closeout: String,
    pub memory_proposal: String,
    pub next_action: String,
}

impl ActiveTaskPlan {
    pub fn from_goal_and_trace(goal: Option<&SessionGoal>, trace: Option<&TurnTrace>) -> Self {
        let objective = goal
            .map(|goal| goal.title.clone())
            .or_else(|| latest_execution_objective(trace))
            .unwrap_or_else(|| "none".to_string());
        let (plan_progress, active_step) =
            latest_plan_progress(trace).unwrap_or_else(|| ("none".to_string(), "none".to_string()));
        let verification = latest_verification(trace).unwrap_or_else(|| "none".to_string());
        let closeout = latest_closeout(trace).unwrap_or_else(|| "none".to_string());
        let memory_proposal = trace
            .and_then(latest_memory_proposal_summary)
            .unwrap_or_else(|| "none".to_string());
        let next_action = if closeout == "none" {
            active_step.clone()
        } else if memory_proposal.contains("proposed") {
            "/memory-proposals list".to_string()
        } else {
            "inspect /quick or continue current goal".to_string()
        };

        Self {
            objective,
            plan_progress,
            active_step,
            verification,
            closeout,
            memory_proposal,
            next_action,
        }
    }

    pub fn format(&self) -> String {
        format!(
            "Active Task Plan\n- Objective: {}\n- Plan: {}\n- Active step: {}\n- Verification: {}\n- Closeout: {}\n- Memory proposal: {}\n- Next action: {}",
            self.objective,
            self.plan_progress,
            self.active_step,
            self.verification,
            self.closeout,
            self.memory_proposal,
            self.next_action
        )
    }
}

fn latest_plan_progress(trace: Option<&TurnTrace>) -> Option<(String, String)> {
    trace?.events.iter().rev().find_map(|event| {
        if let TraceEvent::WorkflowPlanProgress {
            total_steps,
            completed_steps,
            active_step,
            top_priority,
            ..
        } = event
        {
            Some((
                format!("{completed_steps}/{total_steps} steps"),
                active_step
                    .clone()
                    .or_else(|| top_priority.clone())
                    .unwrap_or_else(|| "none".to_string()),
            ))
        } else {
            None
        }
    })
}

fn latest_verification(trace: Option<&TurnTrace>) -> Option<String> {
    trace?.events.iter().rev().find_map(|event| {
        if let TraceEvent::FinalCloseoutPrepared {
            verification_proof_status,
            verification_proof_summary,
            validation_items,
            ..
        } = event
        {
            Some(format!(
                "{} validation_items={} {}",
                verification_proof_status.as_deref().unwrap_or("unknown"),
                validation_items,
                verification_proof_summary.as_deref().unwrap_or("")
            ))
        } else {
            None
        }
    })
}

fn latest_closeout(trace: Option<&TurnTrace>) -> Option<String> {
    trace?.events.iter().rev().find_map(|event| {
        if let TraceEvent::FinalCloseoutPrepared {
            status,
            changed_files,
            residual_risks,
            ..
        } = event
        {
            Some(format!(
                "status={} changed_files={} residual_risks={}",
                status, changed_files, residual_risks
            ))
        } else {
            None
        }
    })
}

fn latest_execution_objective(trace: Option<&TurnTrace>) -> Option<String> {
    trace?.events.iter().rev().find_map(|event| {
        if let TraceEvent::ExecutionReportPrepared {
            task_id, status, ..
        } = event
        {
            Some(format!("{task_id} ({status})"))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TraceCollector, TurnStatus, TurnTrace};

    #[test]
    fn active_task_plan_combines_trace_progress_closeout_and_memory() {
        let collector = TraceCollector::new(TurnTrace::new("s1".to_string(), 1, "test"));
        collector.record(TraceEvent::WorkflowPlanProgress {
            total_steps: 3,
            completed_steps: 1,
            active_step: Some("run tests".to_string()),
            top_priority: None,
            top_importance_score: None,
            top_weight_share: None,
            weight_source: None,
            reweighted: false,
        });
        collector.record(TraceEvent::FinalCloseoutPrepared {
            status: "partial".to_string(),
            terminal_status: None,
            stop_reason: None,
            stop_action: None,
            failure_type: None,
            recovery_plan_id: None,
            rollback_status: None,
            changed_files: 1,
            validation_items: 1,
            tool_records: 0,
            tool_evidence: None,
            verification_proof_status: Some("partial".to_string()),
            verification_proof_summary: Some("tests failed".to_string()),
            verification_proof_kind_summary: None,
            verification_proof_support_status: None,
            verification_proof_support_summary: None,
            verification_proof_supports_verified: None,
            verification_proof_residual_risk: None,
            acceptance_items: 0,
            residual_risks: 1,
        });
        let trace = collector.finish(TurnStatus::Completed);
        let plan = ActiveTaskPlan::from_goal_and_trace(None, Some(&trace));

        assert_eq!(plan.plan_progress, "1/3 steps");
        assert_eq!(plan.active_step, "run tests");
        assert!(plan.closeout.contains("partial"));
    }
}

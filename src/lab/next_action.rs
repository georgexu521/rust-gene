//! Deterministic LabRun next-action recommendations.
//!
//! This module keeps `/lab next` and runtime Lab context injection aligned so
//! users and models see the same safe next step without duplicating rules.

use std::path::Path;

use serde::Serialize;

use crate::lab::model::{
    ArtifactGate, GraduateTask, LabProposalStatus, LabRole, LabRun, LabRunStatus, LabTaskStatus,
};
use crate::lab::orchestrator::LabOrchestrator;
use crate::lab::store::LabStore;

/// Machine-readable recommendation for the next safe LabRun action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LabNextAction {
    pub state: String,
    pub lab_run_id: Option<String>,
    pub proposal_id: Option<String>,
    pub current_stage: Option<String>,
    pub owner: Option<LabRole>,
    pub run_status: Option<LabRunStatus>,
    pub blocker: Option<String>,
    pub current_gate_requirement: Option<String>,
    pub open_task_count: usize,
    pub blocked_task_count: usize,
    pub next_task_id: Option<String>,
    pub recommended_command: String,
    pub reason: String,
    pub alternatives: Vec<String>,
}

impl LabNextAction {
    fn no_labrun() -> Self {
        Self {
            state: "no_labrun".to_string(),
            lab_run_id: None,
            proposal_id: None,
            current_stage: None,
            owner: None,
            run_status: None,
            blocker: None,
            current_gate_requirement: None,
            open_task_count: 0,
            blocked_task_count: 0,
            next_task_id: None,
            recommended_command: "/lab propose <idea>".to_string(),
            reason: "No LabRun or proposal exists yet.".to_string(),
            alternatives: vec!["/lab propose llm <idea>".to_string()],
        }
    }

    /// Compact context lines suitable for `<lab-context>` injection.
    pub fn context_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("next_safe_action: {}", self.recommended_command),
            format!("next_safe_action_state: {}", self.state),
            format!("next_safe_action_reason: {}", self.reason),
            format!("open_task_count: {}", self.open_task_count),
            format!("blocked_task_count: {}", self.blocked_task_count),
        ];
        if let Some(blocker) = self.blocker.as_deref() {
            lines.push(format!("current_blocker: {blocker}"));
        }
        if let Some(gate) = self.current_gate_requirement.as_deref() {
            lines.push(format!("current_gate_requirement: {gate}"));
        }
        if !self.alternatives.is_empty() {
            lines.push(format!(
                "safe_alternatives: {}",
                self.alternatives.join(" | ")
            ));
        }
        lines
    }
}

/// Computes the latest LabRun recommendation for a project.
pub fn recommend_next_action(project_root: &Path) -> anyhow::Result<LabNextAction> {
    let store = LabStore::for_project(project_root);
    let orchestrator = LabOrchestrator::for_project(project_root);
    recommend_next_action_from_store(&store, &orchestrator)
}

/// Computes a recommendation from already-created LabRun boundaries.
pub fn recommend_next_action_from_store(
    store: &LabStore,
    orchestrator: &LabOrchestrator,
) -> anyhow::Result<LabNextAction> {
    let Some(run) = store.latest_run()? else {
        return Ok(match store.latest_proposal()? {
            Some(proposal) => match proposal.status {
                LabProposalStatus::Draft | LabProposalStatus::AwaitingApproval => LabNextAction {
                    state: "proposal_awaiting_approval".to_string(),
                    lab_run_id: None,
                    proposal_id: Some(proposal.proposal_id.clone()),
                    current_stage: None,
                    owner: Some(LabRole::Professor),
                    run_status: None,
                    blocker: None,
                    current_gate_requirement: None,
                    open_task_count: 0,
                    blocked_task_count: 0,
                    next_task_id: None,
                    recommended_command: format!("/lab approve {}", proposal.proposal_id),
                    reason: "Latest professor proposal is waiting for explicit approval."
                        .to_string(),
                    alternatives: vec![
                        "/lab status".to_string(),
                        "/lab propose <refined idea>".to_string(),
                    ],
                },
                LabProposalStatus::Approved => LabNextAction {
                    state: "proposal_approved_missing_run".to_string(),
                    lab_run_id: proposal.approval.created_lab_run_id.clone(),
                    proposal_id: Some(proposal.proposal_id.clone()),
                    current_stage: None,
                    owner: Some(LabRole::Runtime),
                    run_status: None,
                    blocker: Some(
                        "Approved proposal exists but no latest LabRun was loaded.".to_string(),
                    ),
                    current_gate_requirement: None,
                    open_task_count: 0,
                    blocked_task_count: 0,
                    next_task_id: None,
                    recommended_command: "/lab status".to_string(),
                    reason: "Inspect persisted LabRun state before creating new work.".to_string(),
                    alternatives: vec!["/lab runs".to_string()],
                },
                LabProposalStatus::Rejected | LabProposalStatus::Superseded => {
                    LabNextAction::no_labrun()
                }
            },
            None => LabNextAction::no_labrun(),
        });
    };

    let tasks = store
        .list_graduate_tasks(&run.lab_run_id)
        .unwrap_or_default();
    let gate = store
        .load_artifact_gate(&run.lab_run_id, &run.current_stage)
        .or_else(|_| orchestrator.required_gate_for_latest())
        .ok();
    Ok(recommend_for_run(&run, &tasks, gate.as_ref()))
}

fn recommend_for_run(
    run: &LabRun,
    tasks: &[GraduateTask],
    gate: Option<&ArtifactGate>,
) -> LabNextAction {
    let open_task_count = tasks.iter().filter(|task| task.status.is_open()).count();
    let blocked_task_count = tasks
        .iter()
        .filter(|task| matches!(task.status, LabTaskStatus::Blocked))
        .count();
    let gate_requirement = gate.map(format_gate_requirement);
    let base = |state: &str,
                command: String,
                reason: String,
                alternatives: Vec<String>,
                blocker: Option<String>,
                next_task_id: Option<String>| LabNextAction {
        state: state.to_string(),
        lab_run_id: Some(run.lab_run_id.clone()),
        proposal_id: run.proposal_id.clone(),
        current_stage: Some(run.current_stage.clone()),
        owner: Some(run.internal_owner),
        run_status: Some(run.status),
        blocker,
        current_gate_requirement: gate_requirement.clone(),
        open_task_count,
        blocked_task_count,
        next_task_id,
        recommended_command: command,
        reason,
        alternatives,
    };

    if matches!(
        run.status,
        LabRunStatus::Completed | LabRunStatus::Cancelled | LabRunStatus::Failed
    ) {
        return base(
            "terminal",
            "/lab report latest".to_string(),
            format!("Latest LabRun is terminal: {:?}.", run.status),
            vec!["/lab runs".to_string(), "/lab start <new goal>".to_string()],
            run.blocked_reason.clone(),
            None,
        );
    }

    if matches!(
        run.status,
        LabRunStatus::Paused | LabRunStatus::PausedShutdown
    ) {
        return base(
            "paused_recoverable",
            "/lab resume".to_string(),
            run.pause_reason
                .as_deref()
                .map(|reason| format!("LabRun is paused: {reason}."))
                .unwrap_or_else(|| "LabRun is paused and can be resumed explicitly.".to_string()),
            vec!["/lab recovery".to_string(), "/lab dashboard".to_string()],
            run.pause_reason
                .clone()
                .or_else(|| run.blocked_reason.clone()),
            None,
        );
    }

    if run.needs_user || matches!(run.status, LabRunStatus::NeedsUser) {
        if run.current_stage == "user_report" {
            return base(
                "needs_user_report",
                "/lab closeout auto".to_string(),
                "LabRun is waiting for user review at the user_report boundary.".to_string(),
                vec![
                    "/lab continue [note]".to_string(),
                    "/lab report latest".to_string(),
                ],
                run.blocked_reason.clone(),
                None,
            );
        }
        return base(
            "needs_user",
            "/lab recovery".to_string(),
            "LabRun requires user input before safe automation can continue.".to_string(),
            vec!["/lab resume".to_string(), "/lab dashboard".to_string()],
            run.blocked_reason.clone(),
            None,
        );
    }

    if matches!(run.status, LabRunStatus::Blocked) {
        return base(
            "blocked",
            "/lab blocker status".to_string(),
            "LabRun is blocked; inspect or escalate the persisted blocker first.".to_string(),
            vec![
                "/lab blocker escalate".to_string(),
                "/lab recovery".to_string(),
            ],
            run.blocked_reason.clone(),
            None,
        );
    }

    if run.current_stage == "graduate_work" {
        if let Some(task) = tasks
            .iter()
            .find(|task| matches!(task.status, LabTaskStatus::Queued))
        {
            return base(
                "queued_graduate_task",
                format!("/lab task run {}", task.task_id),
                "A queued GraduateTask has scope and validation requirements ready for execution."
                    .to_string(),
                vec![
                    format!("/lab task envelope {}", task.task_id),
                    "/lab task list".to_string(),
                    "/lab dashboard".to_string(),
                ],
                None,
                Some(task.task_id.clone()),
            );
        }
        if let Some(task) = tasks
            .iter()
            .find(|task| matches!(task.status, LabTaskStatus::InProgress))
        {
            return base(
                "graduate_task_in_progress",
                format!("/lab task sync {}", task.task_id),
                "A GraduateTask is already in progress; sync durable result before dispatching more work."
                    .to_string(),
                vec![
                    format!("/lab task worktree review {}", task.task_id),
                    "/lab dashboard".to_string(),
                ],
                None,
                Some(task.task_id.clone()),
            );
        }
        if let Some(task) = tasks
            .iter()
            .find(|task| matches!(task.status, LabTaskStatus::Blocked))
        {
            return base(
                "blocked_graduate_task",
                format!("/lab task revise {} | <scope_csv> | <validation_csv> | <instructions>", task.task_id),
                "A GraduateTask is blocked; revise its scope/validation or escalate before continuing."
                    .to_string(),
                vec![
                    format!("/lab task envelope {}", task.task_id),
                    "/lab blocker status".to_string(),
                    "/lab blocker escalate".to_string(),
                ],
                task.blocker.clone(),
                Some(task.task_id.clone()),
            );
        }
        if gate.is_some_and(ArtifactGate::is_satisfied) {
            return base(
                "graduate_result_ready",
                "/lab advance".to_string(),
                "Graduate work gate is satisfied and can advance to postdoc review.".to_string(),
                vec!["/lab review".to_string(), "/lab dashboard".to_string()],
                None,
                None,
            );
        }
        return base(
            "graduate_work_missing_task_or_result",
            "/lab task list".to_string(),
            "graduate_work needs either an open GraduateTask or a verified GraduateResult."
                .to_string(),
            vec![
                "/lab task create <title> | <scope_csv> | <validation_csv> | <instructions>"
                    .to_string(),
                "/lab gate".to_string(),
            ],
            run.blocked_reason.clone(),
            None,
        );
    }

    if let Some(gate) = gate {
        if gate.is_satisfied() {
            return base(
                "gate_satisfied",
                "/lab advance".to_string(),
                format!("Current {} gate is satisfied.", gate.stage),
                stage_alternatives(&run.current_stage, true),
                None,
                None,
            );
        }
        let blocker = if gate.blockers.is_empty() {
            run.blocked_reason.clone()
        } else {
            Some(gate.blockers.join("; "))
        };
        return base(
            "gate_required",
            stage_creation_command(&run.current_stage),
            format!(
                "Current {} gate still needs {}.",
                gate.stage,
                gate.missing_fields().join(", ")
            ),
            stage_alternatives(&run.current_stage, false),
            blocker,
            None,
        );
    }

    if run.current_stage == "user_report" {
        return base(
            "user_report",
            "/lab closeout auto".to_string(),
            "LabRun reached the user_report boundary.".to_string(),
            vec![
                "/lab continue [note]".to_string(),
                "/lab report latest".to_string(),
            ],
            run.blocked_reason.clone(),
            None,
        );
    }

    base(
        "inspect",
        "/lab dashboard".to_string(),
        "No configured gate or graduate task recommendation matched the current state.".to_string(),
        vec!["/lab review".to_string(), "/lab recovery".to_string()],
        run.blocked_reason.clone(),
        None,
    )
}

fn format_gate_requirement(gate: &ArtifactGate) -> String {
    format!(
        "stage={} artifact_type={} owner={:?} satisfied={} artifact={} validation={} next_action={}",
        gate.stage,
        gate.required_artifact_type,
        gate.owner,
        gate.is_satisfied(),
        gate.artifact_id.as_deref().unwrap_or("none"),
        gate.validation_status.as_deref().unwrap_or("none"),
        gate.next_action.as_deref().unwrap_or("none")
    )
}

fn stage_creation_command(stage: &str) -> String {
    match stage {
        "postdoc_review" => "/lab integrate [note]".to_string(),
        "professor_review" => "/lab professor-review [note]".to_string(),
        _ => "/lab plan <note>".to_string(),
    }
}

fn stage_alternatives(stage: &str, gate_satisfied: bool) -> Vec<String> {
    let mut alternatives = vec!["/lab gate".to_string(), "/lab dashboard".to_string()];
    if !gate_satisfied {
        alternatives.push("/lab step llm [instructions]".to_string());
    }
    match stage {
        "postdoc_review" if !gate_satisfied => {
            alternatives.insert(0, "/lab review".to_string());
        }
        "professor_review" if !gate_satisfied => {
            alternatives.insert(0, "/lab review".to_string());
        }
        _ => {}
    }
    alternatives
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommend_no_labrun_starts_with_proposal() {
        let temp = tempfile::tempdir().unwrap();
        let action = recommend_next_action(temp.path()).unwrap();

        assert_eq!(action.state, "no_labrun");
        assert_eq!(action.recommended_command, "/lab propose <idea>");
        assert_eq!(action.open_task_count, 0);
    }
}

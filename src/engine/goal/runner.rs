//! Goal runner — deterministic outer scheduler for Codex-style goal mode.
//!
//! The runner bridges `/goal <objective>`, turn execution, and the decision
//! engine. It persists `GoalRun`/`GoalStep` through `SessionStore`, reads
//! evidence from completed `TurnTrace`s, calls `GoalDecisionEngine`, and
//! produces continuation prompts or terminal decisions.

use std::sync::Arc;

use super::decision::{GoalDecisionEngine, GoalDecisionInput};
use super::model::{
    GoalBudget, GoalDecision, GoalRun, GoalRunStatus, GoalStep, GoalStopRules, ScoredEvalConfig,
};
use crate::engine::session_goal::SessionGoalManager;
use crate::engine::trace::TurnTrace;
use crate::session_store::{GoalRunRecord, GoalStepInsert, SessionStore};

#[derive(Debug, Clone)]
pub struct GoalStartResult {
    pub goal_id: String,
    pub first_prompt: String,
}

#[derive(Debug, Clone)]
pub enum GoalAfterTurnResult {
    Continue {
        prompt: String,
        step: GoalStep,
    },
    Terminal {
        decision: GoalDecision,
        status: GoalRunStatus,
        step: GoalStep,
    },
}

#[derive(Debug, Clone)]
pub struct GoalStatusInfo {
    pub goal: Option<GoalRun>,
    pub steps: Vec<GoalStep>,
}

#[derive(Clone)]
pub struct GoalRunner {
    store: SessionStore,
    goal_manager: Arc<SessionGoalManager>,
}

impl GoalRunner {
    pub fn new(store: SessionStore, goal_manager: Arc<SessionGoalManager>) -> Self {
        Self {
            store,
            goal_manager,
        }
    }

    pub fn start(&self, session_id: &str, objective: &str) -> anyhow::Result<GoalStartResult> {
        let objective = objective.trim();
        if objective.is_empty() {
            return Err(anyhow::anyhow!("goal objective must be non-empty"));
        }
        if objective.chars().count() > 4000 {
            return Err(anyhow::anyhow!(
                "goal objective is {} characters, maximum is 4000",
                objective.chars().count()
            ));
        }

        let goal_id = new_goal_id();

        self.store.create_goal_run(
            &goal_id,
            session_id,
            objective,
            GoalRunStatus::Active.as_str(),
            &serde_json::to_string(&GoalStopRules::default())?,
            &serde_json::to_string(&GoalBudget::default())?,
        )?;

        self.goal_manager.hydrate_from_objective(objective);

        let first_prompt = format!(
            "Goal: {}\n\nWork toward this objective. Take the smallest useful step first. \
             Stop if you need user input, credentials, network access, or encounter a risk boundary.",
            objective
        );

        Ok(GoalStartResult {
            goal_id,
            first_prompt,
        })
    }

    pub fn after_turn(
        &self,
        session_id: &str,
        trace: &TurnTrace,
    ) -> anyhow::Result<GoalAfterTurnResult> {
        let Some(run_record) = self.store.get_active_goal_run(session_id)? else {
            return Err(anyhow::anyhow!("no active goal run for session"));
        };

        let run = goal_run_from_record(&run_record)?;

        let repeated_blocker_count =
            compute_repeated_blocker_count(&run, trace, &run_record.last_blocker);

        let (previous_score, score_no_improvement_count) = compute_score_state(&self.store, &run);

        let current_score = extract_score_from_trace(trace, &run.stop_rules.scored_eval);

        let mut input = GoalDecisionInput::from_trace_and_run(
            trace,
            &run,
            repeated_blocker_count,
            previous_score,
            score_no_improvement_count,
        );
        input.current_score = current_score;

        let decision = GoalDecisionEngine::decide(&input);

        let (new_status, terminal) = goal_status_from_decision(&decision);

        let blocker = extract_blocker(trace);
        let summary = format_decision_summary(&decision, &input);

        let closeout_status = input.closeout_status.clone();
        let verification_status = input.verification_proof_status.clone();

        let step_id = new_step_id();
        let step = GoalStep {
            id: step_id.clone(),
            goal_id: run.id.clone(),
            session_id: session_id.to_string(),
            turn_index: run.turn_count + 1,
            prompt: run.objective.clone(),
            closeout_status: closeout_status.clone(),
            verification_status: verification_status.clone(),
            changed_files: input.changed_files,
            validation_items: input.validation_items,
            decision: decision.clone(),
            summary: summary.clone(),
            score: current_score,
            created_at: chrono_utc_now(),
        };

        self.store.record_goal_step(&GoalStepInsert {
            id: step_id,
            goal_id: run.id.clone(),
            session_id: session_id.to_string(),
            turn_index: (run.turn_count + 1) as i64,
            prompt: run.objective.clone(),
            closeout_status: closeout_status.clone(),
            verification_status: verification_status.clone(),
            changed_files: input.changed_files as i64,
            validation_items: input.validation_items as i64,
            decision: serde_json::to_string(&decision)?,
            summary: summary.clone(),
            score: current_score,
        })?;

        self.store.update_goal_run_after_turn(
            &run.id,
            new_status.as_str(),
            closeout_status.as_deref(),
            blocker.as_deref(),
        )?;

        if terminal {
            Ok(GoalAfterTurnResult::Terminal {
                decision,
                status: new_status,
                step,
            })
        } else {
            let prompt = continuation_prompt(&run.objective, &input);
            Ok(GoalAfterTurnResult::Continue { prompt, step })
        }
    }

    pub fn pause(&self, session_id: &str) -> anyhow::Result<bool> {
        let Some(run_record) = self.store.get_active_goal_run(session_id)? else {
            return Ok(false);
        };
        self.store.update_goal_run_status(
            &run_record.id,
            GoalRunStatus::Paused.as_str(),
            None,
            None,
        )?;
        Ok(true)
    }

    pub fn resume(&self, session_id: &str) -> anyhow::Result<bool> {
        let Some(run_record) = self
            .store
            .get_latest_goal_run_by_status(session_id, GoalRunStatus::Paused)?
        else {
            return Ok(false);
        };
        let run = goal_run_from_record(&run_record)?;
        if run.status != GoalRunStatus::Paused {
            return Err(anyhow::anyhow!(
                "goal is not paused, current status is {:?}",
                run.status
            ));
        }
        self.store.update_goal_run_status(
            &run_record.id,
            GoalRunStatus::Active.as_str(),
            None,
            None,
        )?;

        Ok(true)
    }

    pub fn clear(&self, session_id: &str) -> anyhow::Result<bool> {
        let Some(run_record) = self.store.get_current_goal_run(session_id)? else {
            return Ok(false);
        };
        self.store.update_goal_run_status(
            &run_record.id,
            GoalRunStatus::Cancelled.as_str(),
            None,
            None,
        )?;
        self.goal_manager.clear();
        Ok(true)
    }

    pub fn edit_objective(
        &self,
        session_id: &str,
        new_objective: &str,
    ) -> anyhow::Result<Option<GoalRun>> {
        let objective = new_objective.trim();
        if objective.is_empty() {
            return Err(anyhow::anyhow!("goal objective must be non-empty"));
        }
        if objective.chars().count() > 4000 {
            return Err(anyhow::anyhow!(
                "goal objective is {} characters, maximum is 4000",
                objective.chars().count()
            ));
        }
        let conn = self.store.shared_conn();
        let conn = conn.lock().unwrap_or_else(|e| e.into_inner());
        let updated = conn.execute(
            "UPDATE goal_runs SET objective = ?1, updated_at = datetime('now')
             WHERE session_id = ?2 AND status = ?3",
            rusqlite::params![objective, session_id, GoalRunStatus::Active.as_str()],
        )?;
        if updated > 0 {
            self.goal_manager.hydrate_from_objective(objective);
            self.store
                .get_active_goal_run(session_id)?
                .and_then(|record| goal_run_from_record(&record).ok())
                .map_or(Ok(None), |run| Ok(Some(run)))
        } else {
            Ok(None)
        }
    }

    pub fn status(&self, session_id: &str) -> anyhow::Result<GoalStatusInfo> {
        let run = self
            .store
            .get_current_goal_run(session_id)?
            .and_then(|record| goal_run_from_record(&record).ok());

        let steps = if let Some(ref run) = run {
            self.store
                .list_goal_steps(&run.id, 20)?
                .into_iter()
                .map(|record| goal_step_from_record(&record))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        Ok(GoalStatusInfo { goal: run, steps })
    }

    pub fn has_active_goal(&self, session_id: &str) -> anyhow::Result<bool> {
        Ok(self.store.get_active_goal_run(session_id)?.is_some())
    }
}

pub fn pause_all_active_goals(store: &SessionStore) -> anyhow::Result<usize> {
    let conn = store.shared_conn();
    let conn = conn.lock().unwrap_or_else(|e| e.into_inner());
    let count = conn.execute(
        "UPDATE goal_runs SET status = ?1, updated_at = datetime('now')
         WHERE status IN (?2, ?3)",
        rusqlite::params![
            GoalRunStatus::Paused.as_str(),
            GoalRunStatus::Active.as_str(),
            GoalRunStatus::Active.legacy_json_str(),
        ],
    )?;
    Ok(count)
}

fn goal_run_from_record(record: &GoalRunRecord) -> anyhow::Result<GoalRun> {
    Ok(GoalRun {
        id: record.id.clone(),
        session_id: record.session_id.clone(),
        objective: record.objective.clone(),
        status: GoalRunStatus::from_storage(&record.status)
            .ok_or_else(|| anyhow::anyhow!("unknown goal run status: {}", record.status))?,
        stop_rules: record
            .stop_rules_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or_default(),
        budget: record
            .budget_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or_default(),
        turn_count: record.turn_count as u32,
        created_at: record.created_at.clone(),
        updated_at: record.updated_at.clone(),
        last_closeout_status: record.last_closeout_status.clone(),
        last_blocker: record.last_blocker.clone(),
    })
}

fn goal_step_from_record(
    record: &crate::session_store::GoalStepRecord,
) -> anyhow::Result<GoalStep> {
    Ok(GoalStep {
        id: record.id.clone(),
        goal_id: record.goal_id.clone(),
        session_id: record.session_id.clone(),
        turn_index: record.turn_index as u32,
        prompt: record.prompt.clone(),
        closeout_status: record.closeout_status.clone(),
        verification_status: record.verification_status.clone(),
        changed_files: record.changed_files as usize,
        validation_items: record.validation_items as usize,
        decision: serde_json::from_str(&record.decision)?,
        summary: record.summary.clone().unwrap_or_default(),
        score: record.score,
        created_at: record.created_at.clone(),
    })
}

fn goal_status_from_decision(decision: &GoalDecision) -> (GoalRunStatus, bool) {
    match decision {
        GoalDecision::Complete => (GoalRunStatus::Completed, true),
        GoalDecision::Failed => (GoalRunStatus::Failed, true),
        GoalDecision::Blocked => (GoalRunStatus::Blocked, true),
        GoalDecision::NeedsUser => (GoalRunStatus::NeedsUser, true),
        GoalDecision::Pause => (GoalRunStatus::Paused, true),
        GoalDecision::Continue => (GoalRunStatus::Active, false),
    }
}

fn compute_repeated_blocker_count(
    run: &GoalRun,
    trace: &TurnTrace,
    last_blocker: &Option<String>,
) -> u32 {
    let current = extract_blocker(trace);
    match (&current, last_blocker) {
        (Some(curr), Some(prev)) if curr == prev => run.turn_count.min(999) + 1,
        (Some(_), _) => 1,
        _ => 0,
    }
}

fn extract_blocker(trace: &TurnTrace) -> Option<String> {
    for event in trace.events.iter().rev() {
        match event {
            crate::engine::trace::TraceEvent::GuidedDebuggingCompleted { blocker, .. }
                if *blocker =>
            {
                return Some("guided_debug_blocker".to_string());
            }
            crate::engine::trace::TraceEvent::PermissionResolved { approved, tool, .. }
                if !approved =>
            {
                return Some(format!("permission_denied:{}", tool));
            }
            crate::engine::trace::TraceEvent::FinalCloseoutPrepared {
                terminal_status: Some(status),
                ..
            } if status == "blocked" => {
                return Some("terminal_blocked".to_string());
            }
            _ => {}
        }
    }
    None
}

fn continuation_prompt(objective: &str, input: &GoalDecisionInput) -> String {
    format!(
        "Goal: {}\nStop criteria: verified closeout\nPrevious step:\n- closeout={}\n- verification={}\n- changed_files={}\n- blocker={}\n\nContinue the goal by taking the smallest useful next step.\nDo not repeat completed work. Stop with a clear blocker if the next step\nrequires user input, approval, credentials, network access, or a risk boundary.",
        objective,
        input.closeout_status.as_deref().unwrap_or("none"),
        input.verification_proof_status.as_deref().unwrap_or("none"),
        input.changed_files,
        extract_blocker_from_input(input).as_deref().unwrap_or("none"),
    )
}

fn extract_blocker_from_input(input: &GoalDecisionInput) -> Option<String> {
    if input.blocker_detected {
        Some("guided_debug_blocker".to_string())
    } else if input.permission_denied {
        Some("permission_denied".to_string())
    } else {
        None
    }
}

fn format_decision_summary(decision: &GoalDecision, input: &GoalDecisionInput) -> String {
    format!(
        "{:?} closeout={} proof={} files={} validation={}",
        decision,
        input.closeout_status.as_deref().unwrap_or("none"),
        input.verification_proof_status.as_deref().unwrap_or("none"),
        input.changed_files,
        input.validation_items
    )
}

fn new_goal_id() -> String {
    format!("goalrun_{}", uuid::Uuid::new_v4())
}

fn new_step_id() -> String {
    format!("step_{}", uuid::Uuid::new_v4())
}

fn chrono_utc_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn compute_score_state(store: &SessionStore, run: &GoalRun) -> (Option<f64>, u32) {
    if run.stop_rules.scored_eval.is_none() {
        return (None, 0);
    }
    let steps = match store.list_goal_steps(&run.id, 5) {
        Ok(s) => s,
        Err(_) => return (None, 0),
    };
    let previous_score = steps.last().and_then(|r| r.score);
    let no_improvement = steps
        .iter()
        .rev()
        .take_while(|s| s.score == previous_score)
        .count() as u32;
    (previous_score, no_improvement.saturating_sub(1))
}

fn extract_score_from_trace(
    trace: &TurnTrace,
    scored_eval: &Option<ScoredEvalConfig>,
) -> Option<f64> {
    let _eval = scored_eval.as_ref()?;
    for event in trace.events.iter().rev() {
        if let crate::engine::trace::TraceEvent::VerificationCompleted { passed, .. } = event {
            return Some(if *passed { 1.0 } else { 0.0 });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::session_goal::SessionGoalManager;
    use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};

    fn test_runner() -> (SessionStore, GoalRunner) {
        let store = SessionStore::in_memory().unwrap();
        store.create_session("s1", "Test", "model").unwrap();
        let runner = GoalRunner::new(store.clone(), Arc::new(SessionGoalManager::new()));
        (store, runner)
    }

    #[test]
    fn start_pause_resume_uses_queryable_status_labels_without_turn_increment() {
        let (store, runner) = test_runner();
        let started = runner.start("s1", "ship goal mode").unwrap();
        let active = store.get_active_goal_run("s1").unwrap().unwrap();
        assert_eq!(active.id, started.goal_id);
        assert_eq!(active.status, GoalRunStatus::Active.as_str());
        assert_eq!(active.turn_count, 0);

        assert!(runner.pause("s1").unwrap());
        assert!(store.get_active_goal_run("s1").unwrap().is_none());
        let paused = store
            .get_latest_goal_run_by_status("s1", GoalRunStatus::Paused)
            .unwrap()
            .unwrap();
        assert_eq!(paused.id, started.goal_id);
        assert_eq!(paused.turn_count, 0);

        let visible = runner.status("s1").unwrap().goal.unwrap();
        assert_eq!(visible.status, GoalRunStatus::Paused);

        assert!(runner.resume("s1").unwrap());
        let active = store.get_active_goal_run("s1").unwrap().unwrap();
        assert_eq!(active.id, started.goal_id);
        assert_eq!(active.turn_count, 0);
    }

    #[test]
    fn legacy_json_status_rows_are_still_queryable() {
        let (store, _runner) = test_runner();
        store
            .create_goal_run(
                "legacy",
                "s1",
                "old goal",
                GoalRunStatus::Active.legacy_json_str(),
                &serde_json::to_string(&GoalStopRules::default()).unwrap(),
                &serde_json::to_string(&GoalBudget::default()).unwrap(),
            )
            .unwrap();

        let active = store.get_active_goal_run("s1").unwrap().unwrap();
        assert_eq!(active.id, "legacy");
        let parsed = goal_run_from_record(&active).unwrap();
        assert_eq!(parsed.status, GoalRunStatus::Active);
    }

    #[test]
    fn after_turn_records_current_score_and_increments_turn_once() {
        let (store, runner) = test_runner();
        let started = runner.start("s1", "improve score").unwrap();
        let stop_rules = GoalStopRules {
            scored_eval: Some(ScoredEvalConfig {
                max_attempts: 5,
                target_threshold: 0.9,
                ..Default::default()
            }),
            ..Default::default()
        };
        {
            let conn = store.shared_conn();
            let conn = conn.lock().unwrap();
            conn.execute(
                "UPDATE goal_runs SET stop_rules_json = ?1 WHERE id = ?2",
                rusqlite::params![serde_json::to_string(&stop_rules).unwrap(), started.goal_id],
            )
            .unwrap();
        }

        let mut trace = TurnTrace::new("s1", 1, "improve score");
        trace.events.push(TraceEvent::VerificationCompleted {
            changed_files: 1,
            passed: true,
            check_passed: true,
            tests_passed: true,
            review_passed: true,
            failed_commands: Vec::new(),
        });
        trace.events.push(TraceEvent::FinalCloseoutPrepared {
            status: "partial".to_string(),
            terminal_status: None,
            stop_reason: None,
            stop_action: None,
            failure_type: None,
            recovery_plan_id: None,
            rollback_status: None,
            changed_files: 1,
            validation_items: 1,
            tool_records: 1,
            tool_evidence: None,
            verification_proof_status: Some("partial".to_string()),
            verification_proof_summary: None,
            verification_proof_kind_summary: None,
            verification_proof_support_status: None,
            verification_proof_support_summary: None,
            verification_proof_supports_verified: None,
            verification_proof_residual_risk: None,
            acceptance_items: 0,
            residual_risks: 0,
        });
        trace.finish(TurnStatus::Completed);

        let result = runner.after_turn("s1", &trace).unwrap();
        let step = match result {
            GoalAfterTurnResult::Continue { step, .. } => step,
            other => panic!("expected continuation, got {other:?}"),
        };
        assert_eq!(step.score, Some(1.0));

        let active = store.get_active_goal_run("s1").unwrap().unwrap();
        assert_eq!(active.turn_count, 1);
        let steps = store.list_goal_steps(&active.id, 10).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].score, Some(1.0));
    }
}

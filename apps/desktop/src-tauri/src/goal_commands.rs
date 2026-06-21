use super::*;
use priority_agent::engine::goal::runner::GoalRunner;
use serde::Serialize;

async fn lazy_goal_runner(state: &State<'_, DesktopAppState>) -> Option<GoalRunner> {
    {
        let runner = state.goal_runner.lock().await;
        if let Some(ref runner) = *runner {
            return Some(runner.clone());
        }
    }
    let runtime = state.runtime.lock().await;
    if let Some(ref rt) = *runtime {
        if let Some((store, _session_id)) = rt.streaming_engine().session_binding() {
            let goal_manager = rt.streaming_engine().goal_manager();
            let runner = GoalRunner::new((*store).clone(), goal_manager);
            let mut guard = state.goal_runner.lock().await;
            *guard = Some(runner.clone());
            return Some(runner);
        }
    }
    None
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopGoalStatus {
    goal_id: Option<String>,
    objective: Option<String>,
    status: Option<String>,
    turn_count: Option<u32>,
    max_turns: Option<u32>,
    last_decision: Option<String>,
    last_closeout: Option<String>,
    last_proof: Option<String>,
    last_blocker: Option<String>,
    step_count: usize,
    steps: Vec<DesktopGoalStep>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopGoalStep {
    turn_index: u32,
    decision: String,
    closeout_status: Option<String>,
    verification_status: Option<String>,
    changed_files: usize,
    validation_items: usize,
    summary: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopGoalCommandResult {
    status: DesktopGoalStatus,
    next_prompt: Option<String>,
}

#[tauri::command]
pub(crate) async fn goal_status(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopGoalStatus, String> {
    let Some(runner) = lazy_goal_runner(&state).await else {
        return Ok(DesktopGoalStatus {
            goal_id: None,
            objective: None,
            status: None,
            turn_count: None,
            max_turns: None,
            last_decision: None,
            last_closeout: None,
            last_proof: None,
            last_blocker: None,
            step_count: 0,
            steps: Vec::new(),
        });
    };
    let session_id = state
        .active_session_id
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no active session".to_string())?;

    let info = runner.status(&session_id).map_err(|e| e.to_string())?;
    let steps: Vec<DesktopGoalStep> = info
        .steps
        .iter()
        .map(|s| DesktopGoalStep {
            turn_index: s.turn_index,
            decision: format!("{:?}", s.decision),
            closeout_status: s.closeout_status.clone(),
            verification_status: s.verification_status.clone(),
            changed_files: s.changed_files,
            validation_items: s.validation_items,
            summary: s.summary.clone(),
        })
        .collect();

    let last_step = steps.last();
    Ok(DesktopGoalStatus {
        goal_id: info.goal.as_ref().map(|g| g.id.clone()),
        objective: info.goal.as_ref().map(|g| g.objective.clone()),
        status: info.goal.as_ref().map(|g| format!("{:?}", g.status)),
        turn_count: info.goal.as_ref().map(|g| g.turn_count),
        max_turns: info.goal.as_ref().map(|g| g.budget.max_turns),
        last_decision: last_step.map(|s| s.decision.clone()),
        last_closeout: last_step.and_then(|s| s.closeout_status.clone()),
        last_proof: last_step.and_then(|s| s.verification_status.clone()),
        last_blocker: info.goal.and_then(|g| g.last_blocker.clone()),
        step_count: steps.len(),
        steps,
    })
}

#[tauri::command]
pub(crate) async fn goal_start(
    objective: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopGoalCommandResult, String> {
    let Some(runner) = lazy_goal_runner(&state).await else {
        return Err("no engine available".to_string());
    };
    let session_id = state
        .active_session_id
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no active session".to_string())?;

    let result = runner
        .start(&session_id, &objective)
        .map_err(|e| e.to_string())?;

    Ok(DesktopGoalCommandResult {
        status: goal_status(state).await?,
        next_prompt: Some(result.first_prompt),
    })
}

#[tauri::command]
pub(crate) async fn goal_pause(state: State<'_, DesktopAppState>) -> Result<bool, String> {
    let Some(runner) = lazy_goal_runner(&state).await else {
        return Ok(false);
    };
    let session_id = state
        .active_session_id
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no active session".to_string())?;

    runner.pause(&session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn goal_resume(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopGoalCommandResult, String> {
    let Some(runner) = lazy_goal_runner(&state).await else {
        return Ok(DesktopGoalCommandResult {
            status: goal_status(state).await?,
            next_prompt: None,
        });
    };
    let session_id = state
        .active_session_id
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no active session".to_string())?;

    let resumed = runner.resume(&session_id).map_err(|e| e.to_string())?;
    Ok(DesktopGoalCommandResult {
        status: goal_status(state).await?,
        next_prompt: resumed.then(|| "Continue working toward the active goal.".to_string()),
    })
}

#[tauri::command]
pub(crate) async fn goal_clear(state: State<'_, DesktopAppState>) -> Result<bool, String> {
    let Some(runner) = lazy_goal_runner(&state).await else {
        return Ok(false);
    };
    let session_id = state
        .active_session_id
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no active session".to_string())?;

    runner.clear(&session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn goal_edit(
    objective: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopGoalStatus, String> {
    let Some(runner) = lazy_goal_runner(&state).await else {
        return Err("no engine available".to_string());
    };
    let session_id = state
        .active_session_id
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no active session".to_string())?;

    runner
        .edit_objective(&session_id, &objective)
        .map_err(|e| e.to_string())?;

    goal_status(state).await
}

#[tauri::command]
pub(crate) async fn goal_log(
    state: State<'_, DesktopAppState>,
) -> Result<Vec<DesktopGoalStep>, String> {
    let s = goal_status(state).await?;
    Ok(s.steps)
}

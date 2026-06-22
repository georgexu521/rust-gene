//! LabRun support module.
//!
//! Keeps LabRun scheduling, delegation, reporting, and certification helpers separate from normal agent turns.

use crate::lab::model::{LabDaemonMode, LabSchedulerState, LabSchedulerStatus, LAB_SCHEMA_VERSION};
use crate::lab::orchestrator::{LabOrchestrator, LabSchedulerStepAction};
use crate::services::api::LlmProvider;
use crate::tools::ToolContext;
use anyhow::anyhow;
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

const DEFAULT_BACKGROUND_MAX_STEPS: usize = 20;
const DEFAULT_BACKGROUND_INTERVAL_MS: u64 = 1_000;

#[derive(Debug)]
struct BackgroundHandle {
    cancel: Arc<AtomicBool>,
    join: JoinHandle<()>,
}

static BACKGROUND_HANDLES: OnceLock<Mutex<HashMap<String, BackgroundHandle>>> = OnceLock::new();

fn handles() -> &'static Mutex<HashMap<String, BackgroundHandle>> {
    BACKGROUND_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
pub struct LabBackgroundStart {
    pub lab_run_id: String,
    pub max_steps: usize,
    pub interval_ms: u64,
}

#[derive(Debug, Clone)]
pub struct LabBackgroundStatus {
    pub lab_run_id: String,
    pub running_in_process: bool,
    pub persisted: Option<LabSchedulerState>,
}

#[derive(Debug, Clone)]
pub struct LabDaemonStart {
    pub lab_run_id: String,
    pub mode: LabDaemonMode,
    pub max_steps: usize,
    pub interval_ms: u64,
}

pub struct LabHybridCycleBackgroundRequest {
    pub context: ToolContext,
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
    pub max_cycles: usize,
    pub max_steps_per_cycle: usize,
    pub interval_ms: u64,
    pub instructions: String,
}

struct HybridLoopConfig {
    project_root: PathBuf,
    lab_run_id: String,
    context: ToolContext,
    provider: Arc<dyn LlmProvider>,
    model: String,
    instructions: String,
    cancel: Arc<AtomicBool>,
    interval_ms: u64,
}

pub fn default_background_max_steps() -> usize {
    DEFAULT_BACKGROUND_MAX_STEPS
}

pub fn default_background_interval_ms() -> u64 {
    DEFAULT_BACKGROUND_INTERVAL_MS
}

pub fn start_background_scheduler(
    project_root: impl AsRef<Path>,
    context: ToolContext,
    max_steps: usize,
    interval_ms: u64,
) -> anyhow::Result<LabBackgroundStart> {
    let project_root = project_root.as_ref().to_path_buf();
    let orchestrator = LabOrchestrator::for_project(&project_root);
    let run = orchestrator
        .store()
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for background scheduler"))?;
    orchestrator
        .store()
        .ensure_current_process_holds_fresh_lease(&run)?;
    let lab_run_id = run.lab_run_id.clone();
    let max_steps = max_steps.clamp(1, 100);
    let interval_ms = interval_ms.clamp(100, 60_000);

    let mut handles = handles()
        .lock()
        .map_err(|_| anyhow!("background scheduler registry is poisoned"))?;
    if handles.contains_key(&lab_run_id) {
        return Err(anyhow!(
            "background scheduler is already running for {}",
            lab_run_id
        ));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();
    let lab_run_id_for_task = lab_run_id.clone();
    let project_root_for_task = project_root.clone();
    let context_for_task = context.clone();
    let handle = tokio::spawn(async move {
        run_background_loop(
            project_root_for_task,
            lab_run_id_for_task,
            context_for_task,
            cancel_for_task,
            max_steps,
            interval_ms,
        )
        .await;
    });
    handles.insert(
        lab_run_id.clone(),
        BackgroundHandle {
            cancel,
            join: handle,
        },
    );
    drop(handles);

    let store = orchestrator.store();
    let now = Utc::now();
    store.write_scheduler_state(&LabSchedulerState {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: lab_run_id.clone(),
        status: LabSchedulerStatus::Running,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
        max_steps,
        steps_completed: 0,
        interval_ms,
        last_action: None,
        last_message: Some("background scheduler started".to_string()),
        stop_reason: None,
    })?;

    Ok(LabBackgroundStart {
        lab_run_id,
        max_steps,
        interval_ms,
    })
}

pub fn start_background_hybrid_scheduler(
    project_root: impl AsRef<Path>,
    context: ToolContext,
    provider: Arc<dyn LlmProvider>,
    model: String,
    max_steps: usize,
    interval_ms: u64,
    instructions: String,
) -> anyhow::Result<LabBackgroundStart> {
    let project_root = project_root.as_ref().to_path_buf();
    let orchestrator = LabOrchestrator::for_project(&project_root);
    let run = orchestrator
        .store()
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for hybrid background scheduler"))?;
    orchestrator
        .store()
        .ensure_current_process_holds_fresh_lease(&run)?;
    let lab_run_id = run.lab_run_id.clone();
    let max_steps = max_steps.clamp(1, 100);
    let interval_ms = interval_ms.clamp(100, 60_000);

    let mut handles = handles()
        .lock()
        .map_err(|_| anyhow!("background scheduler registry is poisoned"))?;
    if handles.contains_key(&lab_run_id) {
        return Err(anyhow!(
            "background scheduler is already running for {}",
            lab_run_id
        ));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let handle = tokio::spawn(run_background_hybrid_loop(
        HybridLoopConfig {
            project_root: project_root.clone(),
            lab_run_id: lab_run_id.clone(),
            context: context.clone(),
            provider,
            model,
            instructions,
            cancel: cancel.clone(),
            interval_ms,
        },
        max_steps,
    ));
    handles.insert(
        lab_run_id.clone(),
        BackgroundHandle {
            cancel,
            join: handle,
        },
    );
    drop(handles);

    let store = orchestrator.store();
    let now = Utc::now();
    store.write_scheduler_state(&LabSchedulerState {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: lab_run_id.clone(),
        status: LabSchedulerStatus::Running,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
        max_steps,
        steps_completed: 0,
        interval_ms,
        last_action: Some("HybridBackground".to_string()),
        last_message: Some("hybrid background scheduler started".to_string()),
        stop_reason: None,
    })?;

    Ok(LabBackgroundStart {
        lab_run_id,
        max_steps,
        interval_ms,
    })
}

pub fn start_background_hybrid_cycle_scheduler(
    project_root: impl AsRef<Path>,
    request: LabHybridCycleBackgroundRequest,
) -> anyhow::Result<LabBackgroundStart> {
    let project_root = project_root.as_ref().to_path_buf();
    let orchestrator = LabOrchestrator::for_project(&project_root);
    let run = orchestrator
        .store()
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for hybrid-cycle background scheduler"))?;
    orchestrator
        .store()
        .ensure_current_process_holds_fresh_lease(&run)?;
    let lab_run_id = run.lab_run_id.clone();
    let max_cycles = request.max_cycles.clamp(1, 20);
    let max_steps_per_cycle = request.max_steps_per_cycle.clamp(1, 100);
    let interval_ms = request.interval_ms.clamp(100, 60_000);

    let mut handles = handles()
        .lock()
        .map_err(|_| anyhow!("background scheduler registry is poisoned"))?;
    if handles.contains_key(&lab_run_id) {
        return Err(anyhow!(
            "background scheduler is already running for {}",
            lab_run_id
        ));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let handle = tokio::spawn(run_background_hybrid_cycle_loop(
        HybridLoopConfig {
            project_root: project_root.clone(),
            lab_run_id: lab_run_id.clone(),
            context: request.context.clone(),
            provider: request.provider,
            model: request.model,
            instructions: request.instructions,
            cancel: cancel.clone(),
            interval_ms,
        },
        max_cycles,
        max_steps_per_cycle,
    ));
    handles.insert(
        lab_run_id.clone(),
        BackgroundHandle {
            cancel,
            join: handle,
        },
    );
    drop(handles);

    let store = orchestrator.store();
    let now = Utc::now();
    store.write_scheduler_state(&LabSchedulerState {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: lab_run_id.clone(),
        status: LabSchedulerStatus::Running,
        updated_at: now,
        started_at: Some(now),
        stopped_at: None,
        max_steps: max_cycles,
        steps_completed: 0,
        interval_ms,
        last_action: Some("HybridCycleBackground".to_string()),
        last_message: Some(format!(
            "hybrid-cycle background scheduler started max_cycles={} max_steps_per_cycle={}",
            max_cycles, max_steps_per_cycle
        )),
        stop_reason: None,
    })?;

    Ok(LabBackgroundStart {
        lab_run_id,
        max_steps: max_cycles,
        interval_ms,
    })
}

pub fn start_daemon_scheduler_from_policy(
    project_root: impl AsRef<Path>,
    context: ToolContext,
    provider: Arc<dyn LlmProvider>,
    model: String,
) -> anyhow::Result<LabDaemonStart> {
    let project_root = project_root.as_ref();
    let store = crate::lab::store::LabStore::for_project(project_root);
    let policy = store
        .load_daemon_state()?
        .ok_or_else(|| anyhow!("no Lab daemon policy found"))?;
    if !policy.enabled {
        return Err(anyhow!("Lab daemon policy is disabled"));
    }

    let started = match policy.mode {
        LabDaemonMode::Strict => {
            start_background_scheduler(project_root, context, policy.max_steps, policy.interval_ms)?
        }
        LabDaemonMode::Hybrid => start_background_hybrid_scheduler(
            project_root,
            context,
            provider,
            model,
            policy.max_steps,
            policy.interval_ms,
            policy.instructions.clone(),
        )?,
        LabDaemonMode::HybridCycles => start_background_hybrid_cycle_scheduler(
            project_root,
            LabHybridCycleBackgroundRequest {
                context,
                provider,
                model,
                max_cycles: policy.max_steps,
                max_steps_per_cycle: policy.max_steps_per_cycle,
                interval_ms: policy.interval_ms,
                instructions: policy.instructions.clone(),
            },
        )?,
    };
    let _ = store.record_daemon_start_result(Some(&started.lab_run_id), None);
    Ok(LabDaemonStart {
        lab_run_id: started.lab_run_id,
        mode: policy.mode,
        max_steps: started.max_steps,
        interval_ms: started.interval_ms,
    })
}

pub fn stop_background_scheduler(
    project_root: impl AsRef<Path>,
) -> anyhow::Result<LabSchedulerState> {
    let orchestrator = LabOrchestrator::for_project(project_root);
    let run = orchestrator
        .store()
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for background scheduler"))?;
    let lab_run_id = run.lab_run_id.clone();
    let handle = handles()
        .lock()
        .map_err(|_| anyhow!("background scheduler registry is poisoned"))?
        .remove(&lab_run_id);

    if let Some(handle) = handle {
        handle.cancel.store(true, Ordering::SeqCst);
        handle.join.abort();
    }

    let now = Utc::now();
    let state = LabSchedulerState {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: lab_run_id.clone(),
        status: LabSchedulerStatus::Stopped,
        updated_at: now,
        started_at: None,
        stopped_at: Some(now),
        max_steps: 0,
        steps_completed: 0,
        interval_ms: 0,
        last_action: None,
        last_message: Some("background scheduler stopped by user".to_string()),
        stop_reason: Some("user".to_string()),
    };
    orchestrator.store().write_scheduler_state(&state)?;
    Ok(state)
}

pub fn background_scheduler_status(
    project_root: impl AsRef<Path>,
) -> anyhow::Result<LabBackgroundStatus> {
    let orchestrator = LabOrchestrator::for_project(project_root);
    let run = orchestrator
        .store()
        .latest_run()?
        .ok_or_else(|| anyhow!("no LabRun found for background scheduler"))?;
    let lab_run_id = run.lab_run_id.clone();
    let running_in_process = handles()
        .lock()
        .map_err(|_| anyhow!("background scheduler registry is poisoned"))?
        .contains_key(&lab_run_id);
    let persisted = orchestrator.store().load_scheduler_state(&lab_run_id)?;
    Ok(LabBackgroundStatus {
        lab_run_id,
        running_in_process,
        persisted,
    })
}

async fn run_background_loop(
    project_root: PathBuf,
    lab_run_id: String,
    context: ToolContext,
    cancel: Arc<AtomicBool>,
    max_steps: usize,
    interval_ms: u64,
) {
    let orchestrator = LabOrchestrator::for_project(&project_root);
    let mut steps_completed = 0usize;
    let mut final_status = LabSchedulerStatus::Completed;
    let mut last_action = None;
    let mut last_message = None;
    let mut stop_reason = Some("max_steps_reached".to_string());

    for _ in 0..max_steps {
        if cancel.load(Ordering::SeqCst) {
            final_status = LabSchedulerStatus::Stopped;
            stop_reason = Some("cancelled".to_string());
            break;
        }
        let _ = orchestrator.store().refresh_latest_run_heartbeat();
        match orchestrator
            .run_scheduler_step_latest_with_context(context.clone())
            .await
        {
            Ok(step) => {
                steps_completed = steps_completed.saturating_add(1);
                last_action = Some(format!("{:?}", step.action));
                last_message = Some(step.message.clone());
                match step.action {
                    LabSchedulerStepAction::TickAdvanced => {}
                    LabSchedulerStepAction::GraduateDispatched => {
                        final_status = LabSchedulerStatus::Blocked;
                        stop_reason = Some("graduate_dispatched_waiting_for_result".to_string());
                        break;
                    }
                    LabSchedulerStepAction::NeedsUser => {
                        final_status = LabSchedulerStatus::NeedsUser;
                        stop_reason = Some("needs_user".to_string());
                        break;
                    }
                    LabSchedulerStepAction::Blocked => {
                        final_status = LabSchedulerStatus::Blocked;
                        stop_reason = Some("blocked".to_string());
                        break;
                    }
                }
            }
            Err(err) => {
                final_status = LabSchedulerStatus::Failed;
                last_message = Some(err.to_string());
                stop_reason = Some("error".to_string());
                break;
            }
        }
        sleep(Duration::from_millis(interval_ms)).await;
    }

    let now = Utc::now();
    let state = LabSchedulerState {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: lab_run_id.clone(),
        status: final_status,
        updated_at: now,
        started_at: None,
        stopped_at: Some(now),
        max_steps,
        steps_completed,
        interval_ms,
        last_action,
        last_message,
        stop_reason,
    };
    let _ = orchestrator.store().write_scheduler_state(&state);
    if let Ok(mut handles) = handles().lock() {
        handles.remove(&lab_run_id);
    }
}

async fn run_background_hybrid_loop(config: HybridLoopConfig, max_steps: usize) {
    let HybridLoopConfig {
        project_root,
        lab_run_id,
        context,
        provider,
        model,
        instructions,
        cancel,
        interval_ms,
    } = config;
    let orchestrator = LabOrchestrator::for_project(&project_root);
    let mut steps_completed = 0usize;
    let mut final_status = LabSchedulerStatus::Completed;
    let mut last_action = Some("HybridBackground".to_string());
    let mut last_message = None;
    let mut stop_reason = Some("max_steps_reached".to_string());

    for _ in 0..max_steps {
        if cancel.load(Ordering::SeqCst) {
            final_status = LabSchedulerStatus::Stopped;
            stop_reason = Some("cancelled".to_string());
            break;
        }
        let _ = orchestrator.store().refresh_latest_run_heartbeat();
        match crate::lab::draft::run_hybrid_lab_steps_until_boundary(
            &project_root,
            provider.clone(),
            model.clone(),
            1,
            &instructions,
            context.clone(),
        )
        .await
        {
            Ok(outcome) => {
                steps_completed = steps_completed.saturating_add(outcome.steps.len());
                last_action = Some(format!("Hybrid::{:?}", outcome.stop_reason));
                last_message = Some(format!(
                    "hybrid background final_stage={} steps={}",
                    outcome.final_stage,
                    outcome.steps.len()
                ));
                match outcome.stop_reason {
                    crate::lab::draft::LabHybridRunStopReason::MaxSteps => {}
                    crate::lab::draft::LabHybridRunStopReason::NeedsUser => {
                        final_status = LabSchedulerStatus::NeedsUser;
                        stop_reason = Some("needs_user".to_string());
                        break;
                    }
                    crate::lab::draft::LabHybridRunStopReason::NotActive => {
                        final_status = LabSchedulerStatus::Stopped;
                        stop_reason = Some("not_active".to_string());
                        break;
                    }
                    crate::lab::draft::LabHybridRunStopReason::RevisionRequested
                    | crate::lab::draft::LabHybridRunStopReason::DeterministicGateBlocked => {
                        final_status = LabSchedulerStatus::Blocked;
                        stop_reason = Some("blocked".to_string());
                        break;
                    }
                    crate::lab::draft::LabHybridRunStopReason::SchedulerStopped(action) => {
                        match action {
                            LabSchedulerStepAction::TickAdvanced => {}
                            LabSchedulerStepAction::GraduateDispatched => {
                                final_status = LabSchedulerStatus::Blocked;
                                stop_reason =
                                    Some("graduate_dispatched_waiting_for_result".to_string());
                                break;
                            }
                            LabSchedulerStepAction::NeedsUser => {
                                final_status = LabSchedulerStatus::NeedsUser;
                                stop_reason = Some("needs_user".to_string());
                                break;
                            }
                            LabSchedulerStepAction::Blocked => {
                                final_status = LabSchedulerStatus::Blocked;
                                stop_reason = Some("blocked".to_string());
                                break;
                            }
                        }
                    }
                }
            }
            Err(err) => {
                final_status = LabSchedulerStatus::Failed;
                last_message = Some(err.to_string());
                stop_reason = Some("error".to_string());
                break;
            }
        }
        sleep(Duration::from_millis(interval_ms)).await;
    }

    let now = Utc::now();
    let state = LabSchedulerState {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: lab_run_id.clone(),
        status: final_status,
        updated_at: now,
        started_at: None,
        stopped_at: Some(now),
        max_steps,
        steps_completed,
        interval_ms,
        last_action,
        last_message,
        stop_reason,
    };
    let _ = orchestrator.store().write_scheduler_state(&state);
    if let Ok(mut handles) = handles().lock() {
        handles.remove(&lab_run_id);
    }
}

async fn run_background_hybrid_cycle_loop(
    config: HybridLoopConfig,
    max_cycles: usize,
    max_steps_per_cycle: usize,
) {
    let HybridLoopConfig {
        project_root,
        lab_run_id,
        context,
        provider,
        model,
        instructions,
        cancel,
        interval_ms,
    } = config;
    let orchestrator = LabOrchestrator::for_project(&project_root);
    let mut cycles_completed = 0usize;
    let mut final_status = LabSchedulerStatus::Completed;
    let mut last_action = Some("HybridCycleBackground".to_string());
    let mut last_message = None;
    let mut stop_reason = Some("max_cycles_reached".to_string());

    if cancel.load(Ordering::SeqCst) {
        final_status = LabSchedulerStatus::Stopped;
        stop_reason = Some("cancelled".to_string());
    } else {
        let _ = orchestrator.store().refresh_latest_run_heartbeat();
        match crate::lab::draft::run_hybrid_lab_cycles_until_boundary(
            &project_root,
            provider,
            model,
            max_cycles,
            max_steps_per_cycle,
            &instructions,
            context,
        )
        .await
        {
            Ok(outcome) => {
                cycles_completed = outcome.cycles.len();
                last_action = Some(format!("HybridCycles::{:?}", outcome.stop_reason));
                last_message = Some(format!(
                    "hybrid-cycle background final_stage={} cycles={} final_cycle_count={}",
                    outcome.final_stage,
                    outcome.cycles.len(),
                    outcome.final_cycle_count
                ));
                match outcome.stop_reason {
                    crate::lab::draft::LabHybridCycleRunStopReason::MaxCycles => {}
                    crate::lab::draft::LabHybridCycleRunStopReason::CostBudgetExceeded {
                        ..
                    } => {
                        final_status = LabSchedulerStatus::Blocked;
                        stop_reason = Some("cost_budget_exceeded".to_string());
                    }
                    crate::lab::draft::LabHybridCycleRunStopReason::Stopped(reason) => match reason
                    {
                        crate::lab::draft::LabHybridRunStopReason::MaxSteps => {
                            final_status = LabSchedulerStatus::Blocked;
                            stop_reason = Some("max_steps_per_cycle_reached".to_string());
                        }
                        crate::lab::draft::LabHybridRunStopReason::NeedsUser => {
                            final_status = LabSchedulerStatus::NeedsUser;
                            stop_reason = Some("needs_user".to_string());
                        }
                        crate::lab::draft::LabHybridRunStopReason::NotActive => {
                            final_status = LabSchedulerStatus::Stopped;
                            stop_reason = Some("not_active".to_string());
                        }
                        crate::lab::draft::LabHybridRunStopReason::RevisionRequested
                        | crate::lab::draft::LabHybridRunStopReason::DeterministicGateBlocked => {
                            final_status = LabSchedulerStatus::Blocked;
                            stop_reason = Some("blocked".to_string());
                        }
                        crate::lab::draft::LabHybridRunStopReason::SchedulerStopped(action) => {
                            match action {
                                LabSchedulerStepAction::TickAdvanced => {
                                    final_status = LabSchedulerStatus::Blocked;
                                    stop_reason = Some("scheduler_step_bound".to_string());
                                }
                                LabSchedulerStepAction::GraduateDispatched => {
                                    final_status = LabSchedulerStatus::Blocked;
                                    stop_reason =
                                        Some("graduate_dispatched_waiting_for_result".to_string());
                                }
                                LabSchedulerStepAction::NeedsUser => {
                                    final_status = LabSchedulerStatus::NeedsUser;
                                    stop_reason = Some("needs_user".to_string());
                                }
                                LabSchedulerStepAction::Blocked => {
                                    final_status = LabSchedulerStatus::Blocked;
                                    stop_reason = Some("blocked".to_string());
                                }
                            }
                        }
                    },
                }
            }
            Err(err) => {
                final_status = LabSchedulerStatus::Failed;
                last_message = Some(err.to_string());
                stop_reason = Some("error".to_string());
            }
        }
    }
    sleep(Duration::from_millis(interval_ms)).await;

    let now = Utc::now();
    let state = LabSchedulerState {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: lab_run_id.clone(),
        status: final_status,
        updated_at: now,
        started_at: None,
        stopped_at: Some(now),
        max_steps: max_cycles,
        steps_completed: cycles_completed,
        interval_ms,
        last_action,
        last_message,
        stop_reason,
    };
    let _ = orchestrator.store().write_scheduler_state(&state);
    if let Ok(mut handles) = handles().lock() {
        handles.remove(&lab_run_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn background_scheduler_status_reports_missing_run() {
        let temp = tempfile::tempdir().unwrap();
        let err = background_scheduler_status(temp.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("no LabRun"));
    }
}

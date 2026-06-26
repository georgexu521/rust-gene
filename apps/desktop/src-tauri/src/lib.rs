use futures::StreamExt;
use priority_agent::desktop_runtime::{DesktopContextSnapshot, DesktopRunEvent, DesktopRuntime};
use priority_agent::engine::goal::runner::GoalRunner;
use priority_agent::engine::streaming::StreamEvent;
use priority_agent::engine::turn_ingress::classify_turn_ingress;
use priority_agent::permissions::PermissionMode;
use priority_agent::session_store::SessionStore;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::Mutex;

mod desktop_context;
mod desktop_state;
mod desktop_support;
mod desktop_types;
mod diagnostics;
mod diagnostics_export;
mod goal_commands;
mod health_commands;
mod lab_daemon_commands;
mod native_smoke;
mod onboarding_commands;
mod open_commands;
mod preview_commands;
mod revert_commands;
mod session_commands;
pub(crate) use desktop_context::*;
use desktop_state::*;
use desktop_support::*;
pub(crate) use desktop_types::*;
use diagnostics::*;
use diagnostics_export::*;
use goal_commands::*;
use health_commands::*;
use lab_daemon_commands::*;
use native_smoke::*;
use onboarding_commands::*;
use open_commands::*;
use preview_commands::*;
use revert_commands::*;
use session_commands::*;

#[derive(Debug, Clone)]
struct DesktopRunHandle {
    id: String,
    cancel_requested: bool,
}

struct DesktopAppState {
    runtime: Mutex<Option<DesktopRuntime>>,
    active_run: Mutex<Option<DesktopRunHandle>>,
    selected_project: Mutex<PathBuf>,
    active_session_id: Mutex<Option<String>>,
    permission_mode: Mutex<Option<String>>,
    detail_level: Mutex<Option<String>>,
    agent_mode: Mutex<Option<String>>,
    provider_name: Mutex<Option<String>>,
    model: Mutex<Option<String>>,
    recent_projects: Mutex<Vec<PathBuf>>,
    archived_session_ids: Mutex<Vec<String>>,
    lab_daemon_supervision_enabled: Mutex<bool>,
    lab_daemon_last_supervision: Mutex<Option<String>>,
    lab_daemon_last_supervision_result: Mutex<Option<String>>,
    lab_daemon_next_supervision: Mutex<Option<String>>,
    onboarding_state: Mutex<Option<DesktopOnboardingState>>,
    workspace_trust: Mutex<Option<DesktopWorkspaceTrustStatus>>,
    native_smoke_permission_pending: Mutex<bool>,
    goal_runner: Mutex<Option<GoalRunner>>,
    settings_path: PathBuf,
    diagnostic_logs_path: PathBuf,
}

#[tauri::command]
async fn desktop_settings(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let active_session_id = active_session_id_if_present(&state).await?;
    let recent_projects = state
        .recent_projects
        .lock()
        .await
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    let archived_session_ids = state.archived_session_ids.lock().await.clone();
    let lab_daemon_supervision_enabled = *state.lab_daemon_supervision_enabled.lock().await;
    let lab_daemon_last_supervision = state.lab_daemon_last_supervision.lock().await.clone();
    let lab_daemon_last_supervision_result = state
        .lab_daemon_last_supervision_result
        .lock()
        .await
        .clone();
    let lab_daemon_next_supervision = state.lab_daemon_next_supervision.lock().await.clone();
    let onboarding_state = state.onboarding_state.lock().await.clone();
    let stored_workspace_trust = state.workspace_trust.lock().await.clone();
    let workspace_trust = desktop_workspace_trust_status(&selected_project, stored_workspace_trust);

    Ok(DesktopSettingsResponse {
        selected_project: selected_project.display().to_string(),
        startup_state: desktop_startup_state(&selected_project, active_session_id.as_deref()),
        active_session_id,
        permission_mode: normalized_permission_mode_label(
            state.permission_mode.lock().await.as_deref(),
        )
        .to_string(),
        detail_level: normalized_detail_level_label(state.detail_level.lock().await.as_deref())
            .to_string(),
        agent_mode: state
            .agent_mode
            .lock()
            .await
            .clone()
            .unwrap_or_else(|| "auto".to_string()),
        provider_name: state.provider_name.lock().await.clone(),
        model: state.model.lock().await.clone(),
        settings_path: state.settings_path.display().to_string(),
        diagnostic_logs_path: state.diagnostic_logs_path.display().to_string(),
        recent_projects,
        archived_session_ids,
        lab_daemon_supervision_enabled,
        lab_daemon_last_supervision,
        lab_daemon_last_supervision_result,
        lab_daemon_next_supervision,
        onboarding_state,
        workspace_trust,
        credential_storage: desktop_credential_storage_status_value(),
    })
}

#[tauri::command]
async fn set_permission_mode(
    mode: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let normalized = normalized_permission_mode_label(Some(&mode)).to_string();
    {
        let mut permission_mode = state.permission_mode.lock().await;
        *permission_mode = Some(normalized.clone());
    }
    {
        let runtime = state.runtime.lock().await;
        if let Some(runtime) = runtime.as_ref() {
            runtime
                .streaming_engine()
                .set_permission_mode(parse_desktop_permission_mode(&normalized));
        }
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
async fn set_detail_level(
    level: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let normalized = normalized_detail_level_label(Some(&level)).to_string();
    {
        let mut detail_level = state.detail_level.lock().await;
        *detail_level = Some(normalized);
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
async fn set_agent_mode(
    mode: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let normalized = mode.trim().to_ascii_lowercase();
    if !matches!(
        normalized.as_str(),
        "auto" | "build" | "plan" | "explore" | "review"
    ) {
        return Err(format!(
            "Invalid agent mode '{}'. Valid modes: auto, build, plan, explore, review.",
            mode
        ));
    }
    {
        let mut agent_mode = state.agent_mode.lock().await;
        *agent_mode = Some(normalized);
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
async fn set_lab_daemon_supervision_enabled(
    enabled: bool,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    {
        let mut stored = state.lab_daemon_supervision_enabled.lock().await;
        *stored = enabled;
    }
    {
        let mut next = state.lab_daemon_next_supervision.lock().await;
        *next = None;
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
fn agent_mode_options() -> Vec<AgentModeOption> {
    vec![
        AgentModeOption {
            id: "auto".to_string(),
            label: "Auto".to_string(),
            description: "Let the agent choose the right mode".to_string(),
        },
        AgentModeOption {
            id: "build".to_string(),
            label: "Build".to_string(),
            description: "Full coding — read, edit, shell, validation".to_string(),
        },
        AgentModeOption {
            id: "plan".to_string(),
            label: "Plan".to_string(),
            description: "Explore and plan — no file changes".to_string(),
        },
        AgentModeOption {
            id: "explore".to_string(),
            label: "Explore".to_string(),
            description: "Read and search — no edits".to_string(),
        },
        AgentModeOption {
            id: "review".to_string(),
            label: "Review".to_string(),
            description: "Diff analysis and findings — no edits".to_string(),
        },
    ]
}

#[tauri::command]
fn permission_mode_options() -> Vec<PermissionModeOption> {
    desktop_permission_mode_options()
}

#[tauri::command]
async fn provider_model_status(
    state: State<'_, DesktopAppState>,
) -> Result<ProviderModelStatus, String> {
    provider_model_status_for_state(&state).await
}

#[tauri::command]
async fn set_provider_model(
    provider_id: String,
    model: String,
    state: State<'_, DesktopAppState>,
) -> Result<ProviderModelStatus, String> {
    let normalized_provider = provider_id.trim().to_ascii_lowercase();
    let normalized_model = model.trim().to_string();
    if normalized_provider.is_empty() {
        return Err("provider id cannot be empty".to_string());
    }
    if normalized_model.is_empty() {
        return Err("model cannot be empty".to_string());
    }

    let registry = priority_agent::services::api::provider::ProviderRegistry::from_env();
    let provider = registry
        .get(&normalized_provider)
        .ok_or_else(|| format!("provider is not configured: {normalized_provider}"))?;

    {
        let mut provider_name = state.provider_name.lock().await;
        *provider_name = Some(normalized_provider.clone());
    }
    {
        let mut stored_model = state.model.lock().await;
        *stored_model = Some(normalized_model.clone());
    }
    {
        let runtime = state.runtime.lock().await;
        if let Some(runtime) = runtime.as_ref() {
            runtime
                .streaming_engine()
                .set_provider(provider, normalized_model.clone());
        }
    }

    persist_current_settings(&state).await?;
    provider_model_status_for_state(&state).await
}

#[tauri::command]
async fn desktop_diagnostics(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopDiagnosticsResponse, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let settings_path = state.settings_path.clone();
    let diagnostic_logs_path = state.diagnostic_logs_path.clone();

    Ok(DesktopDiagnosticsResponse {
        items: collect_desktop_diagnostics(
            &selected_project,
            &settings_path,
            &diagnostic_logs_path,
        ),
    })
}

#[tauri::command]
async fn export_session(
    session_id: Option<String>,
    format: Option<String>,
    privacy: Option<String>,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopExportResult, String> {
    let session_id = match session_id {
        Some(id) if !id.trim().is_empty() => id,
        _ => state
            .active_session_id
            .lock()
            .await
            .clone()
            .ok_or_else(|| "No active session to export.".to_string())?,
    };
    let format = parse_desktop_export_format(format.as_deref())?;
    let privacy = parse_desktop_export_privacy(privacy.as_deref())?;
    let store = open_session_store()?;
    let session = store
        .get_session(&session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;
    let manager = priority_agent::tui::session_manager::TuiSessionManager::from_store(
        std::sync::Arc::new(store),
        session_id.clone(),
        session.title,
        &session.model,
        session.workspace_root.as_deref(),
    )
    .map_err(|err| err.to_string())?;
    let path = manager
        .write_session_export(&session_id, format, privacy)
        .map_err(|err| err.to_string())?;

    Ok(DesktopExportResult {
        session_id,
        path: path.display().to_string(),
        format: format!("{:?}", format).to_lowercase(),
        privacy: privacy.label().to_string(),
    })
}

fn parse_desktop_export_format(
    value: Option<&str>,
) -> Result<priority_agent::session_store::export::SessionExportFormat, String> {
    match value
        .unwrap_or("markdown")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => Ok(priority_agent::session_store::export::SessionExportFormat::Json),
        "md" | "markdown" => {
            Ok(priority_agent::session_store::export::SessionExportFormat::Markdown)
        }
        other => Err(format!("Unsupported export format: {other}")),
    }
}

fn parse_desktop_export_privacy(
    value: Option<&str>,
) -> Result<priority_agent::session_store::export::SessionExportPrivacy, String> {
    match value
        .unwrap_or("redacted")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "full" => Ok(priority_agent::session_store::export::SessionExportPrivacy::Full),
        "redacted" => Ok(priority_agent::session_store::export::SessionExportPrivacy::Redacted),
        "summary" => Ok(priority_agent::session_store::export::SessionExportPrivacy::Summary),
        other => Err(format!("Unsupported export privacy: {other}")),
    }
}

#[tauri::command]
async fn provider_setup_info() -> Result<ProviderSetupInfo, String> {
    Ok(provider_setup_info_value())
}

#[tauri::command]
async fn record_native_smoke_result(
    result: String,
    state: State<'_, DesktopAppState>,
) -> Result<(), String> {
    let diagnostic_logs_path = state.diagnostic_logs_path.clone();
    let smoke_name = if result.contains("native_live_provider_smoke") {
        "native_live_provider_smoke"
    } else if result.contains("native_multitool_smoke") {
        "native_multitool_smoke"
    } else if result.contains("native_soak_smoke") {
        "native_soak_smoke"
    } else if result.contains("native_extended_soak_smoke") {
        "native_extended_soak_smoke"
    } else if result.contains("native_soak_restart_smoke") {
        "native_soak_restart_smoke"
    } else if result.contains("native_extended_soak_restart_smoke") {
        "native_extended_soak_restart_smoke"
    } else if result.contains("native_lab_recovery_smoke") {
        "native_lab_recovery_smoke"
    } else if result.contains("native_restart_smoke") {
        "native_restart_smoke"
    } else {
        "native_interaction_smoke"
    };
    let ok = result.contains(&format!("{smoke_name} ok=true"));
    let status = if ok { "ok=true" } else { "ok=false" };
    append_desktop_log(
        &diagnostic_logs_path,
        &format!(
            "{smoke_name} {status} result={}",
            sanitize_log_value(&result)
        ),
    )
}

fn validate_project_path(path: impl Into<PathBuf>) -> Result<PathBuf, String> {
    let project = path.into();
    if !project.exists() {
        return Err(format!(
            "project path does not exist: {}",
            project.display()
        ));
    }
    if !project.is_dir() {
        return Err(format!(
            "project path is not a directory: {}",
            project.display()
        ));
    }

    let project = project.canonicalize().map_err(|err| err.to_string())?;
    if project.parent().is_none() {
        return Err("project path cannot be the filesystem root".to_string());
    }

    Ok(project)
}

fn selected_project_response(project: PathBuf) -> SelectedProject {
    SelectedProject {
        path: project.display().to_string(),
    }
}

fn list_recent_sessions_from_store(
    store: &SessionStore,
    limit: i64,
    archived_session_ids: &[String],
) -> Result<Vec<RecentSession>, String> {
    let archived = archived_session_ids.iter().collect::<HashSet<_>>();
    let limit = limit.clamp(1, 100);
    let fetch_limit = (limit + archived_session_ids.len() as i64).clamp(1, 100);
    let sessions = store
        .list_sessions(fetch_limit)
        .map_err(|err| err.to_string())?;

    sessions
        .into_iter()
        .filter(|session| !archived.contains(&session.id))
        .take(limit as usize)
        .map(|session| {
            let message_count = store
                .message_count(&session.id)
                .map_err(|err| err.to_string())?;
            Ok(RecentSession {
                id: session.id,
                title: session.title,
                updated_at: session.updated_at,
                model: session.model,
                message_count,
            })
        })
        .collect()
}

fn search_sessions_from_store(
    store: &SessionStore,
    query: &str,
    limit: i64,
    archived_session_ids: &[String],
) -> Result<Vec<RecentSession>, String> {
    let archived = archived_session_ids.iter().collect::<HashSet<_>>();
    let limit = limit.clamp(1, 100);
    let fetch_limit = (limit + archived_session_ids.len() as i64).clamp(1, 100);
    let sessions = store
        .search_sessions(query, fetch_limit)
        .map_err(|err| err.to_string())?;

    sessions
        .into_iter()
        .filter(|session| !archived.contains(&session.id))
        .take(limit as usize)
        .map(|session| {
            let message_count = store
                .message_count(&session.id)
                .map_err(|err| err.to_string())?;
            Ok(RecentSession {
                id: session.id,
                title: session.title,
                updated_at: session.updated_at,
                model: session.model,
                message_count,
            })
        })
        .collect()
}

fn recent_session_from_store(
    store: &SessionStore,
    session_id: &str,
) -> Result<RecentSession, String> {
    let session = store
        .get_session(session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;
    let message_count = store
        .message_count(&session.id)
        .map_err(|err| err.to_string())?;

    Ok(RecentSession {
        id: session.id,
        title: session.title,
        updated_at: session.updated_at,
        model: session.model,
        message_count,
    })
}

#[tauri::command]
async fn send_message(
    app: AppHandle,
    message: String,
    contexts: Vec<DesktopRunContext>,
    state: State<'_, DesktopAppState>,
) -> Result<(), String> {
    let run_guard = match acquire_desktop_run_guard(&state).await {
        Ok(run_id) => run_id,
        Err(err) => {
            let _ = append_desktop_log(
                &state.diagnostic_logs_path,
                &format!("run_error message={}", sanitize_log_value(&err)),
            );
            let _ = app.emit(
                "desktop-run-event",
                DesktopRunEvent::RunError {
                    message: err.clone(),
                },
            );
            return Err(err);
        }
    };
    let result = send_message_inner(app, message, contexts, &state, &run_guard).await;
    release_desktop_run_guard(&state, &run_guard).await;
    result
}

async fn send_message_inner(
    app: AppHandle,
    message: String,
    contexts: Vec<DesktopRunContext>,
    state: &State<'_, DesktopAppState>,
    run_guard: &str,
) -> Result<(), String> {
    let agent_mode_label = state
        .agent_mode
        .lock()
        .await
        .clone()
        .unwrap_or_else(|| "auto".to_string());
    let agent_mode =
        priority_agent::engine::agent_mode::AgentMode::parse(&agent_mode_label).unwrap_or_default();
    // ... rest of function
    let diagnostic_logs_path = state.diagnostic_logs_path.clone();
    let _ = append_desktop_log(
        &diagnostic_logs_path,
        &format!(
            "run_submit chars={} contexts={}",
            message.chars().count(),
            contexts.len()
        ),
    );
    if native_smoke_enabled() {
        return emit_native_smoke_run_fixture(app, message, state).await;
    }

    let runtime = runtime_for_state(state).await?;
    let ingress_lane = classify_turn_ingress(&message, !contexts.is_empty());
    let force_full_agent_lane = agent_mode != priority_agent::engine::agent_mode::AgentMode::Auto;
    if ingress_lane.is_lightweight() && !force_full_agent_lane {
        let outcome = runtime
            .run_lightweight_turn(&message, ingress_lane)
            .await
            .map_err(|err| err.to_string())?;
        let active_session_id = runtime.streaming_engine().current_session_id();
        if active_session_id.is_some() {
            {
                let mut stored_session_id = state.active_session_id.lock().await;
                *stored_session_id = active_session_id;
            }
            persist_current_settings(state).await?;
        }
        let usage_log = outcome
            .usage
            .as_ref()
            .map(|usage| {
                format!(
                    " prompt={} completion={} cached={}",
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.cached_tokens.unwrap_or(0)
                )
            })
            .unwrap_or_default();
        let _ = append_desktop_log(
            &diagnostic_logs_path,
            &format!("run_lightweight lane={}{}", outcome.lane.label(), usage_log),
        );
        app.emit(
            "desktop-run-event",
            DesktopRunEvent::AssistantDelta {
                text: outcome.answer,
            },
        )
        .map_err(|err| err.to_string())?;
        app.emit("desktop-run-event", DesktopRunEvent::RunCompleted)
            .map_err(|err| err.to_string())?;
        let _ = append_desktop_log(&diagnostic_logs_path, "run_completed");
        return Ok(());
    }

    let selected_project = state.selected_project.lock().await.clone();
    let message = match enrich_message_with_desktop_contexts(message, &contexts, &selected_project)
    {
        Ok(message) => message,
        Err(err) => {
            let _ = append_desktop_log(
                &diagnostic_logs_path,
                &format!("run_error message={}", sanitize_log_value(&err)),
            );
            return Err(err);
        }
    };
    let active_session_id = runtime.current_session_id();
    if active_session_id.is_some() {
        {
            let mut stored_session_id = state.active_session_id.lock().await;
            *stored_session_id = active_session_id.clone();
        }
        persist_current_settings(state).await?;
    }
    let desktop_run_id = format!(
        "desktop-run-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    app.emit(
        "desktop-run-event",
        DesktopRunEvent::RunStarted {
            run_id: desktop_run_id,
            session_id: active_session_id.clone(),
        },
    )
    .map_err(|err| err.to_string())?;
    let _ = append_desktop_log(&diagnostic_logs_path, "run_started");

    let mut stream = runtime
        .run_full_turn_with_agent_mode(message, agent_mode)
        .await;
    let _ = append_desktop_log(&diagnostic_logs_path, "run_stream_opened");

    loop {
        let event = stream.next().await;
        let Some(event) = event else {
            let _ = append_desktop_log(&diagnostic_logs_path, "run_stream_ended");
            break;
        };
        if let Some(entry) = desktop_stream_event_log(&event) {
            let _ = append_desktop_log(&diagnostic_logs_path, &entry);
        }
        if matches!(event, StreamEvent::Start) {
            continue;
        }
        if desktop_run_cancel_requested(state, run_guard).await {
            let message = "Desktop run cancelled by user".to_string();
            app.emit(
                "desktop-run-event",
                DesktopRunEvent::RunError {
                    message: message.clone(),
                },
            )
            .map_err(|err| err.to_string())?;
            let _ = append_desktop_log(
                &diagnostic_logs_path,
                &format!("run_error message={}", sanitize_log_value(&message)),
            );
            break;
        }
        let mut desktop_event = DesktopRunEvent::from_stream_event(event);
        if let DesktopRunEvent::RunStarted { session_id, .. } = &mut desktop_event {
            *session_id = active_session_id.clone();
            if active_session_id.is_some() {
                {
                    let mut stored_session_id = state.active_session_id.lock().await;
                    *stored_session_id = active_session_id.clone();
                }
                persist_current_settings(state).await?;
            }
        }
        let is_terminal = matches!(
            desktop_event,
            DesktopRunEvent::RunCompleted | DesktopRunEvent::RunError { .. }
        );
        let terminal_log = match &desktop_event {
            DesktopRunEvent::RunCompleted => Some("run_completed".to_string()),
            DesktopRunEvent::RunError { message } => {
                Some(format!("run_error message={}", sanitize_log_value(message)))
            }
            _ => None,
        };
        app.emit("desktop-run-event", desktop_event)
            .map_err(|err| err.to_string())?;
        if let Some(entry) = terminal_log {
            let _ = append_desktop_log(&diagnostic_logs_path, &entry);
        }
        if is_terminal {
            break;
        }
    }

    Ok(())
}

async fn acquire_desktop_run_guard(state: &State<'_, DesktopAppState>) -> Result<String, String> {
    let mut active_run = state.active_run.lock().await;
    if let Some(existing) = active_run.as_ref() {
        return Err(format!(
            "A desktop run is already active: {}. Cancel or force reset it before starting another run.",
            existing.id
        ));
    }
    let id = format!(
        "desktop-run-lock-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    *active_run = Some(DesktopRunHandle {
        id: id.clone(),
        cancel_requested: false,
    });
    Ok(id)
}

async fn release_desktop_run_guard(state: &State<'_, DesktopAppState>, run_id: &str) {
    let mut active_run = state.active_run.lock().await;
    if active_run
        .as_ref()
        .is_some_and(|handle| handle.id.as_str() == run_id)
    {
        *active_run = None;
    }
}

async fn desktop_run_cancel_requested(state: &State<'_, DesktopAppState>, run_id: &str) -> bool {
    state
        .active_run
        .lock()
        .await
        .as_ref()
        .is_some_and(|handle| handle.id == run_id && handle.cancel_requested)
}

#[tauri::command]
async fn cancel_run(app: AppHandle, state: State<'_, DesktopAppState>) -> Result<bool, String> {
    let mut active_run = state.active_run.lock().await;
    let Some(handle) = active_run.as_mut() else {
        return Ok(false);
    };
    handle.cancel_requested = true;
    let _ = append_desktop_log(
        &state.diagnostic_logs_path,
        &format!("run_cancel_requested id={}", sanitize_log_value(&handle.id)),
    );
    let _ = app.emit(
        "desktop-run-event",
        DesktopRunEvent::RuntimeDiagnostic {
            diagnostic: serde_json::json!({
                "schema": "desktop_run_guard",
                "status": "cancel_requested",
                "run_id": handle.id,
            }),
        },
    );
    Ok(true)
}

#[tauri::command]
async fn force_reset_run(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<bool, String> {
    let mut active_run = state.active_run.lock().await;
    let had_active_run = active_run.take().is_some();
    if had_active_run {
        let _ = append_desktop_log(&state.diagnostic_logs_path, "run_force_reset");
        let _ = app.emit(
            "desktop-run-event",
            DesktopRunEvent::RunError {
                message: "Desktop run state was force reset by user".to_string(),
            },
        );
    }
    Ok(had_active_run)
}

fn desktop_stream_event_log(event: &StreamEvent) -> Option<String> {
    match event {
        StreamEvent::Start => Some("stream_event start".to_string()),
        StreamEvent::TextChunk(text) => Some(format!(
            "stream_event text_chunk chars={}",
            text.chars().count()
        )),
        StreamEvent::ThinkingStart => Some("stream_event thinking_start".to_string()),
        StreamEvent::ThinkingChunk(text) => Some(format!(
            "stream_event thinking_chunk chars={}",
            text.chars().count()
        )),
        StreamEvent::ThinkingComplete => Some("stream_event thinking_complete".to_string()),
        StreamEvent::ToolCallStart { id, name } => Some(format!(
            "stream_event tool_call_start id={} name={}",
            sanitize_log_value(id),
            sanitize_log_value(name)
        )),
        StreamEvent::ToolCallArgs { id, args_delta } => Some(format!(
            "stream_event tool_call_args id={} chars={}",
            sanitize_log_value(id),
            args_delta.chars().count()
        )),
        StreamEvent::ToolCallComplete { id } => Some(format!(
            "stream_event tool_call_complete id={}",
            sanitize_log_value(id)
        )),
        StreamEvent::ToolExecutionStart { id, name, .. } => Some(format!(
            "stream_event tool_execution_start id={} name={}",
            sanitize_log_value(id),
            sanitize_log_value(name)
        )),
        StreamEvent::ToolExecutionProgress { id, progress } => Some(format!(
            "stream_event tool_execution_progress id={} chars={}",
            sanitize_log_value(id),
            progress.chars().count()
        )),
        StreamEvent::ToolExecutionComplete { id, result, .. } => Some(format!(
            "stream_event tool_execution_complete id={} chars={}",
            sanitize_log_value(id),
            result.chars().count()
        )),
        StreamEvent::ToolResultsReadyForModel { ids } => Some(format!(
            "stream_event tool_results_ready_for_model count={}",
            ids.len()
        )),
        StreamEvent::PermissionRequest { id, tool_name, .. } => Some(format!(
            "stream_event permission_request id={} tool={}",
            sanitize_log_value(id),
            sanitize_log_value(tool_name)
        )),
        StreamEvent::Usage {
            prompt_tokens,
            completion_tokens,
            reasoning_tokens,
            cached_tokens,
            cache_write_tokens,
        } => Some(format!(
            "stream_event usage prompt_tokens={} completion_tokens={} reasoning_tokens={} cached_tokens={} cache_write_tokens={}",
            prompt_tokens,
            completion_tokens,
            reasoning_tokens
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            cached_tokens
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
            ,
            cache_write_tokens
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        )),
        StreamEvent::RuntimeDiagnostic { diagnostic } => {
            let schema = diagnostic
                .get("schema")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let stage = diagnostic
                .get("stage")
                .or_else(|| diagnostic.pointer("/task_state/stage"))
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            Some(format!(
                "stream_event runtime_diagnostic schema={} stage={}",
                sanitize_log_value(schema),
                sanitize_log_value(stage)
            ))
        }
        StreamEvent::Closeout {
            status,
            evidence_summary,
        } => Some(format!(
            "stream_event closeout status={} evidence_chars={}",
            sanitize_log_value(status),
            evidence_summary
                .as_deref()
                .map(|summary| summary.chars().count())
                .unwrap_or(0)
        )),
        StreamEvent::Complete => Some("stream_event complete".to_string()),
        StreamEvent::OutputTruncated => Some("stream_event output_truncated".to_string()),
        StreamEvent::Error(message) => Some(format!(
            "stream_event error message={}",
            sanitize_log_value(message)
        )),
    }
}

#[tauri::command]
async fn desktop_run_context_detail(
    context: DesktopRunContext,
    state: State<'_, DesktopAppState>,
) -> Result<ResolvedDesktopRunContext, String> {
    let selected_project = state.selected_project.lock().await.clone();
    match context.context_type.as_str() {
        "current_diff" => resolve_current_diff_context(&context, &selected_project),
        "file" => resolve_file_context(&context, &selected_project),
        other => Err(format!("Unsupported desktop run context: {}", other)),
    }
}

#[tauri::command]
async fn compact_context(
    state: State<'_, DesktopAppState>,
) -> Result<Option<priority_agent::engine::context_compressor::CompactionAttemptRecord>, String> {
    let runtime = runtime_for_state(&state).await?;
    Ok(runtime.compact_context().await)
}

#[tauri::command]
async fn desktop_context_snapshot(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopContextSnapshot, String> {
    let runtime = runtime_for_state(&state).await?;
    Ok(runtime.context_snapshot().await)
}

#[tauri::command]
async fn desktop_workbench_snapshot(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopWorkbenchSnapshot, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let project_map = match priority_agent::engine::project_map::load_project_map_zone_with_limit(
        &selected_project,
        8_000,
    ) {
        Some(zone) => DesktopProjectMapSnapshot {
            available: true,
            source: Some(zone.source.display().to_string()),
            freshness: zone.freshness.label(),
            chars: zone.chars,
            truncated: zone.truncated,
            content_preview: zone.content,
        },
        None => DesktopProjectMapSnapshot {
            available: false,
            source: Some(
                selected_project
                    .join(priority_agent::engine::project_map::PROJECT_MAP_PATH)
                    .display()
                    .to_string(),
            ),
            freshness: "missing".to_string(),
            chars: 0,
            truncated: false,
            content_preview: "No docs/PROJECT_MAP.md found for this project.".to_string(),
        },
    };
    let index =
        priority_agent::engine::project_map::build_project_symbol_index(&selected_project, 24, 12);
    let runtime_context = {
        let runtime = state.runtime.lock().await.clone();
        match runtime {
            Some(runtime) => Some(runtime.context_snapshot().await),
            None => None,
        }
    };
    let active_session_id = state.active_session_id.lock().await.clone();
    let subagent_tasks = match open_session_store() {
        Ok(store) => desktop_subagent_tasks_for_session(&store, active_session_id.as_deref()),
        Err(_) => Vec::new(),
    };

    Ok(DesktopWorkbenchSnapshot {
        selected_project: selected_project.display().to_string(),
        project_map,
        symbol_index: DesktopSymbolIndexSnapshot {
            schema_version: index.schema_version,
            total_symbols: index.total_symbols,
            files: index.files,
            truncated: index.truncated,
        },
        runtime_context,
        lab_status: desktop_lab_status_for_project(&selected_project),
        subagent_tasks,
    })
}

#[tauri::command]
async fn answer_permission(
    app: AppHandle,
    approved: bool,
    state: State<'_, DesktopAppState>,
) -> Result<bool, String> {
    let diagnostic_logs_path = state.diagnostic_logs_path.clone();
    if native_smoke_enabled() {
        let mut pending = state.native_smoke_permission_pending.lock().await;
        if !*pending {
            return Ok(false);
        }
        *pending = false;
        let _ = append_desktop_log(
            &diagnostic_logs_path,
            if approved {
                "permission_answer approved=true"
            } else {
                "permission_answer approved=false"
            },
        );
        emit_native_smoke_permission_resolution(app, diagnostic_logs_path, approved).await;
        return Ok(true);
    }

    let runtime = {
        let runtime = state.runtime.lock().await;
        runtime.clone()
    };
    let Some(runtime) = runtime else {
        return Ok(false);
    };
    if runtime.controller().approve_pending(approved).await {
        let _ = append_desktop_log(
            &diagnostic_logs_path,
            if approved {
                "permission_answer approved=true"
            } else {
                "permission_answer approved=false"
            },
        );
        return Ok(true);
    }

    Ok(false)
}

async fn runtime_for_state(state: &State<'_, DesktopAppState>) -> Result<DesktopRuntime, String> {
    if let Some(runtime) = state.runtime.lock().await.clone() {
        return Ok(runtime);
    }

    let selected_project = state.selected_project.lock().await.clone();
    let active_session_id = active_session_id_if_present(state).await?;
    let permission_mode_label =
        normalized_permission_mode_label(state.permission_mode.lock().await.as_deref());
    let permission_mode = parse_desktop_permission_mode(permission_mode_label);
    let provider_name = state.provider_name.lock().await.clone();
    let model = state.model.lock().await.clone();
    let runtime = if let Some(session_id) = active_session_id {
        DesktopRuntime::initialize_for_session(&selected_project, &session_id)
            .await
            .map_err(|err| err.to_string())?
    } else {
        DesktopRuntime::initialize(&selected_project)
            .await
            .map_err(|err| err.to_string())?
    };
    runtime
        .streaming_engine()
        .set_permission_mode(permission_mode);
    apply_desktop_provider_model(&runtime, provider_name.as_deref(), model.as_deref())?;

    let mut stored_runtime = state.runtime.lock().await;
    *stored_runtime = Some(runtime.clone());
    Ok(runtime)
}

pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let settings_path = desktop_settings_path(app.handle());
            let diagnostic_logs_path = desktop_diagnostic_logs_path(app.handle());
            let mut settings = load_desktop_settings(&settings_path).unwrap_or_default();
            let smoke_provider_override =
                std::env::var("PRIORITY_AGENT_DESKTOP_SMOKE_PROVIDER")
                    .ok()
                    .map(|provider| provider.trim().to_ascii_lowercase())
                    .filter(|provider| !provider.is_empty());
            let smoke_model_override = std::env::var("PRIORITY_AGENT_DESKTOP_SMOKE_MODEL")
                .ok()
                .map(|model| model.trim().to_string())
                .filter(|model| !model.is_empty());
            let smoke_agent_mode_override =
                std::env::var("PRIORITY_AGENT_DESKTOP_SMOKE_AGENT_MODE")
                    .ok()
                    .map(|mode| mode.trim().to_ascii_lowercase())
                    .filter(|mode| {
                        matches!(
                            mode.as_str(),
                            "auto" | "build" | "plan" | "explore" | "review"
                        )
                    });
            let live_provider_smoke_requested =
                std::env::var("PRIORITY_AGENT_DESKTOP_LIVE_PROVIDER_SMOKE").as_deref() == Ok("1")
                    || std::env::var("PRIORITY_AGENT_DESKTOP_MULTI_TOOL_SMOKE").as_deref()
                        == Ok("1")
                    || std::env::var("PRIORITY_AGENT_DESKTOP_SOAK_SMOKE").as_deref()
                        == Ok("1")
                    || std::env::var("PRIORITY_AGENT_DESKTOP_EXTENDED_SOAK_SMOKE").as_deref()
                        == Ok("1")
                    || std::env::var("PRIORITY_AGENT_DESKTOP_SOAK_RESTART_SMOKE").as_deref()
                        == Ok("1")
                    || std::env::var("PRIORITY_AGENT_DESKTOP_EXTENDED_SOAK_RESTART_SMOKE")
                        .as_deref()
                        == Ok("1");
            if live_provider_smoke_requested {
                if smoke_provider_override.is_some() {
                    settings.provider_name = smoke_provider_override.clone();
                }
                if smoke_model_override.is_some() {
                    settings.model = smoke_model_override.clone();
                }
                if smoke_agent_mode_override.is_some() {
                    settings.agent_mode = smoke_agent_mode_override.clone();
                }
            }
            let selected_project = initial_desktop_project(cwd.clone(), &settings);
            let recent_projects = initial_recent_projects(&selected_project, &settings);
            let _ = append_desktop_log(
                &diagnostic_logs_path,
                &format!(
                    "desktop_start project={} settings={}",
                    selected_project.display(),
                    settings_path.display()
                ),
            );
            let lab_recovery_smoke_requested =
                std::env::var("PRIORITY_AGENT_DESKTOP_LAB_RECOVERY_SMOKE").as_deref() == Ok("1");
            let lab_recovery_restart_smoke_requested =
                std::env::var("PRIORITY_AGENT_DESKTOP_LAB_RECOVERY_RESTART_SMOKE").as_deref()
                    == Ok("1");
            if lab_recovery_smoke_requested {
                prepare_native_lab_recovery_smoke_project(&selected_project, &diagnostic_logs_path);
            }
            if live_provider_smoke_requested {
                let provider = settings
                    .provider_name
                    .as_deref()
                    .unwrap_or("environment-default");
                let model = settings.model.as_deref().unwrap_or("provider-default");
                let mode = settings.agent_mode.as_deref().unwrap_or("auto");
                let _ = append_desktop_log(
                    &diagnostic_logs_path,
                    &format!(
                        "desktop_live_provider_smoke_config provider={provider} model={model} agent_mode={mode}"
                    ),
                );
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_NATIVE_SMOKE").as_deref() == Ok("1") {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_interaction_smoke(window, diagnostic_logs_path.clone());
                }
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_LIVE_PROVIDER_SMOKE").as_deref() == Ok("1") {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_live_provider_smoke(window, diagnostic_logs_path.clone());
                }
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_MULTI_TOOL_SMOKE").as_deref() == Ok("1") {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_multitool_smoke(window, diagnostic_logs_path.clone());
                }
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_SOAK_SMOKE").as_deref() == Ok("1") {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_soak_smoke(window, diagnostic_logs_path.clone());
                }
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_EXTENDED_SOAK_SMOKE").as_deref() == Ok("1")
            {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_extended_soak_smoke(window, diagnostic_logs_path.clone());
                }
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_SOAK_RESTART_SMOKE").as_deref() == Ok("1") {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_soak_restart_smoke(window, diagnostic_logs_path.clone());
                }
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_EXTENDED_SOAK_RESTART_SMOKE").as_deref()
                == Ok("1")
            {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_extended_soak_restart_smoke(
                        window,
                        diagnostic_logs_path.clone(),
                    );
                }
            }
            if lab_recovery_smoke_requested || lab_recovery_restart_smoke_requested {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_lab_recovery_smoke(window, diagnostic_logs_path.clone());
                }
            }
            if std::env::var("PRIORITY_AGENT_DESKTOP_RESTART_SMOKE").as_deref() == Ok("1") {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_restart_smoke(window, diagnostic_logs_path.clone());
                }
            }
            app.manage(DesktopAppState {
                runtime: Mutex::new(None),
                active_run: Mutex::new(None),
                selected_project: Mutex::new(selected_project),
                active_session_id: Mutex::new(settings.active_session_id),
                permission_mode: Mutex::new(Some(
                    normalized_permission_mode_label(settings.permission_mode.as_deref())
                        .to_string(),
                )),
                detail_level: Mutex::new(Some(
                    normalized_detail_level_label(settings.detail_level.as_deref()).to_string(),
                )),
                agent_mode: Mutex::new(settings.agent_mode),
                provider_name: Mutex::new(settings.provider_name),
                model: Mutex::new(settings.model),
                recent_projects: Mutex::new(recent_projects),
                archived_session_ids: Mutex::new(settings.archived_session_ids.unwrap_or_default()),
                lab_daemon_supervision_enabled: Mutex::new(
                    settings.lab_daemon_supervision_enabled.unwrap_or(false),
                ),
                lab_daemon_last_supervision: Mutex::new(None),
                lab_daemon_last_supervision_result: Mutex::new(None),
                lab_daemon_next_supervision: Mutex::new(None),
                onboarding_state: Mutex::new(settings.onboarding_state),
                workspace_trust: Mutex::new(settings.workspace_trust),
                native_smoke_permission_pending: Mutex::new(false),
                goal_runner: Mutex::new(None),
                settings_path,
                diagnostic_logs_path,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_health,
            desktop_settings,
            set_permission_mode,
            set_detail_level,
            set_agent_mode,
            set_lab_daemon_supervision_enabled,
            complete_desktop_onboarding,
            skip_desktop_onboarding,
            set_workspace_trust,
            reset_workspace_trust,
            permission_mode_options,
            agent_mode_options,
            provider_model_status,
            set_provider_model,
            desktop_diagnostics,
            desktop_credential_storage_status,
            export_desktop_diagnostics_bundle,
            export_session,
            provider_setup_info,
            open_settings_folder,
            open_diagnostics_folder,
            open_shell_profile,
            open_file_path,
            save_provider_credential,
            record_native_smoke_result,
            select_project,
            new_conversation,
            list_recent_sessions,
            search_sessions,
            rename_session,
            archive_session,
            restore_archived_session,
            delete_session,
            load_session_messages,
            resume_session,
            list_session_reverts,
            desktop_tool_output_index,
            desktop_tool_output_page,
            desktop_lab_report_page,
            desktop_lab_artifact_body,
            desktop_file_preview,
            revert_last_turn,
            send_message,
            cancel_run,
            force_reset_run,
            compact_context,
            desktop_context_snapshot,
            desktop_workbench_snapshot,
            lab_daemon_supervise,
            desktop_run_context_detail,
            answer_permission,
            goal_status,
            goal_start,
            goal_pause,
            goal_resume,
            goal_clear,
            goal_edit,
            goal_log,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Priority Agent desktop app");
}

#[cfg(test)]
mod tests;

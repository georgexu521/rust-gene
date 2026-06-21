use futures::StreamExt;
use priority_agent::desktop_runtime::{DesktopContextSnapshot, DesktopRunEvent, DesktopRuntime};
use priority_agent::engine::goal::runner::GoalRunner;
use priority_agent::engine::streaming::StreamEvent;
use priority_agent::engine::turn_ingress::classify_turn_ingress;
use priority_agent::permissions::PermissionMode;
use priority_agent::session_store::SessionStore;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::Mutex;

mod desktop_context;
mod desktop_state;
mod desktop_types;
mod diagnostics;
mod goal_commands;
mod health_commands;
mod preview_commands;
mod revert_commands;
mod session_commands;
use desktop_state::*;
pub(crate) use desktop_context::*;
pub(crate) use desktop_types::*;
use diagnostics::*;
use goal_commands::*;
use health_commands::*;
use preview_commands::*;
use revert_commands::*;
use session_commands::*;

struct DesktopAppState {
    runtime: Mutex<Option<DesktopRuntime>>,
    selected_project: Mutex<PathBuf>,
    active_session_id: Mutex<Option<String>>,
    permission_mode: Mutex<Option<String>>,
    detail_level: Mutex<Option<String>>,
    agent_mode: Mutex<Option<String>>,
    provider_name: Mutex<Option<String>>,
    model: Mutex<Option<String>>,
    recent_projects: Mutex<Vec<PathBuf>>,
    archived_session_ids: Mutex<Vec<String>>,
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
    if !matches!(normalized.as_str(), "auto" | "build" | "plan" | "explore" | "review") {
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
    match value.unwrap_or("markdown").trim().to_ascii_lowercase().as_str() {
        "json" => Ok(priority_agent::session_store::export::SessionExportFormat::Json),
        "md" | "markdown" => Ok(priority_agent::session_store::export::SessionExportFormat::Markdown),
        other => Err(format!("Unsupported export format: {other}")),
    }
}

fn parse_desktop_export_privacy(
    value: Option<&str>,
) -> Result<priority_agent::session_store::export::SessionExportPrivacy, String> {
    match value.unwrap_or("redacted").trim().to_ascii_lowercase().as_str() {
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
async fn open_settings_folder(state: State<'_, DesktopAppState>) -> Result<(), String> {
    let folder = state
        .settings_path
        .parent()
        .ok_or_else(|| "settings path has no parent directory".to_string())?;
    open_path(folder)
}

#[tauri::command]
async fn open_diagnostics_folder(state: State<'_, DesktopAppState>) -> Result<(), String> {
    let folder = state
        .diagnostic_logs_path
        .parent()
        .ok_or_else(|| "diagnostic log path has no parent directory".to_string())?;
    std::fs::create_dir_all(folder).map_err(|err| err.to_string())?;
    open_path(folder)
}

#[tauri::command]
async fn open_file_path(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        if parent.exists() {
            return open_path(parent);
        }
    }
    Err(format!("path does not exist: {}", path))
}

#[tauri::command]
async fn open_shell_profile() -> Result<(), String> {
    let profile = shell_profile_path();
    if !profile.exists() {
        if let Some(parent) = profile.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        std::fs::write(&profile, "").map_err(|err| err.to_string())?;
    }
    open_path(&profile)
}

#[tauri::command]
async fn save_provider_credential(provider_id: String, key: String) -> Result<String, String> {
    match priority_agent::services::api::credentials::save_credential(&provider_id, &key) {
        priority_agent::services::api::credentials::CredentialSaveOutcome::Verified => {
            Ok(format!("Saved and activated key for {}", provider_id))
        }
        priority_agent::services::api::credentials::CredentialSaveOutcome::SavedUnverified => {
            Ok(format!(
                "Saved key for {}, but provider activation could not be verified",
                provider_id
            ))
        }
        priority_agent::services::api::credentials::CredentialSaveOutcome::Rejected { reason } => {
            Err(reason)
        }
    }
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
        &format!("{smoke_name} {status} result={}", sanitize_log_value(&result)),
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
    let agent_mode_label = state
        .agent_mode
        .lock()
        .await
        .clone()
        .unwrap_or_else(|| "auto".to_string());
    let agent_mode = priority_agent::engine::agent_mode::AgentMode::parse(&agent_mode_label)
        .unwrap_or_default();
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

    let runtime = runtime_for_state(&state).await?;
    let ingress_lane = classify_turn_ingress(&message, !contexts.is_empty());
    let force_full_agent_lane =
        agent_mode != priority_agent::engine::agent_mode::AgentMode::Auto;
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
            persist_current_settings(&state).await?;
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
        persist_current_settings(&state).await?;
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
        let mut desktop_event = DesktopRunEvent::from_stream_event(event);
        if let DesktopRunEvent::RunStarted { session_id, .. } = &mut desktop_event {
            *session_id = active_session_id.clone();
            if active_session_id.is_some() {
                {
                    let mut stored_session_id = state.active_session_id.lock().await;
                    *stored_session_id = active_session_id.clone();
                }
                persist_current_settings(&state).await?;
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
async fn lab_daemon_supervise(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopLabDaemonActionResult, String> {
    let selected_project = state.selected_project.lock().await.clone();
    Ok(desktop_lab_daemon_supervise_for_project(&selected_project))
}

fn desktop_lab_daemon_supervise_for_project(
    project: &std::path::Path,
) -> DesktopLabDaemonActionResult {
    let output =
        priority_agent::lab::commands::handle_lab_command(project, None, "daemon service supervise");
    DesktopLabDaemonActionResult {
        action: "supervise",
        output,
        lab_status: desktop_lab_status_for_project(project),
    }
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

fn native_smoke_enabled() -> bool {
    std::env::var("PRIORITY_AGENT_DESKTOP_NATIVE_SMOKE").as_deref() == Ok("1")
}

async fn emit_native_smoke_run_fixture(
    app: AppHandle,
    message: String,
    state: State<'_, DesktopAppState>,
) -> Result<(), String> {
    {
        let mut pending = state.native_smoke_permission_pending.lock().await;
        *pending = true;
    }
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let session_id = state.active_session_id.lock().await.clone();
    let events = vec![
        DesktopRunEvent::RunStarted {
            run_id: "native-smoke-run".to_string(),
            session_id,
        },
        DesktopRunEvent::ThinkingStarted,
        DesktopRunEvent::ThinkingCompleted,
        DesktopRunEvent::ToolStarted {
            id: "native-smoke-validation".to_string(),
            name: "bash".to_string(),
        },
        DesktopRunEvent::ToolExecutionProgress {
            id: "native-smoke-validation".to_string(),
            progress: "Running native validation fixture".to_string(),
        },
        DesktopRunEvent::ToolCompleted {
            id: "native-smoke-validation".to_string(),
            result_preview: "native smoke validation passed".to_string(),
            metadata: Some(serde_json::json!({
                "tool": "bash",
                "call_id": "native-smoke-validation",
                "success": true,
                "command": "scripts/desktop-native-smoke.sh --fixture-run",
                "command_category": "validation",
                "validation_family": "native_smoke",
                "command_kind": "script",
                "duration_ms": 410,
                "output_chars": 30,
                "terminal_task": {
                    "status": "completed",
                    "exit_code": 0,
                    "duration_ms": 410
                }
            })),
        },
        DesktopRunEvent::ToolStarted {
            id: "native-smoke-file".to_string(),
            name: "file_edit".to_string(),
        },
        DesktopRunEvent::ToolCompleted {
            id: "native-smoke-file".to_string(),
            result_preview: "Edited apps/desktop/src/app/Composer.tsx".to_string(),
            metadata: Some(serde_json::json!({
                "tool": "file_edit",
                "call_id": "native-smoke-file",
                "success": true,
                "path": "apps/desktop/src/app/Composer.tsx",
                "replacements": 1,
                "additions": 4,
                "deletions": 1,
                "diff_preview": "@@ -140,6 +140,9 @@\n <textarea aria-label=\"Message\" />\n+<button aria-label=\"Send message\" />\n",
                "diff_preview_truncated": false,
                "duration_ms": 55,
                "output_chars": 48
            })),
        },
        DesktopRunEvent::PermissionRequest {
            id: "native-smoke-permission".to_string(),
            tool_name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "git status --short"
            }),
            prompt: format!("Allow native smoke permission check for: {message}"),
            metadata: Some(serde_json::json!({
                "permission_evidence": {
                    "schema": "permission_decision_evidence.v1",
                    "request_kind": "runtime_rule",
                    "permission_family": "shell",
                    "decision": "ask",
                    "risk_level": "low",
                    "recovery": {
                        "recommended_action": "Approve once to continue the native smoke run."
                    },
                    "command_classification": {
                        "parser_status": "simple",
                        "category": "git",
                        "mutation": false
                    }
                },
                "action_review": {
                    "schema": "action_review.v1",
                    "tool": "bash",
                    "call_id": "native-smoke-permission",
                    "decision": "ask_user",
                    "primary_reason": "permission_required",
                    "permission": {
                        "allowed_by_context": true,
                        "requires_confirmation": true,
                        "decision": "Ask",
                        "risk_level": "Low",
                        "confidence": 0.82,
                        "warnings": []
                    },
                    "scope": {
                        "allowed": true,
                        "reason": "native smoke request is inside the selected project"
                    },
                    "budget": {
                        "allowed": true,
                        "scheduled_count": 0,
                        "max_tool_calls": 4,
                        "reason": "tool-call budget still has room"
                    },
                    "checkpoint": {
                        "required": false,
                        "status": "not_needed",
                        "enforcement": "none",
                        "rollback_scope": "none",
                        "requires_user_approval": false,
                        "reason": "git status is observational"
                    },
                    "side_effects": {
                        "schema": "action_side_effect_profile.v1",
                        "external_side_effect": "none",
                        "network": {
                            "class": "none",
                            "target": null,
                            "trusted": true,
                            "reason": "no network access detected"
                        },
                        "mutates_local_workspace": false,
                        "mutates_local_machine": false,
                        "remote_side_effect": false,
                        "paths": [],
                        "summary": "external_effect=None network=None paths=0"
                    },
                    "user_reason": "Action requires user confirmation before execution: permission_required.",
                    "model_recovery": "Action needs user approval before execution: permission_required. Wait for the permission result and do not claim the tool ran until it succeeds."
                }
            })),
            review: None,
        },
    ];

    for event in events {
        app.emit("desktop-run-event", event)
            .map_err(|err| err.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(75)).await;
    }
    let _ = append_desktop_log(
        &state.diagnostic_logs_path,
        "native_smoke_fixture permission_request=true",
    );
    Ok(())
}

async fn emit_native_smoke_permission_resolution(
    app: AppHandle,
    diagnostic_logs_path: PathBuf,
    approved: bool,
) {
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    if approved {
        let Some(window) = app.get_webview_window("main") else {
            let _ = append_desktop_log(
                &diagnostic_logs_path,
                "native_smoke_fixture emit_error=missing main window",
            );
            return;
        };
        let events = vec![
            DesktopRunEvent::ToolCompleted {
                id: "native-smoke-permission-result".to_string(),
                result_preview: "Permission approved; inspected git status".to_string(),
                metadata: Some(serde_json::json!({
                    "tool": "bash",
                    "call_id": "native-smoke-permission-result",
                    "success": true,
                    "command": "git status --short",
                    "command_category": "inspection",
                    "command_kind": "git",
                    "duration_ms": 75,
                    "output_chars": 12,
                    "terminal_task": {
                        "status": "completed",
                        "exit_code": 0,
                        "duration_ms": 75
                    }
                })),
            },
            DesktopRunEvent::AssistantDelta {
                text: "Native smoke fixture completed. Timeline cards, permission approval, and final answer rendering are visible.".to_string(),
            },
            DesktopRunEvent::RuntimeDiagnostic {
                diagnostic: serde_json::json!({
                    "schema": "desktop_runtime_diagnostic.v1",
                    "task_state": {
                        "goal": "native smoke fixture",
                        "mode": "full",
                        "stage": "closeout",
                        "mode_score": {
                            "confidence": 82,
                            "complexity": 7,
                            "risk": 5,
                            "uncertainty": 3,
                            "tool_need": 8,
                            "user_impact": 7
                        },
                        "lightweight_plan": null,
                        "verification": {
                            "status": "verified",
                            "required_checks": ["scripts/desktop-native-smoke.sh --fixture-run"]
                        },
                        "done": {
                            "satisfied": true,
                            "summary": "native smoke fixture completed"
                        },
                        "active_files": ["apps/desktop/src/app/Composer.tsx"],
                        "stop_check": {
                            "status": "stop",
                            "reason": "verification_ready",
                            "summary": "ready for closeout"
                        }
                    },
                    "verification_proof": {
                        "status": "verified",
                        "summary": "native smoke validation passed",
                        "closeout_status": "passed",
                        "changed_files": 1,
                        "validation_items": 1,
                        "acceptance_items": 1,
                        "residual_risks": 0
                    },
                    "control_loop": {
                        "coverage": "7/7",
                        "summary": "native smoke runtime diagnostic",
                        "phases": [
                            { "phase": "context", "events": 1, "latest_label": "task.context" },
                            { "phase": "decision", "events": 1, "latest_label": "action.decision" },
                            { "phase": "permission", "events": 1, "latest_label": "permission.resolve" },
                            { "phase": "tool_execution", "events": 3, "latest_label": "tool.done" },
                            { "phase": "state_update", "events": 1, "latest_label": "stop.check" },
                            { "phase": "verification", "events": 1, "latest_label": "verify.done" },
                            { "phase": "closeout", "events": 2, "latest_label": "assistant" }
                        ]
                    }
                }),
            },
            DesktopRunEvent::Usage {
                prompt_tokens: 32,
                completion_tokens: 18,
                reasoning_tokens: Some(4),
                cached_tokens: Some(8),
                cache_write_tokens: Some(12),
            },
            DesktopRunEvent::RunCompleted,
        ];
        for event in events {
            if let Err(err) = window.emit("desktop-run-event", event) {
                let _ = append_desktop_log(
                    &diagnostic_logs_path,
                    &format!(
                        "native_smoke_fixture emit_error={}",
                        sanitize_log_value(&err.to_string())
                    ),
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(75)).await;
        }
        let _ = append_desktop_log(&diagnostic_logs_path, "run_completed");
    } else {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.emit(
                "desktop-run-event",
                DesktopRunEvent::RunError {
                    message: "Native smoke permission rejected".to_string(),
                },
            );
        }
        let _ = append_desktop_log(
            &diagnostic_logs_path,
            "run_error message=Native smoke permission rejected",
        );
    }
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
            permission_mode_options,
            agent_mode_options,
            provider_model_status,
            set_provider_model,
            desktop_diagnostics,
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

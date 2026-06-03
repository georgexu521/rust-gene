use futures::StreamExt;
use priority_agent::desktop_runtime::{DesktopContextSnapshot, DesktopRunEvent, DesktopRuntime};
use priority_agent::engine::streaming::StreamEvent;
use priority_agent::engine::turn_ingress::classify_turn_ingress;
use priority_agent::permissions::PermissionMode;
use priority_agent::session_store::SessionStore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::Mutex;

struct DesktopAppState {
    runtime: Mutex<Option<DesktopRuntime>>,
    selected_project: Mutex<PathBuf>,
    active_session_id: Mutex<Option<String>>,
    permission_mode: Mutex<Option<String>>,
    detail_level: Mutex<Option<String>>,
    provider_name: Mutex<Option<String>>,
    model: Mutex<Option<String>>,
    recent_projects: Mutex<Vec<PathBuf>>,
    archived_session_ids: Mutex<Vec<String>>,
    native_smoke_permission_pending: Mutex<bool>,
    settings_path: PathBuf,
    diagnostic_logs_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct DesktopHealth {
    status: &'static str,
    version: &'static str,
    cwd: String,
}

#[derive(Debug, Serialize)]
struct SelectedProject {
    path: String,
}

#[derive(Debug, Serialize)]
struct RecentSession {
    id: String,
    title: String,
    updated_at: String,
    model: String,
    message_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DesktopMessage {
    id: i64,
    role: String,
    content: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct ResumedSession {
    session_id: String,
    messages: Vec<DesktopMessage>,
    compact_boundaries: Vec<DesktopCompactBoundary>,
}

#[derive(Debug, Serialize)]
struct DesktopCompactBoundary {
    boundary_id: String,
    strategy: String,
    trigger: String,
    before_tokens: i64,
    after_tokens: i64,
    messages_before: i64,
    messages_after: i64,
    summary: String,
    created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DesktopRunContext {
    #[serde(rename = "type")]
    context_type: String,
    label: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResolvedDesktopRunContext {
    #[serde(rename = "type")]
    context_type: String,
    label: String,
    shortstat: String,
    files: Vec<String>,
    stat: String,
    #[serde(rename = "patch_preview")]
    patch_preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relative_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<String>,
    truncated: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct DesktopSettings {
    selected_project: Option<String>,
    active_session_id: Option<String>,
    permission_mode: Option<String>,
    detail_level: Option<String>,
    provider_name: Option<String>,
    model: Option<String>,
    recent_projects: Option<Vec<String>>,
    archived_session_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct DesktopSettingsResponse {
    selected_project: String,
    active_session_id: Option<String>,
    permission_mode: String,
    detail_level: String,
    provider_name: Option<String>,
    model: Option<String>,
    settings_path: String,
    diagnostic_logs_path: String,
    recent_projects: Vec<String>,
    archived_session_ids: Vec<String>,
    startup_state: DesktopStartupState,
}

#[derive(Debug, Serialize)]
struct DesktopStartupState {
    status: &'static str,
    detail: String,
}

#[derive(Debug, Serialize)]
struct PermissionModeOption {
    id: &'static str,
    label: &'static str,
    description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopDiagnostic {
    id: &'static str,
    label: &'static str,
    status: DiagnosticStatus,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
struct DesktopDiagnosticsResponse {
    items: Vec<DesktopDiagnostic>,
}

#[derive(Debug, Serialize)]
struct ProviderSetupInfo {
    shell_profile_path: String,
    provider_env_vars: Vec<&'static str>,
    example: &'static str,
}

#[derive(Debug, Serialize)]
struct ProviderModelStatus {
    active_provider: Option<String>,
    active_model: String,
    configured_count: usize,
    providers: Vec<DesktopProviderOption>,
    models: Vec<DesktopModelOption>,
}

#[derive(Debug, Serialize)]
struct DesktopWorkbenchSnapshot {
    selected_project: String,
    project_map: DesktopProjectMapSnapshot,
    symbol_index: DesktopSymbolIndexSnapshot,
    runtime_context: Option<DesktopContextSnapshot>,
}

#[derive(Debug, Serialize)]
struct DesktopProjectMapSnapshot {
    available: bool,
    source: Option<String>,
    freshness: String,
    chars: usize,
    truncated: bool,
    content_preview: String,
}

#[derive(Debug, Serialize)]
struct DesktopSymbolIndexSnapshot {
    schema_version: u8,
    total_symbols: usize,
    files: Vec<priority_agent::engine::project_map::ProjectIndexedFile>,
    truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopProviderOption {
    id: String,
    label: String,
    provider_type: String,
    model: String,
    base_url: String,
    configured: bool,
    active: bool,
    note: String,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopModelOption {
    id: String,
    label: String,
    provider_id: String,
    active: bool,
    note: String,
}

#[tauri::command]
fn desktop_health() -> Result<DesktopHealth, String> {
    let cwd = std::env::current_dir()
        .map_err(|err| err.to_string())?
        .canonicalize()
        .map_err(|err| err.to_string())?;

    Ok(desktop_health_value(default_desktop_project(cwd)))
}

#[tauri::command]
async fn desktop_settings(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let active_session_id = state.active_session_id.lock().await.clone();
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
async fn record_native_smoke_result(
    result: String,
    state: State<'_, DesktopAppState>,
) -> Result<(), String> {
    let diagnostic_logs_path = state.diagnostic_logs_path.clone();
    let ok = result.contains("native_interaction_smoke ok=true");
    let status = if ok { "ok=true" } else { "ok=false" };
    append_desktop_log(
        &diagnostic_logs_path,
        &format!(
            "native_interaction_smoke {status} result={}",
            sanitize_log_value(&result)
        ),
    )
}

#[tauri::command]
async fn select_project(
    path: String,
    state: State<'_, DesktopAppState>,
) -> Result<SelectedProject, String> {
    let project = validate_project_path(path)?;
    {
        let mut selected_project = state.selected_project.lock().await;
        *selected_project = project.clone();
    }
    {
        let mut recent_projects = state.recent_projects.lock().await;
        remember_recent_project(&mut recent_projects, project.clone());
    }
    {
        let mut runtime = state.runtime.lock().await;
        *runtime = None;
    }
    {
        let mut active_session_id = state.active_session_id.lock().await;
        *active_session_id = None;
    }
    persist_current_settings(&state).await?;

    Ok(selected_project_response(project))
}

#[tauri::command]
async fn new_conversation(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    {
        let mut runtime = state.runtime.lock().await;
        *runtime = None;
    }
    {
        let mut active_session_id = state.active_session_id.lock().await;
        *active_session_id = None;
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
async fn list_recent_sessions(
    limit: Option<i64>,
    state: State<'_, DesktopAppState>,
) -> Result<Vec<RecentSession>, String> {
    let store = open_session_store()?;
    let archived_session_ids = state.archived_session_ids.lock().await.clone();
    list_recent_sessions_from_store(&store, limit.unwrap_or(20), &archived_session_ids)
}

#[tauri::command]
async fn search_sessions(
    query: String,
    limit: Option<i64>,
    state: State<'_, DesktopAppState>,
) -> Result<Vec<RecentSession>, String> {
    let store = open_session_store()?;
    let archived_session_ids = state.archived_session_ids.lock().await.clone();
    search_sessions_from_store(&store, &query, limit.unwrap_or(20), &archived_session_ids)
}

#[tauri::command]
fn rename_session(session_id: String, title: String) -> Result<RecentSession, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("session title cannot be empty".to_string());
    }

    let store = open_session_store()?;
    store
        .get_session(&session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;
    store
        .update_session_title(&session_id, title)
        .map_err(|err| err.to_string())?;
    recent_session_from_store(&store, &session_id)
}

#[tauri::command]
async fn archive_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    {
        let mut archived_session_ids = state.archived_session_ids.lock().await;
        if !archived_session_ids.iter().any(|id| id == &session_id) {
            archived_session_ids.push(session_id.clone());
        }
    }
    clear_active_session_if_matches(&state, &session_id).await;
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
async fn restore_archived_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    {
        let mut archived_session_ids = state.archived_session_ids.lock().await;
        archived_session_ids.retain(|id| id != &session_id);
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
async fn delete_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let store = open_session_store()?;
    store
        .delete_session(&session_id)
        .map_err(|err| err.to_string())?;
    {
        let mut archived_session_ids = state.archived_session_ids.lock().await;
        archived_session_ids.retain(|id| id != &session_id);
    }
    clear_active_session_if_matches(&state, &session_id).await;
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
fn load_session_messages(session_id: String) -> Result<Vec<DesktopMessage>, String> {
    let store = open_session_store()?;
    load_messages_from_store(&store, &session_id)
}

#[tauri::command]
async fn resume_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<ResumedSession, String> {
    let store = open_session_store()?;
    store
        .get_session(&session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;
    let messages = load_messages_from_store(&store, &session_id)?;
    let compact_boundaries = load_compact_boundaries_from_store(&store, &session_id)?;
    let selected_project = state.selected_project.lock().await.clone();
    let runtime = DesktopRuntime::initialize_for_session(&selected_project, &session_id)
        .await
        .map_err(|err| err.to_string())?;
    let permission_mode_label =
        normalized_permission_mode_label(state.permission_mode.lock().await.as_deref());
    runtime
        .streaming_engine()
        .set_permission_mode(parse_desktop_permission_mode(permission_mode_label));
    let provider_name = state.provider_name.lock().await.clone();
    let model = state.model.lock().await.clone();
    apply_desktop_provider_model(&runtime, provider_name.as_deref(), model.as_deref())?;

    {
        let mut stored_runtime = state.runtime.lock().await;
        *stored_runtime = Some(runtime);
    }
    {
        let mut active_session_id = state.active_session_id.lock().await;
        *active_session_id = Some(session_id.clone());
    }
    persist_current_settings(&state).await?;

    Ok(ResumedSession {
        session_id,
        messages,
        compact_boundaries,
    })
}

fn desktop_health_value(cwd: PathBuf) -> DesktopHealth {
    DesktopHealth {
        status: "ready",
        version: env!("CARGO_PKG_VERSION"),
        cwd: cwd.display().to_string(),
    }
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
    if ingress_lane.is_lightweight() {
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

    let mut stream = runtime.run_full_turn(message).await;
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
        } => Some(format!(
            "stream_event usage prompt_tokens={} completion_tokens={} reasoning_tokens={} cached_tokens={}",
            prompt_tokens,
            completion_tokens,
            reasoning_tokens
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            cached_tokens
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
    })
}

fn enrich_message_with_desktop_contexts(
    message: String,
    contexts: &[DesktopRunContext],
    project: &Path,
) -> Result<String, String> {
    if contexts.is_empty() {
        return Ok(message);
    }

    let mut blocks = Vec::new();
    for context in contexts {
        match context.context_type.as_str() {
            "current_diff" => {
                let resolved = resolve_current_diff_context(context, project)?;
                blocks.push(format_desktop_context_block(&resolved));
            }
            "file" => {
                let resolved = resolve_file_context(context, project)?;
                blocks.push(format_desktop_context_block(&resolved));
            }
            other => {
                return Err(format!("Unsupported desktop run context: {}", other));
            }
        }
    }

    Ok(format!("{}\n\n{}", message.trim_end(), blocks.join("\n\n")))
}

fn resolve_current_diff_context(
    context: &DesktopRunContext,
    project: &Path,
) -> Result<ResolvedDesktopRunContext, String> {
    let unstaged_shortstat = run_git(project, &["diff", "--shortstat"])?;
    let staged_shortstat = run_git(project, &["diff", "--cached", "--shortstat"])?;
    let unstaged_stat = run_git(project, &["diff", "--stat", "--find-renames"])?;
    let staged_stat = run_git(project, &["diff", "--cached", "--stat", "--find-renames"])?;
    let unstaged_files = run_git(project, &["diff", "--name-only"])?;
    let staged_files = run_git(project, &["diff", "--cached", "--name-only"])?;
    let unstaged_patch = run_git(project, &["diff", "--no-ext-diff", "--find-renames"])?;
    let staged_patch = run_git(
        project,
        &["diff", "--cached", "--no-ext-diff", "--find-renames"],
    )?;

    let shortstat = join_non_empty(&[
        label_section("unstaged", unstaged_shortstat.trim()),
        label_section("staged", staged_shortstat.trim()),
    ])
    .unwrap_or_else(|| "No staged or unstaged git diff detected.".to_string());
    let stat = join_non_empty(&[
        label_section("unstaged", unstaged_stat.trim()),
        label_section("staged", staged_stat.trim()),
    ])
    .unwrap_or_else(|| "No changed files detected.".to_string());
    let files = collect_diff_files(&unstaged_files, &staged_files);
    let patch = join_non_empty(&[
        label_section("unstaged", unstaged_patch.trim()),
        label_section("staged", staged_patch.trim()),
    ])
    .unwrap_or_default();
    let (patch_preview, truncated) = truncate_chars(&patch, 12_000);

    Ok(ResolvedDesktopRunContext {
        context_type: context.context_type.clone(),
        label: context
            .label
            .clone()
            .unwrap_or_else(|| "Current diff".to_string()),
        shortstat,
        files,
        stat,
        patch_preview,
        path: None,
        relative_path: None,
        size_bytes: None,
        line_count: None,
        preview: None,
        truncated,
    })
}

fn resolve_file_context(
    context: &DesktopRunContext,
    project: &Path,
) -> Result<ResolvedDesktopRunContext, String> {
    let raw_path = context
        .path
        .as_ref()
        .ok_or_else(|| "File context requires a path.".to_string())?;
    let requested_path = PathBuf::from(raw_path);
    let file_path = if requested_path.is_absolute() {
        requested_path
    } else {
        project.join(requested_path)
    };
    let project_root = project
        .canonicalize()
        .map_err(|err| format!("Failed to resolve selected project: {}", err))?;
    let file_path = file_path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve file context path: {}", err))?;

    if !file_path.starts_with(&project_root) {
        return Err("File context must be inside the selected project.".to_string());
    }
    if !file_path.is_file() {
        return Err("File context path is not a file.".to_string());
    }

    let bytes =
        std::fs::read(&file_path).map_err(|err| format!("Failed to read file context: {}", err))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let line_count = text.lines().count();
    let (preview, truncated) = truncate_chars(&text, 12_000);
    let relative_path = file_path
        .strip_prefix(&project_root)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();
    let label = context.label.clone().unwrap_or_else(|| {
        file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("File")
            .to_string()
    });

    Ok(ResolvedDesktopRunContext {
        context_type: context.context_type.clone(),
        label,
        shortstat: format!(
            "{} ({} bytes, {} lines)",
            relative_path,
            bytes.len(),
            line_count
        ),
        files: vec![relative_path.clone()],
        stat: format!(
            "{} | {} bytes | {} lines",
            relative_path,
            bytes.len(),
            line_count
        ),
        patch_preview: String::new(),
        path: Some(file_path.to_string_lossy().to_string()),
        relative_path: Some(relative_path),
        size_bytes: Some(bytes.len() as u64),
        line_count: Some(line_count),
        preview: Some(preview),
        truncated,
    })
}

fn format_desktop_context_block(context: &ResolvedDesktopRunContext) -> String {
    if context.context_type == "file" {
        let relative_path = context.relative_path.as_deref().unwrap_or(&context.label);
        let preview = context
            .preview
            .as_deref()
            .filter(|preview| !preview.is_empty())
            .unwrap_or("No file preview available.");
        let truncated = if context.truncated { "true" } else { "false" };

        return format!(
            "<desktop_context type=\"{}\" label=\"{}\">\nPath: {}\nSize bytes: {}\nLines: {}\nPreview truncated: {}\n```text\n{}\n```\n</desktop_context>",
            escape_context_attr(&context.context_type),
            escape_context_attr(&context.label),
            relative_path,
            context.size_bytes.unwrap_or_default(),
            context.line_count.unwrap_or_default(),
            truncated,
            preview
        );
    }

    let files = if context.files.is_empty() {
        "- No changed files detected.".to_string()
    } else {
        context
            .files
            .iter()
            .map(|file| format!("- {}", file))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let patch_preview = if context.patch_preview.is_empty() {
        "No diff preview available.".to_string()
    } else {
        context.patch_preview.clone()
    };
    let truncated = if context.truncated { "true" } else { "false" };

    format!(
        "<desktop_context type=\"{}\" label=\"{}\">\nSummary:\n{}\n\nFiles:\n{}\n\nStat:\n{}\n\nPatch preview truncated: {}\n```diff\n{}\n```\n</desktop_context>",
        escape_context_attr(&context.context_type),
        escape_context_attr(&context.label),
        context.shortstat,
        files,
        context.stat,
        truncated,
        patch_preview
    )
}

fn run_git(project: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .output()
        .map_err(|err| format!("Failed to run git {}: {}", args.join(" "), err))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "git {} failed{}",
            args.join(" "),
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", stderr)
            }
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn collect_diff_files(unstaged: &str, staged: &str) -> Vec<String> {
    let mut files = unstaged
        .lines()
        .chain(staged.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files
}

fn join_non_empty(parts: &[Option<String>]) -> Option<String> {
    let joined = parts
        .iter()
        .filter_map(|part| part.as_ref())
        .filter(|part| !part.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n");
    if joined.trim().is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn label_section(label: &str, text: &str) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(format!("{}:\n{}", label, text.trim()))
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
    let mut iter = text.chars();
    let preview = iter.by_ref().take(max_chars).collect::<String>();
    let truncated = iter.next().is_some();
    (preview, truncated)
}

fn escape_context_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
    let Some(channel) = runtime.streaming_engine().approval_channel() else {
        return Ok(false);
    };

    if let Some((_request, tx)) = channel.take_pending().await {
        let response = if approved {
            priority_agent::engine::conversation_loop::ToolApprovalResponse::approved_once()
        } else {
            priority_agent::engine::conversation_loop::ToolApprovalResponse::rejected_once()
        };
        if tx.send(response).is_err() {
            let _ = append_desktop_log(&diagnostic_logs_path, "permission_answer stale=true");
            return Ok(false);
        }
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
    let active_session_id = state.active_session_id.lock().await.clone();
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

fn open_session_store() -> Result<SessionStore, String> {
    SessionStore::open(SessionStore::default_path()).map_err(|err| err.to_string())
}

async fn clear_active_session_if_matches(state: &State<'_, DesktopAppState>, session_id: &str) {
    let should_clear = state
        .active_session_id
        .lock()
        .await
        .as_deref()
        .is_some_and(|active| active == session_id);
    if !should_clear {
        return;
    }

    {
        let mut runtime = state.runtime.lock().await;
        *runtime = None;
    }
    {
        let mut active_session_id = state.active_session_id.lock().await;
        *active_session_id = None;
    }
}

async fn persist_current_settings(state: &State<'_, DesktopAppState>) -> Result<(), String> {
    let selected_project = state.selected_project.lock().await.clone();
    let active_session_id = state.active_session_id.lock().await.clone();
    let permission_mode =
        normalized_permission_mode_label(state.permission_mode.lock().await.as_deref()).to_string();
    let detail_level =
        normalized_detail_level_label(state.detail_level.lock().await.as_deref()).to_string();
    let provider_name = state.provider_name.lock().await.clone();
    let model = state.model.lock().await.clone();
    let recent_projects = state
        .recent_projects
        .lock()
        .await
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    let archived_session_ids = state.archived_session_ids.lock().await.clone();
    write_desktop_settings(
        &state.settings_path,
        &DesktopSettings {
            selected_project: Some(selected_project.display().to_string()),
            active_session_id,
            permission_mode: Some(permission_mode),
            detail_level: Some(detail_level),
            provider_name,
            model,
            recent_projects: Some(recent_projects),
            archived_session_ids: Some(archived_session_ids),
        },
    )
}

fn load_desktop_settings(path: &PathBuf) -> Result<DesktopSettings, String> {
    if !path.exists() {
        return Ok(DesktopSettings::default());
    }

    let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

fn write_desktop_settings(path: &PathBuf, settings: &DesktopSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let text = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    std::fs::write(path, text).map_err(|err| err.to_string())
}

fn initial_desktop_project(cwd: PathBuf, settings: &DesktopSettings) -> PathBuf {
    let configured_project = configured_desktop_project();
    let default_project = configured_project
        .clone()
        .unwrap_or_else(|| discover_project_root(&cwd).unwrap_or(cwd));
    if configured_project.is_some() {
        return default_project;
    }

    settings
        .selected_project
        .as_deref()
        .and_then(|path| validate_project_path(path).ok())
        .map(|project| migrate_accidental_desktop_subdir(project, &default_project))
        .unwrap_or(default_project)
}

fn default_desktop_project(cwd: PathBuf) -> PathBuf {
    if let Some(project) = configured_desktop_project() {
        return project;
    }

    discover_project_root(&cwd).unwrap_or(cwd)
}

fn configured_desktop_project() -> Option<PathBuf> {
    std::env::var("PRIORITY_AGENT_DESKTOP_PROJECT_DIR")
        .ok()
        .and_then(|path| validate_project_path(path).ok())
}

fn discover_project_root(start: &Path) -> Option<PathBuf> {
    let start = start.canonicalize().ok()?;
    start
        .ancestors()
        .find(|path| path.join(".git").exists() && path.join("Cargo.toml").exists())
        .map(PathBuf::from)
}

fn migrate_accidental_desktop_subdir(project: PathBuf, default_project: &Path) -> PathBuf {
    let Ok(relative) = project.strip_prefix(default_project) else {
        return project;
    };
    if relative == Path::new("apps/desktop") || relative == Path::new("apps/desktop/src-tauri") {
        default_project.to_path_buf()
    } else {
        project
    }
}

fn initial_recent_projects(selected_project: &Path, settings: &DesktopSettings) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    remember_recent_project(&mut projects, selected_project.to_path_buf());

    for project in settings
        .recent_projects
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|path| validate_project_path(path).ok())
    {
        remember_recent_project(&mut projects, project);
    }

    projects
}

fn remember_recent_project(projects: &mut Vec<PathBuf>, project: PathBuf) {
    projects.retain(|existing| existing != &project);
    projects.insert(0, project);
    projects.truncate(8);
}

fn desktop_startup_state(project: &Path, active_session_id: Option<&str>) -> DesktopStartupState {
    if let Some(session_id) = active_session_id {
        DesktopStartupState {
            status: "restored_session",
            detail: format!(
                "Restored {} in {}",
                session_id,
                project
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("selected project")
            ),
        }
    } else {
        DesktopStartupState {
            status: "new_conversation",
            detail: format!(
                "Ready for a new conversation in {}",
                project
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("selected project")
            ),
        }
    }
}

fn desktop_permission_mode_options() -> Vec<PermissionModeOption> {
    vec![
        PermissionModeOption {
            id: "default",
            label: "Ask every time",
            description: "Ask before tool actions that require approval.",
        },
        PermissionModeOption {
            id: "auto_low_risk",
            label: "Auto low risk",
            description: "Allow low-risk read/search actions and ask for writes.",
        },
        PermissionModeOption {
            id: "auto",
            label: "Developer auto",
            description: "Allow normal development actions while guarding high-risk operations.",
        },
        PermissionModeOption {
            id: "read_only",
            label: "Read only",
            description: "Hide write tools and only allow read-oriented work.",
        },
    ]
}

fn normalized_permission_mode_label(mode: Option<&str>) -> &'static str {
    match mode.unwrap_or("auto").trim() {
        "default" | "ask" | "ask_each_time" => "default",
        "auto_low_risk" | "autolowrisk" | "low_risk" => "auto_low_risk",
        "auto" | "auto_all" | "developer_auto" => "auto",
        "read_only" | "readonly" => "read_only",
        _ => "auto",
    }
}

fn normalized_detail_level_label(level: Option<&str>) -> &'static str {
    match level.unwrap_or("coding").trim() {
        "default" | "daily" | "daily_work" => "daily",
        _ => "coding",
    }
}

fn parse_desktop_permission_mode(mode: &str) -> PermissionMode {
    match normalized_permission_mode_label(Some(mode)) {
        "default" => PermissionMode::Default,
        "auto_low_risk" => PermissionMode::AutoLowRisk,
        "read_only" => PermissionMode::ReadOnly,
        _ => PermissionMode::AutoAll,
    }
}

async fn provider_model_status_for_state(
    state: &State<'_, DesktopAppState>,
) -> Result<ProviderModelStatus, String> {
    let runtime = state.runtime.lock().await.clone();
    let configured_provider = state.provider_name.lock().await.clone();
    let configured_model = state.model.lock().await.clone();
    let runtime_base_url = runtime
        .as_ref()
        .map(|runtime| runtime.streaming_engine().provider_base_url())
        .unwrap_or_default();
    let runtime_model = runtime
        .as_ref()
        .map(|runtime| runtime.streaming_engine().model_name());

    let registry = priority_agent::services::api::provider::ProviderRegistry::from_env();
    let active_provider = configured_provider
        .or_else(|| provider_id_for_base_url(&registry, &runtime_base_url))
        .or_else(|| default_provider_id_from_env(&registry));
    let active_model = runtime_model
        .or(configured_model)
        .or_else(|| {
            active_provider
                .as_deref()
                .and_then(|provider| registry.get_config(provider))
                .map(|config| config.default_model.clone())
        })
        .unwrap_or_else(|| "unconfigured".to_string());
    let providers = desktop_provider_options(&registry, active_provider.as_deref());
    let configured_count = providers
        .iter()
        .filter(|provider| provider.configured)
        .count();
    let models = active_provider
        .as_deref()
        .map(|provider| desktop_model_options(&registry, provider, &active_model))
        .unwrap_or_default();

    Ok(ProviderModelStatus {
        active_provider,
        active_model,
        configured_count,
        providers,
        models,
    })
}

fn apply_desktop_provider_model(
    runtime: &DesktopRuntime,
    provider_name: Option<&str>,
    model: Option<&str>,
) -> Result<(), String> {
    let Some(provider_name) = provider_name else {
        return Ok(());
    };
    let provider_name = provider_name.trim().to_ascii_lowercase();
    if provider_name.is_empty() {
        return Ok(());
    }

    let registry = priority_agent::services::api::provider::ProviderRegistry::from_env();
    let provider = registry
        .get(&provider_name)
        .ok_or_else(|| format!("configured desktop provider is unavailable: {provider_name}"))?;
    let selected_model = model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            registry
                .get_config(&provider_name)
                .map(|config| config.default_model.clone())
        })
        .ok_or_else(|| format!("configured desktop provider has no model: {provider_name}"))?;

    runtime
        .streaming_engine()
        .set_provider(provider, selected_model);
    Ok(())
}

fn desktop_provider_options(
    registry: &priority_agent::services::api::provider::ProviderRegistry,
    active_provider: Option<&str>,
) -> Vec<DesktopProviderOption> {
    let mut providers = registry
        .list_configs()
        .into_iter()
        .map(|config| {
            let active = active_provider == Some(config.name.as_str());
            DesktopProviderOption {
                id: config.name.clone(),
                label: provider_label(&config.name),
                provider_type: format!("{:?}", config.provider_type),
                model: config.default_model,
                base_url: config.base_url.unwrap_or_default(),
                configured: true,
                active,
                note: if active { "current" } else { "configured" }.to_string(),
            }
        })
        .collect::<Vec<_>>();

    for spec in priority_agent::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS {
        if providers.iter().any(|provider| provider.id == spec.id) {
            continue;
        }
        providers.push(DesktopProviderOption {
            id: spec.id.to_string(),
            label: spec.label.to_string(),
            provider_type: format!("{:?}", spec.provider_type),
            model: spec.default_model.to_string(),
            base_url: String::new(),
            configured: false,
            active: false,
            note: format!("missing {}", spec.key_env_hint()),
        });
    }

    providers.sort_by_key(|provider| {
        (
            !provider.active,
            !provider.configured,
            provider.label.to_ascii_lowercase(),
        )
    });
    providers
}

fn desktop_model_options(
    registry: &priority_agent::services::api::provider::ProviderRegistry,
    provider_id: &str,
    active_model: &str,
) -> Vec<DesktopModelOption> {
    let mut models = default_models_for_provider(provider_id)
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if let Some(config) = registry.get_config(provider_id) {
        if !models.iter().any(|model| model == &config.default_model) {
            models.insert(0, config.default_model.clone());
        }
    }
    if !active_model.is_empty() && !models.iter().any(|model| model == active_model) {
        models.insert(0, active_model.to_string());
    }

    models
        .into_iter()
        .map(|model| DesktopModelOption {
            id: model.clone(),
            label: model.clone(),
            provider_id: provider_id.to_string(),
            active: model == active_model,
            note: if model == active_model {
                "current".to_string()
            } else {
                "takes effect next request".to_string()
            },
        })
        .collect()
}

fn provider_id_for_base_url(
    registry: &priority_agent::services::api::provider::ProviderRegistry,
    base_url: &str,
) -> Option<String> {
    if base_url.trim().is_empty() {
        return None;
    }
    registry
        .list_configs()
        .into_iter()
        .find_map(|config| (config.base_url.as_deref() == Some(base_url)).then_some(config.name))
}

fn default_provider_id_from_env(
    registry: &priority_agent::services::api::provider::ProviderRegistry,
) -> Option<String> {
    priority_agent::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS
        .iter()
        .find(|spec| registry.get(spec.id).is_some())
        .map(|spec| spec.id.to_string())
}

fn default_models_for_provider(provider_id: &str) -> Vec<&'static str> {
    match provider_id {
        "minimax" => vec![
            "MiniMax-M3",
            "MiniMax-M2.7",
            "MiniMax-M2.7-highspeed",
            "MiniMax-M2.5",
            "MiniMax-M2",
        ],
        "kimi-code" => vec!["kimi-for-coding"],
        "deepseek" => vec!["deepseek-v4-pro", "deepseek-v4-flash", "deepseek-chat"],
        "glm" => vec!["glm-5.1", "glm-4.7", "glm-4.6"],
        "openai" => vec!["gpt-4o", "gpt-4o-mini"],
        "kimi" => vec!["kimi-k2.5", "kimi-k2.5-thinking"],
        _ => Vec::new(),
    }
}

fn provider_label(provider_id: &str) -> String {
    priority_agent::services::api::provider::default_provider_env_spec(provider_id)
        .map(|spec| spec.label.to_string())
        .unwrap_or_else(|| provider_id.to_string())
}

fn desktop_settings_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| {
            SessionStore::default_path()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".priority-agent"))
        })
        .join("desktop-settings.json")
}

fn desktop_diagnostic_logs_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| {
            SessionStore::default_path()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".priority-agent"))
        })
        .join("logs")
        .join("desktop.log")
}

fn append_desktop_log(path: &Path, message: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());
    let line = format!("{timestamp} {message}\n");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    use std::io::Write;
    file.write_all(line.as_bytes())
        .map_err(|err| err.to_string())
}

fn sanitize_log_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn schedule_native_interaction_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_interaction_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_interaction_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

fn native_interaction_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const candidates = () => Array.from(document.querySelectorAll("button, [role='button'], [aria-label]"));
  const buttonCandidates = () => Array.from(document.querySelectorAll("button, [role='button']"));
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const byLabel = (label) => candidates().find((element) => element.getAttribute("aria-label") === label && visible(element));
  const byEnabledLabel = (label) => candidates().find((element) => element.getAttribute("aria-label") === label && !element.disabled && visible(element));
  const byText = (label) => buttonCandidates().find((element) => element.textContent?.trim() === label && visible(element));
  const byTextIncludes = (label) => buttonCandidates().find((element) => element.textContent?.trim().includes(label) && visible(element));
  const setTextareaValue = (label, value) => {
    const element = document.querySelector(`textarea[aria-label="${label}"]`);
    if (!element) {
      throw new Error(`missing textarea ${label}`);
    }
    const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, "value")?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    steps.push(`typed-${label}`);
  };
  const click = async (name, findElement) => {
    const element = findElement();
    if (!element) {
      throw new Error(`missing ${name}`);
    }
    element.click();
    steps.push(name);
    await sleep(350);
    };
    const waitFor = async (name, predicate) => {
    for (let index = 0; index < 30; index += 1) {
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(200);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };

  try {
    await waitFor("app-ready", () => text().includes("What should we build in rust-agent?"));
    await click("settings-open", () => byText("Settings"));
    await waitFor("settings-visible", () => document.querySelector("[aria-label='Settings']"));
    await click("settings-close", () => byTextIncludes("Back to app"));
    await waitFor("settings-closed", () => !document.querySelector("[aria-label='Settings']"));
    await click("context-menu-open", () => byLabel("Add context"));
    await waitFor("context-menu-visible", () => text().includes("Add context") && text().includes("Current diff"));
    await click("current-diff-add", () => byLabel("Reference current diff"));
    await waitFor("context-chip-visible", () => Boolean(byLabel("Open context Current diff")));
    await click("context-detail-open", () => byLabel("Open context Current diff"));
    await waitFor("context-detail-visible", () => document.querySelector("[aria-label='Context details']"));
    await click("context-detail-close", () => byLabel("Close context details"));
    await waitFor("context-detail-closed", () => !document.querySelector("[aria-label='Context details']"));
    await click("trace-open", () => byText("Trace"));
    await waitFor("trace-visible", () => document.querySelector("[aria-label='Run trace']"));
    await click("trace-close", () => byText("Close"));
    await waitFor("trace-closed", () => !document.querySelector("[aria-label='Run trace']"));
    setTextareaValue("Message", "Native smoke real run");
    await waitFor("send-enabled", () => Boolean(byEnabledLabel("Send message")));
    await click("run-submit", () => byEnabledLabel("Send message"));
    await waitFor("run-started", () => text().includes("Runtime connected"));
    await waitFor("shell-card-visible", () => text().includes("scripts/desktop-native-smoke.sh --fixture-run"));
    await waitFor("file-card-visible", () => text().includes("Edited file") && text().includes("Composer.tsx"));
    await waitFor("permission-waiting", () => text().includes("Permission needed: bash") && Boolean(byText("Approve")));
    await click("permission-approve", () => byText("Approve"));
    await waitFor("permission-approved", () => text().includes("Permission approved"));
    await waitFor("assistant-answer-visible", () => text().includes("Native smoke fixture completed"));
    await waitFor("assistant-final", () => Boolean(document.querySelector(".message.assistant.final")));
    await waitFor("run-completed", () => text().includes("Run completed"));
    await waitFor("usage-visible", () => text().includes("Token usage"));
    return await record(`native_interaction_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_interaction_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

fn load_messages_from_store(
    store: &SessionStore,
    session_id: &str,
) -> Result<Vec<DesktopMessage>, String> {
    let messages = store
        .get_messages(session_id)
        .map_err(|err| err.to_string())?;

    Ok(messages
        .into_iter()
        .map(|message| DesktopMessage {
            id: message.id,
            role: message.role,
            content: message.content,
            created_at: message.created_at,
        })
        .collect())
}

fn load_compact_boundaries_from_store(
    store: &SessionStore,
    session_id: &str,
) -> Result<Vec<DesktopCompactBoundary>, String> {
    let boundaries = store
        .list_compact_boundaries(session_id, 8)
        .map_err(|err| err.to_string())?;

    Ok(boundaries
        .into_iter()
        .map(|boundary| DesktopCompactBoundary {
            boundary_id: boundary.boundary_id,
            strategy: boundary.strategy,
            trigger: boundary.trigger.unwrap_or_default(),
            before_tokens: boundary.before_tokens,
            after_tokens: boundary.after_tokens,
            messages_before: boundary.messages_before,
            messages_after: boundary.messages_after,
            summary: boundary.summary,
            created_at: boundary.created_at,
        })
        .collect())
}

pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let settings_path = desktop_settings_path(app.handle());
            let diagnostic_logs_path = desktop_diagnostic_logs_path(app.handle());
            let settings = load_desktop_settings(&settings_path).unwrap_or_default();
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
            if std::env::var("PRIORITY_AGENT_DESKTOP_NATIVE_SMOKE").as_deref() == Ok("1") {
                if let Some(window) = app.get_webview_window("main") {
                    schedule_native_interaction_smoke(window, diagnostic_logs_path.clone());
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
                provider_name: Mutex::new(settings.provider_name),
                model: Mutex::new(settings.model),
                recent_projects: Mutex::new(recent_projects),
                archived_session_ids: Mutex::new(settings.archived_session_ids.unwrap_or_default()),
                native_smoke_permission_pending: Mutex::new(false),
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
            permission_mode_options,
            provider_model_status,
            set_provider_model,
            desktop_diagnostics,
            provider_setup_info,
            open_settings_folder,
            open_diagnostics_folder,
            open_shell_profile,
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
            send_message,
            compact_context,
            desktop_context_snapshot,
            desktop_workbench_snapshot,
            desktop_run_context_detail,
            answer_permission,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Priority Agent desktop app");
}

#[cfg(test)]
mod tests;

fn collect_desktop_diagnostics(
    selected_project: &Path,
    settings_path: &Path,
    diagnostic_logs_path: &Path,
) -> Vec<DesktopDiagnostic> {
    vec![
        provider_key_diagnostic(),
        shell_diagnostic(),
        command_diagnostic("git", "Git command", "git"),
        command_diagnostic("cargo", "Rust toolchain", "cargo"),
        command_diagnostic("corepack", "Node package manager bridge", "corepack"),
        xcode_tools_diagnostic(),
        project_access_diagnostic(selected_project),
        settings_access_diagnostic(settings_path),
        diagnostic_logs_access_diagnostic(diagnostic_logs_path),
    ]
}

fn provider_setup_info_value() -> ProviderSetupInfo {
    ProviderSetupInfo {
        shell_profile_path: shell_profile_path().display().to_string(),
        provider_env_vars: priority_agent::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS
            .iter()
            .flat_map(|spec| spec.key_env_vars.iter().copied())
            .collect(),
        example: "export MINIMAX_API_KEY=\"your-key-here\"",
    }
}

fn provider_key_diagnostic() -> DesktopDiagnostic {
    let configured: Vec<&str> = priority_agent::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS
        .iter()
        .filter_map(|spec| {
            spec.key_env_vars
                .iter()
                .any(|env| env_is_set(env))
                .then_some(spec.label)
        })
        .collect();

    if configured.is_empty() {
        DesktopDiagnostic {
            id: "provider_keys",
            label: "Provider keys",
            status: DiagnosticStatus::Error,
            detail: format!(
                "No provider key found. Set one of {} before running real agent sessions.",
                priority_agent::services::api::provider::provider_key_env_hint()
            ),
        }
    } else {
        DesktopDiagnostic {
            id: "provider_keys",
            label: "Provider keys",
            status: DiagnosticStatus::Ok,
            detail: format!("Configured providers: {}", configured.join(", ")),
        }
    }
}

fn command_diagnostic(id: &'static str, label: &'static str, command: &str) -> DesktopDiagnostic {
    if command_available(command) {
        DesktopDiagnostic {
            id,
            label,
            status: DiagnosticStatus::Ok,
            detail: format!("`{command}` is available on PATH."),
        }
    } else {
        DesktopDiagnostic {
            id,
            label,
            status: DiagnosticStatus::Warning,
            detail: format!("`{command}` was not found on PATH."),
        }
    }
}

fn shell_diagnostic() -> DesktopDiagnostic {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    if Path::new(&shell).exists() {
        DesktopDiagnostic {
            id: "shell",
            label: "Shell",
            status: DiagnosticStatus::Ok,
            detail: format!("Using shell: {shell}"),
        }
    } else {
        DesktopDiagnostic {
            id: "shell",
            label: "Shell",
            status: DiagnosticStatus::Warning,
            detail: format!("Configured shell does not exist: {shell}"),
        }
    }
}

fn shell_profile_path() -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    if shell.ends_with("bash") {
        home.join(".bash_profile")
    } else {
        home.join(".zshrc")
    }
}

fn xcode_tools_diagnostic() -> DesktopDiagnostic {
    match Command::new("xcode-select").arg("-p").output() {
        Ok(output) if output.status.success() => DesktopDiagnostic {
            id: "xcode_select",
            label: "Xcode command line tools",
            status: DiagnosticStatus::Ok,
            detail: format!(
                "Developer tools path: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ),
        },
        _ => DesktopDiagnostic {
            id: "xcode_select",
            label: "Xcode command line tools",
            status: DiagnosticStatus::Warning,
            detail: "Xcode command line tools are not configured; run `xcode-select --install` if builds fail.".to_string(),
        },
    }
}

fn project_access_diagnostic(project: &Path) -> DesktopDiagnostic {
    if !project.exists() {
        return DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Error,
            detail: format!("Project path does not exist: {}", project.display()),
        };
    }
    if !project.is_dir() {
        return DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Error,
            detail: format!("Project path is not a directory: {}", project.display()),
        };
    }
    if std::fs::read_dir(project).is_err() {
        return DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Error,
            detail: format!("Project path is not readable: {}", project.display()),
        };
    }

    if directory_writable(project) {
        DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Ok,
            detail: format!(
                "Project path is readable and writable: {}",
                project.display()
            ),
        }
    } else {
        DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Warning,
            detail: format!(
                "Project path is readable but may not be writable: {}",
                project.display()
            ),
        }
    }
}

fn settings_access_diagnostic(settings_path: &Path) -> DesktopDiagnostic {
    let Some(parent) = settings_path.parent() else {
        return DesktopDiagnostic {
            id: "settings_access",
            label: "Settings storage",
            status: DiagnosticStatus::Error,
            detail: format!(
                "Settings path has no parent directory: {}",
                settings_path.display()
            ),
        };
    };

    if directory_writable(parent)
        || std::fs::create_dir_all(parent).is_ok() && directory_writable(parent)
    {
        DesktopDiagnostic {
            id: "settings_access",
            label: "Settings storage",
            status: DiagnosticStatus::Ok,
            detail: format!("Settings can be stored at {}", settings_path.display()),
        }
    } else {
        DesktopDiagnostic {
            id: "settings_access",
            label: "Settings storage",
            status: DiagnosticStatus::Error,
            detail: format!("Settings directory is not writable: {}", parent.display()),
        }
    }
}

fn diagnostic_logs_access_diagnostic(log_path: &Path) -> DesktopDiagnostic {
    let Some(parent) = log_path.parent() else {
        return DesktopDiagnostic {
            id: "diagnostic_logs",
            label: "Diagnostic logs",
            status: DiagnosticStatus::Error,
            detail: format!(
                "Diagnostic log path has no parent directory: {}",
                log_path.display()
            ),
        };
    };

    if directory_writable(parent)
        || std::fs::create_dir_all(parent).is_ok() && directory_writable(parent)
    {
        DesktopDiagnostic {
            id: "diagnostic_logs",
            label: "Diagnostic logs",
            status: DiagnosticStatus::Ok,
            detail: format!("Desktop logs can be written at {}", log_path.display()),
        }
    } else {
        DesktopDiagnostic {
            id: "diagnostic_logs",
            label: "Diagnostic logs",
            status: DiagnosticStatus::Warning,
            detail: format!(
                "Diagnostic log directory is not writable: {}",
                parent.display()
            ),
        }
    }
}

fn env_is_set(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn command_available(command: &str) -> bool {
    Command::new("/bin/sh")
        .arg("-lc")
        .arg(format!("command -v {command} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn directory_writable(path: &Path) -> bool {
    let test_path = path.join(format!(".priority-agent-write-test-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&test_path)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(test_path);
            true
        }
        Err(_) => false,
    }
}

fn open_path(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .status()
        .map_err(|err| err.to_string())
        .and_then(|status| {
            status
                .success()
                .then_some(())
                .ok_or_else(|| format!("failed to open {}", path.display()))
        })
}

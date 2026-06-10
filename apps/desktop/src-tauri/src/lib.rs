use futures::StreamExt;
use priority_agent::desktop_runtime::{DesktopContextSnapshot, DesktopRunEvent, DesktopRuntime};
use priority_agent::engine::streaming::StreamEvent;
use priority_agent::engine::turn_ingress::classify_turn_ingress;
use priority_agent::permissions::PermissionMode;
use priority_agent::session_store::SessionStore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::Mutex;

mod desktop_state;
mod diagnostics;
mod desktop_context;
use desktop_state::*;
pub(crate) use desktop_context::*;
use diagnostics::*;

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
    session_parts: Vec<DesktopSessionPart>,
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

#[derive(Debug, Serialize)]
struct DesktopSessionPart {
    id: i64,
    part_index: i64,
    part_id: String,
    kind: String,
    tool_call_id: Option<String>,
    tool_name: Option<String>,
    status: Option<String>,
    payload: serde_json::Value,
    projected_to_seq: i64,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct DesktopToolOutputPage {
    id: String,
    uri: String,
    tool_name: String,
    mime: String,
    content: String,
    offset: u64,
    limit: u64,
    total_bytes: u64,
    has_more: bool,
}

#[derive(Debug, Serialize)]
struct DesktopToolOutputMeta {
    id: String,
    uri: String,
    tool_call_id: String,
    tool_name: String,
    mime: String,
    original_bytes: u64,
    created_at_ms: u64,
}

#[derive(Debug, Serialize)]
struct DesktopRevertResult {
    session_id: String,
    status: String,
    message_id: Option<String>,
    part_ids: Vec<String>,
    tool_round_id: Option<String>,
    file_change_ids: Vec<String>,
    checkpoint_ids: Vec<String>,
    paths: Vec<String>,
    restored_files: Vec<String>,
    removed_files: Vec<String>,
    errors: Vec<String>,
    change_count: usize,
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
    agent_mode: String,
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
struct DesktopExportResult {
    session_id: String,
    path: String,
    format: String,
    privacy: String,
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
    active_provider_label: Option<String>,
    active_model: String,
    active_base_url: String,
    runtime_model: Option<String>,
    runtime_provider_ready: bool,
    selection_source: String,
    configured_count: usize,
    providers: Vec<DesktopProviderOption>,
    models: Vec<DesktopModelOption>,
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
    use priority_agent::engine::agent_mode::AgentMode;
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

#[derive(Debug, Serialize)]
struct AgentModeOption {
    id: String,
    label: String,
    description: String,
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
    let session_parts = load_session_parts_from_store(&store, &session_id)?;
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
        session_parts,
    })
}

#[tauri::command]
fn list_session_reverts(
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<priority_agent::session_store::SessionRevertRecord>, String> {
    let store = open_session_store()?;
    store
        .list_session_reverts(&session_id, limit.unwrap_or(20).clamp(1, 100))
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_tool_output_index(session_id: String) -> Result<Vec<DesktopToolOutputMeta>, String> {
    let store = priority_agent::tool_output_store::ToolOutputStore::new();
    let metas = store
        .list_for_session(&session_id)
        .map_err(|err| err.to_string())?;
    Ok(metas
        .into_iter()
        .map(|meta| DesktopToolOutputMeta {
            id: meta.id.clone(),
            uri: meta.uri(),
            tool_call_id: meta.tool_call_id,
            tool_name: meta.tool_name,
            mime: meta.mime,
            original_bytes: meta.original_bytes,
            created_at_ms: meta.created_at_ms,
        })
        .collect())
}

#[tauri::command]
fn desktop_tool_output_page(
    session_id: String,
    id_or_uri: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<DesktopToolOutputPage, String> {
    let store = priority_agent::tool_output_store::ToolOutputStore::new();
    let page = store
        .read_page(
            &session_id,
            &id_or_uri,
            offset.unwrap_or(0),
            limit.unwrap_or(64 * 1024),
        )
        .map_err(|err| err.to_string())?;
    let meta = store
        .read_meta(&id_or_uri)
        .map_err(|err| err.to_string())?;
    Ok(DesktopToolOutputPage {
        id: meta.id.clone(),
        uri: meta.uri(),
        tool_name: meta.tool_name,
        mime: meta.mime,
        content: page.content,
        offset: page.offset,
        limit: page.limit,
        total_bytes: page.total_bytes,
        has_more: page.has_more,
    })
}

#[tauri::command]
async fn revert_last_turn(session_id: String) -> Result<DesktopRevertResult, String> {
    let store = open_session_store()?;
    store
        .get_session(&session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;

    let manager = priority_agent::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let checkpoint_guard = manager.lock().await;
    let result = checkpoint_guard.revert_latest_assistant_turn().await?;
    let payload = serde_json::to_value(&result).map_err(|err| err.to_string())?;
    store
        .record_session_revert(&priority_agent::session_store::SessionRevertInsert {
            session_id: result.session_id.clone(),
            operation: "revert".to_string(),
            status: result.status.clone(),
            message_id: result.message_id.clone(),
            target_part_id: result.target_part_id.clone(),
            part_ids: result.part_ids.clone(),
            checkpoint_ids: result.checkpoint_ids.clone(),
            snapshot_checkpoint_id: result.snapshot_checkpoint_id.clone(),
            paths: result.paths.clone(),
            restored_files: result.restored_files.clone(),
            removed_files: result.removed_files.clone(),
            errors: result.errors.clone(),
            diff_summary: result.diff_summary.clone(),
            unrevert_possible: result.unrevert_possible,
            unreverted: false,
            payload: payload.clone(),
        })
        .map_err(|err| err.to_string())?;
    let writer =
        priority_agent::session_store::SessionEventWriter::new(store.shared_conn(), &session_id);
    writer
        .write_event("revert", &payload.to_string())
        .map_err(|err| err.to_string())?;

    Ok(DesktopRevertResult {
        session_id: result.session_id,
        status: result.status,
        message_id: result.message_id,
        part_ids: result.part_ids,
        tool_round_id: result.tool_round_id,
        file_change_ids: result.file_change_ids,
        checkpoint_ids: result.checkpoint_ids,
        paths: result.paths,
        restored_files: result.restored_files,
        removed_files: result.removed_files,
        errors: result.errors,
        change_count: result.change_count,
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
    let agent_mode = state.agent_mode.lock().await.clone();
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
            revert_last_turn,
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

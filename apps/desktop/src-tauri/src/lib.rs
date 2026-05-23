use futures::StreamExt;
use priority_agent::desktop_runtime::{DesktopRunEvent, DesktopRuntime};
use priority_agent::permissions::PermissionMode;
use priority_agent::session_store::SessionStore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter, Manager, State};
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
    settings_path: PathBuf,
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
}

#[derive(Debug, Clone, Deserialize)]
struct DesktopRunContext {
    #[serde(rename = "type")]
    context_type: String,
    label: Option<String>,
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

    Ok(DesktopDiagnosticsResponse {
        items: collect_desktop_diagnostics(&selected_project, &settings_path),
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

    project.canonicalize().map_err(|err| err.to_string())
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
    let runtime = runtime_for_state(&state).await?;
    let selected_project = state.selected_project.lock().await.clone();
    let message = enrich_message_with_desktop_contexts(message, &contexts, &selected_project)?;
    let engine = runtime.streaming_engine();
    let active_session_id = engine.current_session_id();
    let mut stream = engine.query_stream(message).await;

    while let Some(event) = stream.next().await {
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
        app.emit("desktop-run-event", desktop_event)
            .map_err(|err| err.to_string())?;
        if is_terminal {
            break;
        }
    }

    Ok(())
}

#[tauri::command]
async fn desktop_run_context_detail(
    context: DesktopRunContext,
    state: State<'_, DesktopAppState>,
) -> Result<ResolvedDesktopRunContext, String> {
    let selected_project = state.selected_project.lock().await.clone();
    match context.context_type.as_str() {
        "current_diff" => resolve_current_diff_context(&context, &selected_project),
        other => Err(format!("Unsupported desktop run context: {}", other)),
    }
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
        truncated,
    })
}

fn format_desktop_context_block(context: &ResolvedDesktopRunContext) -> String {
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
    approved: bool,
    state: State<'_, DesktopAppState>,
) -> Result<bool, String> {
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
        let _ = tx.send(response);
        return Ok(true);
    }

    Ok(false)
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
    let default_project = default_desktop_project(cwd);
    settings
        .selected_project
        .as_deref()
        .and_then(|path| validate_project_path(path).ok())
        .map(|project| migrate_accidental_desktop_subdir(project, &default_project))
        .unwrap_or(default_project)
}

fn default_desktop_project(cwd: PathBuf) -> PathBuf {
    if let Some(project) = std::env::var("PRIORITY_AGENT_DESKTOP_PROJECT_DIR")
        .ok()
        .and_then(|path| validate_project_path(path).ok())
    {
        return project;
    }

    discover_project_root(&cwd).unwrap_or(cwd)
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

    for (id, label, provider_type, model, env_key) in default_desktop_providers() {
        if providers.iter().any(|provider| provider.id == id) {
            continue;
        }
        providers.push(DesktopProviderOption {
            id: id.to_string(),
            label: label.to_string(),
            provider_type: provider_type.to_string(),
            model: model.to_string(),
            base_url: String::new(),
            configured: false,
            active: false,
            note: format!("missing {env_key}"),
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
    ["minimax", "openai", "kimi"]
        .into_iter()
        .find(|provider| registry.get(provider).is_some())
        .map(ToString::to_string)
}

fn default_desktop_providers() -> [(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
); 3] {
    [
        (
            "minimax",
            "MiniMax",
            "Minimax",
            "MiniMax-M2.7",
            "MINIMAX_API_KEY",
        ),
        ("openai", "OpenAI", "OpenAI", "gpt-4o", "OPENAI_API_KEY"),
        ("kimi", "Kimi", "Kimi", "kimi-k2.5", "MOONSHOT_API_KEY"),
    ]
}

fn default_models_for_provider(provider_id: &str) -> Vec<&'static str> {
    match provider_id {
        "minimax" => vec!["MiniMax-M2.7", "MiniMax-M1"],
        "openai" => vec!["gpt-4o", "gpt-4o-mini"],
        "kimi" => vec!["kimi-k2.5", "kimi-k2.5-thinking"],
        _ => Vec::new(),
    }
}

fn provider_label(provider_id: &str) -> String {
    default_desktop_providers()
        .into_iter()
        .find_map(|(id, label, _, _, _)| (id == provider_id).then_some(label.to_string()))
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

pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let settings_path = desktop_settings_path(app.handle());
            let settings = load_desktop_settings(&settings_path).unwrap_or_default();
            let selected_project = initial_desktop_project(cwd.clone(), &settings);
            let recent_projects = initial_recent_projects(&selected_project, &settings);
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
                settings_path,
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
            open_shell_profile,
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
            desktop_run_context_detail,
            answer_permission,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Priority Agent desktop app");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_smoke_health_reports_ready_and_cwd() {
        let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
        let health = desktop_health_value(cwd.clone());

        assert_eq!(health.status, "ready");
        assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(health.cwd, cwd.display().to_string());
    }

    #[test]
    fn desktop_smoke_project_path_validation_accepts_directory() {
        let cwd = std::env::current_dir().unwrap();
        let project = validate_project_path(&cwd).unwrap();
        let selected = selected_project_response(project.clone());

        assert!(project.is_dir());
        assert_eq!(selected.path, project.display().to_string());
    }

    #[test]
    fn desktop_smoke_project_path_validation_rejects_missing_path() {
        let missing = std::env::temp_dir().join(format!(
            "priority-agent-desktop-missing-{}",
            std::process::id()
        ));
        let err = validate_project_path(&missing).unwrap_err();

        assert!(err.contains("project path does not exist"));
        assert!(err.contains(&missing.display().to_string()));
    }

    #[test]
    fn desktop_smoke_recent_sessions_include_message_counts() {
        let store = SessionStore::in_memory().unwrap();
        store
            .create_session("desktop-session", "Desktop Session", "mock-model")
            .unwrap();
        store
            .add_message("desktop-session", "user", "hello", None, None)
            .unwrap();
        store
            .add_message("desktop-session", "assistant", "hi", None, None)
            .unwrap();

        let sessions = list_recent_sessions_from_store(&store, 20, &[]).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "desktop-session");
        assert_eq!(sessions[0].title, "Desktop Session");
        assert_eq!(sessions[0].model, "mock-model");
        assert_eq!(sessions[0].message_count, 2);
        assert!(!sessions[0].updated_at.is_empty());
    }

    #[test]
    fn desktop_smoke_recent_sessions_skip_archived_ids() {
        let store = SessionStore::in_memory().unwrap();
        store
            .create_session("visible-session", "Visible Session", "mock-model")
            .unwrap();
        store
            .create_session("archived-session", "Archived Session", "mock-model")
            .unwrap();

        let sessions =
            list_recent_sessions_from_store(&store, 20, &["archived-session".to_string()]).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "visible-session");
    }

    #[test]
    fn desktop_smoke_search_sessions_uses_message_fts() {
        let store = SessionStore::in_memory().unwrap();
        store
            .create_session("desktop-session", "Desktop Session", "mock-model")
            .unwrap();
        store
            .add_message(
                "desktop-session",
                "user",
                "Find onboarding diagnostics",
                None,
                None,
            )
            .unwrap();

        let sessions = search_sessions_from_store(&store, "onboarding", 20, &[]).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "desktop-session");
    }

    #[test]
    fn desktop_smoke_load_messages_preserves_order_and_roles() {
        let store = SessionStore::in_memory().unwrap();
        store
            .create_session("desktop-session", "Desktop Session", "mock-model")
            .unwrap();
        store
            .add_message("desktop-session", "user", "first", None, None)
            .unwrap();
        store
            .add_message("desktop-session", "assistant", "second", None, None)
            .unwrap();

        let messages = load_messages_from_store(&store, "desktop-session").unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "first");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "second");
        assert!(!messages[0].created_at.is_empty());
    }

    #[test]
    fn desktop_run_context_enriches_message_with_git_diff() {
        let project = std::env::temp_dir().join(format!(
            "priority-agent-desktop-diff-context-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).unwrap();
        run_test_command(&project, &["git", "init"]);
        run_test_command(
            &project,
            &["git", "config", "user.email", "desktop@example.com"],
        );
        run_test_command(&project, &["git", "config", "user.name", "Desktop Test"]);
        std::fs::write(project.join("README.md"), "old\n").unwrap();
        run_test_command(&project, &["git", "add", "README.md"]);
        run_test_command(&project, &["git", "commit", "-m", "initial"]);
        std::fs::write(project.join("README.md"), "new\n").unwrap();

        let message = enrich_message_with_desktop_contexts(
            "Review this".to_string(),
            &[DesktopRunContext {
                context_type: "current_diff".to_string(),
                label: Some("Current diff".to_string()),
            }],
            &project,
        )
        .unwrap();

        assert!(message.contains("Review this"));
        assert!(message.contains("<desktop_context type=\"current_diff\" label=\"Current diff\">"));
        assert!(message.contains("README.md"));
        assert!(message.contains("-old"));
        assert!(message.contains("+new"));

        let _ = std::fs::remove_dir_all(&project);
    }

    #[test]
    fn desktop_run_context_rejects_unknown_context_type() {
        let err = enrich_message_with_desktop_contexts(
            "hello".to_string(),
            &[DesktopRunContext {
                context_type: "unknown".to_string(),
                label: None,
            }],
            &std::env::current_dir().unwrap(),
        )
        .unwrap_err();

        assert!(err.contains("Unsupported desktop run context"));
    }

    #[test]
    fn desktop_smoke_settings_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-desktop-settings-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let settings = DesktopSettings {
            selected_project: Some("/tmp/project".to_string()),
            active_session_id: Some("session-1".to_string()),
            permission_mode: Some("auto_low_risk".to_string()),
            detail_level: Some("daily".to_string()),
            provider_name: Some("kimi".to_string()),
            model: Some("kimi-k2.5".to_string()),
            recent_projects: Some(vec!["/tmp/project".to_string()]),
            archived_session_ids: Some(vec!["old-session".to_string()]),
        };

        write_desktop_settings(&path, &settings).unwrap();
        let loaded = load_desktop_settings(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.selected_project.as_deref(), Some("/tmp/project"));
        assert_eq!(loaded.active_session_id.as_deref(), Some("session-1"));
        assert_eq!(loaded.permission_mode.as_deref(), Some("auto_low_risk"));
        assert_eq!(loaded.detail_level.as_deref(), Some("daily"));
        assert_eq!(loaded.provider_name.as_deref(), Some("kimi"));
        assert_eq!(loaded.model.as_deref(), Some("kimi-k2.5"));
        assert_eq!(
            loaded.recent_projects.as_deref(),
            Some(["/tmp/project".to_string()].as_slice())
        );
        assert_eq!(
            loaded.archived_session_ids.as_deref(),
            Some(["old-session".to_string()].as_slice())
        );
    }

    #[test]
    fn desktop_smoke_initial_project_falls_back_when_saved_path_is_missing() {
        let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
        let expected = default_desktop_project(cwd.clone());
        let settings = DesktopSettings {
            selected_project: Some(
                std::env::temp_dir()
                    .join(format!("priority-agent-missing-{}", std::process::id()))
                    .display()
                    .to_string(),
            ),
            active_session_id: None,
            permission_mode: None,
            detail_level: None,
            provider_name: None,
            model: None,
            recent_projects: None,
            archived_session_ids: None,
        };

        assert_eq!(initial_desktop_project(cwd, &settings), expected);
    }

    #[test]
    fn desktop_smoke_default_project_discovers_repo_root_from_tauri_dir() {
        let root = std::env::temp_dir().join(format!("priority-agent-root-{}", std::process::id()));
        let tauri_dir = root.join("apps/desktop/src-tauri");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(&tauri_dir).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();

        let expected = root.canonicalize().unwrap();
        let discovered = default_desktop_project(tauri_dir.canonicalize().unwrap());
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(discovered, expected);
    }

    #[test]
    fn desktop_smoke_initial_project_migrates_old_tauri_subdir_default() {
        let root =
            std::env::temp_dir().join(format!("priority-agent-migrate-{}", std::process::id()));
        let tauri_dir = root.join("apps/desktop/src-tauri");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(&tauri_dir).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();

        let settings = DesktopSettings {
            selected_project: Some(tauri_dir.display().to_string()),
            active_session_id: None,
            permission_mode: None,
            detail_level: None,
            provider_name: None,
            model: None,
            recent_projects: None,
            archived_session_ids: None,
        };
        let expected = root.canonicalize().unwrap();
        let selected = initial_desktop_project(tauri_dir.canonicalize().unwrap(), &settings);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(selected, expected);
    }

    #[test]
    fn desktop_smoke_diagnostics_include_project_and_settings_access() {
        let project = std::env::current_dir().unwrap().canonicalize().unwrap();
        let settings_path = std::env::temp_dir()
            .join(format!("priority-agent-settings-{}", std::process::id()))
            .join("desktop-settings.json");

        let diagnostics = collect_desktop_diagnostics(&project, &settings_path);

        assert!(diagnostics.iter().any(|item| item.id == "provider_keys"));
        assert!(
            diagnostics
                .iter()
                .any(|item| item.id == "project_access"
                    && matches!(item.status, DiagnosticStatus::Ok))
        );
        assert!(diagnostics.iter().any(
            |item| item.id == "settings_access" && matches!(item.status, DiagnosticStatus::Ok)
        ));
    }

    #[test]
    fn desktop_smoke_project_diagnostic_reports_missing_path() {
        let missing = std::env::temp_dir().join(format!(
            "priority-agent-diagnostic-missing-{}",
            std::process::id()
        ));

        let diagnostic = project_access_diagnostic(&missing);

        assert!(matches!(diagnostic.status, DiagnosticStatus::Error));
        assert!(diagnostic.detail.contains("does not exist"));
    }

    #[test]
    fn desktop_smoke_provider_setup_info_uses_shell_profile() {
        let info = provider_setup_info_value();

        assert!(
            info.shell_profile_path.ends_with(".zshrc")
                || info.shell_profile_path.ends_with(".bash_profile")
        );
        assert!(info.provider_env_vars.contains(&"MOONSHOT_API_KEY"));
        assert!(info.example.contains("export "));
    }

    #[test]
    fn desktop_smoke_permission_mode_normalization() {
        assert_eq!(normalized_permission_mode_label(Some("ask")), "default");
        assert_eq!(
            normalized_permission_mode_label(Some("auto_low_risk")),
            "auto_low_risk"
        );
        assert_eq!(normalized_permission_mode_label(Some("auto_all")), "auto");
        assert_eq!(
            normalized_permission_mode_label(Some("readonly")),
            "read_only"
        );
        assert_eq!(normalized_permission_mode_label(Some("once")), "auto");
        assert_eq!(
            parse_desktop_permission_mode("read_only"),
            PermissionMode::ReadOnly
        );
    }

    #[test]
    fn desktop_smoke_detail_level_normalization() {
        assert_eq!(normalized_detail_level_label(None), "coding");
        assert_eq!(normalized_detail_level_label(Some("daily_work")), "daily");
        assert_eq!(normalized_detail_level_label(Some("default")), "daily");
        assert_eq!(normalized_detail_level_label(Some("unknown")), "coding");
    }

    #[test]
    fn desktop_smoke_provider_options_include_missing_defaults() {
        let registry = priority_agent::services::api::provider::ProviderRegistry::new();
        let providers = desktop_provider_options(&registry, Some("kimi"));

        assert!(providers
            .iter()
            .any(|provider| provider.id == "minimax" && !provider.configured));
        assert!(providers
            .iter()
            .any(|provider| provider.id == "openai" && provider.note.contains("OPENAI_API_KEY")));
        assert!(providers
            .iter()
            .any(|provider| provider.id == "kimi" && provider.model == "kimi-k2.5"));
    }

    #[test]
    fn desktop_smoke_model_options_include_current_model() {
        let registry = priority_agent::services::api::provider::ProviderRegistry::new();
        let models = desktop_model_options(&registry, "openai", "custom-model");

        assert!(models
            .iter()
            .any(|model| model.id == "custom-model" && model.active));
        assert!(models.iter().any(|model| model.id == "gpt-4o"));
        assert!(models.iter().any(|model| model.id == "gpt-4o-mini"));
    }

    fn run_test_command(project: &Path, command: &[&str]) {
        let status = Command::new(command[0])
            .current_dir(project)
            .args(&command[1..])
            .status()
            .unwrap();
        assert!(status.success(), "command failed: {:?}", command);
    }
}

fn collect_desktop_diagnostics(
    selected_project: &Path,
    settings_path: &Path,
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
    ]
}

fn provider_setup_info_value() -> ProviderSetupInfo {
    ProviderSetupInfo {
        shell_profile_path: shell_profile_path().display().to_string(),
        provider_env_vars: vec!["MINIMAX_API_KEY", "OPENAI_API_KEY", "MOONSHOT_API_KEY"],
        example: "export MOONSHOT_API_KEY=\"your-key-here\"",
    }
}

fn provider_key_diagnostic() -> DesktopDiagnostic {
    let configured: Vec<&str> = [
        ("MINIMAX_API_KEY", "MiniMax"),
        ("OPENAI_API_KEY", "OpenAI"),
        ("MOONSHOT_API_KEY", "Moonshot/Kimi"),
    ]
    .into_iter()
    .filter_map(|(env, label)| env_is_set(env).then_some(label))
    .collect();

    if configured.is_empty() {
        DesktopDiagnostic {
            id: "provider_keys",
            label: "Provider keys",
            status: DiagnosticStatus::Error,
            detail: "No provider key found. Set MINIMAX_API_KEY, OPENAI_API_KEY, or MOONSHOT_API_KEY before running real agent sessions.".to_string(),
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

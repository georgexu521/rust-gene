use super::*;
use std::path::{Path, PathBuf};

pub(super) fn open_session_store() -> Result<SessionStore, String> {
    SessionStore::open(SessionStore::default_path()).map_err(|err| err.to_string())
}

pub(super) async fn clear_active_session_if_matches(
    state: &State<'_, DesktopAppState>,
    session_id: &str,
) {
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

pub(super) async fn persist_current_settings(
    state: &State<'_, DesktopAppState>,
) -> Result<(), String> {
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

pub(super) fn load_desktop_settings(path: &PathBuf) -> Result<DesktopSettings, String> {
    if !path.exists() {
        return Ok(DesktopSettings::default());
    }

    let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

pub(super) fn write_desktop_settings(
    path: &PathBuf,
    settings: &DesktopSettings,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let text = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    std::fs::write(path, text).map_err(|err| err.to_string())
}

pub(super) fn initial_desktop_project(cwd: PathBuf, settings: &DesktopSettings) -> PathBuf {
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

pub(super) fn default_desktop_project(cwd: PathBuf) -> PathBuf {
    if let Some(project) = configured_desktop_project() {
        return project;
    }

    discover_project_root(&cwd).unwrap_or(cwd)
}

pub(super) fn configured_desktop_project() -> Option<PathBuf> {
    std::env::var("PRIORITY_AGENT_DESKTOP_PROJECT_DIR")
        .ok()
        .and_then(|path| validate_project_path(path).ok())
}

pub(super) fn discover_project_root(start: &Path) -> Option<PathBuf> {
    let start = start.canonicalize().ok()?;
    start
        .ancestors()
        .find(|path| path.join(".git").exists() && path.join("Cargo.toml").exists())
        .map(PathBuf::from)
}

pub(super) fn migrate_accidental_desktop_subdir(
    project: PathBuf,
    default_project: &Path,
) -> PathBuf {
    let Ok(relative) = project.strip_prefix(default_project) else {
        return project;
    };
    if relative == Path::new("apps/desktop") || relative == Path::new("apps/desktop/src-tauri") {
        default_project.to_path_buf()
    } else {
        project
    }
}

pub(super) fn initial_recent_projects(
    selected_project: &Path,
    settings: &DesktopSettings,
) -> Vec<PathBuf> {
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

pub(super) fn remember_recent_project(projects: &mut Vec<PathBuf>, project: PathBuf) {
    projects.retain(|existing| existing != &project);
    projects.insert(0, project);
    projects.truncate(8);
}

pub(super) fn desktop_startup_state(
    project: &Path,
    active_session_id: Option<&str>,
) -> DesktopStartupState {
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

pub(super) fn desktop_permission_mode_options() -> Vec<PermissionModeOption> {
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

pub(super) fn normalized_permission_mode_label(mode: Option<&str>) -> &'static str {
    match mode.unwrap_or("auto").trim() {
        "default" | "ask" | "ask_each_time" => "default",
        "auto_low_risk" | "autolowrisk" | "low_risk" => "auto_low_risk",
        "auto" | "auto_all" | "developer_auto" => "auto",
        "read_only" | "readonly" => "read_only",
        _ => "auto",
    }
}

pub(super) fn normalized_detail_level_label(level: Option<&str>) -> &'static str {
    match level.unwrap_or("coding").trim() {
        "default" | "daily" | "daily_work" => "daily",
        _ => "coding",
    }
}

pub(super) fn parse_desktop_permission_mode(mode: &str) -> PermissionMode {
    match normalized_permission_mode_label(Some(mode)) {
        "default" => PermissionMode::Default,
        "auto_low_risk" => PermissionMode::AutoLowRisk,
        "read_only" => PermissionMode::ReadOnly,
        _ => PermissionMode::AutoAll,
    }
}

pub(super) async fn provider_model_status_for_state(
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
    let runtime_provider_ready = runtime.is_some();

    let registry = priority_agent::services::api::provider::ProviderRegistry::from_env();
    let selection_source = if runtime_model.is_some() {
        "runtime"
    } else if configured_model.is_some() || configured_provider.is_some() {
        "desktop_settings"
    } else {
        "environment"
    }
    .to_string();
    let active_provider = configured_provider
        .or_else(|| provider_id_for_base_url(&registry, &runtime_base_url))
        .or_else(|| default_provider_id_from_env(&registry));
    let active_base_url = active_provider
        .as_deref()
        .and_then(|provider| registry.get_config(provider))
        .and_then(|config| config.base_url.clone())
        .filter(|base_url| !base_url.trim().is_empty())
        .unwrap_or(runtime_base_url);
    let active_provider_label = active_provider.as_deref().map(provider_label);
    let active_model = runtime_model
        .clone()
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
        active_provider_label,
        active_model,
        active_base_url,
        runtime_model,
        runtime_provider_ready,
        selection_source,
        configured_count,
        providers,
        models,
    })
}

pub(super) fn apply_desktop_provider_model(
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

pub(super) fn desktop_provider_options(
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

pub(super) fn desktop_model_options(
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

pub(super) fn provider_id_for_base_url(
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

pub(super) fn default_provider_id_from_env(
    registry: &priority_agent::services::api::provider::ProviderRegistry,
) -> Option<String> {
    priority_agent::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS
        .iter()
        .find(|spec| registry.get(spec.id).is_some())
        .map(|spec| spec.id.to_string())
}

pub(super) fn default_models_for_provider(provider_id: &str) -> Vec<&'static str> {
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

pub(super) fn provider_label(provider_id: &str) -> String {
    priority_agent::services::api::provider::default_provider_env_spec(provider_id)
        .map(|spec| spec.label.to_string())
        .unwrap_or_else(|| provider_id.to_string())
}

pub(super) fn desktop_settings_path(app: &AppHandle) -> PathBuf {
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

pub(super) fn desktop_diagnostic_logs_path(app: &AppHandle) -> PathBuf {
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

pub(super) fn append_desktop_log(path: &Path, message: &str) -> Result<(), String> {
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

pub(super) fn sanitize_log_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn schedule_native_interaction_smoke(window: WebviewWindow, log_path: PathBuf) {
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

pub(super) fn native_interaction_smoke_script() -> &'static str {
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

pub(super) fn load_messages_from_store(
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

pub(super) fn load_compact_boundaries_from_store(
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

pub(super) fn load_session_parts_from_store(
    store: &SessionStore,
    session_id: &str,
) -> Result<Vec<DesktopSessionPart>, String> {
    let parts = store
        .get_session_parts(session_id)
        .map_err(|err| err.to_string())?;
    Ok(parts
        .into_iter()
        .map(|part| DesktopSessionPart {
            id: part.id,
            part_index: part.part_index,
            part_id: part.part_id,
            kind: part.kind,
            tool_call_id: part.tool_call_id,
            tool_name: part.tool_name,
            status: part.status,
            payload: part.payload,
            projected_to_seq: part.projected_to_seq,
            updated_at: part.updated_at,
        })
        .collect())
}

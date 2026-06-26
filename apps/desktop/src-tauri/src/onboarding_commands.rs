use super::*;
use serde::{Deserialize, Serialize};

#[tauri::command]
pub(crate) async fn complete_desktop_onboarding(
    input: DesktopOnboardingInput,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    apply_desktop_onboarding_input(&state, input).await?;
    desktop_settings(state).await
}

#[tauri::command]
pub(crate) async fn skip_desktop_onboarding(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let trust_input = DesktopWorkspaceTrustInput {
        package_scripts: "ask".to_string(),
        shell_validation: "ask".to_string(),
        lab_daemon_supervision: false,
        developer_auto_acknowledged: false,
    };
    apply_desktop_onboarding_input(
        &state,
        DesktopOnboardingInput {
            project_root: Some(selected_project.display().to_string()),
            permission_mode: Some("auto_low_risk".to_string()),
            workspace_trust: Some(trust_input),
            credential_storage_acknowledged: false,
            starting_mode: Some("direct".to_string()),
            skipped: Some(true),
        },
    )
    .await?;
    desktop_settings(state).await
}

#[tauri::command]
pub(crate) async fn set_workspace_trust(
    input: DesktopWorkspaceTrustInput,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let trust = apply_workspace_trust_for_project(&selected_project, &input)?;
    {
        let mut stored = state.workspace_trust.lock().await;
        *stored = Some(trust);
    }
    {
        let mut enabled = state.lab_daemon_supervision_enabled.lock().await;
        *enabled = input.lab_daemon_supervision;
    }
    {
        let mut next = state.lab_daemon_next_supervision.lock().await;
        *next = None;
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
pub(crate) async fn reset_workspace_trust(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let input = DesktopWorkspaceTrustInput {
        package_scripts: "ask".to_string(),
        shell_validation: "ask".to_string(),
        lab_daemon_supervision: false,
        developer_auto_acknowledged: false,
    };
    let trust = apply_workspace_trust_for_project(&selected_project, &input)?;
    {
        let mut stored = state.workspace_trust.lock().await;
        *stored = Some(trust);
    }
    {
        let mut enabled = state.lab_daemon_supervision_enabled.lock().await;
        *enabled = false;
    }
    {
        let mut next = state.lab_daemon_next_supervision.lock().await;
        *next = None;
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

async fn apply_desktop_onboarding_input(
    state: &State<'_, DesktopAppState>,
    input: DesktopOnboardingInput,
) -> Result<(), String> {
    let skipped = input.skipped.unwrap_or(false);
    let project = match input.project_root.as_deref() {
        Some(path) if !path.trim().is_empty() => validate_project_path(path)?,
        _ => state.selected_project.lock().await.clone(),
    };
    let permission_mode =
        normalized_permission_mode_label(input.permission_mode.as_deref()).to_string();
    if permission_mode == "auto"
        && !input
            .workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.developer_auto_acknowledged)
    {
        return Err(
            "Developer Auto requires explicit trusted-workspace acknowledgement.".to_string(),
        );
    }
    {
        let mut selected_project = state.selected_project.lock().await;
        *selected_project = project.clone();
    }
    {
        let mut recent_projects = state.recent_projects.lock().await;
        remember_recent_project(&mut recent_projects, project.clone());
    }
    {
        let mut permission = state.permission_mode.lock().await;
        *permission = Some(permission_mode.clone());
    }
    {
        let runtime = state.runtime.lock().await;
        if let Some(runtime) = runtime.as_ref() {
            runtime
                .streaming_engine()
                .set_permission_mode(parse_desktop_permission_mode(&permission_mode));
        }
    }
    let trust_input = input.workspace_trust.unwrap_or(DesktopWorkspaceTrustInput {
        package_scripts: "ask".to_string(),
        shell_validation: "ask".to_string(),
        lab_daemon_supervision: false,
        developer_auto_acknowledged: false,
    });
    let trust = apply_workspace_trust_for_project(&project, &trust_input)?;
    {
        let mut stored = state.workspace_trust.lock().await;
        *stored = Some(trust.clone());
    }
    {
        let mut enabled = state.lab_daemon_supervision_enabled.lock().await;
        *enabled = trust.lab_daemon_supervision;
    }
    {
        let mut next = state.lab_daemon_next_supervision.lock().await;
        *next = None;
    }
    {
        let mut onboarding = state.onboarding_state.lock().await;
        *onboarding = Some(DesktopOnboardingState {
            onboarding_version: 1,
            completed_at: Some(desktop_timestamp()),
            project_root: Some(project.display().to_string()),
            permission_mode: Some(permission_mode),
            workspace_trust_summary: Some(trust),
            credential_storage_acknowledged: input.credential_storage_acknowledged,
            skipped,
            starting_mode: input.starting_mode.or_else(|| Some("direct".to_string())),
        });
    }
    persist_current_settings(state).await
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DesktopTrustedWorkspacesFile {
    #[serde(default)]
    workspaces: Vec<DesktopTrustedWorkspaceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopTrustedWorkspaceRecord {
    canonical_path: String,
    #[serde(default)]
    repo_identity: String,
    #[serde(default)]
    repo_fingerprint: String,
    #[serde(default)]
    trusted_at: String,
    #[serde(default)]
    trust_scope: String,
    #[serde(default)]
    trust_scopes: Vec<String>,
    #[serde(default)]
    approved_by: String,
    #[serde(default)]
    source: String,
}

pub(crate) fn apply_workspace_trust_for_project(
    project: &Path,
    input: &DesktopWorkspaceTrustInput,
) -> Result<DesktopWorkspaceTrustStatus, String> {
    let canonical = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let repo_identity = desktop_repo_identity(&canonical).unwrap_or_else(|| "unknown".to_string());
    let repo_fingerprint = desktop_repo_fingerprint(&canonical, &repo_identity);
    let trusted_capabilities = desktop_trust_capabilities(input);
    let last_updated = Some(desktop_timestamp());
    let status = DesktopWorkspaceTrustStatus {
        canonical_project_path: canonical.display().to_string(),
        repo_identity,
        repo_fingerprint,
        trust_source: if trusted_capabilities.is_empty() {
            "desktop_settings_no_trust".to_string()
        } else {
            "trusted_workspaces_file".to_string()
        },
        package_scripts: normalized_trust_choice(&input.package_scripts),
        shell_validation: normalized_trust_choice(&input.shell_validation),
        lab_daemon_supervision: input.lab_daemon_supervision,
        developer_auto_acknowledged: input.developer_auto_acknowledged,
        trusted_capabilities,
        last_updated,
    };
    sync_desktop_trusted_workspace_record(&status)?;
    Ok(status)
}

pub(crate) fn desktop_workspace_trust_status(
    project: &Path,
    stored: Option<DesktopWorkspaceTrustStatus>,
) -> DesktopWorkspaceTrustStatus {
    let canonical = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let canonical_project_path = canonical.display().to_string();
    let repo_identity = desktop_repo_identity(&canonical).unwrap_or_else(|| "unknown".to_string());
    let repo_fingerprint = desktop_repo_fingerprint(&canonical, &repo_identity);
    if let Some(record) = read_desktop_trusted_workspaces()
        .workspaces
        .into_iter()
        .find(|record| {
            record.canonical_path == canonical_project_path
                && ((!record.repo_identity.is_empty() && record.repo_identity == repo_identity)
                    || (!record.repo_fingerprint.is_empty()
                        && record.repo_fingerprint == repo_fingerprint))
        })
    {
        let capabilities = normalized_desktop_trust_scopes(&record);
        return DesktopWorkspaceTrustStatus {
            canonical_project_path,
            repo_identity,
            repo_fingerprint,
            trust_source: if record.source.is_empty() {
                "trusted_workspaces_file".to_string()
            } else {
                record.source
            },
            package_scripts: trust_choice_for_capability(&capabilities, "allow_package_scripts"),
            shell_validation: trust_choice_for_capability(&capabilities, "allow_shell_validation"),
            lab_daemon_supervision: capabilities
                .iter()
                .any(|capability| capability == "allow_lab_daemon_supervision"),
            developer_auto_acknowledged: capabilities
                .iter()
                .any(|capability| capability == "allow_developer_auto"),
            trusted_capabilities: capabilities,
            last_updated: (!record.trusted_at.is_empty()).then_some(record.trusted_at),
        };
    }
    if let Some(mut stored) = stored.filter(|stored| {
        stored.canonical_project_path == canonical_project_path
            || stored.repo_fingerprint == repo_fingerprint
    }) {
        stored.canonical_project_path = canonical_project_path;
        stored.repo_identity = repo_identity;
        stored.repo_fingerprint = repo_fingerprint;
        return stored;
    }
    DesktopWorkspaceTrustStatus {
        canonical_project_path,
        repo_identity,
        repo_fingerprint,
        trust_source: "no_project_trust_record".to_string(),
        package_scripts: "ask".to_string(),
        shell_validation: "ask".to_string(),
        lab_daemon_supervision: false,
        developer_auto_acknowledged: false,
        trusted_capabilities: Vec::new(),
        last_updated: None,
    }
}

fn sync_desktop_trusted_workspace_record(
    status: &DesktopWorkspaceTrustStatus,
) -> Result<(), String> {
    let Some(path) = desktop_trusted_workspaces_path() else {
        return Ok(());
    };
    let mut file = read_desktop_trusted_workspaces_from_path(&path);
    file.workspaces.retain(|record| {
        !(record.canonical_path == status.canonical_project_path
            && (record.repo_identity == status.repo_identity
                || record.repo_fingerprint == status.repo_fingerprint))
    });
    if !status.trusted_capabilities.is_empty() {
        file.workspaces.push(DesktopTrustedWorkspaceRecord {
            canonical_path: status.canonical_project_path.clone(),
            repo_identity: status.repo_identity.clone(),
            repo_fingerprint: status.repo_fingerprint.clone(),
            trusted_at: status
                .last_updated
                .clone()
                .unwrap_or_else(desktop_timestamp),
            trust_scope: String::new(),
            trust_scopes: status.trusted_capabilities.clone(),
            approved_by: "desktop".to_string(),
            source: "desktop_trust_wizard".to_string(),
        });
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let content = serde_json::to_string_pretty(&file).map_err(|err| err.to_string())?;
    std::fs::write(path, content).map_err(|err| err.to_string())
}

fn read_desktop_trusted_workspaces() -> DesktopTrustedWorkspacesFile {
    desktop_trusted_workspaces_path()
        .map(|path| read_desktop_trusted_workspaces_from_path(&path))
        .unwrap_or_default()
}

fn read_desktop_trusted_workspaces_from_path(path: &Path) -> DesktopTrustedWorkspacesFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn desktop_trusted_workspaces_path() -> Option<PathBuf> {
    std::env::var_os("PRIORITY_AGENT_TRUSTED_WORKSPACES_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                PathBuf::from(home)
                    .join(".priority-agent")
                    .join("trusted-workspaces.json")
            })
        })
}

fn normalized_desktop_trust_scopes(record: &DesktopTrustedWorkspaceRecord) -> Vec<String> {
    let mut scopes = record.trust_scopes.clone();
    if record.trust_scope == "package_scripts" || record.trust_scope == "allow_package_scripts" {
        scopes.push("allow_package_scripts".to_string());
    }
    scopes.sort();
    scopes.dedup();
    scopes
}

fn desktop_trust_capabilities(input: &DesktopWorkspaceTrustInput) -> Vec<String> {
    let mut capabilities = Vec::new();
    if normalized_trust_choice(&input.package_scripts) == "trusted" {
        capabilities.push("allow_package_scripts".to_string());
    }
    if normalized_trust_choice(&input.shell_validation) == "trusted" {
        capabilities.push("allow_shell_validation".to_string());
    }
    if input.lab_daemon_supervision {
        capabilities.push("allow_lab_daemon_supervision".to_string());
    }
    if input.developer_auto_acknowledged {
        capabilities.push("allow_developer_auto".to_string());
    }
    capabilities
}

fn normalized_trust_choice(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "trusted" | "trust" | "allow" | "allowed" | "yes" | "true" => "trusted".to_string(),
        _ => "ask".to_string(),
    }
}

fn trust_choice_for_capability(capabilities: &[String], capability: &str) -> String {
    if capabilities.iter().any(|item| item == capability) {
        "trusted".to_string()
    } else {
        "ask".to_string()
    }
}

fn desktop_repo_identity(cwd: &Path) -> Option<String> {
    git_stdout(cwd, &["config", "--get", "remote.origin.url"])
        .or_else(|| git_stdout(cwd, &["rev-parse", "--show-toplevel"]))
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn desktop_repo_fingerprint(canonical: &Path, repo_identity: &str) -> String {
    sha256_text(&format!("{}\0{}", canonical.display(), repo_identity))
}

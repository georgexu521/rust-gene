use super::*;

#[tauri::command]
pub(crate) async fn open_settings_folder(state: State<'_, DesktopAppState>) -> Result<(), String> {
    let folder = state
        .settings_path
        .parent()
        .ok_or_else(|| "settings path has no parent directory".to_string())?;
    open_path(folder)
}

#[tauri::command]
pub(crate) async fn open_diagnostics_folder(
    state: State<'_, DesktopAppState>,
) -> Result<(), String> {
    let folder = state
        .diagnostic_logs_path
        .parent()
        .ok_or_else(|| "diagnostic log path has no parent directory".to_string())?;
    std::fs::create_dir_all(folder).map_err(|err| err.to_string())?;
    open_path(folder)
}

#[tauri::command]
pub(crate) async fn open_file_path(
    path: String,
    state: State<'_, DesktopAppState>,
) -> Result<(), String> {
    let selected_project = state.selected_project.lock().await.clone();
    let target = scoped_desktop_open_target(
        &path,
        &selected_project,
        &state.settings_path,
        &state.diagnostic_logs_path,
    )?;
    open_path(&target)
}

#[tauri::command]
pub(crate) async fn open_shell_profile() -> Result<(), String> {
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
pub(crate) async fn save_provider_credential(
    provider_id: String,
    key: String,
) -> Result<String, String> {
    save_desktop_provider_credential(&provider_id, &key)
}

#[tauri::command]
pub(crate) async fn provider_credential_backend_status(
    provider_id: String,
) -> Result<DesktopCredentialProviderBackendStatus, String> {
    desktop_provider_credential_backend_status(&provider_id)
}

#[tauri::command]
pub(crate) async fn delete_provider_credential(provider_id: String) -> Result<String, String> {
    delete_desktop_provider_credential(&provider_id)
}

#[tauri::command]
pub(crate) async fn migrate_provider_credential_to_keychain(
    provider_id: String,
) -> Result<String, String> {
    migrate_desktop_provider_credential_to_keychain(&provider_id)
}

pub(crate) fn scoped_desktop_open_target(
    requested: &str,
    selected_project: &Path,
    settings_path: &Path,
    diagnostic_logs_path: &Path,
) -> Result<PathBuf, String> {
    let requested_path = PathBuf::from(requested);
    if requested_path.as_os_str().is_empty() {
        return Err("path cannot be empty".to_string());
    }
    let allowed_roots =
        desktop_open_allowed_roots(selected_project, settings_path, diagnostic_logs_path)?;
    let candidate = if requested_path.is_absolute() {
        requested_path
    } else {
        selected_project.join(requested_path)
    };
    let existing_target = if candidate.exists() {
        candidate
            .canonicalize()
            .map_err(|err| format!("failed to resolve {}: {err}", candidate.display()))?
    } else {
        nearest_existing_allowed_parent(&candidate, &allowed_roots)?
    };
    if allowed_roots
        .iter()
        .any(|root| existing_target == *root || existing_target.starts_with(root))
    {
        Ok(existing_target)
    } else {
        Err(format!(
            "path is outside allowed desktop open roots: {}",
            requested
        ))
    }
}

fn desktop_open_allowed_roots(
    selected_project: &Path,
    settings_path: &Path,
    diagnostic_logs_path: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    for root in [
        selected_project.to_path_buf(),
        selected_project.join(".priority-agent/lab"),
        settings_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| settings_path.to_path_buf()),
        diagnostic_logs_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| diagnostic_logs_path.to_path_buf()),
    ] {
        if root.exists() {
            roots.push(root.canonicalize().map_err(|err| {
                format!("failed to resolve allowed root {}: {err}", root.display())
            })?);
        }
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn nearest_existing_allowed_parent(
    candidate: &Path,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, String> {
    let mut current = candidate;
    while let Some(parent) = current.parent() {
        if parent.exists() {
            let resolved = parent
                .canonicalize()
                .map_err(|err| format!("failed to resolve {}: {err}", parent.display()))?;
            if allowed_roots
                .iter()
                .any(|root| resolved == *root || resolved.starts_with(root))
            {
                return Ok(resolved);
            }
            break;
        }
        current = parent;
    }
    Err(format!("path does not exist: {}", candidate.display()))
}

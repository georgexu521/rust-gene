use super::*;

#[tauri::command]
pub(crate) async fn desktop_credential_storage_status(
) -> Result<DesktopCredentialStorageStatus, String> {
    Ok(desktop_credential_storage_status_value())
}

#[tauri::command]
pub(crate) async fn export_desktop_diagnostics_bundle(
    redaction: Option<DesktopDiagnosticsRedaction>,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopDiagnosticsBundleResult, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let settings_path = state.settings_path.clone();
    let diagnostic_logs_path = state.diagnostic_logs_path.clone();
    let provider = provider_model_status_for_state(&state).await.ok();
    let redaction = redaction.unwrap_or(DesktopDiagnosticsRedaction {
        include_logs: Some(true),
        max_log_bytes: Some(32 * 1024),
        include_full_paths: Some(false),
    });
    let include_logs = redaction.include_logs.unwrap_or(true);
    let include_full_paths = redaction.include_full_paths.unwrap_or(false);
    let max_log_bytes = redaction.max_log_bytes.unwrap_or(32 * 1024).min(128 * 1024);
    let log_preview = if include_logs {
        read_redacted_log_preview(&diagnostic_logs_path, max_log_bytes)
    } else {
        None
    };
    let stored_workspace_trust = state.workspace_trust.lock().await.clone();
    let workspace_trust = desktop_workspace_trust_status(&selected_project, stored_workspace_trust);
    let onboarding_state = state.onboarding_state.lock().await.clone();
    let permission_mode =
        normalized_permission_mode_label(state.permission_mode.lock().await.as_deref()).to_string();
    let detail_level =
        normalized_detail_level_label(state.detail_level.lock().await.as_deref()).to_string();
    let agent_mode = state
        .agent_mode
        .lock()
        .await
        .clone()
        .unwrap_or_else(|| "auto".to_string());
    let lab_status = desktop_lab_status_for_project(&selected_project);
    let diagnostics =
        collect_desktop_diagnostics(&selected_project, &settings_path, &diagnostic_logs_path);
    let payload = serde_json::json!({
        "schema": "priority_agent.desktop_diagnostics_bundle.v1",
        "created_at": desktop_timestamp(),
        "app_version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "privacy": "redacted",
        "selected_project": {
            "basename": selected_project.file_name().and_then(|value| value.to_str()).unwrap_or("project"),
            "path_hash": sha256_text(&selected_project.display().to_string()),
            "canonical_path_hash": workspace_trust.repo_fingerprint,
        },
        "settings": {
            "permission_mode": permission_mode,
            "detail_level": detail_level,
            "agent_mode": agent_mode,
            "settings_path": desktop_path_descriptor(&settings_path, "settings", include_full_paths),
            "diagnostic_logs_path": desktop_path_descriptor(&diagnostic_logs_path, "diagnostics", include_full_paths),
            "onboarding": onboarding_state,
            "workspace_trust": workspace_trust,
            "credential_storage": desktop_credential_storage_status_value(),
        },
        "provider": provider.map(|status| serde_json::json!({
            "active_provider": status.active_provider,
            "active_provider_label": status.active_provider_label,
            "active_model": status.active_model,
            "runtime_provider_ready": status.runtime_provider_ready,
            "selection_source": status.selection_source,
            "configured_count": status.configured_count,
        })),
        "diagnostics": diagnostics,
        "labrun": {
            "available": lab_status.available,
            "state": lab_status.state,
            "detail": lab_status.detail,
            "lab_run_id": lab_status.lab_run_id,
            "stage": lab_status.stage,
            "owner": lab_status.owner,
            "needs_user": lab_status.needs_user,
            "artifact_count": lab_status.artifact_count,
            "task_total": lab_status.task_total,
            "task_open": lab_status.task_open,
            "task_blocked": lab_status.task_blocked,
            "evidence_ref_count": lab_status.evidence_refs.len(),
            "latest_report_path_hash": lab_status.latest_report_path.map(|path| sha256_text(&path)),
        },
        "recent_logs": log_preview,
    });
    let export_dir = diagnostic_logs_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("exports");
    std::fs::create_dir_all(&export_dir).map_err(|err| err.to_string())?;
    let path = export_dir.join(format!(
        "priority-agent-desktop-diagnostics-{}.json",
        desktop_timestamp().replace(':', "-")
    ));
    let text = serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())?;
    std::fs::write(&path, text).map_err(|err| err.to_string())?;
    append_desktop_log(
        &diagnostic_logs_path,
        &format!(
            "diagnostics_export path={}",
            sanitize_log_value(&path.display().to_string())
        ),
    )?;
    Ok(DesktopDiagnosticsBundleResult {
        path: path.display().to_string(),
        privacy: "redacted".to_string(),
        redacted: true,
        summary: "Redacted desktop diagnostics bundle exported".to_string(),
    })
}

fn read_redacted_log_preview(path: &Path, max_bytes: usize) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let start = bytes.len().saturating_sub(max_bytes);
    let text = String::from_utf8_lossy(&bytes[start..]).to_string();
    Some(redact_desktop_support_text(&text))
}

pub(crate) fn desktop_path_descriptor(
    path: &Path,
    kind: &str,
    include_full_path: bool,
) -> serde_json::Value {
    let full_path = path.display().to_string();
    let basename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(kind)
        .to_string();
    let parent_hash = path
        .parent()
        .map(|parent| sha256_text(&parent.display().to_string()));
    let mut value = serde_json::json!({
        "kind": kind,
        "basename": basename,
        "path_hash": sha256_text(&full_path),
        "parent_hash": parent_hash,
    });
    if include_full_path {
        value["full_path"] = serde_json::Value::String(full_path);
    }
    value
}

pub(crate) fn redact_desktop_support_text(text: &str) -> String {
    let mut output = Vec::new();
    let mut private_key_block = false;
    for line in text.lines() {
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----") {
            private_key_block = true;
            output.push("-----BEGIN PRIVATE KEY----- <redacted>".to_string());
            continue;
        }
        if private_key_block {
            if line.contains("-----END") && line.contains("PRIVATE KEY-----") {
                private_key_block = false;
                output.push("-----END PRIVATE KEY----- <redacted>".to_string());
            }
            continue;
        }
        output.push(redact_desktop_support_line(line));
    }
    output.join("\n")
}

fn redact_desktop_support_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("authorization:") {
        return "Authorization: <redacted>".to_string();
    }
    if let Some(index) = lower.find("bearer ") {
        return format!("{}Bearer <redacted>", &line[..index]);
    }
    if is_secret_assignment(&lower) {
        if let Some(index) = line.find('=') {
            return format!("{}=<redacted>", &line[..index]);
        }
    }
    redact_high_entropy_words(&redact_local_paths(line))
}

fn redact_local_paths(line: &str) -> String {
    line.split_whitespace()
        .map(|word| {
            let trimmed = word.trim_matches(|ch: char| {
                matches!(ch, '"' | '\'' | ',' | ';' | ')' | '(' | '[' | ']')
            });
            let local_path_start = ["/Users/", "/private/var/", "/var/folders/"]
                .iter()
                .filter_map(|prefix| trimmed.find(prefix))
                .min();
            let Some(start) = local_path_start else {
                return word.to_string();
            };
            let redacted = format!("{}<redacted-path>", &trimmed[..start]);
            word.replace(trimmed, &redacted)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_secret_assignment(lower: &str) -> bool {
    let Some(index) = lower.find('=') else {
        return false;
    };
    let key = lower[..index].trim();
    key.ends_with("_key")
        || key.ends_with("_token")
        || key.ends_with("_secret")
        || key.contains("api_key")
        || key.contains("access_token")
        || key.contains("credential")
}

fn redact_high_entropy_words(line: &str) -> String {
    line.split_whitespace()
        .map(|word| {
            let trimmed = word.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
            if looks_like_secret_token(trimmed) {
                word.replace(trimmed, "<redacted-token>")
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_secret_token(value: &str) -> bool {
    if value.starts_with("sk-") && value.len() > 12 {
        return true;
    }
    if value.len() < 40 {
        return false;
    }
    let alpha = value.chars().filter(|ch| ch.is_ascii_alphabetic()).count();
    let digit = value.chars().filter(|ch| ch.is_ascii_digit()).count();
    let symbol = value
        .chars()
        .filter(|ch| matches!(ch, '-' | '_' | '.' | '/' | '+'))
        .count();
    alpha > 12 && digit > 4 && symbol > 0
}

use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[tauri::command]
pub(crate) async fn lab_daemon_supervise(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopLabDaemonActionResult, String> {
    let selected_project = state.selected_project.lock().await.clone();
    let result = desktop_lab_daemon_supervise_for_project(&selected_project);
    let now = desktop_unix_millis_string();
    {
        let mut last = state.lab_daemon_last_supervision.lock().await;
        *last = Some(format!("{}: {}", now, result.action));
    }
    {
        let mut last_result = state.lab_daemon_last_supervision_result.lock().await;
        *last_result = Some(compact_desktop_supervision_result(&result.output));
    }
    {
        let enabled = *state.lab_daemon_supervision_enabled.lock().await;
        let mut next = state.lab_daemon_next_supervision.lock().await;
        *next = enabled.then(|| desktop_next_supervision_hint(120));
    }
    Ok(result)
}

pub(crate) fn desktop_lab_daemon_supervise_for_project(
    project: &std::path::Path,
) -> DesktopLabDaemonActionResult {
    let output = priority_agent::lab::commands::handle_lab_command(
        project,
        None,
        "daemon service supervise",
    );
    DesktopLabDaemonActionResult {
        action: "supervise",
        output,
        lab_status: desktop_lab_status_for_project(project),
    }
}

fn desktop_unix_millis_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn desktop_next_supervision_hint(delay_secs: u64) -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| (duration.as_secs() + delay_secs).to_string())
        .unwrap_or_else(|_| delay_secs.to_string())
}

fn compact_desktop_supervision_result(output: &str) -> String {
    const MAX_CHARS: usize = 240;
    let normalized = output.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= MAX_CHARS {
        return normalized;
    }
    let mut compact = normalized.chars().take(MAX_CHARS).collect::<String>();
    compact.push_str("...");
    compact
}

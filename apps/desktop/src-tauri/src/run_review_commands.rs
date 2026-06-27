use super::*;
use std::io::Write;

#[tauri::command]
pub(crate) async fn accept_run_review(
    input: DesktopRunReviewAcceptanceInput,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopRunReviewAcceptanceResult, String> {
    let acceptance = DesktopRunReviewAcceptance {
        schema: "priority_agent.desktop_run_review_acceptance.v1".to_string(),
        run_id: sanitize_acceptance_text(&input.run_id, 160),
        session_id: input
            .session_id
            .as_deref()
            .map(|value| sanitize_acceptance_text(value, 160)),
        accepted_at: desktop_timestamp(),
        changed_files: input
            .changed_files
            .iter()
            .take(32)
            .map(|value| sanitize_acceptance_text(value, 240))
            .collect(),
        validation_status: input
            .validation_status
            .as_deref()
            .map(|value| sanitize_acceptance_text(value, 120))
            .unwrap_or_else(|| "unknown".to_string()),
        permission_summary: input
            .permission_summary
            .as_deref()
            .map(|value| sanitize_acceptance_text(value, 240))
            .unwrap_or_else(|| "not_recorded".to_string()),
        residual_risk_count: input.residual_risk_count.unwrap_or(0).min(128),
        trace_refs: input
            .trace_refs
            .iter()
            .take(32)
            .map(|value| sanitize_acceptance_text(value, 160))
            .collect(),
        tool_output_refs: input
            .tool_output_refs
            .iter()
            .take(32)
            .map(|value| sanitize_acceptance_text(value, 160))
            .collect(),
    };
    let path = desktop_run_review_acceptance_path(&state)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| err.to_string())?;
    let line = serde_json::to_string(&acceptance).map_err(|err| err.to_string())?;
    writeln!(file, "{line}").map_err(|err| err.to_string())?;
    append_desktop_log(
        &state.diagnostic_logs_path,
        &format!(
            "run_review_accepted run_id={} session_id={}",
            sanitize_log_value(&acceptance.run_id),
            acceptance
                .session_id
                .as_deref()
                .map(sanitize_log_value)
                .unwrap_or_else(|| "none".to_string())
        ),
    )?;
    Ok(DesktopRunReviewAcceptanceResult {
        accepted: true,
        run_id: acceptance.run_id,
        path: path.display().to_string(),
        accepted_at: acceptance.accepted_at,
    })
}

fn desktop_run_review_acceptance_path(
    state: &State<'_, DesktopAppState>,
) -> Result<PathBuf, String> {
    let base = state
        .diagnostic_logs_path
        .parent()
        .ok_or_else(|| "diagnostic log path has no parent directory".to_string())?;
    Ok(base.join("run-review-acceptances.jsonl"))
}

fn sanitize_acceptance_text(value: &str, max_chars: usize) -> String {
    let redacted = redact_desktop_support_text(value);
    let compact = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }
    let mut out = compact.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

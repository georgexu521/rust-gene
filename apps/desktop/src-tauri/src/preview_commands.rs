use super::*;
use std::io::Read;

#[tauri::command]
pub(crate) async fn desktop_lab_report_page(
    path: String,
    offset: Option<u64>,
    limit: Option<u64>,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopLabReportPage, String> {
    let selected_project = state.selected_project.lock().await.clone();
    desktop_lab_report_page_for_project(
        &selected_project,
        &path,
        offset.unwrap_or(0),
        limit.unwrap_or(32 * 1024),
    )
}

#[tauri::command]
pub(crate) async fn desktop_lab_artifact_body(
    artifact_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopLabArtifactBody, String> {
    let selected_project = state.selected_project.lock().await.clone();
    desktop_lab_artifact_body_for_project(&selected_project, &artifact_id)
}

pub(crate) fn desktop_lab_artifact_body_for_project(
    selected_project: &std::path::Path,
    artifact_id: &str,
) -> Result<DesktopLabArtifactBody, String> {
    let artifact_id = artifact_id.trim();
    if artifact_id.is_empty() {
        return Err("artifact_id cannot be empty".to_string());
    }
    let store = priority_agent::lab::store::LabStore::for_project(selected_project);
    let run = store
        .latest_run()
        .map_err(|err| format!("Failed to read LabRun state: {err}"))?
        .ok_or_else(|| "No LabRun found for selected project.".to_string())?;
    if !run.artifact_ids.iter().any(|id| id == artifact_id) {
        return Err("Artifact is not registered on the latest LabRun.".to_string());
    }
    let artifact = store
        .load_stage_artifact(&run.lab_run_id, artifact_id)
        .map_err(|err| format!("Failed to read Lab artifact: {err}"))?;
    let body = desktop_lab_artifact_body_value(&artifact)?;
    let content = serde_json::to_string_pretty(&body)
        .map_err(|err| format!("Failed to format Lab artifact body: {err}"))?;
    Ok(DesktopLabArtifactBody {
        artifact_id: artifact.artifact_id().to_string(),
        artifact_type: artifact.artifact_type().as_str().to_string(),
        title: desktop_lab_artifact_title(&artifact).to_string(),
        stage: artifact.stage().to_string(),
        owner: format!("{:?}", artifact.owner()),
        status: format!("{:?}", artifact.status()),
        validation_status: artifact.validation_status().map(str::to_string),
        content,
    })
}

fn desktop_lab_artifact_body_value(
    artifact: &priority_agent::lab::model::StageArtifact,
) -> Result<serde_json::Value, String> {
    match artifact {
        priority_agent::lab::model::StageArtifact::ProfessorPlan(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::PostdocPlan(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::GraduateResult(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::PostdocIntegrationSummary(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::ProfessorReview(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::CycleSummary(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::CompressionSummary(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::LabMeetingRequest(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::LabMeetingSummary(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::LabBlockerReport(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::LabRevisionTask(envelope) => {
            serde_json::to_value(&envelope.body)
        }
        priority_agent::lab::model::StageArtifact::ProfessorSteeringDecision(envelope) => {
            serde_json::to_value(&envelope.body)
        }
    }
    .map_err(|err| format!("Failed to serialize Lab artifact body: {err}"))
}

pub(crate) fn desktop_lab_report_page_for_project(
    selected_project: &std::path::Path,
    path: &str,
    offset: u64,
    limit: u64,
) -> Result<DesktopLabReportPage, String> {
    let requested = std::path::PathBuf::from(path);
    let report_path = if requested.is_absolute() {
        requested
    } else {
        selected_project.join(requested)
    };
    let project_root = selected_project
        .canonicalize()
        .map_err(|err| format!("Failed to resolve selected project: {err}"))?;
    let lab_root = project_root.join(".priority-agent").join("lab");
    let report_path = report_path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve Lab report path: {err}"))?;

    if !report_path.starts_with(&lab_root) {
        return Err("Lab report path must be inside .priority-agent/lab.".to_string());
    }
    if report_path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Err("Lab report path must be a markdown file.".to_string());
    }
    if !report_path.is_file() {
        return Err("Lab report path is not a file.".to_string());
    }

    let bytes =
        std::fs::read(&report_path).map_err(|err| format!("Failed to read Lab report: {err}"))?;
    let total_bytes = bytes.len() as u64;
    let offset = offset.min(total_bytes);
    let limit = limit.clamp(1, 128 * 1024);
    let end = offset.saturating_add(limit).min(total_bytes);
    let content = String::from_utf8_lossy(&bytes[offset as usize..end as usize]).to_string();
    Ok(DesktopLabReportPage {
        path: report_path.display().to_string(),
        content,
        offset,
        limit,
        total_bytes,
        has_more: end < total_bytes,
    })
}

#[tauri::command]
pub(crate) async fn desktop_file_preview(
    path: String,
    limit: Option<u64>,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopFilePreview, String> {
    let selected_project = state.selected_project.lock().await.clone();
    desktop_file_preview_for_project(&selected_project, &path, limit)
}

pub(crate) fn desktop_file_preview_for_project(
    selected_project: &std::path::Path,
    path: &str,
    limit: Option<u64>,
) -> Result<DesktopFilePreview, String> {
    let requested = path.trim();
    if requested.is_empty() {
        return Err("file preview path cannot be empty".to_string());
    }
    let requested_path = std::path::PathBuf::from(requested);
    if requested_path.is_absolute() {
        return Err("file preview path must be relative to the selected project".to_string());
    }

    let project_root = selected_project
        .canonicalize()
        .map_err(|err| format!("Failed to resolve selected project: {}", err))?;
    let file_path = project_root
        .join(requested_path)
        .canonicalize()
        .map_err(|err| format!("Failed to resolve file preview path: {}", err))?;

    if !file_path.starts_with(&project_root) {
        return Err("File preview must stay inside the selected project.".to_string());
    }
    if !file_path.is_file() {
        return Err("File preview path is not a file.".to_string());
    }

    let max_bytes = limit.unwrap_or(32 * 1024).clamp(1024, 64 * 1024);
    let metadata = std::fs::metadata(&file_path)
        .map_err(|err| format!("Failed to inspect file preview metadata: {}", err))?;
    let mut file =
        std::fs::File::open(&file_path).map_err(|err| format!("Failed to open file: {}", err))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("Failed to read file preview: {}", err))?;
    let truncated = bytes.len() as u64 > max_bytes || metadata.len() > max_bytes;
    if bytes.len() as u64 > max_bytes {
        bytes.truncate(max_bytes as usize);
    }

    let content = String::from_utf8_lossy(&bytes).to_string();
    let line_count = content.lines().count() as i64;
    let relative_path = file_path
        .strip_prefix(&project_root)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();

    Ok(DesktopFilePreview {
        path: relative_path,
        content,
        line_count,
        total_bytes: metadata.len(),
        truncated,
    })
}

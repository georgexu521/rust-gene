use priority_agent::desktop_runtime::DesktopContextSnapshot;
use priority_agent::session_store::SessionStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DesktopRunContext {
    #[serde(rename = "type")]
    pub(crate) context_type: String,
    pub(crate) label: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) line_start: Option<usize>,
    pub(crate) line_end: Option<usize>,
    pub(crate) selection_text: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ResolvedDesktopRunContext {
    #[serde(rename = "type")]
    pub(crate) context_type: String,
    pub(crate) label: String,
    pub(crate) shortstat: String,
    pub(crate) files: Vec<String>,
    pub(crate) stat: String,
    #[serde(rename = "patch_preview")]
    pub(crate) patch_preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) relative_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line_start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line_end: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) preview: Option<String>,
    pub(crate) truncated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopWorkbenchSnapshot {
    pub(crate) selected_project: String,
    pub(crate) project_map: DesktopProjectMapSnapshot,
    pub(crate) symbol_index: DesktopSymbolIndexSnapshot,
    pub(crate) changed_files: Vec<String>,
    pub(crate) runtime_context: Option<DesktopContextSnapshot>,
    pub(crate) lab_status: DesktopLabStatusSnapshot,
    pub(crate) subagent_tasks: Vec<DesktopSubagentTaskSnapshot>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopProjectMapSnapshot {
    pub(crate) available: bool,
    pub(crate) source: Option<String>,
    pub(crate) freshness: String,
    pub(crate) chars: usize,
    pub(crate) truncated: bool,
    pub(crate) content_preview: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopSymbolIndexSnapshot {
    pub(crate) schema_version: u8,
    pub(crate) total_symbols: usize,
    pub(crate) files: Vec<priority_agent::engine::project_map::ProjectIndexedFile>,
    pub(crate) truncated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopLabStatusSnapshot {
    pub(crate) available: bool,
    pub(crate) state: String,
    pub(crate) detail: String,
    pub(crate) lab_run_id: Option<String>,
    pub(crate) proposal_id: Option<String>,
    pub(crate) proposal_status: Option<String>,
    pub(crate) run_status: Option<String>,
    pub(crate) stage: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) needs_user: bool,
    pub(crate) cycle_count: u64,
    pub(crate) artifact_count: usize,
    pub(crate) meeting_count: usize,
    pub(crate) task_total: usize,
    pub(crate) task_open: usize,
    pub(crate) task_blocked: usize,
    pub(crate) blockers: Vec<String>,
    pub(crate) validation_retry_count: usize,
    pub(crate) validation_retry_escalated_count: usize,
    pub(crate) latest_validation_retry: Option<String>,
    pub(crate) meeting_recommended: bool,
    pub(crate) meeting_topic: Option<String>,
    pub(crate) latest_report_path: Option<String>,
    pub(crate) daemon_policy: Option<DesktopLabDaemonPolicySnapshot>,
    pub(crate) artifacts: Vec<DesktopLabArtifactSnapshot>,
    pub(crate) reports: Vec<DesktopLabReportSnapshot>,
    pub(crate) evidence_refs: Vec<DesktopLabEvidenceSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopLabDaemonPolicySnapshot {
    pub(crate) enabled: bool,
    pub(crate) mode: priority_agent::lab::model::LabDaemonMode,
    pub(crate) max_steps: usize,
    pub(crate) max_steps_per_cycle: usize,
    pub(crate) interval_ms: u64,
    pub(crate) last_started_at: Option<String>,
    pub(crate) last_start_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopLabArtifactSnapshot {
    pub(crate) artifact_id: String,
    pub(crate) artifact_type: String,
    pub(crate) stage: String,
    pub(crate) owner: String,
    pub(crate) status: String,
    pub(crate) validation_status: Option<String>,
    pub(crate) title: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) report_path: Option<String>,
    pub(crate) report_preview: Option<String>,
    pub(crate) report_preview_truncated: bool,
    pub(crate) evidence_refs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopLabReportSnapshot {
    pub(crate) artifact_id: String,
    pub(crate) path: String,
    pub(crate) preview: Option<String>,
    pub(crate) truncated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopLabEvidenceSnapshot {
    pub(crate) evidence_id: String,
    pub(crate) kind: String,
    pub(crate) role: String,
    pub(crate) reference: String,
    pub(crate) summary: String,
    pub(crate) artifact_id: Option<String>,
    pub(crate) cycle_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) estimated_summary_tokens: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopSubagentTaskSnapshot {
    pub(crate) task_id: String,
    pub(crate) agent_id: String,
    pub(crate) profile: Option<String>,
    pub(crate) role: String,
    pub(crate) status: String,
    pub(crate) description: String,
    pub(crate) child_session_id: Option<String>,
    pub(crate) result_artifact_id: Option<i64>,
    pub(crate) artifact_status: Option<String>,
    pub(crate) result_preview: Option<String>,
    pub(crate) tools_used: Vec<String>,
    pub(crate) proof_kind: Option<String>,
    pub(crate) completion_sink: Option<String>,
    pub(crate) recovery_status: Option<String>,
    pub(crate) recovery_action: Option<String>,
    pub(crate) updated_at: String,
}

pub(crate) fn desktop_subagent_tasks_for_session(
    store: &SessionStore,
    session_id: Option<&str>,
) -> Vec<DesktopSubagentTaskSnapshot> {
    let Some(session_id) = session_id else {
        return Vec::new();
    };
    store
        .recent_agent_task_states(session_id, 8)
        .unwrap_or_default()
        .into_iter()
        .map(|state| {
            let artifact = state
                .result_artifact_id
                .and_then(|id| store.agent_artifact(session_id, id).ok().flatten());
            let result_preview = artifact.as_ref().map(|artifact| {
                compact_desktop_lab_text(
                    artifact
                        .output
                        .lines()
                        .next()
                        .filter(|line| !line.trim().is_empty())
                        .unwrap_or(&artifact.description),
                    120,
                )
            });
            DesktopSubagentTaskSnapshot {
                task_id: state.task_id,
                agent_id: state.agent_id,
                profile: state.profile,
                role: state.role,
                status: state.status,
                description: compact_desktop_lab_text(&state.description, 120),
                child_session_id: state
                    .payload
                    .get("child_session_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                result_artifact_id: state.result_artifact_id,
                artifact_status: artifact.as_ref().map(|artifact| artifact.status.clone()),
                result_preview,
                tools_used: artifact
                    .as_ref()
                    .and_then(|artifact| string_array_field(&artifact.payload, "tools_used"))
                    .or_else(|| string_array_field(&state.payload, "tools_used"))
                    .unwrap_or_default(),
                proof_kind: state
                    .payload
                    .get("proof_kind")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                completion_sink: artifact
                    .as_ref()
                    .and_then(|artifact| artifact.payload.get("completion_sink"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        state
                            .payload
                            .get("completion_sink")
                            .and_then(|value| value.as_str())
                            .map(str::to_string)
                    }),
                recovery_status: state
                    .payload
                    .get("recovery_status")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                recovery_action: state
                    .payload
                    .get("recovery_action")
                    .and_then(|value| value.as_str())
                    .map(|value| compact_desktop_lab_text(value, 120)),
                updated_at: state.updated_at,
            }
        })
        .collect()
}

fn string_array_field(value: &serde_json::Value, field: &str) -> Option<Vec<String>> {
    Some(
        value
            .get(field)?
            .as_array()?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

pub(crate) fn desktop_lab_status_for_project(project: &Path) -> DesktopLabStatusSnapshot {
    let store = priority_agent::lab::store::LabStore::for_project(project);
    let daemon_policy = match store.load_daemon_state() {
        Ok(state) => state.map(desktop_lab_daemon_policy_snapshot),
        Err(err) => return desktop_lab_status_error(err),
    };
    match store.latest_run() {
        Ok(Some(run)) => {
            let tasks = store
                .list_graduate_tasks(&run.lab_run_id)
                .unwrap_or_default();
            let task_open = tasks.iter().filter(|task| task.status.is_open()).count();
            let task_blocked = tasks
                .iter()
                .filter(|task| {
                    matches!(
                        task.status,
                        priority_agent::lab::model::LabTaskStatus::Blocked
                    )
                })
                .count();
            let blockers = tasks
                .iter()
                .filter_map(|task| {
                    task.blocker.as_ref().map(|blocker| {
                        format!("{}: {}", task.title, compact_desktop_lab_text(blocker, 96))
                    })
                })
                .chain(
                    run.blocked_reason
                        .as_ref()
                        .map(|reason| format!("Run: {}", compact_desktop_lab_text(reason, 96))),
                )
                .take(6)
                .collect::<Vec<_>>();
            let validation_retries = store
                .list_validation_retries(&run.lab_run_id)
                .unwrap_or_default();
            let validation_retry_escalated_count = validation_retries
                .iter()
                .filter(|retry| retry.escalated)
                .count();
            let latest_validation_retry = validation_retries.last().map(|retry| {
                format!(
                    "{} attempt {}: {}",
                    retry.task_id,
                    retry.attempt,
                    compact_desktop_lab_text(&retry.validation_summary, 96)
                )
            });
            let latest_report_path = store
                .list_stage_artifact_report_paths(&run.lab_run_id)
                .ok()
                .and_then(|reports| reports.last().map(|(_, path)| path.display().to_string()));
            let reports = store
                .list_stage_artifact_report_paths(&run.lab_run_id)
                .unwrap_or_default();
            let report_paths_by_artifact = reports
                .iter()
                .map(|(artifact_id, path)| (artifact_id.clone(), path.display().to_string()))
                .collect::<HashMap<_, _>>();
            let report_previews_by_artifact = reports
                .iter()
                .map(|(artifact_id, path)| (artifact_id.clone(), desktop_lab_report_preview(path)))
                .collect::<HashMap<_, _>>();
            let artifact_rows = store
                .list_stage_artifacts(&run.lab_run_id)
                .unwrap_or_default()
                .into_iter()
                .rev()
                .take(8)
                .map(|artifact| {
                    desktop_lab_artifact_snapshot(
                        &artifact,
                        report_paths_by_artifact
                            .get(artifact.artifact_id())
                            .cloned(),
                        report_previews_by_artifact
                            .get(artifact.artifact_id())
                            .cloned(),
                    )
                })
                .collect::<Vec<_>>();
            let report_rows = reports
                .into_iter()
                .rev()
                .take(8)
                .map(|(artifact_id, path)| {
                    let (preview, truncated) = desktop_lab_report_preview(&path);
                    DesktopLabReportSnapshot {
                        artifact_id,
                        path: path.display().to_string(),
                        preview,
                        truncated,
                    }
                })
                .collect::<Vec<_>>();
            let evidence_rows = store
                .list_evidence_refs(&run.lab_run_id)
                .unwrap_or_default()
                .into_iter()
                .rev()
                .take(8)
                .map(desktop_lab_evidence_snapshot)
                .collect::<Vec<_>>();
            let recommendation =
                priority_agent::lab::orchestrator::LabOrchestrator::for_project(project)
                    .meeting_recommendation_for_latest()
                    .ok();
            DesktopLabStatusSnapshot {
                available: true,
                state: "run".to_string(),
                detail: format!(
                    "{:?} at {} with {:?}",
                    run.status, run.current_stage, run.internal_owner
                ),
                lab_run_id: Some(run.lab_run_id),
                proposal_id: run.proposal_id,
                proposal_status: None,
                run_status: Some(format!("{:?}", run.status)),
                stage: Some(run.current_stage),
                owner: Some(format!("{:?}", run.internal_owner)),
                needs_user: run.needs_user,
                cycle_count: run.cycle_count,
                artifact_count: run.artifact_ids.len(),
                meeting_count: run.meeting_ids.len(),
                task_total: tasks.len(),
                task_open,
                task_blocked,
                blockers,
                validation_retry_count: validation_retries.len(),
                validation_retry_escalated_count,
                latest_validation_retry,
                meeting_recommended: recommendation
                    .as_ref()
                    .map(|meeting| meeting.recommended)
                    .unwrap_or(false),
                meeting_topic: recommendation.map(|meeting| meeting.topic),
                latest_report_path,
                daemon_policy: daemon_policy.clone(),
                artifacts: artifact_rows,
                reports: report_rows,
                evidence_refs: evidence_rows,
            }
        }
        Ok(None) => match store.latest_proposal() {
            Ok(Some(proposal)) => DesktopLabStatusSnapshot {
                available: true,
                state: "proposal".to_string(),
                detail: format!(
                    "{:?}: {}",
                    proposal.status,
                    compact_desktop_lab_text(&proposal.user_goal, 96)
                ),
                lab_run_id: proposal.approval.created_lab_run_id,
                proposal_id: Some(proposal.proposal_id),
                proposal_status: Some(format!("{:?}", proposal.status)),
                run_status: None,
                stage: None,
                owner: Some("Professor".to_string()),
                needs_user: true,
                cycle_count: 0,
                artifact_count: 0,
                meeting_count: 0,
                task_total: 0,
                task_open: 0,
                task_blocked: 0,
                blockers: Vec::new(),
                validation_retry_count: 0,
                validation_retry_escalated_count: 0,
                latest_validation_retry: None,
                meeting_recommended: false,
                meeting_topic: None,
                latest_report_path: None,
                daemon_policy: daemon_policy.clone(),
                artifacts: Vec::new(),
                reports: Vec::new(),
                evidence_refs: Vec::new(),
            },
            Ok(None) => DesktopLabStatusSnapshot {
                available: false,
                state: "none".to_string(),
                detail: "No LabRun or proposal found for this project.".to_string(),
                lab_run_id: None,
                proposal_id: None,
                proposal_status: None,
                run_status: None,
                stage: None,
                owner: None,
                needs_user: false,
                cycle_count: 0,
                artifact_count: 0,
                meeting_count: 0,
                task_total: 0,
                task_open: 0,
                task_blocked: 0,
                blockers: Vec::new(),
                validation_retry_count: 0,
                validation_retry_escalated_count: 0,
                latest_validation_retry: None,
                meeting_recommended: false,
                meeting_topic: None,
                latest_report_path: None,
                daemon_policy: daemon_policy.clone(),
                artifacts: Vec::new(),
                reports: Vec::new(),
                evidence_refs: Vec::new(),
            },
            Err(err) => desktop_lab_status_error(err),
        },
        Err(err) => desktop_lab_status_error(err),
    }
}

fn desktop_lab_status_error(err: anyhow::Error) -> DesktopLabStatusSnapshot {
    DesktopLabStatusSnapshot {
        available: false,
        state: "error".to_string(),
        detail: format!("Failed to read LabRun state: {err}"),
        lab_run_id: None,
        proposal_id: None,
        proposal_status: None,
        run_status: None,
        stage: None,
        owner: None,
        needs_user: false,
        cycle_count: 0,
        artifact_count: 0,
        meeting_count: 0,
        task_total: 0,
        task_open: 0,
        task_blocked: 0,
        blockers: Vec::new(),
        validation_retry_count: 0,
        validation_retry_escalated_count: 0,
        latest_validation_retry: None,
        meeting_recommended: false,
        meeting_topic: None,
        latest_report_path: None,
        daemon_policy: None,
        artifacts: Vec::new(),
        reports: Vec::new(),
        evidence_refs: Vec::new(),
    }
}

fn desktop_lab_artifact_snapshot(
    artifact: &priority_agent::lab::model::StageArtifact,
    report_path: Option<String>,
    report_preview: Option<(Option<String>, bool)>,
) -> DesktopLabArtifactSnapshot {
    let (report_preview, report_preview_truncated) = report_preview.unwrap_or((None, false));
    DesktopLabArtifactSnapshot {
        artifact_id: artifact.artifact_id().to_string(),
        artifact_type: artifact.artifact_type().as_str().to_string(),
        stage: artifact.stage().to_string(),
        owner: format!("{:?}", artifact.owner()),
        status: format!("{:?}", artifact.status()),
        validation_status: artifact.validation_status().map(str::to_string),
        title: desktop_lab_artifact_title(artifact).to_string(),
        created_at: desktop_lab_artifact_created_at(artifact),
        updated_at: desktop_lab_artifact_updated_at(artifact),
        report_path,
        report_preview,
        report_preview_truncated,
        evidence_refs: artifact.evidence_refs().to_vec(),
    }
}

fn desktop_lab_report_preview(path: &Path) -> (Option<String>, bool) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return (None, false);
    };
    let compact = content.trim();
    if compact.is_empty() {
        return (None, false);
    }
    let max_chars = 900;
    if compact.chars().count() <= max_chars {
        return (Some(compact.to_string()), false);
    }
    (
        Some(compact.chars().take(max_chars).collect::<String>()),
        true,
    )
}

fn desktop_lab_evidence_snapshot(
    evidence: priority_agent::lab::model::LabEvidenceRef,
) -> DesktopLabEvidenceSnapshot {
    DesktopLabEvidenceSnapshot {
        evidence_id: evidence.evidence_id,
        kind: format!("{:?}", evidence.kind),
        role: format!("{:?}", evidence.role),
        reference: evidence.reference,
        summary: compact_desktop_lab_text(&evidence.summary, 160),
        artifact_id: evidence.artifact_id,
        cycle_id: evidence.cycle_id,
        created_at: evidence.created_at.to_rfc3339(),
        estimated_summary_tokens: evidence.estimated_summary_tokens,
    }
}

pub(crate) fn desktop_lab_artifact_title(
    artifact: &priority_agent::lab::model::StageArtifact,
) -> &str {
    match artifact {
        priority_agent::lab::model::StageArtifact::ProfessorPlan(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::PostdocPlan(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::GraduateResult(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::PostdocIntegrationSummary(envelope) => {
            &envelope.title
        }
        priority_agent::lab::model::StageArtifact::ProfessorReview(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::CycleSummary(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::CompressionSummary(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::LabMeetingRequest(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::LabMeetingSummary(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::LabBlockerReport(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::LabRevisionTask(envelope) => &envelope.title,
        priority_agent::lab::model::StageArtifact::ProfessorSteeringDecision(envelope) => {
            &envelope.title
        }
    }
}

fn desktop_lab_artifact_created_at(artifact: &priority_agent::lab::model::StageArtifact) -> String {
    match artifact {
        priority_agent::lab::model::StageArtifact::ProfessorPlan(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::PostdocPlan(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::GraduateResult(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::PostdocIntegrationSummary(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::ProfessorReview(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::CycleSummary(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::CompressionSummary(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabMeetingRequest(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabMeetingSummary(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabBlockerReport(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabRevisionTask(envelope) => {
            envelope.created_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::ProfessorSteeringDecision(envelope) => {
            envelope.created_at.to_rfc3339()
        }
    }
}

fn desktop_lab_artifact_updated_at(artifact: &priority_agent::lab::model::StageArtifact) -> String {
    match artifact {
        priority_agent::lab::model::StageArtifact::ProfessorPlan(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::PostdocPlan(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::GraduateResult(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::PostdocIntegrationSummary(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::ProfessorReview(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::CycleSummary(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::CompressionSummary(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabMeetingRequest(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabMeetingSummary(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabBlockerReport(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::LabRevisionTask(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
        priority_agent::lab::model::StageArtifact::ProfessorSteeringDecision(envelope) => {
            envelope.updated_at.to_rfc3339()
        }
    }
}

fn desktop_lab_daemon_policy_snapshot(
    state: priority_agent::lab::model::LabDaemonState,
) -> DesktopLabDaemonPolicySnapshot {
    DesktopLabDaemonPolicySnapshot {
        enabled: state.enabled,
        mode: state.mode,
        max_steps: state.max_steps,
        max_steps_per_cycle: state.max_steps_per_cycle,
        interval_ms: state.interval_ms,
        last_started_at: state.last_started_at.map(|at| at.to_rfc3339()),
        last_start_error: state.last_start_error,
    }
}

fn compact_desktop_lab_text(input: &str, max_chars: usize) -> String {
    let mut compact = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > max_chars {
        compact = compact.chars().take(max_chars).collect::<String>();
        compact.push_str("...");
    }
    compact
}

pub(crate) fn enrich_message_with_desktop_contexts(
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

pub(crate) fn resolve_current_diff_context(
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
        line_start: None,
        line_end: None,
        preview: None,
        truncated,
    })
}

pub(crate) fn desktop_changed_files(project: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for args in [
        ["diff", "--name-only"].as_slice(),
        ["diff", "--cached", "--name-only"].as_slice(),
    ] {
        if let Ok(output) = run_git(project, args) {
            files.extend(
                output
                    .lines()
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(str::to_string),
            );
        }
    }
    files.sort();
    files.dedup();
    files.truncate(64);
    files
}

pub(crate) fn resolve_file_context(
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
    let selection = selected_file_context_preview(context, &text)?;
    let (preview, truncated) = truncate_chars(&selection.preview, 12_000);
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
        line_start: selection.line_start,
        line_end: selection.line_end,
        preview: Some(preview),
        truncated,
    })
}

#[derive(Debug)]
struct FileContextSelection {
    preview: String,
    line_start: Option<usize>,
    line_end: Option<usize>,
}

fn selected_file_context_preview(
    context: &DesktopRunContext,
    text: &str,
) -> Result<FileContextSelection, String> {
    if let Some(selection_text) = context
        .selection_text
        .as_deref()
        .filter(|selection| !selection.trim().is_empty())
    {
        if let Some(start) = context.line_start.filter(|line| *line > 0) {
            let end = context.line_end.unwrap_or(start).max(start);
            let selected = select_file_lines(text, start, end)?;
            if normalize_selection_text(selection_text) != normalize_selection_text(&selected) {
                return Err(format!(
                    "Provided selection_text does not match file lines {}-{}.",
                    start, end
                ));
            }
            return Ok(FileContextSelection {
                preview: selected,
                line_start: Some(start),
                line_end: Some(end),
            });
        }

        if !text.contains(selection_text) {
            return Err("Provided selection_text was not found in the selected file.".to_string());
        }
        return Ok(FileContextSelection {
            preview: selection_text.to_string(),
            line_start: None,
            line_end: None,
        });
    }

    let Some(start) = context.line_start.filter(|line| *line > 0) else {
        return Ok(FileContextSelection {
            preview: text.to_string(),
            line_start: None,
            line_end: None,
        });
    };
    let end = context.line_end.unwrap_or(start).max(start);
    let selected = select_file_lines(text, start, end)?;

    Ok(FileContextSelection {
        preview: selected,
        line_start: Some(start),
        line_end: Some(end),
    })
}

fn select_file_lines(text: &str, start: usize, end: usize) -> Result<String, String> {
    let selected_lines = text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_no = index + 1;
            (line_no >= start && line_no <= end).then_some(line)
        })
        .collect::<Vec<_>>();
    if selected_lines.is_empty() {
        return Err(format!(
            "Selected range {}-{} is outside the selected file.",
            start, end
        ));
    }
    Ok(selected_lines.join("\n"))
}

fn normalize_selection_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches('\n')
        .to_string()
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

        let selected_range = match (context.line_start, context.line_end) {
            (Some(start), Some(end)) => format!("Selected range: {}-{}\n", start, end),
            (Some(start), None) => format!("Selected range: {}\n", start),
            _ => String::new(),
        };

        return format!(
            "<desktop_context type=\"{}\" label=\"{}\">\nPath: {}\nSize bytes: {}\nLines: {}\n{}Preview truncated: {}\n```text\n{}\n```\n</desktop_context>",
            escape_context_attr(&context.context_type),
            escape_context_attr(&context.label),
            relative_path,
            context.size_bytes.unwrap_or_default(),
            context.line_count.unwrap_or_default(),
            selected_range,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn file_context(
        line_start: Option<usize>,
        line_end: Option<usize>,
        selection_text: Option<&str>,
    ) -> DesktopRunContext {
        DesktopRunContext {
            context_type: "file".to_string(),
            label: None,
            path: Some("src/main.rs".to_string()),
            line_start,
            line_end,
            selection_text: selection_text.map(str::to_string),
        }
    }

    #[test]
    fn selected_preview_uses_file_lines_for_verified_range() {
        let context = file_context(Some(2), Some(3), Some("beta\ngamma\n"));
        let selection = selected_file_context_preview(&context, "alpha\nbeta\ngamma\n").unwrap();

        assert_eq!(selection.preview, "beta\ngamma");
        assert_eq!(selection.line_start, Some(2));
        assert_eq!(selection.line_end, Some(3));
    }

    #[test]
    fn selected_preview_rejects_mismatched_selection_text() {
        let context = file_context(Some(2), Some(2), Some("not beta"));
        let err = selected_file_context_preview(&context, "alpha\nbeta\ngamma\n")
            .expect_err("selection should be rejected");

        assert!(err.contains("does not match"));
    }

    #[test]
    fn selected_preview_rejects_selection_text_outside_file() {
        let context = file_context(None, None, Some("not in file"));
        let err = selected_file_context_preview(&context, "alpha\nbeta\ngamma\n")
            .expect_err("selection should be rejected");

        assert!(err.contains("not found"));
    }
}

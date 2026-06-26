use super::DesktopLabStatusSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct DesktopHealth {
    pub(crate) status: &'static str,
    pub(crate) version: &'static str,
    pub(crate) cwd: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SelectedProject {
    pub(crate) path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecentSession {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) updated_at: String,
    pub(crate) model: String,
    pub(crate) message_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DesktopMessage {
    pub(crate) id: i64,
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ResumedSession {
    pub(crate) session_id: String,
    pub(crate) messages: Vec<DesktopMessage>,
    pub(crate) compact_boundaries: Vec<DesktopCompactBoundary>,
    pub(crate) session_parts: Vec<DesktopSessionPart>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopCompactBoundary {
    pub(crate) boundary_id: String,
    pub(crate) strategy: String,
    pub(crate) trigger: String,
    pub(crate) before_tokens: i64,
    pub(crate) after_tokens: i64,
    pub(crate) messages_before: i64,
    pub(crate) messages_after: i64,
    pub(crate) summary: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopLabArtifactBody {
    pub(crate) artifact_id: String,
    pub(crate) artifact_type: String,
    pub(crate) title: String,
    pub(crate) stage: String,
    pub(crate) owner: String,
    pub(crate) status: String,
    pub(crate) validation_status: Option<String>,
    pub(crate) content: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopSessionPart {
    pub(crate) id: i64,
    pub(crate) part_index: i64,
    pub(crate) part_id: String,
    pub(crate) kind: String,
    pub(crate) tool_call_id: Option<String>,
    pub(crate) tool_name: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) payload: serde_json::Value,
    pub(crate) projected_to_seq: i64,
    pub(crate) updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopToolOutputPage {
    pub(crate) id: String,
    pub(crate) uri: String,
    pub(crate) tool_name: String,
    pub(crate) mime: String,
    pub(crate) content: String,
    pub(crate) offset: u64,
    pub(crate) limit: u64,
    pub(crate) total_bytes: u64,
    pub(crate) has_more: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopLabReportPage {
    pub(crate) path: String,
    pub(crate) content: String,
    pub(crate) offset: u64,
    pub(crate) limit: u64,
    pub(crate) total_bytes: u64,
    pub(crate) has_more: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopFilePreview {
    pub(crate) path: String,
    pub(crate) content: String,
    pub(crate) line_count: i64,
    pub(crate) total_bytes: u64,
    pub(crate) truncated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopToolOutputMeta {
    pub(crate) id: String,
    pub(crate) uri: String,
    pub(crate) tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) mime: String,
    pub(crate) original_bytes: u64,
    pub(crate) created_at_ms: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopRevertResult {
    pub(crate) session_id: String,
    pub(crate) status: String,
    pub(crate) message_id: Option<String>,
    pub(crate) part_ids: Vec<String>,
    pub(crate) tool_round_id: Option<String>,
    pub(crate) file_change_ids: Vec<String>,
    pub(crate) checkpoint_ids: Vec<String>,
    pub(crate) paths: Vec<String>,
    pub(crate) restored_files: Vec<String>,
    pub(crate) removed_files: Vec<String>,
    pub(crate) errors: Vec<String>,
    pub(crate) change_count: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct DesktopSettings {
    pub(crate) selected_project: Option<String>,
    pub(crate) active_session_id: Option<String>,
    pub(crate) permission_mode: Option<String>,
    pub(crate) detail_level: Option<String>,
    pub(crate) agent_mode: Option<String>,
    pub(crate) provider_name: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) recent_projects: Option<Vec<String>>,
    pub(crate) archived_session_ids: Option<Vec<String>>,
    pub(crate) lab_daemon_supervision_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopSettingsResponse {
    pub(crate) selected_project: String,
    pub(crate) active_session_id: Option<String>,
    pub(crate) permission_mode: String,
    pub(crate) detail_level: String,
    pub(crate) agent_mode: String,
    pub(crate) provider_name: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) settings_path: String,
    pub(crate) diagnostic_logs_path: String,
    pub(crate) recent_projects: Vec<String>,
    pub(crate) archived_session_ids: Vec<String>,
    pub(crate) startup_state: DesktopStartupState,
    pub(crate) lab_daemon_supervision_enabled: bool,
    pub(crate) lab_daemon_last_supervision: Option<String>,
    pub(crate) lab_daemon_last_supervision_result: Option<String>,
    pub(crate) lab_daemon_next_supervision: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopStartupState {
    pub(crate) status: &'static str,
    pub(crate) detail: String,
    pub(crate) lab_run_id: Option<String>,
    pub(crate) lab_stage: Option<String>,
    pub(crate) lab_owner: Option<String>,
    pub(crate) lab_pause_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PermissionModeOption {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopDiagnostic {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) status: DiagnosticStatus,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopDiagnosticsResponse {
    pub(crate) items: Vec<DesktopDiagnostic>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopLabDaemonActionResult {
    pub(crate) action: &'static str,
    pub(crate) output: String,
    pub(crate) lab_status: DesktopLabStatusSnapshot,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopExportResult {
    pub(crate) session_id: String,
    pub(crate) path: String,
    pub(crate) format: String,
    pub(crate) privacy: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProviderSetupInfo {
    pub(crate) shell_profile_path: String,
    pub(crate) provider_env_vars: Vec<String>,
    pub(crate) example: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProviderModelStatus {
    pub(crate) active_provider: Option<String>,
    pub(crate) active_provider_label: Option<String>,
    pub(crate) active_model: String,
    pub(crate) active_base_url: String,
    pub(crate) runtime_model: Option<String>,
    pub(crate) runtime_provider_ready: bool,
    pub(crate) selection_source: String,
    pub(crate) configured_count: usize,
    pub(crate) providers: Vec<DesktopProviderOption>,
    pub(crate) models: Vec<DesktopModelOption>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopProviderOption {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) provider_type: String,
    pub(crate) model: String,
    pub(crate) base_url: String,
    pub(crate) configured: bool,
    pub(crate) active: bool,
    pub(crate) note: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopModelOption {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) provider_id: String,
    pub(crate) active: bool,
    pub(crate) note: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentModeOption {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) description: String,
}

//! Runtime-facing app state and selectors.
//!
//! This is the shared state shape for tool activity, permissions, MCP status,
//! and status-line style views. It intentionally sits beside the older TUI
//! fields during migration so UI code can move to selectors incrementally.

use super::app_state::{AppState, TaskStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeAppState {
    #[serde(default)]
    pub tool_uses: Vec<RuntimeToolUse>,
    #[serde(default)]
    pub terminal_tasks: Vec<RuntimeTerminalTask>,
    #[serde(default)]
    pub permission: RuntimePermissionState,
    #[serde(default)]
    pub mcp: RuntimeMcpState,
    #[serde(default)]
    pub bridge: RuntimeBridgeState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeToolUse {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub status: RuntimeToolStatus,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
    #[serde(default)]
    pub latest_progress: Option<String>,
    #[serde(default)]
    pub result_preview: Option<String>,
    #[serde(default)]
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeToolStatus {
    Queued,
    Running,
    Backgrounded,
    WaitingPermission,
    TimedOut,
    Cancelled,
    Completed,
    Failed,
    Denied,
    ProviderExecuted,
}

impl RuntimeToolStatus {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            RuntimeToolStatus::Queued
                | RuntimeToolStatus::Running
                | RuntimeToolStatus::WaitingPermission
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            RuntimeToolStatus::Queued => "queued",
            RuntimeToolStatus::Running => "running",
            RuntimeToolStatus::Backgrounded => "backgrounded",
            RuntimeToolStatus::WaitingPermission => "waiting_permission",
            RuntimeToolStatus::TimedOut => "timed_out",
            RuntimeToolStatus::Cancelled => "cancelled",
            RuntimeToolStatus::Completed => "completed",
            RuntimeToolStatus::Failed => "failed",
            RuntimeToolStatus::Denied => "denied",
            RuntimeToolStatus::ProviderExecuted => "provider_executed",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeTerminalTask {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub terminal_kind: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub handle: Option<String>,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub read_tool: Option<String>,
    #[serde(default)]
    pub cancel_handle: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePermissionState {
    pub mode: String,
    #[serde(default)]
    pub pending_call_id: Option<String>,
    #[serde(default)]
    pub pending_tool: Option<String>,
    #[serde(default)]
    pub pending_prompt: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeMcpState {
    pub server_count: usize,
    pub available_count: usize,
    #[serde(default)]
    pub repair_hints: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBridgeState {
    pub bridge_url_configured: bool,
    #[serde(default)]
    pub bridge_url_source: Option<String>,
    pub auth_token_configured: bool,
    #[serde(default)]
    pub tenant_configured: bool,
    pub cursor_count: usize,
    pub saved_session_count: usize,
    pub remote_env_type: String,
    pub is_remote_env: bool,
    pub remote_trigger_tool_available: bool,
    pub remote_dev_tool_available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStatusSnapshot {
    pub messages: usize,
    pub is_querying: bool,
    pub last_error: Option<String>,
    pub total_tools: usize,
    pub active_tool_count: usize,
    pub active_tool_ids: Vec<String>,
    pub current_tool_label: Option<String>,
    pub failed_tool_count: usize,
    pub backgrounded_tool_count: usize,
    pub terminal_task_count: usize,
    pub running_terminal_task_count: usize,
    pub pty_terminal_task_count: usize,
    pub task_count: usize,
    pub running_task_count: usize,
    pub permission_mode: String,
    pub pending_permission: Option<String>,
    pub mcp_server_count: usize,
    pub mcp_available_count: usize,
    pub mcp_repair_hints: Vec<String>,
    pub bridge_url_configured: bool,
    pub bridge_url_source: Option<String>,
    pub bridge_cursor_count: usize,
    pub remote_env_type: String,
    pub remote_trigger_tool_available: bool,
    pub remote_dev_tool_available: bool,
}

pub fn select_runtime_status(state: &AppState) -> RuntimeStatusSnapshot {
    let tools = select_runtime_tools(state);
    let active_tool_ids = tools
        .iter()
        .filter(|tool| tool.active)
        .map(|tool| tool.id.clone())
        .collect::<Vec<_>>();
    let current_tool_label = tools.iter().rev().find(|tool| tool.active).map(|tool| {
        format!(
            "{} ({})",
            non_empty_summary(tool),
            tool.status.label().replace('_', " ")
        )
    });
    let failed_tool_count = tools
        .iter()
        .filter(|tool| {
            matches!(
                tool.status,
                RuntimeToolStatus::Failed
                    | RuntimeToolStatus::TimedOut
                    | RuntimeToolStatus::Cancelled
                    | RuntimeToolStatus::Denied
            )
        })
        .count();
    let backgrounded_tool_count = tools
        .iter()
        .filter(|tool| tool.status == RuntimeToolStatus::Backgrounded)
        .count();
    let running_terminal_task_count = state
        .runtime
        .terminal_tasks
        .iter()
        .filter(|task| task.status == "running")
        .count();
    let pty_terminal_task_count = state
        .runtime
        .terminal_tasks
        .iter()
        .filter(|task| task.terminal_kind.as_deref() == Some("pty_shell"))
        .count();
    let running_task_count = state
        .tasks
        .values()
        .filter(|task| task.status == TaskStatus::Running)
        .count();
    let pending_permission = state.runtime.permission.pending_tool.as_ref().map(|tool| {
        state
            .runtime
            .permission
            .pending_call_id
            .as_ref()
            .map(|id| format!("{tool} ({id})"))
            .unwrap_or_else(|| tool.clone())
    });

    RuntimeStatusSnapshot {
        messages: state.messages.len(),
        is_querying: state.is_querying,
        last_error: state.last_error.clone(),
        total_tools: tools.len(),
        active_tool_count: active_tool_ids.len(),
        active_tool_ids,
        current_tool_label,
        failed_tool_count,
        backgrounded_tool_count,
        terminal_task_count: state.runtime.terminal_tasks.len(),
        running_terminal_task_count,
        pty_terminal_task_count,
        task_count: state.tasks.len(),
        running_task_count,
        permission_mode: state.runtime.permission.mode.clone(),
        pending_permission,
        mcp_server_count: state.runtime.mcp.server_count,
        mcp_available_count: state.runtime.mcp.available_count,
        mcp_repair_hints: state.runtime.mcp.repair_hints.clone(),
        bridge_url_configured: state.runtime.bridge.bridge_url_configured,
        bridge_url_source: state.runtime.bridge.bridge_url_source.clone(),
        bridge_cursor_count: state.runtime.bridge.cursor_count,
        remote_env_type: state.runtime.bridge.remote_env_type.clone(),
        remote_trigger_tool_available: state.runtime.bridge.remote_trigger_tool_available,
        remote_dev_tool_available: state.runtime.bridge.remote_dev_tool_available,
    }
}

pub fn select_runtime_tools(state: &AppState) -> Vec<RuntimeToolUse> {
    state.runtime.tool_uses.clone()
}

pub fn select_runtime_permission(state: &AppState) -> RuntimePermissionState {
    state.runtime.permission.clone()
}

pub fn select_runtime_mcp(state: &AppState) -> RuntimeMcpState {
    state.runtime.mcp.clone()
}

pub fn select_tool_viewer_tool_id(
    runtime: &RuntimeAppState,
    preferred_id: Option<&str>,
) -> Option<String> {
    preferred_id
        .and_then(|id| runtime.tool_uses.iter().find(|tool| tool.id == id))
        .or_else(|| runtime.tool_uses.iter().next_back())
        .map(|tool| tool.id.clone())
}

fn non_empty_summary(tool: &RuntimeToolUse) -> &str {
    if tool.summary.trim().is_empty() {
        &tool.name
    } else {
        &tool.summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::TaskItem;

    #[test]
    fn selects_runtime_status_from_shared_state() {
        let mut state = AppState::new();
        state.is_querying = true;
        state.add_user_message("hello");
        state.tasks.insert(
            "task_1".to_string(),
            TaskItem {
                status: TaskStatus::Running,
                ..TaskItem::new("task_1", "run tests")
            },
        );
        state.runtime.permission.mode = "default".to_string();
        state.runtime.permission.pending_tool = Some("bash".to_string());
        state.runtime.permission.pending_call_id = Some("call_1".to_string());
        state.runtime.mcp.server_count = 2;
        state.runtime.mcp.available_count = 1;
        state.runtime.bridge.bridge_url_configured = true;
        state.runtime.bridge.bridge_url_source = Some("PRIORITY_AGENT_BRIDGE_URL".to_string());
        state.runtime.bridge.cursor_count = 3;
        state.runtime.bridge.remote_env_type = "ssh".to_string();
        state.runtime.bridge.remote_trigger_tool_available = true;
        state.runtime.tool_uses.push(RuntimeToolUse {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            summary: "Running cargo check".to_string(),
            status: RuntimeToolStatus::Running,
            active: true,
            arguments: None,
            latest_progress: None,
            result_preview: None,
            elapsed_ms: Some(1200),
        });
        state.runtime.terminal_tasks.push(RuntimeTerminalTask {
            id: "shell_1".to_string(),
            status: "running".to_string(),
            terminal_kind: Some("pty_shell".to_string()),
            command: Some("python3".to_string()),
            ..RuntimeTerminalTask::default()
        });

        let snapshot = select_runtime_status(&state);

        assert_eq!(snapshot.messages, 1);
        assert!(snapshot.is_querying);
        assert_eq!(snapshot.active_tool_count, 1);
        assert_eq!(snapshot.active_tool_ids, vec!["call_1".to_string()]);
        assert_eq!(
            snapshot.current_tool_label.as_deref(),
            Some("Running cargo check (running)")
        );
        assert_eq!(snapshot.running_task_count, 1);
        assert_eq!(snapshot.terminal_task_count, 1);
        assert_eq!(snapshot.running_terminal_task_count, 1);
        assert_eq!(snapshot.pty_terminal_task_count, 1);
        assert_eq!(
            snapshot.pending_permission.as_deref(),
            Some("bash (call_1)")
        );
        assert_eq!(snapshot.mcp_server_count, 2);
        assert_eq!(snapshot.mcp_available_count, 1);
        assert!(snapshot.bridge_url_configured);
        assert_eq!(
            snapshot.bridge_url_source.as_deref(),
            Some("PRIORITY_AGENT_BRIDGE_URL")
        );
        assert_eq!(snapshot.bridge_cursor_count, 3);
        assert_eq!(snapshot.remote_env_type, "ssh");
        assert!(snapshot.remote_trigger_tool_available);
    }

    #[test]
    fn selects_tool_viewer_preferred_or_latest_tool() {
        let runtime = RuntimeAppState {
            tool_uses: vec![
                RuntimeToolUse {
                    id: "first".to_string(),
                    name: "grep".to_string(),
                    summary: "Search".to_string(),
                    status: RuntimeToolStatus::Completed,
                    active: false,
                    arguments: None,
                    latest_progress: None,
                    result_preview: None,
                    elapsed_ms: None,
                },
                RuntimeToolUse {
                    id: "second".to_string(),
                    name: "bash".to_string(),
                    summary: "Check".to_string(),
                    status: RuntimeToolStatus::Running,
                    active: true,
                    arguments: None,
                    latest_progress: None,
                    result_preview: None,
                    elapsed_ms: None,
                },
            ],
            ..RuntimeAppState::default()
        };

        assert_eq!(
            select_tool_viewer_tool_id(&runtime, Some("first")).as_deref(),
            Some("first")
        );
        assert_eq!(
            select_tool_viewer_tool_id(&runtime, Some("missing")).as_deref(),
            Some("second")
        );
    }
}

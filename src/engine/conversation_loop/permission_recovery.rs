//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::permission_controller::{
    PermissionRequestKind, PermissionRequestRecord, PermissionToolFamily,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const PERMISSION_DENIAL_RECOVERY_LIMIT: usize = 2;

#[derive(Debug, Clone, Default)]
struct PermissionDenialCounter {
    count: usize,
}

static PERMISSION_DENIAL_COUNTERS: OnceLock<Mutex<HashMap<String, PermissionDenialCounter>>> =
    OnceLock::new();

pub(super) fn permission_denied_message(record: &PermissionRequestRecord) -> String {
    if record.recovery_feedback.trim().is_empty() {
        record.rejection_feedback.clone()
    } else {
        format!(
            "{}\nRecovery: {}",
            record.rejection_feedback, record.recovery_feedback
        )
    }
}

fn permission_denial_key(
    session_id: &str,
    family: PermissionToolFamily,
    tool_name: &str,
) -> String {
    format!("{}:{}:{}", session_id, family.as_str(), tool_name)
}

pub(super) fn permission_denial_state_json(
    session_id: &str,
    family: PermissionToolFamily,
    tool_name: &str,
    increment: bool,
) -> serde_json::Value {
    let key = permission_denial_key(session_id, family, tool_name);
    let counters = PERMISSION_DENIAL_COUNTERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut counters = counters.lock().unwrap_or_else(|e| e.into_inner());
    let entry = counters.entry(key).or_default();
    if increment {
        entry.count = entry.count.saturating_add(1);
    }
    serde_json::json!({
        "schema": "permission_denial_state.v1",
        "session_id": session_id,
        "permission_family": family.as_str(),
        "tool_name": tool_name,
        "denials": entry.count,
        "bounded_recovery": entry.count >= PERMISSION_DENIAL_RECOVERY_LIMIT,
        "limit": PERMISSION_DENIAL_RECOVERY_LIMIT,
    })
}

pub(super) fn record_permission_denial(record: &PermissionRequestRecord) -> serde_json::Value {
    let tool_name = record
        .metadata
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let family = record
        .metadata
        .get("permission_family")
        .and_then(serde_json::Value::as_str)
        .map(permission_tool_family_from_str)
        .unwrap_or(PermissionToolFamily::Other);
    permission_denial_state_json(&record.session_id, family, tool_name, true)
}

fn permission_tool_family_from_str(value: &str) -> PermissionToolFamily {
    match value {
        "shell" => PermissionToolFamily::Shell,
        "file" => PermissionToolFamily::File,
        "external_directory" => PermissionToolFamily::ExternalDirectory,
        "task" => PermissionToolFamily::Task,
        "subagent" => PermissionToolFamily::Subagent,
        "remote" => PermissionToolFamily::Remote,
        _ => PermissionToolFamily::Other,
    }
}

pub(super) fn recovery_feedback(
    kind: PermissionRequestKind,
    family: PermissionToolFamily,
    tool_name: &str,
) -> String {
    if kind == PermissionRequestKind::GoalDrift {
        return "Confirm the current goal or destructive scope with the user before retrying. Do not treat the blocked tool as executed.".to_string();
    }

    match family {
        PermissionToolFamily::Shell => {
            "Ask the user to approve the exact command, or use a read-only inspection command if that answers the task. Do not run a different risky command.".to_string()
        }
        PermissionToolFamily::ExternalDirectory => {
            "Ask the user to approve this external path/scope, or choose a path inside the trusted workspace. Do not claim files outside the workspace were changed.".to_string()
        }
        PermissionToolFamily::File => {
            "Ask the user to approve the file operation, narrow the edit scope, or use a read-only file inspection tool first. Do not claim the file changed.".to_string()
        }
        PermissionToolFamily::Task => {
            "Ask the user to approve task mutation, or continue with local reasoning without changing task state. Do not claim the task was updated.".to_string()
        }
        PermissionToolFamily::Subagent => {
            "Ask the user to approve delegation, or continue locally with the available context. Do not claim a sub-agent was started.".to_string()
        }
        PermissionToolFamily::Remote => {
            "Ask the user to approve the exact remote action. If it failed, inspect `/remote status`, bridge/session configuration, and prior remote side effects before retrying. Do not claim remote work or sync completed.".to_string()
        }
        PermissionToolFamily::Other => {
            format!("Ask the user to approve '{}', or choose a lower-risk alternative. Do not claim the tool ran.", tool_name)
        }
    }
}

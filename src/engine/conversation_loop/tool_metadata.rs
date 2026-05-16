use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use std::sync::Arc;
use tracing::warn;

use super::tool_execution::safe_prefix_by_bytes;

pub(super) fn tool_error_code_label(result: &ToolResult) -> Option<String> {
    result.error_code.as_ref().and_then(|code| {
        serde_json::to_value(code)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
    })
}

pub(super) fn merge_tool_result_metadata(
    result: &mut ToolResult,
    key: &str,
    value: serde_json::Value,
) {
    match result.data.take() {
        Some(serde_json::Value::Object(mut object)) => {
            object.insert(key.to_string(), value);
            result.data = Some(serde_json::Value::Object(object));
        }
        Some(existing) => {
            result.data = Some(serde_json::json!({
                "value": existing,
                key: value,
            }));
        }
        None => {
            result.data = Some(serde_json::json!({
                key: value,
            }));
        }
    }
}

pub(super) fn build_tool_execution_summary(
    tool_call: &ToolCall,
    result: &ToolResult,
) -> serde_json::Value {
    let output_chars = result.content.chars().count();
    let mut summary = serde_json::json!({
        "tool": tool_call.name,
        "call_id": tool_call.id,
        "success": result.success,
        "output_chars": output_chars,
        "duration_ms": result.duration_ms,
    });
    let Some(object) = summary.as_object_mut() else {
        return summary;
    };

    match tool_call.name.as_str() {
        "bash" => {
            if let Some(command) = tool_call.arguments["command"].as_str() {
                let classification =
                    crate::tools::bash_tool::command_classifier::classify_command(command);
                object.insert(
                    "command".to_string(),
                    serde_json::Value::String(safe_prefix_by_bytes(command, 240).to_string()),
                );
                object.insert(
                    "command_kind".to_string(),
                    serde_json::to_value(classification.command_kind)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "command_category".to_string(),
                    serde_json::to_value(classification.category)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "validation_family".to_string(),
                    serde_json::to_value(classification.validation_family)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "safe_for_closeout".to_string(),
                    serde_json::Value::Bool(classification.safe_for_closeout),
                );
            }
        }
        "file_edit" => {
            if let Some(path) = tool_call.arguments["path"].as_str() {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
            if let Some(replacements) = result
                .data
                .as_ref()
                .and_then(|data| data.get("replacements"))
                .and_then(|value| value.as_u64())
            {
                object.insert(
                    "replacements".to_string(),
                    serde_json::Value::Number(replacements.into()),
                );
            }
        }
        "file_write" | "file_read" => {
            if let Some(path) = tool_call.arguments["path"].as_str() {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
        }
        "file_patch" => {
            if let Some(operations) = tool_call.arguments["operations"].as_array() {
                object.insert(
                    "operations".to_string(),
                    serde_json::Value::Number((operations.len() as u64).into()),
                );
            }
        }
        "grep" => {
            if let Some(pattern) = tool_call.arguments["pattern"].as_str() {
                object.insert(
                    "pattern".to_string(),
                    serde_json::Value::String(safe_prefix_by_bytes(pattern, 120).to_string()),
                );
            }
            if let Some(path) = tool_call
                .arguments
                .get("path")
                .or_else(|| tool_call.arguments.get("include"))
                .and_then(|value| value.as_str())
            {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
        }
        "git" => {
            if let Some(action) = tool_call.arguments["action"].as_str() {
                object.insert(
                    "action".to_string(),
                    serde_json::Value::String(action.to_string()),
                );
            }
        }
        _ => {}
    }

    attach_terminal_task_summary(object, result);

    if let Some(error) = result.error.as_deref() {
        object.insert(
            "error_preview".to_string(),
            serde_json::Value::String(safe_prefix_by_bytes(error, 240).to_string()),
        );
    }

    summary
}

fn attach_terminal_task_summary(
    summary: &mut serde_json::Map<String, serde_json::Value>,
    result: &ToolResult,
) {
    let Some(data) = result.data.as_ref() else {
        return;
    };
    if let Some(task) = data
        .get("terminal_task")
        .and_then(serde_json::Value::as_object)
    {
        summary.insert(
            "terminal_task".to_string(),
            terminal_task_summary_object(task),
        );
    }
    if let Some(tasks) = data
        .get("terminal_tasks")
        .and_then(serde_json::Value::as_array)
    {
        summary.insert(
            "terminal_tasks_count".to_string(),
            serde_json::Value::Number((tasks.len() as u64).into()),
        );
        let task_summaries = tasks
            .iter()
            .filter_map(serde_json::Value::as_object)
            .map(terminal_task_summary_object)
            .collect();
        summary.insert(
            "terminal_tasks".to_string(),
            serde_json::Value::Array(task_summaries),
        );
    }
}

fn terminal_task_summary_object(
    task: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut summary = serde_json::Map::new();
    for key in [
        "task_id",
        "handle",
        "status",
        "terminal_kind",
        "pty",
        "read_tool",
        "cancel_handle",
        "output_path",
        "duration_ms",
        "exit_code",
    ] {
        if let Some(value) = task.get(key).filter(|value| !value.is_null()) {
            summary.insert(key.to_string(), value.clone());
        }
    }
    if let Some(command) = task.get("command").and_then(serde_json::Value::as_str) {
        summary.insert(
            "command".to_string(),
            serde_json::Value::String(safe_prefix_by_bytes(command, 240).to_string()),
        );
    }
    serde_json::Value::Object(summary)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub(super) struct ToolExecutionRecord {
    pub(super) call_id: String,
    pub(super) tool_name: String,
    pub(super) status: ToolExecutionStatus,
    pub(super) user_output: String,
    pub(super) machine_metadata: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ToolExecutionStatus {
    Completed,
    Failed,
}

impl ToolExecutionRecord {
    pub(super) fn from_result(tool_call: &ToolCall, result: &ToolResult) -> Self {
        Self {
            call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            status: if result.success {
                ToolExecutionStatus::Completed
            } else {
                ToolExecutionStatus::Failed
            },
            user_output: tool_result_user_output(result),
            machine_metadata: build_tool_execution_summary(tool_call, result),
        }
    }

    pub(super) fn provider_content(&self) -> String {
        let label = match self.status {
            ToolExecutionStatus::Completed => "OK",
            ToolExecutionStatus::Failed => "ERROR",
        };
        format!("Result: {}\n{}", label, self.user_output)
    }
}

pub(super) fn provider_tool_result_content(tool_call: &ToolCall, result: &ToolResult) -> String {
    ToolExecutionRecord::from_result(tool_call, result).provider_content()
}

fn tool_result_user_output(result: &ToolResult) -> String {
    if !result.content.trim().is_empty() {
        result.content.clone()
    } else if let Some(error) = result
        .error
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        error.to_string()
    } else {
        "tool returned no output".to_string()
    }
}

pub(super) fn tool_execution_start_progress(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> String {
    if tool_name == "bash" {
        let Some(command) = arguments["command"].as_str() else {
            return "Executing bash...".to_string();
        };
        if arguments["mode"].as_str() == Some("pty") {
            let command = safe_prefix_by_bytes(command, 80);
            return format!("Running PTY command: {}", command);
        }
        let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
        let prefix = match classification.validation_family {
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoTest) => {
                "Running Rust tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoCheck) => {
                "Running cargo check"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoClippy) => {
                "Running cargo clippy"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::NpmTest)
            | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PnpmTest)
            | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::YarnTest) => {
                "Running JS tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::Pytest)
            | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PythonCompile)
            | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PythonUnittest) => {
                "Running Python tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::GoTest) => {
                "Running Go tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::BashSyntax) => {
                "Checking shell syntax"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::ProjectScript) => {
                "Running project validation"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::RgAssertion) => {
                "Running search assertion"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::ShellAssertion) => {
                "Running shell assertion"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::NodeScript) => {
                "Running Node validation"
            }
            None => match classification.category {
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::List => {
                    "Listing with shell"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::Search => {
                    "Searching with shell"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::Read => {
                    "Inspecting with shell"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::PackageInstall => {
                    "Installing package"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::DevServer => {
                    "Starting dev server"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::Interactive => {
                    "Checking terminal requirement"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::GitMutation => {
                    "Running git mutation"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::FileMutation => {
                    "Running shell mutation"
                }
                crate::tools::bash_tool::command_classifier::ShellCommandCategory::Destructive => {
                    "Reviewing dangerous shell command"
                }
                _ => "Executing shell command",
            },
        };
        let command = safe_prefix_by_bytes(command, 80);
        return format!("{}: {}", prefix, command);
    }

    format!("Executing {}...", tool_name)
}

pub(super) fn attach_tool_execution_metadata(tool_call: &ToolCall, result: &mut ToolResult) {
    fill_shell_result_duration(result);
    let summary = build_tool_execution_summary(tool_call, result);
    merge_tool_result_metadata(result, "tool_summary", summary);

    if result.success {
        return;
    }
    if result.content.trim().is_empty() {
        result.content = result
            .error
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("tool failed")
            .to_string();
    }
    let error = result
        .error
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("tool failed");
    let code = tool_error_code_label(result);
    let plan = crate::engine::recovery_plan::RecoveryPlan::tool_failure(
        &tool_call.name,
        error,
        code.as_deref(),
    );
    let metadata = serde_json::json!({
        "recoverable": plan.retryable,
        "safe_retry": plan.safe_retry,
        "suggested_command": plan.suggested_command,
        "user_note": plan.user_note,
        "recovery_action": plan.action,
        "recovery_category": plan.category,
    });
    merge_tool_result_metadata(result, "recovery", metadata);
}

fn fill_shell_result_duration(result: &mut ToolResult) {
    let Some(duration_ms) = result.duration_ms else {
        return;
    };
    if let Some(shell_result) = result
        .data
        .as_mut()
        .and_then(|data| data.get_mut("shell_result"))
        .and_then(|value| value.as_object_mut())
    {
        shell_result.insert(
            "duration_ms".to_string(),
            serde_json::Value::Number(duration_ms.into()),
        );
    }
    if let Some(terminal_task) = result
        .data
        .as_mut()
        .and_then(|data| data.get_mut("terminal_task"))
        .and_then(|value| value.as_object_mut())
    {
        terminal_task.insert(
            "duration_ms".to_string(),
            serde_json::Value::Number(duration_ms.into()),
        );
    }
}

pub(super) fn persist_tool_outcome_learning_event(
    store: Option<&Arc<crate::session_store::SessionStore>>,
    session_id: &str,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(store) = store else {
        return;
    };
    let code = tool_error_code_label(result);
    let recovery = result
        .data
        .as_ref()
        .and_then(|data| data.get("recovery"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let tool_summary = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_summary"))
        .cloned()
        .unwrap_or_else(|| build_tool_execution_summary(tool_call, result));
    let summary = if result.success {
        format!("Tool {} succeeded", tool_call.name)
    } else {
        format!(
            "Tool {} failed: {}",
            tool_call.name,
            result.error.as_deref().unwrap_or("unknown error")
        )
    };
    let payload = serde_json::json!({
        "tool": tool_call.name,
        "call_id": tool_call.id,
        "success": result.success,
        "error_code": code,
        "error": result.error,
        "duration_ms": result.duration_ms,
        "output_chars": result.content.chars().count(),
        "tool_summary": tool_summary,
        "recovery": recovery,
    });
    let payload = crate::engine::experience_ledger::attach_experience_payload(
        payload,
        crate::engine::experience_ledger::ExperienceRecord::from_tool_outcome(tool_call, result),
    );
    if let Err(e) = store.add_learning_event(
        session_id,
        "tool_outcome",
        "conversation_loop",
        &summary,
        if result.success { 1.0 } else { 0.75 },
        &payload,
    ) {
        warn!("Failed to persist tool outcome learning event: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(name: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            arguments: serde_json::json!({"command": "cargo test -q"}),
        }
    }

    #[test]
    fn provider_tool_result_content_keeps_success_output() {
        let content =
            provider_tool_result_content(&tool_call("bash"), &ToolResult::success("all good"));

        assert_eq!(content, "Result: OK\nall good");
    }

    #[test]
    fn provider_tool_result_content_uses_error_when_output_empty() {
        let content = provider_tool_result_content(&tool_call("bash"), &ToolResult::error("boom"));

        assert_eq!(content, "Result: ERROR\nboom");
    }

    #[test]
    fn tool_execution_record_separates_machine_metadata_from_provider_text() {
        let record =
            ToolExecutionRecord::from_result(&tool_call("bash"), &ToolResult::success("compiled"));

        assert_eq!(record.status, ToolExecutionStatus::Completed);
        assert_eq!(record.provider_content(), "Result: OK\ncompiled");
        assert_eq!(record.machine_metadata["tool"], "bash");
        assert_eq!(record.machine_metadata["command_kind"], "validation");
        assert_eq!(record.machine_metadata["command_category"], "test_run");
    }

    #[test]
    fn tool_execution_summary_includes_terminal_task_metadata() {
        let result = ToolResult::success_with_data(
            "ok",
            serde_json::json!({
                "terminal_task": {
                    "task_id": "shell_foreground_123",
                    "handle": null,
                    "command": "cargo test -q",
                    "status": "completed",
                    "terminal_kind": "foreground_shell",
                    "pty": false,
                    "read_tool": null,
                    "cancel_handle": null,
                    "duration_ms": 42,
                    "exit_code": 0
                }
            }),
        );

        let summary = build_tool_execution_summary(&tool_call("bash"), &result);

        assert_eq!(summary["terminal_task"]["task_id"], "shell_foreground_123");
        assert_eq!(summary["terminal_task"]["status"], "completed");
        assert_eq!(
            summary["terminal_task"]["terminal_kind"],
            "foreground_shell"
        );
        assert_eq!(summary["terminal_task"]["pty"], false);
        assert_eq!(summary["terminal_task"]["duration_ms"], 42);
    }

    #[test]
    fn tool_execution_summary_includes_terminal_tasks_metadata() {
        let result = ToolResult::success_with_data(
            "tasks",
            serde_json::json!({
                "terminal_tasks": [
                    {
                        "task_id": "shell_bg_1",
                        "handle": "shell_bg_1",
                        "command": "npm run dev",
                        "status": "running",
                        "terminal_kind": "background_shell",
                        "read_tool": "bash_output",
                        "cancel_handle": "shell_bg_1"
                    }
                ]
            }),
        );

        let summary = build_tool_execution_summary(&tool_call("bash_tasks"), &result);

        assert_eq!(summary["terminal_tasks_count"], 1);
        assert_eq!(summary["terminal_tasks"][0]["task_id"], "shell_bg_1");
        assert_eq!(summary["terminal_tasks"][0]["status"], "running");
        assert_eq!(summary["terminal_tasks"][0]["read_tool"], "bash_output");
        assert_eq!(summary["terminal_tasks"][0]["cancel_handle"], "shell_bg_1");
    }

    #[test]
    fn attach_tool_execution_metadata_fills_shell_result_duration() {
        let mut result = ToolResult::success_with_data(
            "ok",
            serde_json::json!({
                "shell_result": {
                    "duration_ms": null,
                    "command": "cargo test -q"
                },
                "terminal_task": {
                    "duration_ms": null,
                    "command": "cargo test -q"
                }
            }),
        );
        result.duration_ms = Some(42);

        attach_tool_execution_metadata(&tool_call("bash"), &mut result);

        assert_eq!(
            result.data.as_ref().unwrap()["shell_result"]["duration_ms"],
            42
        );
        assert_eq!(
            result.data.as_ref().unwrap()["terminal_task"]["duration_ms"],
            42
        );
        assert_eq!(
            result.data.as_ref().unwrap()["tool_summary"]["duration_ms"],
            42
        );
    }
}

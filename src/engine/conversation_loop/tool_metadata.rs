use crate::services::api::ToolCall;
use crate::tools::{Tool, ToolResult};
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
        "bash" | "run_tests" | "start_dev_server" => {
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
                    "path_patterns".to_string(),
                    serde_json::to_value(classification.path_patterns)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "safe_for_closeout".to_string(),
                    serde_json::Value::Bool(classification.safe_for_closeout),
                );
                object.insert(
                    "network_access".to_string(),
                    serde_json::Value::Bool(classification.network_access),
                );
                object.insert(
                    "external_path_access".to_string(),
                    serde_json::Value::Bool(classification.external_path_access),
                );
                object.insert(
                    "absolute_path_patterns".to_string(),
                    serde_json::to_value(classification.absolute_path_patterns)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "compound_command".to_string(),
                    serde_json::Value::Bool(classification.compound_command),
                );
                object.insert(
                    "shell_control_operators".to_string(),
                    serde_json::to_value(classification.shell_control_operators)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "risky_shell_wrapper".to_string(),
                    serde_json::Value::Bool(classification.risky_shell_wrapper),
                );
                object.insert(
                    "expected_silent_output".to_string(),
                    serde_json::Value::Bool(classification.expected_silent_output),
                );
                object.insert(
                    "permission_rule_suggestions".to_string(),
                    serde_json::to_value(classification.permission_rule_suggestions)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "parser_status".to_string(),
                    serde_json::Value::String(classification.parser_status),
                );
                object.insert(
                    "subcommands".to_string(),
                    serde_json::to_value(classification.subcommands)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "redirections".to_string(),
                    serde_json::to_value(classification.redirections)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "mutation_paths".to_string(),
                    serde_json::to_value(classification.mutation_paths)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "mutation_indicators".to_string(),
                    serde_json::to_value(classification.mutation_indicators)
                        .unwrap_or(serde_json::Value::Null),
                );
                object.insert(
                    "command_plan".to_string(),
                    serde_json::to_value(classification.command_plan)
                        .unwrap_or(serde_json::Value::Null),
                );
            }
            attach_bash_recovery_summary(object, result);
        }
        "file_edit" => {
            if let Some(path) = tool_call.arguments["path"].as_str() {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
            attach_diff_summary(object, result);
            attach_file_reliability_summary(object, result);
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
            attach_file_reliability_summary(object, result);
        }
        "file_patch" => {
            if let Some(operations) = tool_call.arguments["operations"].as_array() {
                object.insert(
                    "operations".to_string(),
                    serde_json::Value::Number((operations.len() as u64).into()),
                );
            }
            attach_diff_summary(object, result);
            attach_file_reliability_summary(object, result);
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
        "git_status" => {
            object.insert(
                "action".to_string(),
                serde_json::Value::String("status".to_string()),
            );
            if let Some(path) = tool_call.arguments["path"].as_str() {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
        }
        "git_diff" => {
            object.insert(
                "action".to_string(),
                serde_json::Value::String("diff".to_string()),
            );
            if let Some(path) = tool_call.arguments["path"].as_str() {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
        }
        "install_dependencies" => {
            if let Some(dependency_install) = result
                .data
                .as_ref()
                .and_then(|data| data.get("dependency_install"))
            {
                if let Some(command) = dependency_install
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                {
                    object.insert(
                        "command".to_string(),
                        serde_json::Value::String(safe_prefix_by_bytes(command, 240).to_string()),
                    );
                }
                if let Some(manager) = dependency_install
                    .get("manager")
                    .and_then(serde_json::Value::as_str)
                {
                    object.insert(
                        "manager".to_string(),
                        serde_json::Value::String(manager.to_string()),
                    );
                }
                object.insert(
                    "network_class".to_string(),
                    serde_json::json!("package_install"),
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

fn attach_bash_recovery_summary(
    summary: &mut serde_json::Map<String, serde_json::Value>,
    result: &ToolResult,
) {
    let Some(recovery) = result.data.as_ref().and_then(|data| data.get("recovery")) else {
        return;
    };
    for (source, target) in [
        ("category", "recovery_category"),
        ("action", "recovery_action"),
        ("reason", "recovery_reason"),
    ] {
        if let Some(value) = recovery
            .get(source)
            .and_then(serde_json::Value::as_str)
            .filter(|value| *value != "none")
        {
            summary.insert(
                target.to_string(),
                serde_json::Value::String(safe_prefix_by_bytes(value, 240).to_string()),
            );
        }
    }
}

fn attach_file_reliability_summary(
    summary: &mut serde_json::Map<String, serde_json::Value>,
    result: &ToolResult,
) {
    let Some(data) = result.data.as_ref() else {
        return;
    };

    if let Some(failure) = data.get("failure").and_then(serde_json::Value::as_str) {
        summary.insert(
            "failure_kind".to_string(),
            serde_json::Value::String(failure.to_string()),
        );
    }
    if let Some(reason) = data
        .get("guardrail")
        .and_then(|value| value.get("reason"))
        .and_then(serde_json::Value::as_str)
    {
        summary.insert(
            "guardrail_reason".to_string(),
            serde_json::Value::String(safe_prefix_by_bytes(reason, 240).to_string()),
        );
    }
    if let Some(action) = data
        .get("recovery")
        .and_then(|value| value.get("recommended_action"))
        .and_then(serde_json::Value::as_str)
    {
        summary.insert(
            "recovery_action".to_string(),
            serde_json::Value::String(action.to_string()),
        );
    }

    if let Some(checkpoint_id) = data
        .get("checkpoint")
        .and_then(|value| value.get("id"))
        .or_else(|| {
            data.get("edit_preview")
                .and_then(|value| value.get("checkpoint_id"))
        })
        .and_then(serde_json::Value::as_str)
    {
        summary.insert(
            "checkpoint_id".to_string(),
            serde_json::Value::String(checkpoint_id.to_string()),
        );
        summary.insert(
            "rollback_id".to_string(),
            serde_json::Value::String(checkpoint_id.to_string()),
        );
    }

    if let Some(delta) = data.get("diagnostics_delta") {
        if let Some(status) = delta.get("status").and_then(serde_json::Value::as_str) {
            summary.insert(
                "diagnostics_delta_status".to_string(),
                serde_json::Value::String(status.to_string()),
            );
        }
        if let Some(change) = delta.get("change") {
            for key in ["diagnostic_count", "error_count", "warning_count"] {
                if let Some(value) = change.get(key).and_then(serde_json::Value::as_i64) {
                    summary.insert(
                        format!("diagnostics_delta_{key}"),
                        serde_json::Value::Number(value.into()),
                    );
                }
            }
        }
    }
}

fn attach_diff_summary(
    summary: &mut serde_json::Map<String, serde_json::Value>,
    result: &ToolResult,
) {
    let Some(diff) = result.data.as_ref().and_then(|data| data.get("diff")) else {
        return;
    };
    if let Some(additions) = diff.get("additions").and_then(serde_json::Value::as_u64) {
        summary.insert(
            "additions".to_string(),
            serde_json::Value::Number(additions.into()),
        );
    }
    if let Some(deletions) = diff.get("deletions").and_then(serde_json::Value::as_u64) {
        summary.insert(
            "deletions".to_string(),
            serde_json::Value::Number(deletions.into()),
        );
    }
    if let Some(truncated) = diff
        .get("preview_truncated")
        .and_then(serde_json::Value::as_bool)
    {
        summary.insert(
            "diff_preview_truncated".to_string(),
            serde_json::Value::Bool(truncated),
        );
    }
    if let Some(unified_diff) = diff
        .get("unified_diff")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        summary.insert(
            "diff_preview".to_string(),
            serde_json::Value::String(safe_prefix_by_bytes(unified_diff, 1200).to_string()),
        );
    }
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
        "failure_reason",
        "recovery_action",
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
pub(super) struct ProviderToolResultRecord {
    pub(super) call_id: String,
    pub(super) tool_name: String,
    pub(super) status: ProviderToolResultStatus,
    pub(super) user_output: String,
    pub(super) machine_metadata: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProviderToolResultStatus {
    Completed,
    Failed,
}

impl ProviderToolResultRecord {
    pub(super) fn from_result(tool_call: &ToolCall, result: &ToolResult) -> Self {
        Self {
            call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            status: if result.success {
                ProviderToolResultStatus::Completed
            } else {
                ProviderToolResultStatus::Failed
            },
            user_output: tool_result_user_output(result),
            machine_metadata: build_tool_execution_summary(tool_call, result),
        }
    }

    pub(super) fn provider_content(&self) -> String {
        let label = match self.status {
            ProviderToolResultStatus::Completed => "OK",
            ProviderToolResultStatus::Failed => "ERROR",
        };
        format!("Result: {}\n{}", label, self.user_output)
    }
}

pub(super) fn provider_tool_result_content(tool_call: &ToolCall, result: &ToolResult) -> String {
    ProviderToolResultRecord::from_result(tool_call, result).provider_content()
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
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoFmtCheck) => {
                "Checking cargo fmt"
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
        "plan_id": plan.id,
        "recoverable": plan.retryable,
        "safe_retry": plan.safe_retry,
        "failure_type": plan.failure_type,
        "recovery_kind": plan.recovery_kind,
        "allowed_alternatives": plan.allowed_alternatives,
        "retry_budget": plan.retry_budget,
        "side_effect_uncertain": plan.side_effect_uncertain,
        "requires_user_decision": plan.requires_user_decision,
        "suggested_command": plan.suggested_command,
        "user_note": plan.user_note,
        "recovery_action": plan.action,
        "recovery_category": plan.category,
    });
    merge_tool_result_metadata(result, "recovery", metadata);
}

pub(super) fn attach_tool_contract_metadata(
    tool: &dyn Tool,
    tool_call: &ToolCall,
    result: &mut ToolResult,
) {
    let params = &tool_call.arguments;
    let mut observable_input = params.clone();
    tool.backfill_observable_input(&mut observable_input);
    let contract = serde_json::json!({
        "operation_kind": serde_json::to_value(tool.operation_kind(params)).unwrap_or(serde_json::Value::Null),
        "tool_kind": serde_json::to_value(tool.tool_kind(params)).unwrap_or(serde_json::Value::Null),
        "tool_family": serde_json::to_value(tool.tool_family(params)).unwrap_or(serde_json::Value::Null),
        "read_only": tool.is_read_only(params),
        "concurrency_safe": tool.is_concurrency_safe(params),
        "destructive": tool.is_destructive(params),
        "aliases": tool.aliases(),
        "search_hint": tool.search_hint(),
        "should_defer": tool.should_defer(),
        "always_load": tool.always_load(),
        "strict_schema": tool.strict_schema(),
        "interrupt_behavior": serde_json::to_value(tool.interrupt_behavior()).unwrap_or(serde_json::Value::Null),
        "requires_user_interaction": tool.requires_user_interaction(),
        "open_world": tool.is_open_world(params),
        "search_or_read": serde_json::to_value(tool.is_search_or_read_command(params)).unwrap_or(serde_json::Value::Null),
        "input_paths": tool.input_paths(params),
        "permission_matcher_input": tool.permission_matcher_input(params),
        "observable_input": observable_input,
        "transcript_summary": tool.transcript_summary(params),
        "ui_render_kind": serde_json::to_value(tool.ui_render_kind(params)).unwrap_or(serde_json::Value::Null),
        "user_facing_name": tool.user_facing_name(params),
        "tool_use_summary": tool.tool_use_summary(params),
        "activity_description": tool.activity_description(params),
        "max_result_size_chars": tool.max_result_size_chars(),
    });
    merge_tool_result_metadata(result, "tool_contract", contract.clone());

    let Some(summary) = result
        .data
        .as_mut()
        .and_then(|data| data.get_mut("tool_summary"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    for key in [
        "operation_kind",
        "read_only",
        "concurrency_safe",
        "destructive",
        "aliases",
        "search_hint",
        "should_defer",
        "always_load",
        "strict_schema",
        "interrupt_behavior",
        "requires_user_interaction",
        "open_world",
        "search_or_read",
        "input_paths",
        "permission_matcher_input",
        "transcript_summary",
        "ui_render_kind",
        "user_facing_name",
        "tool_use_summary",
        "activity_description",
        "max_result_size_chars",
    ] {
        if let Some(value) = contract.get(key) {
            summary.insert(key.to_string(), value.clone());
        }
    }
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
    crate::engine::context_ledger::record_tool_context_evidence(
        store, session_id, tool_call, result,
    );
}

/// Persist a shell job record to session_jobs when a bash/shell tool completes.
pub(super) fn persist_session_job_if_shell(
    store: Option<&Arc<crate::session_store::SessionStore>>,
    session_id: &str,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(store) = store else { return };
    if !tool_call.name.starts_with("bash") && tool_call.name != "shell" {
        return;
    }
    let command = tool_call
        .arguments
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let timed_out = result
        .data
        .as_ref()
        .and_then(|d| d.get("shell_result"))
        .and_then(|s| s.get("timed_out"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let shell_result = result.data.as_ref().and_then(|d| d.get("shell_result"));
    let exit_code = shell_result
        .and_then(|s| s.get("exit_code"))
        .and_then(|v| v.as_i64())
        .and_then(|v| i32::try_from(v).ok());
    let cwd = shell_result
        .and_then(|s| s.get("cwd"))
        .and_then(|v| v.as_str());
    let explicit_cancelled = shell_result
        .and_then(|s| s.get("cancelled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || shell_result
            .and_then(|s| s.get("status"))
            .and_then(|v| v.as_str())
            .is_some_and(|status| status == "cancelled")
        || result
            .error
            .as_deref()
            .is_some_and(|error| error.contains("cancelled") || error.contains("canceled"));
    let status = if explicit_cancelled {
        "cancelled"
    } else if timed_out {
        "timed_out"
    } else if result.success {
        "completed"
    } else {
        "failed"
    };
    let tool_output_uri = result
        .data
        .as_ref()
        .and_then(|d| d.get("tool_output_id"))
        .and_then(|v| v.as_str())
        .map(|id| format!("tool-output://{id}"));
    let _ = store.record_session_job(
        session_id,
        &tool_call.id,
        command,
        cwd,
        status,
        exit_code,
        timed_out,
        tool_output_uri.as_deref(),
    );
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
    fn persist_tool_outcome_also_records_context_ledger_evidence() {
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store.create_session("s1", "test", "model").unwrap();
        let call = tool_call("bash");
        let result = ToolResult::success_with_data(
            "ok",
            serde_json::json!({
                "shell_result": {
                    "command": "cargo test -q",
                    "cwd": "/tmp/project",
                    "exit_code": 0,
                    "timed_out": false
                }
            }),
        );

        persist_tool_outcome_learning_event(Some(&store), "s1", &call, &result);

        let learning_events = store.recent_learning_events("s1", 10).unwrap();
        assert!(learning_events
            .iter()
            .any(|event| event.kind == "tool_outcome"));
        let context_events = store.recent_context_ledger_events("s1", 10).unwrap();
        assert!(context_events.iter().any(|event| {
            crate::engine::context_ledger::validation_entry_from_event(event)
                .map(|entry| entry.command == "cargo test -q")
                .unwrap_or(false)
        }));
    }

    #[test]
    fn persist_session_job_if_shell_records_shell_metadata() {
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store.create_session("s1", "test", "model").unwrap();
        let call = tool_call("bash");
        let result = ToolResult::success_with_data(
            "ok",
            serde_json::json!({
                "shell_result": {
                    "command": "cargo test -q",
                    "cwd": "/tmp/project",
                    "exit_code": 0,
                    "timed_out": false
                },
                "tool_output_id": "out_123"
            }),
        );

        persist_session_job_if_shell(Some(&store), "s1", &call, &result);

        let jobs = store.get_session_jobs("s1").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].command, "cargo test -q");
        assert_eq!(jobs[0].cwd.as_deref(), Some("/tmp/project"));
        assert_eq!(jobs[0].status, "completed");
        assert_eq!(jobs[0].exit_code, Some(0));
        assert!(!jobs[0].timed_out);
        assert_eq!(
            jobs[0].tool_output_uri.as_deref(),
            Some("tool-output://out_123")
        );
        assert!(!jobs[0].cancelled);
    }

    #[test]
    fn persist_session_job_if_shell_records_cancelled_status() {
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store.create_session("s1", "test", "model").unwrap();
        let call = tool_call("bash");
        let mut result = ToolResult::error("cancelled");
        result.data = Some(serde_json::json!({
            "shell_result": {
                "command": "cargo test -q",
                "cwd": "/tmp/project",
                "exit_code": null,
                "timed_out": false,
                "status": "cancelled",
                "cancelled": true
            }
        }));

        persist_session_job_if_shell(Some(&store), "s1", &call, &result);

        let jobs = store.get_session_jobs("s1").unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "cancelled");
        assert!(jobs[0].cancelled);
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
    fn provider_tool_result_record_separates_machine_metadata_from_provider_text() {
        let record = ProviderToolResultRecord::from_result(
            &tool_call("bash"),
            &ToolResult::success("compiled"),
        );

        assert_eq!(record.status, ProviderToolResultStatus::Completed);
        assert_eq!(record.provider_content(), "Result: OK\ncompiled");
        assert_eq!(record.machine_metadata["tool"], "bash");
        assert_eq!(record.machine_metadata["command_kind"], "validation");
        assert_eq!(record.machine_metadata["command_category"], "test_run");
        assert_eq!(record.machine_metadata["network_access"], false);
        assert_eq!(record.machine_metadata["external_path_access"], false);
        assert_eq!(
            record.machine_metadata["permission_rule_suggestions"][1]["pattern"],
            "cargo test"
        );
        assert_eq!(record.machine_metadata["parser_status"], "simple");
        assert!(record.machine_metadata["subcommands"].is_array());
    }

    #[test]
    fn tool_execution_summary_includes_bash_path_patterns() {
        let mut call = tool_call("bash");
        call.arguments = serde_json::json!({
            "command": "rg -n TODO src src/tools && sed -i '' 's/a/b/' src/lib.rs"
        });

        let summary = build_tool_execution_summary(&call, &ToolResult::success("src/lib.rs:1"));

        assert_eq!(summary["parser_status"], "compound");
        assert!(summary["path_patterns"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("src/lib.rs")));
        assert_eq!(summary["subcommands"][1]["category"], "file_mutation");
        assert_eq!(summary["subcommands"][1]["mutation"], true);
        assert!(summary["mutation_indicators"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("sed_in_place")));
        assert_eq!(summary["network_access"], false);
        assert_eq!(summary["external_path_access"], false);
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
                    "exit_code": 101,
                    "failure_reason": "validation command returned a non-zero exit code",
                    "recovery_action": "inspect_output_then_fix_code"
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
        assert_eq!(
            summary["terminal_task"]["recovery_action"],
            "inspect_output_then_fix_code"
        );
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
    fn tool_execution_summary_includes_file_diff_preview() {
        let call = ToolCall {
            id: "call_1".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({"path": "src/lib.rs"}),
        };
        let result = ToolResult::success_with_data(
            "edited",
            serde_json::json!({
                "replacements": 1,
                "diff": {
                    "additions": 2,
                    "deletions": 1,
                    "preview_truncated": false,
                    "unified_diff": "@@ -1 +1 @@\n-old\n+new\n+again\n"
                },
                "checkpoint": {
                    "id": "cp_123"
                },
                "diagnostics_delta": {
                    "status": "new_errors",
                    "change": {
                        "diagnostic_count": 1,
                        "error_count": 1,
                        "warning_count": 0
                    }
                }
            }),
        );

        let summary = build_tool_execution_summary(&call, &result);

        assert_eq!(summary["path"], "src/lib.rs");
        assert_eq!(summary["replacements"], 1);
        assert_eq!(summary["additions"], 2);
        assert_eq!(summary["deletions"], 1);
        assert_eq!(summary["diff_preview_truncated"], false);
        assert_eq!(summary["checkpoint_id"], "cp_123");
        assert_eq!(summary["rollback_id"], "cp_123");
        assert_eq!(summary["diagnostics_delta_status"], "new_errors");
        assert_eq!(summary["diagnostics_delta_error_count"], 1);
        assert!(summary["diff_preview"]
            .as_str()
            .unwrap_or_default()
            .contains("+new"));
    }

    #[test]
    fn tool_execution_summary_includes_file_failure_recovery() {
        let call = ToolCall {
            id: "call_1".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({"path": ".env"}),
        };
        let result = ToolResult::error_with_content(
            "Refusing file_edit for '.env': target looks like a credential file.",
            serde_json::json!({
                "failure": "secret_or_credential_target",
                "guardrail": {
                    "reason": "target looks like an environment, credential, certificate, or SSH key file"
                },
                "recovery": {
                    "recommended_action": "ask_user_for_explicit_secret_file_plan"
                }
            })
            .to_string(),
        );
        let mut result = result;
        result.data = Some(serde_json::json!({
            "failure": "secret_or_credential_target",
            "guardrail": {
                "reason": "target looks like an environment, credential, certificate, or SSH key file"
            },
            "recovery": {
                "recommended_action": "ask_user_for_explicit_secret_file_plan"
            }
        }));

        let summary = build_tool_execution_summary(&call, &result);

        assert_eq!(summary["failure_kind"], "secret_or_credential_target");
        assert_eq!(
            summary["recovery_action"],
            "ask_user_for_explicit_secret_file_plan"
        );
        assert!(summary["guardrail_reason"]
            .as_str()
            .unwrap_or_default()
            .contains("credential"));
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

    #[test]
    fn attach_tool_contract_metadata_adds_runtime_semantics() {
        let call = ToolCall {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "ls -la src"}),
        };
        let mut result = ToolResult::success("listed files");

        attach_tool_execution_metadata(&call, &mut result);
        attach_tool_contract_metadata(&crate::tools::BashTool, &call, &mut result);

        let data = result.data.as_ref().unwrap();
        assert_eq!(data["tool_contract"]["operation_kind"], "list");
        assert_eq!(data["tool_contract"]["read_only"], true);
        assert_eq!(data["tool_contract"]["concurrency_safe"], true);
        assert_eq!(
            data["tool_contract"]["aliases"],
            serde_json::json!(["shell"])
        );
        assert_eq!(data["tool_contract"]["strict_schema"], true);
        assert_eq!(data["tool_contract"]["interrupt_behavior"], "block");
        assert_eq!(
            data["tool_contract"]["search_or_read"],
            serde_json::json!({"is_search": false, "is_read": false, "is_list": true})
        );
        assert_eq!(
            data["tool_contract"]["input_paths"],
            serde_json::json!(["src"])
        );
        assert_eq!(
            data["tool_contract"]["permission_matcher_input"],
            "ls -la src"
        );
        assert_eq!(data["tool_contract"]["ui_render_kind"], "search");
        assert_eq!(data["tool_summary"]["operation_kind"], "list");
        assert_eq!(data["tool_summary"]["read_only"], true);
        assert_eq!(
            data["tool_summary"]["activity_description"],
            "Inspecting: ls -la src"
        );
        assert_eq!(data["tool_summary"]["ui_render_kind"], "search");
    }
}

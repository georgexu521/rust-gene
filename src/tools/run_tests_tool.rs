//! Narrow validation facade over safe test/check command families.

use crate::tools::bash_tool::command_classifier::{classify_command_with_working_dir, CommandKind};
use crate::tools::{Tool, ToolContext, ToolErrorCode, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub struct RunTestsTool;

#[async_trait]
impl Tool for RunTestsTool {
    fn name(&self) -> &str {
        "run_tests"
    }

    fn description(&self) -> &str {
        "Run a safe validation command; prefer this over bash for user-requested \
         tests/checks. Supports cargo test/check/clippy, pytest, npm test, go \
         test, py_compile, bash -n, and known local assertion scripts. Rejects \
         mutation, install, network, interactive, and arbitrary shell commands."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Safe validation command to run, for example 'cargo test -q' or 'python3 -m py_compile scripts/live_eval_report_parser.py'."
                },
                "timeout_secs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                    "default": 300,
                    "description": "Maximum runtime in seconds."
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let requested_command = params["command"].as_str().unwrap_or_default().trim();
        if requested_command.is_empty() {
            return ToolResult::error("command is required");
        }
        let normalized_command =
            normalize_validation_command(requested_command, &context.working_dir);
        let command = normalized_command.as_str();
        let classification = classify_command_with_working_dir(command, &context.working_dir);
        if !command_allowed_for_run_tests(&classification) {
            let mut result = ToolResult::error(format!(
                "run_tests only accepts safe validation commands; rejected: {requested_command}"
            ));
            result.error_code = Some(ToolErrorCode::InvalidParams);
            result.data = Some(json!({
                "tool": "run_tests",
                "failure": "unsafe_validation_command",
                "requested_command": requested_command,
                "normalized_command": command,
                "command_classification": classification,
            }));
            return result;
        }

        let timeout_secs = params["timeout_secs"]
            .as_u64()
            .unwrap_or(300)
            .clamp(1, 3600);
        let started = Instant::now();
        let output = timeout(
            Duration::from_secs(timeout_secs),
            Command::new("sh")
                .arg("-lc")
                .arg(command)
                .current_dir(&context.working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        match output {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code();
                let signal = exit_signal(&output.status);
                let content = validation_output(command, exit_code, signal, &stdout, &stderr);
                let data = json!({
                    "tool": "run_tests",
                    "shell_result": {
                        "command": command,
                        "requested_command": requested_command,
                        "cwd": context.working_dir.display().to_string(),
                        "exit_code": exit_code,
                        "signal": signal,
                        "stdout_bytes": output.stdout.len(),
                        "stderr_bytes": output.stderr.len(),
                        "timed_out": false,
                    },
                    "command_classification": classification,
                    "validation_result": {
                        "status": if output.status.success() { "passed" } else { "failed" },
                        "duration_ms": started.elapsed().as_millis() as u64,
                    }
                });
                if output.status.success() {
                    ToolResult::success_with_data(content, data)
                } else {
                    let mut result = ToolResult::error(content);
                    result.error_code = Some(ToolErrorCode::ExecutionFailed);
                    result.data = Some(data);
                    result
                }
            }
            Ok(Err(err)) => {
                let mut result =
                    ToolResult::error(format!("Failed to run validation command: {err}"));
                result.error_code = Some(ToolErrorCode::ExecutionFailed);
                result.data = Some(json!({
                    "tool": "run_tests",
                    "shell_result": {
                        "command": command,
                        "requested_command": requested_command,
                        "cwd": context.working_dir.display().to_string(),
                        "exit_code": null,
                        "signal": null,
                        "stdout_bytes": 0,
                        "stderr_bytes": 0,
                        "timed_out": false,
                    },
                    "command_classification": classification,
                }));
                result
            }
            Err(_) => {
                let mut result = ToolResult::error(format!(
                    "Validation command timed out after {timeout_secs}s: {command}"
                ));
                result.error_code = Some(ToolErrorCode::Timeout);
                result.data = Some(json!({
                    "tool": "run_tests",
                    "shell_result": {
                        "command": command,
                        "requested_command": requested_command,
                        "cwd": context.working_dir.display().to_string(),
                        "exit_code": null,
                        "signal": null,
                        "stdout_bytes": 0,
                        "stderr_bytes": 0,
                        "timed_out": true,
                    },
                    "command_classification": classification,
                }));
                result
            }
        }
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Task
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("safe validation test check commands")
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        format!(
            "run_tests: {}",
            params["command"].as_str().unwrap_or_default()
        )
    }

    fn permission_matcher_input(&self, params: &serde_json::Value) -> Option<String> {
        params["command"]
            .as_str()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .map(str::to_string)
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        params["command"]
            .as_str()
            .map(|command| command.to_string())
    }
}

fn command_allowed_for_run_tests(
    classification: &crate::tools::bash_tool::command_classifier::CommandClassification,
) -> bool {
    classification.command_kind == CommandKind::Validation
        && classification.is_safe_validation()
        && !classification.network_access
        && !classification.external_path_access
        && classification.mutation_paths.is_empty()
        && classification.mutation_indicators.is_empty()
        && !classification.command_plan.has_write_redirection
        && !classification.command_plan.fail_closed
}

fn normalize_validation_command(command: &str, working_dir: &Path) -> String {
    let trimmed = command.trim();
    let Some((lhs, rhs)) = trimmed.split_once("&&") else {
        return trimmed.to_string();
    };
    let lhs = lhs.trim();
    let Some(target) = lhs.strip_prefix("cd ") else {
        return trimmed.to_string();
    };
    let target = target.trim().trim_matches('"').trim_matches('\'');
    if !same_working_dir(target, working_dir) {
        return trimmed.to_string();
    }
    let rhs = rhs.trim();
    if rhs.is_empty() {
        trimmed.to_string()
    } else {
        rhs.to_string()
    }
}

fn same_working_dir(target: &str, working_dir: &Path) -> bool {
    if target == "." {
        return true;
    }
    let target_path = Path::new(target);
    if !target_path.is_absolute() {
        return false;
    }
    match (
        std::fs::canonicalize(target_path),
        std::fs::canonicalize(working_dir),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => target_path == working_dir,
    }
}

fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}

fn validation_output(
    command: &str,
    exit_code: Option<i32>,
    signal: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> String {
    let mut out = format!(
        "Validation command `{}` exited with {}",
        command,
        exit_code
            .map(|code| code.to_string())
            .or_else(|| signal.map(|signal| format!("signal {signal}")))
            .unwrap_or_else(|| "unknown".to_string())
    );
    if !stdout.trim().is_empty() {
        out.push_str("\n\nstdout:\n");
        out.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        out.push_str("\n\nstderr:\n");
        out.push_str(stderr.trim_end());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolPermissions;
    use std::collections::HashMap;

    fn context() -> ToolContext {
        ToolContext {
            working_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            session_id: "run-tests-tool-test".to_string(),
            model: "test".to_string(),
            permissions: ToolPermissions::default(),
            permission_context: crate::permissions::PermissionContext::new("."),
            metadata: HashMap::new(),
            retained_context: Default::default(),
            parent_assistant_tool_calls: Vec::new(),
            parent_assistant_content: String::new(),
            llm_provider: None,
            agent_manager: None,
            trace_collector: None,
            session_store: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            task_manager: None,
            cost_tracker: None,
            file_cache: None,
            diagnostic_tracker: None,
            checkpoint_manager: None,
            memory_manager: None,
            read_tracker: None,
        }
    }

    #[test]
    fn run_tests_contract_prefers_user_requested_validation() {
        let tool = RunTestsTool;
        assert!(tool.description().contains("prefer this over bash"));
        assert!(tool.description().contains("user-requested"));
        assert!(tool.description().contains("Rejects mutation"));
    }

    #[tokio::test]
    async fn run_tests_rejects_mutating_shell_command() {
        let tool = RunTestsTool;
        let result = tool
            .execute(json!({"command": "rm -rf target"}), context())
            .await;

        assert!(!result.success);
        assert_eq!(result.error_code, Some(ToolErrorCode::InvalidParams));
        assert_eq!(result.data.unwrap()["failure"], "unsafe_validation_command");
    }

    #[tokio::test]
    async fn run_tests_runs_safe_python_compile_command() {
        let tool = RunTestsTool;
        let result = tool
            .execute(
                json!({
                    "command": "python3 -m py_compile scripts/live_eval_report_parser.py",
                    "timeout_secs": 30
                }),
                context(),
            )
            .await;

        assert!(
            result.success,
            "content={} error={:?} data={:?}",
            result.content, result.error, result.data
        );
        let data = result.data.expect("metadata");
        assert_eq!(
            data["shell_result"]["command"],
            "python3 -m py_compile scripts/live_eval_report_parser.py"
        );
        assert_eq!(data["validation_result"]["status"], "passed");
    }

    #[tokio::test]
    async fn run_tests_accepts_safe_command_with_workdir_cd_prefix() {
        let tool = RunTestsTool;
        let cwd = std::env::current_dir().unwrap();
        let result = tool
            .execute(
                json!({
                    "command": format!(
                        "cd {} && python3 -m py_compile scripts/live_eval_report_parser.py",
                        cwd.display()
                    ),
                    "timeout_secs": 30
                }),
                context(),
            )
            .await;

        assert!(
            result.success,
            "content={} error={:?} data={:?}",
            result.content, result.error, result.data
        );
        let data = result.data.expect("metadata");
        assert_eq!(
            data["shell_result"]["command"],
            "python3 -m py_compile scripts/live_eval_report_parser.py"
        );
    }

    #[tokio::test]
    async fn run_tests_rejects_cd_to_different_absolute_directory() {
        let tool = RunTestsTool;
        let result = tool
            .execute(
                json!({
                    "command": "cd /tmp && python3 -m py_compile scripts/live_eval_report_parser.py"
                }),
                context(),
            )
            .await;

        assert!(!result.success);
        assert_eq!(result.error_code, Some(ToolErrorCode::InvalidParams));
    }
}

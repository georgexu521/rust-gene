//! Narrow facade for starting local development servers as background tasks.

use crate::tools::bash_tool::command_classifier::{
    classify_command, CommandClassification, ShellCommandCategory,
};
use crate::tools::{BashTool, Tool, ToolContext, ToolErrorCode, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::json;

pub struct StartDevServerTool;

#[async_trait]
impl Tool for StartDevServerTool {
    fn name(&self) -> &str {
        "start_dev_server"
    }

    fn description(&self) -> &str {
        "Start a local development server as a managed background terminal task. Accepts only dev-server commands such as npm run dev, pnpm dev, yarn dev, vite, next dev, trunk serve, cargo watch, or python3 -m http.server."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Dev-server command to start, for example 'npm run dev' or 'python3 -m http.server 8000'."
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory inside the project."
                },
                "timeout_secs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                    "default": 3600,
                    "description": "Maximum lifetime for the background server task."
                },
                "expected_url": {
                    "type": "string",
                    "description": "Optional localhost URL expected to become available, for example 'http://localhost:5173'."
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let command = params["command"].as_str().unwrap_or_default().trim();
        if command.is_empty() {
            return invalid_dev_server_command(command, None, "command is required");
        }

        let classification = classify_command(command);
        if let Err(reason) = command_allowed_for_dev_server(&classification) {
            return invalid_dev_server_command(command, Some(&classification), reason);
        }

        let timeout_secs = params["timeout_secs"]
            .as_u64()
            .unwrap_or(3600)
            .clamp(1, 3600);
        let expected_url = params["expected_url"]
            .as_str()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_string);
        let working_dir = params["working_dir"]
            .as_str()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string);

        let mut bash_params = json!({
            "command": command,
            "description": "Start dev server",
            "timeout": timeout_secs,
            "mode": "background"
        });
        if let Some(working_dir) = working_dir.as_deref() {
            bash_params["working_dir"] = json!(working_dir);
        }

        let bash = BashTool;
        let mut result = bash.execute(bash_params, context).await;
        attach_dev_server_metadata(
            &mut result,
            command,
            timeout_secs,
            expected_url.as_deref(),
            &classification,
        );
        if result.success {
            result.content = result
                .data
                .as_ref()
                .and_then(|data| data.get("dev_server"))
                .and_then(|server| server.get("handle"))
                .and_then(serde_json::Value::as_str)
                .map(|handle| {
                    format!(
                        "Started dev server task.\nHandle: {handle}\nUse bash_output with this handle to read output; use bash_cancel to stop it."
                    )
                })
                .unwrap_or(result.content);
        }
        result
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
        Some("start local dev server background task")
    }

    fn input_paths(&self, params: &serde_json::Value) -> Vec<String> {
        params["working_dir"]
            .as_str()
            .filter(|path| !path.trim().is_empty())
            .map(|path| vec![path.to_string()])
            .unwrap_or_default()
    }

    fn permission_matcher_input(&self, params: &serde_json::Value) -> Option<String> {
        params["command"]
            .as_str()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .map(str::to_string)
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        format!(
            "start_dev_server: {}",
            params["command"].as_str().unwrap_or_default()
        )
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        params["command"]
            .as_str()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .map(str::to_string)
    }
}

fn command_allowed_for_dev_server(
    classification: &CommandClassification,
) -> Result<(), &'static str> {
    if classification.category != ShellCommandCategory::DevServer {
        return Err("command is not classified as a dev server");
    }
    if classification.network_access {
        return Err("dev server command must not perform remote network access");
    }
    if classification.external_path_access {
        return Err("dev server command must not access external paths");
    }
    if classification.compound_command || classification.risky_shell_wrapper {
        return Err("dev server command must not use compound shell control flow");
    }
    if !classification.mutation_paths.is_empty() || !classification.mutation_indicators.is_empty() {
        return Err("dev server command must not declare file mutation side effects");
    }
    if classification.command_plan.has_write_redirection || classification.command_plan.fail_closed
    {
        return Err("dev server command failed shell safety analysis");
    }
    Ok(())
}

fn invalid_dev_server_command(
    command: &str,
    classification: Option<&CommandClassification>,
    reason: &str,
) -> ToolResult {
    let mut result = ToolResult::error(format!(
        "start_dev_server only accepts safe local dev-server commands; rejected: {command}"
    ));
    result.error_code = Some(ToolErrorCode::InvalidParams);
    result.data = Some(json!({
        "tool": "start_dev_server",
        "failure": "invalid_dev_server_command",
        "reason": reason,
        "command_classification": classification,
    }));
    result
}

fn attach_dev_server_metadata(
    result: &mut ToolResult,
    command: &str,
    timeout_secs: u64,
    expected_url: Option<&str>,
    classification: &CommandClassification,
) {
    let data = result.data.get_or_insert_with(|| json!({}));
    let Some(object) = data.as_object_mut() else {
        return;
    };
    object.insert("tool".to_string(), json!("start_dev_server"));

    let handle = object
        .get("terminal_task")
        .and_then(|task| task.get("handle").or_else(|| task.get("task_id")))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            object
                .get("shell_result")
                .and_then(|shell| shell.get("handle"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    let status = object
        .get("terminal_task")
        .and_then(|task| task.get("status"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(if result.success { "running" } else { "failed" });

    object.insert(
        "dev_server".to_string(),
        json!({
            "schema": "dev_server_task.v1",
            "command": command,
            "handle": handle,
            "task_id": handle,
            "status": status,
            "timeout_secs": timeout_secs,
            "expected_url": expected_url,
            "read_tool": "bash_output",
            "cancel_tool": "bash_cancel",
            "classification": classification,
        }),
    );
    if let Some(terminal_task) = object
        .get_mut("terminal_task")
        .and_then(serde_json::Value::as_object_mut)
    {
        terminal_task.insert("facade_tool".to_string(), json!("start_dev_server"));
        terminal_task.insert("dev_server".to_string(), json!(true));
        if let Some(expected_url) = expected_url {
            terminal_task.insert("expected_url".to_string(), json!(expected_url));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::BashCancelTool;

    #[tokio::test]
    async fn start_dev_server_rejects_non_dev_server_command() {
        let result = StartDevServerTool
            .execute(
                json!({"command": "cargo test -q"}),
                ToolContext::new(".", "test-start-dev-server-reject"),
            )
            .await;

        assert!(!result.success);
        assert_eq!(result.error_code, Some(ToolErrorCode::InvalidParams));
        assert_eq!(
            result.data.as_ref().unwrap()["failure"],
            "invalid_dev_server_command"
        );
    }

    #[tokio::test]
    async fn start_dev_server_returns_background_terminal_task() {
        let dir = tempfile::tempdir().expect("temp dir");
        let result = StartDevServerTool
            .execute(
                json!({
                    "command": "python3 -m http.server 0",
                    "timeout_secs": 30,
                    "expected_url": "http://localhost:0"
                }),
                ToolContext::new(dir.path(), "test-start-dev-server"),
            )
            .await;

        assert!(result.success, "{}", result.content);
        let data = result.data.as_ref().expect("metadata");
        assert_eq!(data["tool"], "start_dev_server");
        assert_eq!(data["dev_server"]["schema"], "dev_server_task.v1");
        assert_eq!(data["terminal_task"]["facade_tool"], "start_dev_server");
        assert_eq!(data["terminal_task"]["read_tool"], "bash_output");
        assert_eq!(data["terminal_task"]["cancel_tool"], "bash_cancel");
        let handle = data["dev_server"]["handle"]
            .as_str()
            .expect("background handle")
            .to_string();

        let cancel = BashCancelTool
            .execute(
                json!({"handle": handle}),
                ToolContext::new(dir.path(), "test-start-dev-server"),
            )
            .await;
        assert!(cancel.success, "{}", cancel.content);
    }
}

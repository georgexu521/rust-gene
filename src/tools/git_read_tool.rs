//! Read-only git facade tools with narrow schemas.

use crate::tools::{
    Tool, ToolContext, ToolOperationKind, ToolResult, ToolSearchOrReadSemantics, ToolUiRenderKind,
};
use async_trait::async_trait;
use serde_json::json;
use tokio::process::Command;

pub struct GitStatusTool;
pub struct GitDiffTool;

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Show read-only git status with optional path filtering. This never stages, commits, checks out, or pushes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional file or directory path to filter status output."
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let path = params["path"].as_str().unwrap_or("").trim();
        let mut command = Command::new("git");
        command.current_dir(&context.working_dir);
        command.args(["status", "--short"]);
        if !path.is_empty() {
            command.arg("--").arg(path);
        }

        git_read_result(
            "git_status",
            "status",
            git_status_summary,
            command.output().await,
        )
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn is_search_or_read_command(&self, _params: &serde_json::Value) -> ToolSearchOrReadSemantics {
        ToolSearchOrReadSemantics {
            is_read: true,
            ..Default::default()
        }
    }

    fn input_paths(&self, params: &serde_json::Value) -> Vec<String> {
        optional_path(params)
    }

    fn ui_render_kind(&self, _params: &serde_json::Value) -> ToolUiRenderKind {
        ToolUiRenderKind::Search
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let path = params["path"].as_str().unwrap_or("").trim();
        if path.is_empty() {
            "git status --short".to_string()
        } else {
            format!("git status --short -- {path}")
        }
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str().unwrap_or("").trim();
        if path.is_empty() {
            Some("status".to_string())
        } else {
            Some(format!("status {path}"))
        }
    }
}

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Show read-only git diff output with optional path, range, cached, or stat filtering. This never stages, commits, checks out, or pushes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional file or directory path to filter diff output."
                },
                "range": {
                    "type": "string",
                    "description": "Optional commit range, for example HEAD~1..HEAD."
                },
                "cached": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show staged changes."
                },
                "stat": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show diffstat instead of full patch output."
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let path = params["path"].as_str().unwrap_or("").trim();
        let range = params["range"].as_str().unwrap_or("").trim();
        let cached = params["cached"].as_bool().unwrap_or(false);
        let stat = params["stat"].as_bool().unwrap_or(false);

        if !range.is_empty() && !is_valid_git_range(range) {
            return git_read_error(
                "git_diff",
                "diff",
                format!("Invalid git diff range: '{range}'"),
                "Use a plain git ref or range such as HEAD~1..HEAD; flags and shell metacharacters are blocked.",
            );
        }

        let mut command = Command::new("git");
        command.current_dir(&context.working_dir);
        command.arg("diff");
        if cached {
            command.arg("--cached");
        }
        if stat {
            command.arg("--stat");
        }
        if !range.is_empty() {
            command.arg(range);
        }
        if !path.is_empty() {
            command.arg("--").arg(path);
        }

        git_read_result("git_diff", "diff", git_diff_summary, command.output().await)
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn is_search_or_read_command(&self, _params: &serde_json::Value) -> ToolSearchOrReadSemantics {
        ToolSearchOrReadSemantics {
            is_read: true,
            ..Default::default()
        }
    }

    fn input_paths(&self, params: &serde_json::Value) -> Vec<String> {
        optional_path(params)
    }

    fn ui_render_kind(&self, _params: &serde_json::Value) -> ToolUiRenderKind {
        ToolUiRenderKind::Search
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let mut parts = vec!["git diff".to_string()];
        if params["cached"].as_bool().unwrap_or(false) {
            parts.push("--cached".to_string());
        }
        if params["stat"].as_bool().unwrap_or(false) {
            parts.push("--stat".to_string());
        }
        if let Some(range) = params["range"]
            .as_str()
            .filter(|range| !range.trim().is_empty())
        {
            parts.push(range.trim().to_string());
        }
        if let Some(path) = params["path"]
            .as_str()
            .filter(|path| !path.trim().is_empty())
        {
            parts.push("--".to_string());
            parts.push(path.trim().to_string());
        }
        parts.join(" ")
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let mut parts = vec!["diff".to_string()];
        if params["cached"].as_bool().unwrap_or(false) {
            parts.push("cached".to_string());
        }
        if params["stat"].as_bool().unwrap_or(false) {
            parts.push("stat".to_string());
        }
        if let Some(path) = params["path"]
            .as_str()
            .filter(|path| !path.trim().is_empty())
        {
            parts.push(path.trim().to_string());
        }
        Some(parts.join(" "))
    }
}

fn git_read_result(
    tool: &str,
    action: &str,
    summary_fn: fn(&str) -> String,
    output: Result<std::process::Output, std::io::Error>,
) -> ToolResult {
    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let text = if stdout.is_empty() && !stderr.is_empty() {
                stderr.to_string()
            } else {
                stdout.to_string()
            };
            ToolResult::success_with_data(
                text.clone(),
                json!({
                    "tool": tool,
                    "action": action,
                    "summary": summary_fn(&text),
                }),
            )
        }
        Ok(output) => git_read_error(
            tool,
            action,
            format!(
                "git {action} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
            "Confirm the working directory is a git repository and retry with a valid path/ref.",
        ),
        Err(error) => git_read_error(
            tool,
            action,
            format!("Failed to run git {action}: {error}"),
            "Verify git is installed and the working directory is accessible.",
        ),
    }
}

fn git_read_error(
    tool: &str,
    action: &str,
    error: impl Into<String>,
    recovery: impl Into<String>,
) -> ToolResult {
    let recovery = recovery.into();
    let mut result = ToolResult::error(error);
    result.data = Some(json!({
        "tool": tool,
        "action": action,
        "summary": format!("git {action} failed"),
        "recovery": recovery,
    }));
    result
}

fn git_status_summary(output: &str) -> String {
    let changed = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    if changed == 0 {
        "git status: clean working tree".to_string()
    } else {
        format!("git status: {changed} changed paths")
    }
}

fn git_diff_summary(output: &str) -> String {
    if output.trim().is_empty() {
        "git diff: no changes".to_string()
    } else {
        "git diff: changes displayed".to_string()
    }
}

fn optional_path(params: &serde_json::Value) -> Vec<String> {
    params["path"]
        .as_str()
        .filter(|path| !path.trim().is_empty())
        .map(|path| vec![path.to_string()])
        .unwrap_or_default()
}

fn is_valid_git_range(range: &str) -> bool {
    if range.is_empty() || range.starts_with('-') {
        return false;
    }
    let forbidden = [';', '|', '&', '$', '`', '\n', '\r', '\t', '<', '>'];
    if range.chars().any(|ch| forbidden.contains(&ch)) {
        return false;
    }
    range.chars().all(|ch| {
        ch.is_alphanumeric()
            || ch == '.'
            || ch == '~'
            || ch == '^'
            || ch == '/'
            || ch == '-'
            || ch == '_'
            || ch == ':'
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    fn init_git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        assert!(StdCommand::new("git")
            .arg("init")
            .current_dir(dir.path())
            .output()
            .unwrap()
            .status
            .success());
        assert!(StdCommand::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .status
            .success());
        assert!(StdCommand::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .status
            .success());
        dir
    }

    #[tokio::test]
    async fn git_status_reports_changed_paths() {
        let dir = init_git_repo();
        std::fs::write(dir.path().join("changed.txt"), "hello\n").unwrap();
        let result = GitStatusTool
            .execute(json!({}), ToolContext::new(dir.path(), "test"))
            .await;

        assert!(result.success, "{}", result.content);
        assert!(result.content.contains("?? changed.txt"));
        assert_eq!(
            result.data.as_ref().unwrap()["summary"],
            "git status: 1 changed paths"
        );
    }

    #[tokio::test]
    async fn git_diff_reports_file_diff_without_mutating_repo() {
        let dir = init_git_repo();
        std::fs::write(dir.path().join("tracked.txt"), "before\n").unwrap();
        assert!(StdCommand::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .status
            .success());
        assert!(StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .status
            .success());
        std::fs::write(dir.path().join("tracked.txt"), "after\n").unwrap();

        let result = GitDiffTool
            .execute(
                json!({"path": "tracked.txt"}),
                ToolContext::new(dir.path(), "test"),
            )
            .await;

        assert!(result.success, "{}", result.content);
        assert!(result.content.contains("-before"));
        assert!(result.content.contains("+after"));
        assert_eq!(
            result.data.as_ref().unwrap()["summary"],
            "git diff: changes displayed"
        );
    }

    #[tokio::test]
    async fn git_diff_rejects_flag_like_range() {
        let result = GitDiffTool
            .execute(json!({"range": "--help"}), ToolContext::new(".", "test"))
            .await;

        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid git diff range"));
    }
}

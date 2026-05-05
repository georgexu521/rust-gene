//! Git 工具 - 执行常见的 git 操作
//!
//! 支持只读操作（status/diff/log/show）和写操作（add/commit/push/checkout/branch）。
//! 写操作需要用户确认，并有过滤危险参数的安全校验。

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use tokio::process::Command;

/// Git 工具
pub struct GitTool;

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Run common git operations. \
         Read-only actions: 'status', 'diff', 'log', 'show'. \
         Write actions: 'add', 'commit', 'push', 'checkout', 'branch'. \
         Write actions require user confirmation."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "diff", "log", "show", "add", "commit", "push", "checkout", "branch"],
                    "description": "The git action to perform"
                },
                "path": {
                    "type": "string",
                    "description": "Optional file path filter (for status, diff, add)"
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of file paths (for add action)"
                },
                "commit": {
                    "type": "string",
                    "description": "Commit hash for 'show' action, or commit message for 'commit' action"
                },
                "range": {
                    "type": "string",
                    "description": "Commit range for 'diff' action (e.g. HEAD~1..HEAD)"
                },
                "cached": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show staged changes (for 'diff' action)"
                },
                "stat": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether to show diffstat"
                },
                "n": {
                    "type": "integer",
                    "default": 10,
                    "description": "Number of commits for 'log' action"
                },
                "branch": {
                    "type": "string",
                    "description": "Branch name for 'checkout' or 'branch' action"
                },
                "create_branch": {
                    "type": "boolean",
                    "default": false,
                    "description": "Create a new branch (for checkout -b)"
                },
                "remote": {
                    "type": "string",
                    "default": "origin",
                    "description": "Remote name for 'push' action"
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "Force push (DANGEROUS — requires extra confirmation)"
                },
                "all": {
                    "type": "boolean",
                    "default": false,
                    "description": "Stage all changes (for commit -a)"
                }
            },
            "required": ["action"]
        })
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        matches!(
            params["action"].as_str(),
            Some("add" | "commit" | "push" | "checkout" | "branch")
        )
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "add" => {
                let path = params["path"].as_str().unwrap_or("");
                if !path.is_empty() {
                    Some(format!("Stage file '{}' for commit?", path))
                } else {
                    Some("Stage all changes for commit?".to_string())
                }
            }
            "commit" => {
                let msg = params["commit"].as_str().unwrap_or("(no message)");
                Some(format!("Create git commit with message: '{}'", msg))
            }
            "push" => {
                let remote = params["remote"].as_str().unwrap_or("origin");
                let force = params["force"].as_bool().unwrap_or(false);
                if force {
                    Some(format!(
                        "⚠️ FORCE push to {}? This will overwrite remote history!",
                        remote
                    ))
                } else {
                    Some(format!("Push current branch to {}?", remote))
                }
            }
            "checkout" => {
                let branch = params["branch"].as_str().unwrap_or("");
                let create = params["create_branch"].as_bool().unwrap_or(false);
                if create {
                    Some(format!("Create and switch to new branch '{}'?", branch))
                } else {
                    Some(format!("Switch to branch '{}'?", branch))
                }
            }
            "branch" => {
                let branch = params["branch"].as_str().unwrap_or("");
                Some(format!("Create new branch '{}'?", branch))
            }
            _ => None,
        }
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "commit" => {
                let msg = params["commit"].as_str().unwrap_or("");
                format!("git commit: '{}'", msg)
            }
            "push" => {
                let remote = params["remote"].as_str().unwrap_or("origin");
                let force = params["force"].as_bool().unwrap_or(false);
                format!("git push: remote={} force={}", remote, force)
            }
            "checkout" | "branch" => {
                let branch = params["branch"].as_str().unwrap_or("");
                format!("git {}: {}", action, branch)
            }
            "add" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    "git add: all".to_string()
                } else {
                    format!("git add: {}", path)
                }
            }
            _ => format!("git {}", action),
        }
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        let path = params["path"].as_str().unwrap_or("");
        let stat = params["stat"].as_bool().unwrap_or(false);
        let cwd = context.working_dir;

        let result = match action {
            // ── 只读操作 ───────────────────────────────
            "status" => {
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                cmd.arg("status").arg("--short");
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            "diff" => {
                let cached = params["cached"].as_bool().unwrap_or(false);
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                if stat {
                    cmd.arg("diff").arg("--stat");
                } else if cached {
                    cmd.arg("diff").arg("--cached");
                } else {
                    cmd.arg("diff");
                }
                if let Some(range) = params["range"].as_str() {
                    if !is_valid_git_range(range) {
                        return git_error(
                            action,
                            format!("Invalid git diff range: '{}'", range),
                            "Use a plain git ref or range such as HEAD~1..HEAD; flags and shell metacharacters are blocked.",
                        );
                    }
                    cmd.arg(range);
                }
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            "log" => {
                let n = params["n"].as_u64().unwrap_or(10) as usize;
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                cmd.args(["log", "--oneline", "-n", &n.to_string()]);
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            "show" => {
                let commit = params["commit"].as_str().unwrap_or("HEAD");
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                cmd.arg("show").arg("--stat").arg(commit);
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            // ── 写操作 ─────────────────────────────────
            "add" => {
                if let Some(paths) = params["paths"].as_array() {
                    let mut cmd = Command::new("git");
                    cmd.current_dir(&cwd);
                    cmd.arg("add");
                    for p in paths {
                        if let Some(s) = p.as_str() {
                            cmd.arg(s);
                        }
                    }
                    cmd.output().await
                } else if !path.is_empty() {
                    let mut cmd = Command::new("git");
                    cmd.current_dir(&cwd);
                    cmd.arg("add").arg(path).output().await
                } else {
                    let mut cmd = Command::new("git");
                    cmd.current_dir(&cwd);
                    cmd.arg("add").arg("-A").output().await
                }
            }
            "commit" => {
                let message = params["commit"].as_str().unwrap_or("");
                if message.is_empty() {
                    return git_error(
                        action,
                        "Commit message cannot be empty",
                        "Pass a non-empty 'commit' message before retrying git commit.",
                    );
                }
                let all = params["all"].as_bool().unwrap_or(false);
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                cmd.arg("commit").arg("-m").arg(message);
                if all {
                    cmd.arg("-a");
                }
                cmd.output().await
            }
            "push" => {
                let remote = params["remote"].as_str().unwrap_or("origin");
                let force = params["force"].as_bool().unwrap_or(false);
                if force && !is_safe_force_push(remote) {
                    return git_error(
                        action,
                        "Force push to this remote is blocked for safety.",
                        "Use a non-protected fork remote or retry without force.",
                    );
                }
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                cmd.arg("push").arg(remote);
                if force {
                    cmd.arg("--force-with-lease");
                }
                // 推送当前分支
                cmd.arg("HEAD");
                cmd.output().await
            }
            "checkout" => {
                let branch = params["branch"].as_str().unwrap_or("");
                if branch.is_empty() {
                    return git_error(
                        action,
                        "Branch name required for checkout",
                        "Pass the 'branch' parameter before retrying checkout.",
                    );
                }
                let create = params["create_branch"].as_bool().unwrap_or(false);
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                cmd.arg("checkout");
                if create {
                    cmd.arg("-b");
                }
                cmd.arg(branch);
                cmd.output().await
            }
            "branch" => {
                let branch = params["branch"].as_str().unwrap_or("");
                if branch.is_empty() {
                    return git_error(
                        action,
                        "Branch name required",
                        "Pass the 'branch' parameter before retrying branch creation.",
                    );
                }
                let mut cmd = Command::new("git");
                cmd.current_dir(&cwd);
                cmd.arg("branch").arg(branch).output().await
            }
            _ => {
                return git_error(
                    action,
                    format!("Unknown git action: {}", action),
                    "Use one of: status, diff, log, show, add, commit, push, checkout, branch.",
                );
            }
        };

        match result {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let output = if text.is_empty() && !stderr.is_empty() {
                    stderr.to_string()
                } else {
                    text.to_string()
                };
                git_success(action, output)
            }
            Ok(out) => git_error(
                action,
                format!(
                    "git {} failed: {}",
                    action,
                    String::from_utf8_lossy(&out.stderr)
                ),
                git_recovery(action),
            ),
            Err(e) => git_error(
                action,
                format!("Failed to run git: {}", e),
                "Verify git is installed and the working directory is accessible.",
            ),
        }
    }
}

fn git_success(action: &str, output: String) -> ToolResult {
    ToolResult::success_with_data(
        output.clone(),
        json!({
            "action": action,
            "summary": git_success_summary(action, &output),
        }),
    )
}

fn git_error(action: &str, error: impl Into<String>, recovery: impl Into<String>) -> ToolResult {
    let recovery = recovery.into();
    let mut result = ToolResult::error(error);
    result.data = Some(json!({
        "action": action,
        "summary": format!("git {} failed", action),
        "recovery": recovery,
    }));
    result
}

fn git_success_summary(action: &str, output: &str) -> String {
    match action {
        "status" => {
            let changed = output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
            if changed == 0 {
                "git status: clean working tree".to_string()
            } else {
                format!("git status: {} changed paths", changed)
            }
        }
        "diff" => {
            if output.trim().is_empty() {
                "git diff: no changes".to_string()
            } else {
                "git diff: changes displayed".to_string()
            }
        }
        "log" => {
            let commits = output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();
            format!("git log: {} commits", commits)
        }
        "show" => "git show: commit details displayed".to_string(),
        "add" => "git add: staged requested paths".to_string(),
        "commit" => first_nonempty_line(output)
            .unwrap_or("git commit: created commit")
            .to_string(),
        "push" => "git push: pushed current HEAD".to_string(),
        "checkout" => first_nonempty_line(output)
            .unwrap_or("git checkout: switched branch")
            .to_string(),
        "branch" => "git branch: branch created".to_string(),
        _ => format!("git {}: completed", action),
    }
}

fn first_nonempty_line(output: &str) -> Option<&str> {
    output.lines().find(|line| !line.trim().is_empty())
}

fn git_recovery(action: &str) -> &'static str {
    match action {
        "status" | "diff" | "log" | "show" => {
            "Confirm the working directory is a git repository and retry with a valid path/ref."
        }
        "add" => "Confirm the path exists and is inside the repository, then retry git add.",
        "commit" => {
            "Check staged changes with git status, provide a non-empty message, then retry commit."
        }
        "push" => "Check the remote name, branch permissions, and network access before retrying.",
        "checkout" | "branch" => {
            "Check the branch name and current working tree state before retrying."
        }
        _ => "Use a supported git action and valid arguments.",
    }
}

/// 检查 git diff range 参数是否合法
fn is_valid_git_range(range: &str) -> bool {
    if range.is_empty() {
        return false;
    }
    // 禁止以 - 开头，防止被解析为 git flag
    if range.starts_with('-') {
        return false;
    }
    // 禁止包含 shell 元字符或控制字符
    let forbidden = [';', '|', '&', '$', '`', '\n', '\r', '\t', '<', '>'];
    if range.chars().any(|c| forbidden.contains(&c)) {
        return false;
    }
    // 只允许常见的 git ref 字符
    range.chars().all(|c| {
        c.is_alphanumeric()
            || c == '.'
            || c == '~'
            || c == '^'
            || c == '/'
            || c == '-'
            || c == '_'
            || c == ':'
    })
}

/// 检查 force push 是否安全
fn is_safe_force_push(remote: &str) -> bool {
    // 禁止向 origin 直接 force push（保护主分支）
    // 允许向个人 fork 或 feature remote force push
    let lower = remote.to_lowercase();
    !matches!(lower.as_str(), "origin" | "upstream")
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
    async fn test_git_status() {
        let tool = GitTool;
        let result = tool
            .execute(json!({ "action": "status" }), ToolContext::new(".", "test"))
            .await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_git_log() {
        let tool = GitTool;
        let result = tool
            .execute(
                json!({ "action": "log", "n": 3 }),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        // Should contain commit hashes (7 chars) and messages
        assert!(!result.content.is_empty());
    }

    #[tokio::test]
    async fn test_git_branch_list_implicit() {
        let tool = GitTool;
        let result = tool
            .execute(json!({ "action": "branch" }), ToolContext::new(".", "test"))
            .await;
        assert!(!result.success);
        assert!(result
            .data
            .as_ref()
            .and_then(|data| data.get("recovery"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .contains("branch"));
    }

    #[test]
    fn test_is_valid_git_range() {
        assert!(is_valid_git_range("HEAD~1..HEAD"));
        assert!(is_valid_git_range("main...feature"));
        assert!(is_valid_git_range("abc1234"));
        assert!(!is_valid_git_range("--help"));
        assert!(!is_valid_git_range("HEAD;rm -rf /"));
        assert!(!is_valid_git_range("`whoami`"));
    }

    #[test]
    fn test_safe_force_push() {
        assert!(!is_safe_force_push("origin"));
        assert!(!is_safe_force_push("upstream"));
        assert!(is_safe_force_push("myfork"));
        assert!(is_safe_force_push("fork"));
    }

    #[test]
    fn test_requires_confirmation() {
        let tool = GitTool;
        assert!(!tool.requires_confirmation(&json!({ "action": "status" })));
        assert!(!tool.requires_confirmation(&json!({ "action": "diff" })));
        assert!(!tool.requires_confirmation(&json!({ "action": "log" })));
        assert!(!tool.requires_confirmation(&json!({ "action": "show" })));
        assert!(tool.requires_confirmation(&json!({ "action": "add" })));
        assert!(tool.requires_confirmation(&json!({ "action": "commit" })));
        assert!(tool.requires_confirmation(&json!({ "action": "push" })));
        assert!(tool.requires_confirmation(&json!({ "action": "checkout" })));
        assert!(tool.requires_confirmation(&json!({ "action": "branch" })));
    }

    #[test]
    fn test_to_classifier_input() {
        let tool = GitTool;
        assert_eq!(
            tool.to_classifier_input(&json!({ "action": "commit", "commit": "fix bug" })),
            "git commit: 'fix bug'"
        );
        assert_eq!(
            tool.to_classifier_input(
                &json!({ "action": "push", "remote": "origin", "force": true })
            ),
            "git push: remote=origin force=true"
        );
        assert_eq!(
            tool.to_classifier_input(&json!({ "action": "add", "path": "src/main.rs" })),
            "git add: src/main.rs"
        );
    }

    #[test]
    fn test_git_success_summaries_cover_diff_log_and_commit() {
        assert_eq!(git_success_summary("diff", ""), "git diff: no changes");
        assert_eq!(
            git_success_summary("diff", "diff --git a/a b/a\n+new\n"),
            "git diff: changes displayed"
        );
        assert_eq!(
            git_success_summary("log", "abc123 first\nabc124 second\n"),
            "git log: 2 commits"
        );
        assert_eq!(
            git_success_summary("commit", "[main abc123] fix bug\n 1 file changed"),
            "[main abc123] fix bug"
        );
    }

    #[tokio::test]
    async fn test_git_status_uses_context_working_dir_and_summary() {
        let dir = init_git_repo();
        std::fs::write(dir.path().join("only-here.txt"), "hello\n").unwrap();
        let tool = GitTool;

        let result = tool
            .execute(
                json!({ "action": "status" }),
                ToolContext::new(dir.path(), "test"),
            )
            .await;

        assert!(result.success);
        assert!(result.content.contains("?? only-here.txt"));
        assert_eq!(
            result
                .data
                .as_ref()
                .and_then(|data| data.get("summary"))
                .and_then(|value| value.as_str()),
            Some("git status: 1 changed paths")
        );
    }

    #[tokio::test]
    async fn test_git_add_uses_context_working_dir_and_summary() {
        let dir = init_git_repo();
        std::fs::write(dir.path().join("staged.txt"), "hello\n").unwrap();
        let tool = GitTool;

        let result = tool
            .execute(
                json!({ "action": "add", "path": "staged.txt" }),
                ToolContext::new(dir.path(), "test"),
            )
            .await;

        assert!(result.success);
        assert_eq!(
            result
                .data
                .as_ref()
                .and_then(|data| data.get("summary"))
                .and_then(|value| value.as_str()),
            Some("git add: staged requested paths")
        );
        let status = StdCommand::new("git")
            .args(["status", "--short"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let status_text = String::from_utf8_lossy(&status.stdout);
        assert!(status_text.contains("A  staged.txt"));
    }

    #[tokio::test]
    async fn test_git_invalid_diff_range_returns_recovery_metadata() {
        let tool = GitTool;
        let result = tool
            .execute(
                json!({ "action": "diff", "range": "--help" }),
                ToolContext::new(".", "test"),
            )
            .await;

        assert!(!result.success);
        assert!(result
            .error
            .unwrap_or_default()
            .contains("Invalid git diff range"));
        assert!(result
            .data
            .as_ref()
            .and_then(|data| data.get("recovery"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .contains("HEAD~1..HEAD"));
    }

    #[tokio::test]
    async fn test_git_commit_empty_message_returns_recovery_metadata() {
        let tool = GitTool;
        let result = tool
            .execute(
                json!({ "action": "commit", "commit": "" }),
                ToolContext::new(".", "test"),
            )
            .await;

        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some("Commit message cannot be empty")
        );
        assert!(result
            .data
            .as_ref()
            .and_then(|data| data.get("recovery"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .contains("non-empty"));
    }
}

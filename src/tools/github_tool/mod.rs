//! GitHub 工具 - 获取仓库信息、创建 PR 等
//!
//! 依赖 `gh` CLI 工具，会自动检测其可用性。

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

/// GitHub 工具
pub struct GitHubTool;

#[async_trait]
impl Tool for GitHubTool {
    fn name(&self) -> &str {
        "github"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Network
    }

    fn description(&self) -> &str {
        "GitHub operations using the gh CLI. \
         Read: 'collect' (issues/PRs/CI), 'pr_list', 'issue_list'. \
         Write: 'pr_create' (requires confirmation)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["collect", "pr_list", "issue_list", "pr_create"],
                    "default": "collect",
                    "description": "The GitHub action to perform"
                },
                "title": {
                    "type": "string",
                    "description": "PR title (for pr_create)"
                },
                "body": {
                    "type": "string",
                    "description": "PR body/description (for pr_create)"
                },
                "base": {
                    "type": "string",
                    "default": "main",
                    "description": "Base branch for PR (for pr_create)"
                },
                "draft": {
                    "type": "boolean",
                    "default": false,
                    "description": "Create as draft PR (for pr_create)"
                }
            },
            "required": []
        })
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        matches!(params["action"].as_str(), Some("pr_create"))
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        if params["action"].as_str() == Some("pr_create") {
            let title = params["title"].as_str().unwrap_or("(no title)");
            Some(format!("Create Pull Request: '{}'", title))
        } else {
            None
        }
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let action = params["action"].as_str().unwrap_or("collect");
        match action {
            "pr_create" => {
                let title = params["title"].as_str().unwrap_or("");
                format!("github pr_create: '{}'", title)
            }
            _ => format!("github {}", action),
        }
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("collect");

        match action {
            "collect" => match crate::github::GitHubData::collect().await {
                Ok(data) => {
                    let formatted = data.format_for_llm();
                    if formatted.trim().is_empty() {
                        ToolResult::success(
                                "No GitHub data available. Make sure you are in a Git repository with a GitHub remote and that the `gh` CLI is installed and authenticated."
                            )
                    } else {
                        ToolResult::success(formatted)
                    }
                }
                Err(e) => ToolResult::error(format!("Failed to collect GitHub data: {}", e)),
            },
            "pr_list" => {
                let out = Command::new("gh")
                    .args([
                        "pr",
                        "list",
                        "--limit",
                        "20",
                        "--json",
                        "number,title,author,state,url",
                    ])
                    .output()
                    .await;
                match out {
                    Ok(o) if o.status.success() => {
                        ToolResult::success(String::from_utf8_lossy(&o.stdout).to_string())
                    }
                    Ok(o) => ToolResult::error(format!(
                        "gh pr list failed: {}",
                        String::from_utf8_lossy(&o.stderr)
                    )),
                    Err(e) => ToolResult::error(format!("Failed to run gh: {}", e)),
                }
            }
            "issue_list" => {
                let out = Command::new("gh")
                    .args([
                        "issue",
                        "list",
                        "--limit",
                        "20",
                        "--json",
                        "number,title,author,state,url",
                    ])
                    .output()
                    .await;
                match out {
                    Ok(o) if o.status.success() => {
                        ToolResult::success(String::from_utf8_lossy(&o.stdout).to_string())
                    }
                    Ok(o) => ToolResult::error(format!(
                        "gh issue list failed: {}",
                        String::from_utf8_lossy(&o.stderr)
                    )),
                    Err(e) => ToolResult::error(format!("Failed to run gh: {}", e)),
                }
            }
            "pr_create" => {
                let title = params["title"].as_str().unwrap_or("");
                if title.is_empty() {
                    return ToolResult::error("PR title cannot be empty");
                }
                let body = params["body"].as_str().unwrap_or("");
                let base = params["base"].as_str().unwrap_or("main");
                let draft = params["draft"].as_bool().unwrap_or(false);

                // 获取当前分支名
                let branch_out = Command::new("git")
                    .args(["branch", "--show-current"])
                    .output()
                    .await;
                let current_branch = match branch_out {
                    Ok(o) if o.status.success() => {
                        String::from_utf8_lossy(&o.stdout).trim().to_string()
                    }
                    _ => {
                        return ToolResult::error("Failed to determine current branch".to_string());
                    }
                };

                if current_branch.is_empty() || current_branch == "HEAD" {
                    return ToolResult::error(
                        "Cannot create PR from detached HEAD. Please checkout a branch first."
                            .to_string(),
                    );
                }

                let mut cmd = Command::new("gh");
                cmd.args([
                    "pr",
                    "create",
                    "--title",
                    title,
                    "--base",
                    base,
                    "--head",
                    &current_branch,
                ]);
                if draft {
                    cmd.arg("--draft");
                }
                if !body.is_empty() {
                    cmd.arg("--body").arg(body);
                } else {
                    cmd.arg("--fill");
                }

                match cmd.output().await {
                    Ok(o) if o.status.success() => {
                        let url = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        ToolResult::success_with_data(
                            format!("Pull request created successfully!\n{}", url),
                            json!({ "pr_url": url, "branch": current_branch, "base": base }),
                        )
                    }
                    Ok(o) => ToolResult::error(format!(
                        "gh pr create failed: {}",
                        String::from_utf8_lossy(&o.stderr)
                    )),
                    Err(e) => ToolResult::error(format!("Failed to run gh: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown github action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_github_tool_collect() {
        let tool = GitHubTool;
        let result = tool
            .execute(json!({"action": "collect"}), ToolContext::new(".", "test"))
            .await;
        // 结果可能成功也可能提示 gh 未安装
        assert!(
            result.success,
            "GitHubTool should succeed (even if gh is not available it returns a friendly message)"
        );
    }

    #[test]
    fn test_requires_confirmation() {
        let tool = GitHubTool;
        assert!(!tool.requires_confirmation(&json!({"action": "collect"})));
        assert!(!tool.requires_confirmation(&json!({"action": "pr_list"})));
        assert!(!tool.requires_confirmation(&json!({"action": "issue_list"})));
        assert!(tool.requires_confirmation(&json!({"action": "pr_create"})));
    }

    #[test]
    fn test_to_classifier_input() {
        let tool = GitHubTool;
        assert_eq!(
            tool.to_classifier_input(&json!({"action": "pr_create", "title": "Fix bug"})),
            "github pr_create: 'Fix bug'"
        );
        assert_eq!(
            tool.to_classifier_input(&json!({"action": "collect"})),
            "github collect"
        );
    }
}

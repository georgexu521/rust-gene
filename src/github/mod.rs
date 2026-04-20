//! GitHub 集成
//!
//! 通过 gh CLI 获取 issues、PRs 等数据，作为 AI 权重分析的输入

use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// GitHub Issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhIssue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub body: Option<String>,
}

/// GitHub Pull Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhPullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub head_ref: String,
    pub base_ref: String,
    pub labels: Vec<String>,
    pub review_decision: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// GitHub CI 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhCiStatus {
    pub name: String,
    pub conclusion: Option<String>,
    pub status: String,
}

/// GitHub 项目数据
#[derive(Debug, Clone, Default)]
pub struct GitHubData {
    pub open_issues: Vec<GhIssue>,
    pub open_prs: Vec<GhPullRequest>,
    pub ci_status: Vec<GhCiStatus>,
    pub repo_info: Option<String>,
}

impl GitHubData {
    /// 从当前目录收集 GitHub 数据
    pub async fn collect() -> anyhow::Result<Self> {
        let mut data = Self::default();

        // 检查 gh 是否可用
        if !check_gh_available().await {
            warn!("gh CLI not found, skipping GitHub integration");
            return Ok(data);
        }

        // 获取 repo 信息
        data.repo_info = get_repo_info().await.ok();

        // 获取 open issues
        match get_open_issues().await {
            Ok(issues) => {
                info!("Fetched {} open issues", issues.len());
                data.open_issues = issues;
            }
            Err(e) => warn!("Failed to fetch issues: {}", e),
        }

        // 获取 open PRs
        match get_open_prs().await {
            Ok(prs) => {
                info!("Fetched {} open PRs", prs.len());
                data.open_prs = prs;
            }
            Err(e) => warn!("Failed to fetch PRs: {}", e),
        }

        // 获取 CI 状态
        match get_ci_status().await {
            Ok(status) => {
                data.ci_status = status;
            }
            Err(e) => debug!("Failed to fetch CI status: {}", e),
        }

        Ok(data)
    }

    /// 格式化为 LLM 可读的文本
    pub fn format_for_llm(&self) -> String {
        let mut output = String::new();

        if let Some(ref repo) = self.repo_info {
            output.push_str(&format!("## GitHub 仓库: {}\n\n", repo));
        }

        // Issues
        if !self.open_issues.is_empty() {
            output.push_str(&format!("## Open Issues ({})\n", self.open_issues.len()));
            for issue in &self.open_issues {
                let labels = if issue.labels.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", issue.labels.join(", "))
                };
                let assignee = if issue.assignees.is_empty() {
                    "unassigned".to_string()
                } else {
                    issue.assignees.join(", ")
                };
                output.push_str(&format!(
                    "- #{} {}{} (assigned: {}, updated: {})\n",
                    issue.number,
                    issue.title,
                    labels,
                    assignee,
                    format_date(&issue.updated_at)
                ));
                // 取 body 前 200 字符
                if let Some(ref body) = issue.body {
                    let preview: String = body.chars().take(200).collect();
                    if !preview.trim().is_empty() {
                        output.push_str(&format!("  {}\n", preview.trim()));
                    }
                }
            }
            output.push('\n');
        }

        // PRs
        if !self.open_prs.is_empty() {
            output.push_str(&format!(
                "## Open Pull Requests ({})\n",
                self.open_prs.len()
            ));
            for pr in &self.open_prs {
                let review = pr.review_decision.as_deref().unwrap_or("pending");
                let labels = if pr.labels.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", pr.labels.join(", "))
                };
                output.push_str(&format!(
                    "- #{} {}{} ({} → {}, review: {}, updated: {})\n",
                    pr.number,
                    pr.title,
                    labels,
                    pr.head_ref,
                    pr.base_ref,
                    review,
                    format_date(&pr.updated_at)
                ));
            }
            output.push('\n');
        }

        // CI Status
        if !self.ci_status.is_empty() {
            output.push_str("## CI Status\n");
            for ci in &self.ci_status {
                let icon = match ci.conclusion.as_deref() {
                    Some("success") => "✅",
                    Some("failure") => "❌",
                    Some("cancelled") => "⏹️",
                    _ => "🔄",
                };
                output.push_str(&format!("- {} {} ({})\n", icon, ci.name, ci.status));
            }
            output.push('\n');
        }

        output
    }
}

/// 检查 gh CLI 是否可用
async fn check_gh_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 获取 repo 信息
async fn get_repo_info() -> anyhow::Result<String> {
    let output = Command::new("gh")
        .args([
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "-q",
            ".nameWithOwner",
        ])
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 获取 open issues
async fn get_open_issues() -> anyhow::Result<Vec<GhIssue>> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            "30",
            "--json",
            "number,title,state,labels,assignees,createdAt,updatedAt,body",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("gh issue list failed: {}", stderr));
    }

    let raw: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let issues = raw
        .into_iter()
        .map(|v| GhIssue {
            number: v["number"].as_u64().unwrap_or(0),
            title: v["title"].as_str().unwrap_or("").to_string(),
            state: v["state"].as_str().unwrap_or("OPEN").to_string(),
            labels: v["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            assignees: v["assignees"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a["login"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            created_at: v["createdAt"].as_str().unwrap_or("").to_string(),
            updated_at: v["updatedAt"].as_str().unwrap_or("").to_string(),
            body: v["body"].as_str().map(String::from),
        })
        .collect();

    Ok(issues)
}

/// 获取 open PRs
async fn get_open_prs() -> anyhow::Result<Vec<GhPullRequest>> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "20",
            "--json",
            "number,title,state,headRefName,baseRefName,labels,reviewDecision,createdAt,updatedAt",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("gh pr list failed: {}", stderr));
    }

    let raw: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let prs = raw
        .into_iter()
        .map(|v| GhPullRequest {
            number: v["number"].as_u64().unwrap_or(0),
            title: v["title"].as_str().unwrap_or("").to_string(),
            state: v["state"].as_str().unwrap_or("OPEN").to_string(),
            head_ref: v["headRefName"].as_str().unwrap_or("").to_string(),
            base_ref: v["baseRefName"].as_str().unwrap_or("").to_string(),
            labels: v["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            review_decision: v["reviewDecision"].as_str().map(String::from),
            created_at: v["createdAt"].as_str().unwrap_or("").to_string(),
            updated_at: v["updatedAt"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    Ok(prs)
}

/// 获取 CI 状态（最近一次 commit 的 checks）
async fn get_ci_status() -> anyhow::Result<Vec<GhCiStatus>> {
    let output = Command::new("gh")
        .args([
            "run",
            "list",
            "--limit",
            "5",
            "--json",
            "name,conclusion,status",
        ])
        .output()
        .await?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let raw: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let status = raw
        .into_iter()
        .map(|v| GhCiStatus {
            name: v["name"].as_str().unwrap_or("").to_string(),
            conclusion: v["conclusion"].as_str().map(String::from),
            status: v["status"].as_str().unwrap_or("unknown").to_string(),
        })
        .collect();

    Ok(status)
}

/// 格式化 ISO 日期为相对时间
fn format_date(iso: &str) -> String {
    // 简单实现：只显示日期部分
    if iso.len() >= 10 {
        iso[..10].to_string()
    } else {
        iso.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_data_format() {
        let mut data = GitHubData::default();
        data.open_issues.push(GhIssue {
            number: 1,
            title: "Fix login bug".to_string(),
            state: "OPEN".to_string(),
            labels: vec!["bug".to_string(), "P0".to_string()],
            assignees: vec!["alice".to_string()],
            created_at: "2026-04-01T00:00:00Z".to_string(),
            updated_at: "2026-04-10T00:00:00Z".to_string(),
            body: Some("Login fails when password has special chars".to_string()),
        });
        data.open_prs.push(GhPullRequest {
            number: 2,
            title: "Add auth middleware".to_string(),
            state: "OPEN".to_string(),
            head_ref: "feature/auth".to_string(),
            base_ref: "main".to_string(),
            labels: vec!["feature".to_string()],
            review_decision: Some("APPROVED".to_string()),
            created_at: "2026-04-05T00:00:00Z".to_string(),
            updated_at: "2026-04-09T00:00:00Z".to_string(),
        });

        let formatted = data.format_for_llm();
        assert!(formatted.contains("Open Issues"));
        assert!(formatted.contains("#1 Fix login bug"));
        assert!(formatted.contains("Open Pull Requests"));
        assert!(formatted.contains("#2 Add auth middleware"));
    }
}

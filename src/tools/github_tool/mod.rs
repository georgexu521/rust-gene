//! GitHub 工具 - 获取当前仓库的 Issues、PRs 和 CI 状态
//!
//! 依赖 `gh` CLI 工具，会自动检测其可用性。

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// GitHub 信息收集工具
pub struct GitHubTool;

#[async_trait]
impl Tool for GitHubTool {
    fn name(&self) -> &str {
        "github"
    }

    fn description(&self) -> &str {
        "Fetch open issues, open PRs, and CI status for the current GitHub repository using the gh CLI."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _params: serde_json::Value, _context: ToolContext) -> ToolResult {
        match crate::github::GitHubData::collect().await {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_github_tool_basic() {
        let tool = GitHubTool;
        let result = tool.execute(json!({}), ToolContext::new(".", "test")).await;
        // 结果可能成功也可能提示 gh 未安装
        assert!(
            result.success,
            "GitHubTool should succeed (even if gh is not available it returns a friendly message)"
        );
    }
}

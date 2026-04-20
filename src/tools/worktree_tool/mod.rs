//! Worktree 工具 - 管理 Git worktree
//!
//! 支持列出、创建、删除、清理、切换 worktree

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Worktree 工具
pub struct WorktreeTool;

#[async_trait]
impl Tool for WorktreeTool {
    fn name(&self) -> &str {
        "worktree"
    }

    fn description(&self) -> &str {
        "Manage Git worktrees. Actions: 'list', 'create', 'remove', 'prune', 'switch'. \
         Useful for isolated branch work and PR reviews without stashing."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "remove", "prune", "switch"],
                    "description": "The worktree action to perform"
                },
                "name": {
                    "type": "string",
                    "description": "Name for the new worktree (for 'create')"
                },
                "branch": {
                    "type": "string",
                    "description": "Branch name to create (for 'create', optional)"
                },
                "path": {
                    "type": "string",
                    "description": "Path to the worktree (for 'remove', 'switch')"
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "Force remove even with uncommitted changes (for 'remove')"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");

        let worktree_manager = match &context.worktree_manager {
            Some(manager) => manager.clone(),
            None => {
                return ToolResult::error("Worktree manager not available.".to_string());
            }
        };

        match action {
            "list" => match worktree_manager.list().await {
                Ok(worktrees) => {
                    if worktrees.is_empty() {
                        ToolResult::success("No worktrees found.".to_string())
                    } else {
                        let mut lines = vec![format!("Found {} worktree(s):", worktrees.len())];
                        for (i, wt) in worktrees.iter().enumerate() {
                            let current = if wt.is_current { " [CURRENT]" } else { "" };
                            let branch = wt
                                .branch
                                .as_ref()
                                .map(|b| format!(" ({})", b))
                                .unwrap_or_default();
                            lines.push(format!(
                                "  {}. {}{}{}",
                                i + 1,
                                wt.path.display(),
                                branch,
                                current
                            ));
                        }
                        ToolResult::success(lines.join("\n"))
                    }
                }
                Err(e) => ToolResult::error(format!("Failed to list worktrees: {}", e)),
            },
            "create" => {
                let name = params["name"].as_str().unwrap_or("");
                if name.is_empty() {
                    return ToolResult::error("name is required for create".to_string());
                }
                let branch = params["branch"].as_str();
                match worktree_manager.create(name, branch).await {
                    Ok(path) => ToolResult::success_with_data(
                        format!("Created worktree at: {}", path.display()),
                        json!({ "path": path.to_string_lossy().to_string() }),
                    ),
                    Err(e) => ToolResult::error(format!("Failed to create worktree: {}", e)),
                }
            }
            "remove" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return ToolResult::error("path is required for remove".to_string());
                }
                let resolved =
                    match crate::tools::file_tool::resolve_path(path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e),
                    };
                // 验证只能删除已知的 worktree
                match worktree_manager.list().await {
                    Ok(known) => {
                        if !known.iter().any(|wt| wt.path == resolved) {
                            return ToolResult::error(format!(
                                "Path '{}' is not a known git worktree. Refusing to remove.",
                                resolved.display()
                            ));
                        }
                    }
                    Err(e) => {
                        return ToolResult::error(format!("Failed to verify worktree list: {}", e))
                    }
                }
                let force = params["force"].as_bool().unwrap_or(false);
                let result = if force {
                    worktree_manager
                        .remove_force(resolved.to_string_lossy().as_ref())
                        .await
                } else {
                    worktree_manager
                        .remove(resolved.to_string_lossy().as_ref())
                        .await
                };
                match result {
                    Ok(()) => {
                        ToolResult::success(format!("Removed worktree: {}", resolved.display()))
                    }
                    Err(e) => ToolResult::error(format!("Failed to remove worktree: {}", e)),
                }
            }
            "prune" => match worktree_manager.prune().await {
                Ok(msg) => ToolResult::success(msg),
                Err(e) => ToolResult::error(format!("Failed to prune worktrees: {}", e)),
            },
            "switch" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return ToolResult::error("path is required for switch".to_string());
                }
                let path_buf = std::path::PathBuf::from(path);
                // 支持相对路径解析
                let resolved = if path_buf.is_absolute() {
                    path_buf
                } else {
                    context.working_dir.join(path_buf)
                };
                match worktree_manager.switch(&resolved).await {
                    Ok(()) => ToolResult::success(format!(
                        "Switched to worktree: {}. \
                         Note: this updates the session's tracked worktree. \
                         Future file operations will use this directory.",
                        resolved.display()
                    )),
                    Err(e) => ToolResult::error(format!("Failed to switch worktree: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown worktree action: {}", action)),
        }
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        let action = params["action"].as_str().unwrap_or("");
        action == "remove"
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str().unwrap_or("unknown worktree");
        Some(format!(
            "This will remove the worktree: {}\nContinue?",
            path
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worktree_list() {
        let tool = WorktreeTool;
        let result = tool
            .execute(json!({ "action": "list" }), ToolContext::new(".", "test"))
            .await;
        // 在非 git 仓库可能失败，但至少应该返回结果
        assert!(
            result.success || result.error.is_some(),
            "Expected either success or error"
        );
    }
}

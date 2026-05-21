//! Worktree 工具 - 管理 Git worktree
//!
//! 支持列出、创建、删除、清理、切换 worktree

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Worktree 工具
pub struct WorktreeTool;

#[derive(Debug, Clone)]
struct AgentWorktreeRef {
    agent_id: String,
    task_id: String,
    status: String,
    description: String,
    path: PathBuf,
    branch: Option<String>,
}

fn short_branch_name(branch: &str) -> &str {
    branch.strip_prefix("refs/heads/").unwrap_or(branch)
}

fn is_safe_agent_branch(branch: &str) -> bool {
    short_branch_name(branch).starts_with("codex/agent-")
}

fn status_is_dirty(status: &str) -> bool {
    status.lines().any(|line| !line.trim().is_empty())
}

fn untracked_paths(status: &str) -> Vec<String> {
    status
        .lines()
        .filter_map(|line| line.strip_prefix("?? ").map(|path| path.to_string()))
        .collect()
}

fn task_is_isolated_agent(state: &crate::session_store::AgentTaskStateRecord) -> bool {
    state
        .cleanup_hooks
        .iter()
        .any(|hook| hook == "worktree_cleanup")
        || state.payload.get("isolated_worktree").is_some()
}

fn extract_agent_worktree(
    state: &crate::session_store::AgentTaskStateRecord,
) -> Result<AgentWorktreeRef, String> {
    if !task_is_isolated_agent(state) {
        return Err(format!(
            "Agent task {} is not an isolated worktree task",
            state.agent_id
        ));
    }
    let isolated = state.payload.get("isolated_worktree").ok_or_else(|| {
        format!(
            "Agent task {} has no isolated_worktree payload",
            state.agent_id
        )
    })?;
    let path = isolated
        .get("path")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "Agent task {} isolated_worktree payload has no path",
                state.agent_id
            )
        })?;
    let branch = isolated
        .get("branch")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(short_branch_name)
        .map(str::to_string);
    Ok(AgentWorktreeRef {
        agent_id: state.agent_id.clone(),
        task_id: state.task_id.clone(),
        status: state.status.clone(),
        description: state.description.clone(),
        path: PathBuf::from(path),
        branch,
    })
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

async fn resolve_agent_worktree(
    context: &ToolContext,
    agent_id: &str,
) -> Result<AgentWorktreeRef, String> {
    let store = context.session_store.as_ref().ok_or_else(|| {
        "Session store not available. Agent worktree commands require durable task state."
            .to_string()
    })?;
    let state = store
        .agent_task_state(&context.session_id, agent_id)
        .map_err(|err| format!("Failed to read agent task state: {}", err))?
        .ok_or_else(|| {
            format!(
                "Agent task '{}' was not found in current session {}",
                agent_id, context.session_id
            )
        })?;
    extract_agent_worktree(&state)
}

async fn verify_known_worktree(
    manager: &crate::engine::worktree::WorktreeManager,
    path: &Path,
) -> Result<(), String> {
    let known = manager
        .list()
        .await
        .map_err(|err| format!("Failed to list git worktrees: {}", err))?;
    if known.iter().any(|wt| same_path(&wt.path, path)) {
        Ok(())
    } else {
        Err(format!(
            "Path '{}' is not a known git worktree",
            path.display()
        ))
    }
}

async fn run_git(cwd: &Path, args: Vec<String>) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(&args)
        .output()
        .await
        .map_err(|err| format!("Failed to run git {}: {}", args.join(" "), err))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(if stdout.is_empty() && !stderr.is_empty() {
            stderr.to_string()
        } else {
            stdout.to_string()
        })
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

async fn run_git_with_stdin(cwd: &Path, args: Vec<String>, input: &str) -> Result<String, String> {
    let mut child = Command::new("git")
        .current_dir(cwd)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to run git {}: {}", args.join(" "), err))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|err| format!("Failed to write git {} input: {}", args.join(" "), err))?;
    }
    let output = child
        .wait_with_output()
        .await
        .map_err(|err| format!("Failed to wait for git {}: {}", args.join(" "), err))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(if stdout.is_empty() && !stderr.is_empty() {
            stderr.to_string()
        } else {
            stdout.to_string()
        })
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

async fn commits_ahead(target_dir: &Path, branch: Option<&str>) -> Result<usize, String> {
    let Some(branch) = branch else {
        return Ok(0);
    };
    let output = run_git(
        target_dir,
        vec![
            "rev-list".to_string(),
            "--count".to_string(),
            format!("HEAD..{}", branch),
        ],
    )
    .await?;
    Ok(output.trim().parse::<usize>().unwrap_or(0))
}

fn required_agent_id(params: &serde_json::Value) -> Result<&str, ToolResult> {
    params["agent_id"]
        .as_str()
        .or_else(|| params["task_id"].as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolResult::error("agent_id is required for agent worktree actions"))
}

async fn handle_agent_review(
    params: &serde_json::Value,
    context: &ToolContext,
    manager: &crate::engine::worktree::WorktreeManager,
) -> ToolResult {
    let agent_id = match required_agent_id(params) {
        Ok(agent_id) => agent_id,
        Err(result) => return result,
    };
    let agent = match resolve_agent_worktree(context, agent_id).await {
        Ok(agent) => agent,
        Err(err) => return ToolResult::error(err),
    };
    if let Err(err) = verify_known_worktree(manager, &agent.path).await {
        return ToolResult::error(err);
    }

    let target_dir = manager.original_dir();
    let status = run_git(&agent.path, vec!["status".into(), "--short".into()])
        .await
        .unwrap_or_else(|err| format!("(failed to read status: {})", err));
    let diff_stat = run_git(
        &agent.path,
        vec!["diff".into(), "--stat".into(), "HEAD".into()],
    )
    .await
    .unwrap_or_else(|err| format!("(failed to read diff stat: {})", err));
    let changed_paths = run_git(
        &agent.path,
        vec!["diff".into(), "--name-status".into(), "HEAD".into()],
    )
    .await
    .unwrap_or_else(|err| format!("(failed to read changed paths: {})", err));
    let ahead = commits_ahead(target_dir, agent.branch.as_deref())
        .await
        .unwrap_or(0);
    let committed_stat = if ahead > 0 {
        run_git(
            target_dir,
            vec![
                "diff".into(),
                "--stat".into(),
                format!("HEAD...{}", agent.branch.as_deref().unwrap_or("HEAD")),
            ],
        )
        .await
        .unwrap_or_else(|err| format!("(failed to read committed diff stat: {})", err))
    } else {
        "No committed branch changes ahead of target HEAD.".to_string()
    };

    let status_block = if status.trim().is_empty() {
        "clean".to_string()
    } else {
        status.trim_end().to_string()
    };
    let diff_block = if diff_stat.trim().is_empty() {
        "No uncommitted tracked diff.".to_string()
    } else {
        diff_stat.trim_end().to_string()
    };
    let path_block = if changed_paths.trim().is_empty() {
        "No uncommitted tracked paths.".to_string()
    } else {
        changed_paths.trim_end().to_string()
    };
    let output = format!(
        "Agent worktree review: {}\nStatus: {}\nTask: {}\nPath: {}\nBranch: {}\nCommits ahead of target HEAD: {}\n\nWorktree status:\n{}\n\nUncommitted diff stat:\n{}\n\nUncommitted changed paths:\n{}\n\nCommitted branch diff stat:\n{}",
        agent.agent_id,
        agent.status,
        agent.description,
        agent.path.display(),
        agent.branch.as_deref().unwrap_or("unknown"),
        ahead,
        status_block,
        diff_block,
        path_block,
        committed_stat.trim_end()
    );
    ToolResult::success_with_data(
        output,
        json!({
            "action": "agent_review",
            "agent_id": agent.agent_id,
            "task_id": agent.task_id,
            "status": agent.status,
            "description": agent.description,
            "path": agent.path.to_string_lossy().to_string(),
            "branch": agent.branch,
            "dirty": status_is_dirty(&status),
            "untracked_paths": untracked_paths(&status),
            "commits_ahead": ahead,
        }),
    )
}

async fn cleanup_agent_worktree(
    manager: &crate::engine::worktree::WorktreeManager,
    agent: &AgentWorktreeRef,
    force: bool,
    delete_branch: bool,
) -> Result<String, String> {
    verify_known_worktree(manager, &agent.path).await?;
    let status = run_git(&agent.path, vec!["status".into(), "--short".into()]).await?;
    if status_is_dirty(&status) && !force {
        return Err(format!(
            "Agent worktree has uncommitted changes. Review first or retry cleanup with force=true.\n{}",
            status.trim_end()
        ));
    }
    if force {
        manager
            .remove_force(agent.path.to_string_lossy().as_ref())
            .await
            .map_err(|err| format!("Failed to remove agent worktree: {}", err))?;
    } else {
        manager
            .remove(agent.path.to_string_lossy().as_ref())
            .await
            .map_err(|err| format!("Failed to remove agent worktree: {}", err))?;
    }

    let mut lines = vec![format!("Removed agent worktree: {}", agent.path.display())];
    if delete_branch {
        match agent.branch.as_deref() {
            Some(branch) if is_safe_agent_branch(branch) => match run_git(
                manager.original_dir(),
                vec!["branch".into(), "-D".into(), branch.to_string()],
            )
            .await
            {
                Ok(output) if output.trim().is_empty() => {
                    lines.push(format!("Deleted branch: {}", branch));
                }
                Ok(output) => lines.push(output.trim_end().to_string()),
                Err(err) => lines.push(format!("Branch deletion failed: {}", err)),
            },
            Some(branch) => lines.push(format!(
                "Skipped branch deletion because '{}' is not an agent branch",
                branch
            )),
            None => lines.push("Skipped branch deletion because branch is unknown".to_string()),
        }
    }
    Ok(lines.join("\n"))
}

async fn handle_agent_cleanup(
    params: &serde_json::Value,
    context: &ToolContext,
    manager: &crate::engine::worktree::WorktreeManager,
) -> ToolResult {
    let agent_id = match required_agent_id(params) {
        Ok(agent_id) => agent_id,
        Err(result) => return result,
    };
    let agent = match resolve_agent_worktree(context, agent_id).await {
        Ok(agent) => agent,
        Err(err) => return ToolResult::error(err),
    };
    let force = params["force"].as_bool().unwrap_or(false);
    let delete_branch = params["delete_branch"].as_bool().unwrap_or(false);
    match cleanup_agent_worktree(manager, &agent, force, delete_branch).await {
        Ok(output) => ToolResult::success_with_data(
            output,
            json!({
                "action": "agent_cleanup",
                "agent_id": agent.agent_id,
                "task_id": agent.task_id,
                "path": agent.path.to_string_lossy().to_string(),
                "branch": agent.branch,
                "force": force,
                "delete_branch": delete_branch,
            }),
        ),
        Err(err) => ToolResult::error(err),
    }
}

async fn handle_agent_merge(
    params: &serde_json::Value,
    context: &ToolContext,
    manager: &crate::engine::worktree::WorktreeManager,
) -> ToolResult {
    let agent_id = match required_agent_id(params) {
        Ok(agent_id) => agent_id,
        Err(result) => return result,
    };
    let agent = match resolve_agent_worktree(context, agent_id).await {
        Ok(agent) => agent,
        Err(err) => return ToolResult::error(err),
    };
    if let Err(err) = verify_known_worktree(manager, &agent.path).await {
        return ToolResult::error(err);
    }

    let target_dir = manager.original_dir();
    let allow_dirty_parent = params["allow_dirty_parent"].as_bool().unwrap_or(false);
    let target_status = match run_git(target_dir, vec!["status".into(), "--short".into()]).await {
        Ok(status) => status,
        Err(err) => return ToolResult::error(err),
    };
    if status_is_dirty(&target_status) && !allow_dirty_parent {
        return ToolResult::error(format!(
            "Target worktree is not clean. Commit/stash current changes or retry with allow_dirty_parent=true.\n{}",
            target_status.trim_end()
        ));
    }

    let child_status = match run_git(&agent.path, vec!["status".into(), "--short".into()]).await {
        Ok(status) => status,
        Err(err) => return ToolResult::error(err),
    };
    let untracked = untracked_paths(&child_status);
    if !untracked.is_empty() {
        return ToolResult::error(format!(
            "Agent worktree has untracked files that cannot be safely merged automatically: {}. Commit them in the worktree or copy them intentionally before retrying.",
            untracked.join(", ")
        ));
    }

    let ahead = match commits_ahead(target_dir, agent.branch.as_deref()).await {
        Ok(count) => count,
        Err(err) => return ToolResult::error(err),
    };
    let child_dirty = status_is_dirty(&child_status);
    if ahead > 0 && child_dirty {
        return ToolResult::error(
            "Agent worktree has both committed branch changes and uncommitted changes. Commit or discard the uncommitted changes before agent_merge."
        );
    }

    let mut lines = vec![format!(
        "Merging agent worktree {} into {}",
        agent.agent_id,
        target_dir.display()
    )];
    let mut merge_kind = "none";
    if ahead > 0 {
        let Some(branch) = agent.branch.as_deref() else {
            return ToolResult::error("Agent worktree branch is unknown");
        };
        let output = match run_git(
            target_dir,
            vec![
                "merge".into(),
                "--no-ff".into(),
                "--no-edit".into(),
                branch.to_string(),
            ],
        )
        .await
        {
            Ok(output) => output,
            Err(err) => return ToolResult::error(err),
        };
        merge_kind = "branch";
        lines.push(if output.trim().is_empty() {
            format!("Merged branch: {}", branch)
        } else {
            output.trim_end().to_string()
        });
    } else if child_dirty {
        let diff = match run_git(
            &agent.path,
            vec!["diff".into(), "--binary".into(), "HEAD".into()],
        )
        .await
        {
            Ok(diff) => diff,
            Err(err) => return ToolResult::error(err),
        };
        if diff.trim().is_empty() {
            lines.push("No tracked diff to apply.".to_string());
        } else {
            let output = match run_git_with_stdin(
                target_dir,
                vec![
                    "apply".into(),
                    "--3way".into(),
                    "--whitespace=nowarn".into(),
                ],
                &diff,
            )
            .await
            {
                Ok(output) => output,
                Err(err) => return ToolResult::error(err),
            };
            merge_kind = "tracked_diff";
            lines.push(if output.trim().is_empty() {
                "Applied tracked worktree diff to target worktree.".to_string()
            } else {
                output.trim_end().to_string()
            });
        }
    } else {
        lines.push("No committed or tracked uncommitted changes to merge.".to_string());
    }

    let cleanup = params["cleanup"].as_bool().unwrap_or(false);
    let force = params["force"].as_bool().unwrap_or(false);
    let delete_branch = params["delete_branch"].as_bool().unwrap_or(false);
    if cleanup {
        if merge_kind == "tracked_diff" && !force {
            lines.push(
                "Cleanup skipped: tracked diff was copied and source worktree is still dirty. Retry agent_cleanup with force=true after review."
                    .to_string(),
            );
        } else {
            match cleanup_agent_worktree(manager, &agent, force, delete_branch).await {
                Ok(output) => lines.push(output),
                Err(err) => lines.push(format!("Cleanup failed: {}", err)),
            }
        }
    }

    ToolResult::success_with_data(
        lines.join("\n"),
        json!({
            "action": "agent_merge",
            "agent_id": agent.agent_id,
            "task_id": agent.task_id,
            "path": agent.path.to_string_lossy().to_string(),
            "branch": agent.branch,
            "commits_ahead": ahead,
            "merge_kind": merge_kind,
            "cleanup": cleanup,
            "delete_branch": delete_branch,
        }),
    )
}

#[async_trait]
impl Tool for WorktreeTool {
    fn name(&self) -> &str {
        "worktree"
    }

    fn description(&self) -> &str {
        "Manage Git worktrees. Actions: 'list', 'create', 'remove', 'prune', 'switch', \
         'agent_review', 'agent_merge', 'agent_cleanup'. \
         Useful for isolated branch work, agent worktree review/merge/cleanup, and PR reviews without stashing."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "remove", "prune", "switch", "agent_review", "agent_merge", "agent_cleanup"],
                    "description": "The worktree action to perform"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Agent/task id for agent_review, agent_merge, or agent_cleanup"
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
                    "description": "Force remove even with uncommitted changes (for 'remove' and agent_cleanup)"
                },
                "delete_branch": {
                    "type": "boolean",
                    "default": false,
                    "description": "Delete the safe codex/agent-* branch after agent_cleanup or agent_merge cleanup"
                },
                "cleanup": {
                    "type": "boolean",
                    "default": false,
                    "description": "Cleanup the isolated worktree after a successful agent_merge"
                },
                "allow_dirty_parent": {
                    "type": "boolean",
                    "default": false,
                    "description": "Allow agent_merge into a target worktree that already has local changes"
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
            "agent_review" => handle_agent_review(&params, &context, &worktree_manager).await,
            "agent_merge" => handle_agent_merge(&params, &context, &worktree_manager).await,
            "agent_cleanup" => handle_agent_cleanup(&params, &context, &worktree_manager).await,
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
        matches!(action, "remove" | "agent_merge" | "agent_cleanup")
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        match params["action"].as_str().unwrap_or("") {
            "agent_merge" => {
                let agent_id = params["agent_id"].as_str().unwrap_or("unknown agent");
                Some(format!(
                    "Merge isolated worktree changes from agent {} into the target worktree?",
                    agent_id
                ))
            }
            "agent_cleanup" => {
                let agent_id = params["agent_id"].as_str().unwrap_or("unknown agent");
                Some(format!(
                    "Remove the isolated worktree for agent {}?",
                    agent_id
                ))
            }
            _ => {
                let path = params["path"].as_str().unwrap_or("unknown worktree");
                Some(format!(
                    "This will remove the worktree: {}\nContinue?",
                    path
                ))
            }
        }
    }

    fn is_available(&self, context: &ToolContext) -> bool {
        context.worktree_manager.is_some()
    }

    fn unavailable_reason(&self, _context: &ToolContext) -> Option<String> {
        Some("Worktree manager not configured".to_string())
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

    #[test]
    fn agent_worktree_actions_are_documented_and_confirmed() {
        let tool = WorktreeTool;
        let params = tool.parameters();
        let actions = params["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(actions.contains(&"agent_review"));
        assert!(actions.contains(&"agent_merge"));
        assert!(actions.contains(&"agent_cleanup"));
        assert!(!tool.requires_confirmation(&json!({ "action": "agent_review" })));
        assert!(tool.requires_confirmation(&json!({ "action": "agent_merge" })));
        assert!(tool.requires_confirmation(&json!({ "action": "agent_cleanup" })));
    }

    #[test]
    fn extracts_agent_worktree_from_task_payload() {
        let state = crate::session_store::AgentTaskStateRecord {
            id: 1,
            session_id: "s1".to_string(),
            task_id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            profile: Some("implementer".to_string()),
            role: "specialist".to_string(),
            status: "completed".to_string(),
            description: "edit code".to_string(),
            transcript_path: None,
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: None,
            cleanup_hooks: vec!["worktree_cleanup".to_string()],
            payload: json!({
                "isolated_worktree": {
                    "path": "/tmp/agent-worktree",
                    "branch": "refs/heads/codex/agent-1234"
                }
            }),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };

        let agent = extract_agent_worktree(&state).unwrap();
        assert_eq!(agent.agent_id, "agent_1");
        assert_eq!(agent.branch.as_deref(), Some("codex/agent-1234"));
        assert_eq!(agent.path, PathBuf::from("/tmp/agent-worktree"));
    }
}

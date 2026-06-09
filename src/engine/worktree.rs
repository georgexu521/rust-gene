//! Git Worktree 管理
//!
//! 支持创建、列出、删除、切换 git worktree

use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::info;

/// Worktree 信息
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub is_bare: bool,
    pub is_detached: bool,
    pub is_current: bool,
}

/// Worktree 管理器
pub struct WorktreeManager {
    original_dir: PathBuf,
    current_worktree: RwLock<Option<PathBuf>>,
}

impl WorktreeManager {
    /// 创建新的 Worktree 管理器
    pub async fn new() -> Self {
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let current_worktree = Self::detect_current_worktree().await;
        if let Some(ref wt) = current_worktree {
            info!("Currently in worktree: {}", wt.display());
        }
        Self {
            original_dir,
            current_worktree: RwLock::new(current_worktree),
        }
    }

    pub fn for_root(original_dir: impl Into<PathBuf>) -> Self {
        Self {
            original_dir: original_dir.into(),
            current_worktree: RwLock::new(None),
        }
    }

    /// 检测当前是否在 worktree 中
    async fn detect_current_worktree() -> Option<PathBuf> {
        // 运行 git rev-parse --show-toplevel
        match Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    let path = PathBuf::from(path);
                    // 检查 git-common-dir 是否包含 worktrees 来判断是否是 worktree
                    match Command::new("git")
                        .args(["rev-parse", "--git-common-dir"])
                        .output()
                        .await
                    {
                        Ok(common) if common.status.success() => {
                            let common_dir =
                                String::from_utf8_lossy(&common.stdout).trim().to_string();
                            if common_dir.ends_with("/.git") || common_dir == ".git" {
                                // 在主仓库中
                                None
                            } else {
                                Some(path)
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// 获取当前 worktree 名称（如果有）
    pub async fn active_worktree_name(&self) -> Option<String> {
        self.current_worktree
            .read()
            .await
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    }

    /// 获取原始目录
    pub fn original_dir(&self) -> &Path {
        &self.original_dir
    }

    /// 获取当前 worktree 路径
    pub async fn current_worktree(&self) -> Option<PathBuf> {
        self.current_worktree.read().await.clone()
    }

    /// 非阻塞尝试获取当前 worktree 名称（用于同步渲染）
    pub fn try_active_worktree_name(&self) -> Option<String> {
        if let Ok(lock) = self.current_worktree.try_read() {
            lock.as_ref()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        } else {
            None
        }
    }

    /// 列出所有 worktree
    pub async fn list(&self) -> anyhow::Result<Vec<WorktreeInfo>> {
        let output = Command::new("git")
            .current_dir(&self.original_dir)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "git worktree list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut current: Option<WorktreeInfo> = None;

        for line in text.lines() {
            if line.is_empty() {
                if let Some(wt) = current.take() {
                    worktrees.push(wt);
                }
                continue;
            }

            if current.is_none() {
                current = Some(WorktreeInfo {
                    path: PathBuf::from(line.strip_prefix("worktree ").unwrap_or(line)),
                    commit: None,
                    branch: None,
                    is_bare: false,
                    is_detached: false,
                    is_current: false,
                });
                continue;
            }

            let wt = current
                .as_mut()
                .expect("current worktree must be Some after push");
            if line.starts_with("commit ") {
                wt.commit = Some(line.strip_prefix("commit ").unwrap_or(line).to_string());
            } else if line.starts_with("branch ") {
                wt.branch = Some(line.strip_prefix("branch ").unwrap_or(line).to_string());
            } else if line == "bare" {
                wt.is_bare = true;
            } else if line == "detached" {
                wt.is_detached = true;
            } else if line.starts_with("worktree ") {
                // 新的 worktree 开始
                worktrees.push(
                    current
                        .take()
                        .expect("worktree entry must be Some before new worktree"),
                );
                current = Some(WorktreeInfo {
                    path: PathBuf::from(line.strip_prefix("worktree ").unwrap_or(line)),
                    commit: None,
                    branch: None,
                    is_bare: false,
                    is_detached: false,
                    is_current: false,
                });
            }
        }

        if let Some(wt) = current {
            worktrees.push(wt);
        }

        // 标记当前 worktree
        let current_dir = self.original_dir.clone();
        for wt in &mut worktrees {
            if let Ok(canonical) = current_dir.canonicalize() {
                if let Ok(wt_canonical) = wt.path.canonicalize() {
                    wt.is_current = canonical == wt_canonical;
                }
            }
        }

        Ok(worktrees)
    }

    /// 创建新的 worktree
    pub async fn create(&self, name: &str, branch: Option<&str>) -> anyhow::Result<PathBuf> {
        let safe_name = name.replace(['/', '\\', ':'], "-");
        if safe_name.is_empty() || safe_name == "-" {
            anyhow::bail!("Invalid worktree name: '{}'", name);
        }
        let worktree_dir = self
            .original_dir
            .join(".claude")
            .join("worktrees")
            .join(&safe_name);

        // 确保 .claude/worktrees 目录存在
        if let Some(parent) = worktree_dir.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut cmd = Command::new("git");
        cmd.current_dir(&self.original_dir);
        cmd.args(["worktree", "add"]);
        if let Some(b) = branch {
            cmd.arg("-b").arg(b);
        }
        cmd.arg(&worktree_dir);

        let output = cmd.output().await?;
        if !output.status.success() {
            anyhow::bail!(
                "git worktree add failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        info!("Created worktree at {:?}", worktree_dir);
        Ok(worktree_dir)
    }

    /// 删除 worktree
    pub async fn remove(&self, path: &str) -> anyhow::Result<()> {
        let output = Command::new("git")
            .current_dir(&self.original_dir)
            .args(["worktree", "remove", path])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "git worktree remove failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        info!("Removed worktree {}", path);
        Ok(())
    }

    /// 强制删除 worktree（包括未提交的更改）
    pub async fn remove_force(&self, path: &str) -> anyhow::Result<()> {
        let output = Command::new("git")
            .current_dir(&self.original_dir)
            .args(["worktree", "remove", "--force", path])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "git worktree remove --force failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        info!("Force removed worktree {}", path);
        Ok(())
    }

    /// 清理无效的 worktree 记录
    pub async fn prune(&self) -> anyhow::Result<String> {
        let output = Command::new("git")
            .current_dir(&self.original_dir)
            .args(["worktree", "prune"])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "git worktree prune failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let msg = String::from_utf8_lossy(&output.stdout);
        Ok(if msg.trim().is_empty() {
            "Worktree prune completed.".to_string()
        } else {
            msg.to_string()
        })
    }

    /// 切换到指定 worktree（仅更新跟踪状态，不实际切换进程目录）
    pub async fn switch(&self, path: &Path) -> anyhow::Result<()> {
        if !path.exists() {
            anyhow::bail!("Worktree path does not exist: {}", path.display());
        }
        *self.current_worktree.write().await = Some(path.to_path_buf());
        info!("Switched to worktree {}", path.display());
        Ok(())
    }
}

impl Default for WorktreeManager {
    fn default() -> Self {
        // 同步阻塞构造，仅用于测试和默认值
        let original_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            original_dir,
            current_worktree: RwLock::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worktree_manager_default() {
        let mgr = WorktreeManager::default();
        assert!(mgr.active_worktree_name().await.is_none());
    }

    #[tokio::test]
    async fn test_worktree_list_in_repo() {
        let mgr = WorktreeManager::new().await;
        // 在当前仓库中，list 应该至少返回主 worktree
        let result = mgr.list().await;
        if let Ok(worktrees) = result {
            assert!(
                !worktrees.is_empty(),
                "Expected at least one worktree in a git repo"
            );
        }
    }
}

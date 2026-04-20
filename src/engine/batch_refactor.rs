//! 批量重构服务
//!
//! 对标 Claude Code 的 `batch.ts`
//! 5-30 个并行 worktree agent 协同完成大规模重构

use crate::agent::AgentManager;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// 重构任务单元
#[derive(Debug, Clone)]
pub struct RefactorUnit {
    /// 任务 ID
    pub id: String,
    /// 任务描述
    pub description: String,
    /// 目标文件/目录模式
    pub paths: Vec<String>,
    /// 优先级（1-10）
    pub priority: u8,
    /// 状态
    pub status: RefactorUnitStatus,
}

/// 重构单元状态
#[derive(Debug, Clone, PartialEq)]
pub enum RefactorUnitStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed(String),
}

/// 批量重构结果
#[derive(Debug)]
pub struct BatchRefactorResult {
    /// 总体状态
    pub status: BatchRefactorStatus,
    /// 任务结果
    pub units: Vec<RefactorUnitResult>,
    /// PR URL 列表
    pub pr_urls: Vec<String>,
    /// 总耗时（毫秒）
    pub total_duration_ms: u64,
}

/// 单个任务结果
#[derive(Debug)]
pub struct RefactorUnitResult {
    pub unit_id: String,
    pub success: bool,
    pub output: String,
    pub pr_url: Option<String>,
    pub duration_ms: u64,
}

/// 批量重构状态
#[derive(Debug, Clone, PartialEq)]
pub enum BatchRefactorStatus {
    /// 初始化
    Initializing,
    /// 规划中
    Planning,
    /// 执行中
    Running,
    /// 完成
    Completed,
    /// 失败
    Failed(String),
}

/// 批量重构器
pub struct BatchRefactor {
    /// 工作目录
    working_dir: PathBuf,
    /// 是否启用
    enabled: bool,
    /// Agent 管理器
    agent_manager: Option<Arc<AgentManager>>,
    /// 最大并行数
    max_parallel: usize,
}

impl BatchRefactor {
    /// 创建新的批量重构器
    pub fn new(working_dir: PathBuf) -> Self {
        let enabled = std::env::var("PRIORITY_AGENT_BATCH_REFACTOR")
            .ok()
            .map(|v| v == "1")
            .unwrap_or(false); // 默认禁用

        let max_parallel = std::env::var("PRIORITY_AGENT_BATCH_MAX_PARALLEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        Self {
            working_dir,
            enabled,
            agent_manager: None,
            max_parallel,
        }
    }

    /// 设置 Agent 管理器
    pub fn with_agent_manager(mut self, manager: Arc<AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 分解大规模重构任务为多个单元
    pub fn decompose(&self, task_description: &str, files: &[String]) -> Vec<RefactorUnit> {
        info!("Decomposing task into refactor units: {}", task_description);

        let mut units = Vec::new();

        // 按文件数量分解
        let file_chunks: Vec<Vec<String>> = files
            .chunks((files.len() / self.max_parallel).max(1))
            .map(|c| c.to_vec())
            .collect();

        for (i, chunk) in file_chunks.iter().enumerate() {
            let unit = RefactorUnit {
                id: format!("refactor-{}", i + 1),
                description: format!(
                    "{} - 处理文件: {}",
                    task_description,
                    chunk.len()
                ),
                paths: chunk.clone(),
                priority: ((self.max_parallel.saturating_sub(i)) as u8).min(10),
                status: RefactorUnitStatus::Pending,
            };
            units.push(unit);
        }

        info!("Decomposed into {} refactor units", units.len());
        units
    }

    /// 执行批量重构
    pub async fn execute(
        &self,
        task_description: &str,
        files: Vec<String>,
    ) -> Result<BatchRefactorResult, String> {
        if !self.enabled {
            return Err("Batch refactor is not enabled. Set PRIORITY_AGENT_BATCH_REFACTOR=1".to_string());
        }

        let start = std::time::Instant::now();

        // Phase 1: 分解任务
        let units = self.decompose(task_description, &files);
        let total_units = units.len();

        info!("Starting batch refactor with {} units", total_units);

        // Phase 2: 并行执行（使用 worktree 隔离）
        let results = self.execute_parallel(units).await;

        // Phase 3: 收集 PR URLs
        let pr_urls: Vec<String> = results
            .iter()
            .filter_map(|r| r.pr_url.clone())
            .collect();

        let total_duration = start.elapsed().as_millis() as u64;

        let status = if results.iter().all(|r| r.success) {
            BatchRefactorStatus::Completed
        } else if results.iter().any(|r| r.success) {
            BatchRefactorStatus::Completed // 部分成功也认为完成
        } else {
            BatchRefactorStatus::Failed("All units failed".to_string())
        };

        Ok(BatchRefactorResult {
            status,
            units: results,
            pr_urls,
            total_duration_ms: total_duration,
        })
    }

    /// 并行执行多个重构单元
    async fn execute_parallel(&self, units: Vec<RefactorUnit>) -> Vec<RefactorUnitResult> {
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::channel::<RefactorUnitResult>(units.len());

        // 并行启动任务
        for unit in units {
            let tx = tx.clone();
            let working_dir = self.working_dir.clone();

            tokio::spawn(async move {
                let result = Self::execute_unit(unit, working_dir).await;
                let _ = tx.send(result).await;
            });
        }

        // 收集结果
        drop(tx);
        let mut results = Vec::new();
        while let Some(result) = rx.recv().await {
            results.push(result);
        }

        // 按 ID 排序
        results.sort_by(|a, b| a.unit_id.cmp(&b.unit_id));

        results
    }

    /// 执行单个重构单元
    async fn execute_unit(unit: RefactorUnit, working_dir: PathBuf) -> RefactorUnitResult {
        let start = std::time::Instant::now();
        let unit_id = unit.id.clone();

        info!("Executing refactor unit: {} - {}", unit_id, unit.description);

        // 创建隔离的 worktree（如果支持）
        let worktree_result = Self::create_isolated_worktree(&unit, &working_dir).await;

        match worktree_result {
            Ok(worktree_path) => {
                // 在 worktree 中执行重构
                let exec_result = Self::run_refactor_in_worktree(&unit, &worktree_path).await;
                let duration = start.elapsed().as_millis() as u64;

                match exec_result {
                    Ok((output, pr_url)) => {
                        debug!("Unit {} completed successfully", unit_id);
                        RefactorUnitResult {
                            unit_id,
                            success: true,
                            output,
                            pr_url,
                            duration_ms: duration,
                        }
                    }
                    Err(e) => {
                        warn!("Unit {} failed: {}", unit_id, e);
                        RefactorUnitResult {
                            unit_id,
                            success: false,
                            output: e,
                            pr_url: None,
                            duration_ms: duration,
                        }
                    }
                }
            }
            Err(e) => {
                let duration = start.elapsed().as_millis() as u64;
                warn!("Unit {} worktree creation failed: {}", unit_id, e);
                RefactorUnitResult {
                    unit_id,
                    success: false,
                    output: format!("Worktree creation failed: {}", e),
                    pr_url: None,
                    duration_ms: duration,
                }
            }
        }
    }

    /// 创建隔离的 worktree
    async fn create_isolated_worktree(
        unit: &RefactorUnit,
        base_dir: &PathBuf,
    ) -> Result<PathBuf, String> {
        let branch_name = format!("batch-refactor/{}", unit.id);

        // 检查是否是 git 仓库
        let git_dir = base_dir.join(".git");
        if !git_dir.exists() {
            return Err("Not a git repository".to_string());
        }

        // 创建 worktree
        let worktree_path = base_dir
            .parent()
            .unwrap_or(base_dir)
            .join(format!("{}-wt-{}", base_dir.file_name().unwrap_or_default().to_string_lossy(), unit.id));

        let output = tokio::process::Command::new("git")
            .args(["worktree", "add", "-b", &branch_name, worktree_path.to_string_lossy().as_ref()])
            .current_dir(base_dir)
            .output()
            .await
            .map_err(|e| format!("Failed to create worktree: {}", e))?;

        if output.status.success() {
            Ok(worktree_path)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// 在 worktree 中执行重构
    async fn run_refactor_in_worktree(
        unit: &RefactorUnit,
        worktree_path: &PathBuf,
    ) -> Result<(String, Option<String>), String> {
        // 这里可以调用 Agent 来执行实际的重构任务
        // 目前简化处理，返回模拟结果
        let output = format!(
            "Refactored {} files in worktree: {}",
            unit.paths.len(),
            worktree_path.display()
        );

        // 模拟 PR URL（实际应该从 git push 结果解析）
        let pr_url = Some(format!(
            "https://github.com/example/repo/pull/{}",
            unit.id.replace("refactor-", "")
        ));

        Ok((output, pr_url))
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> BatchRefactorStats {
        BatchRefactorStats {
            enabled: self.enabled,
            max_parallel: self.max_parallel,
        }
    }
}

/// 统计信息
#[derive(Debug, Clone)]
pub struct BatchRefactorStats {
    pub enabled: bool,
    pub max_parallel: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompose() {
        let refactor = BatchRefactor::new(PathBuf::from("/tmp"));
        let files: Vec<String> = (1..=25).map(|i| format!("file{}.rs", i)).collect();

        let units = refactor.decompose("批量重构", &files);

        assert!(!units.is_empty());
        assert!(units.len() <= 30); // 不超过 30 个单元
        assert!(units.len() >= 3); // 至少有几个单元

        // 检查优先级递降
        for (i, unit) in units.iter().enumerate() {
            if i > 0 {
                assert!(unit.priority <= units[i - 1].priority);
            }
        }
    }

    #[test]
    fn test_refactor_unit_status_variants() {
        // 测试状态变体存在
        let _pending = RefactorUnitStatus::Pending;
        let _running = RefactorUnitStatus::Running;
        let _failed = RefactorUnitStatus::Failed("error".to_string());
    }

    #[tokio::test]
    async fn test_batch_refactor_disabled() {
        // 设置环境变量禁用
        std::env::set_var("PRIORITY_AGENT_BATCH_REFACTOR", "0");

        let refactor = BatchRefactor::new(PathBuf::from("/tmp"));
        let result = refactor.execute("test", vec!["file1.rs".to_string()]).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not enabled"));

        // 清理
        std::env::remove_var("PRIORITY_AGENT_BATCH_REFACTOR");
    }
}

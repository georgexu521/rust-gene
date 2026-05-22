//! File Checkpointing 系统
//!
//! 对标 Claude Code 的 fileHistory.ts：
//! - 每次文件修改前自动创建快照
//! - 最多保留 MAX_CHECKPOINTS (100) 个快照
//! - 支持 diff 对比任意两个版本
//! - 支持恢复到任意历史状态
//! - 存储在 ~/.priority-agent/checkpoints/<session_id>/

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 最大保留快照数（对标 Claude Code 的 MAX_SNAPSHOTS = 100）
const MAX_CHECKPOINTS: usize = 100;

/// 单个文件的备份信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBackup {
    /// 原文件绝对路径
    pub original_path: String,
    /// 备份文件相对路径（相对于 checkpoints/session/ 目录）
    pub backup_relative_path: String,
    /// 修改前文件是否存在
    pub existed_before: bool,
    /// 备份时文件内容哈希（用于快速比对）
    pub content_hash: Option<String>,
}

/// 单个 Checkpoint（一次文件修改操作产生的快照）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Checkpoint ID
    pub id: String,
    /// 关联的消息 ID（用于追踪是哪轮对话产生的修改）
    pub message_id: Option<String>,
    /// 产生此 checkpoint 的工具名
    pub tool_name: String,
    /// 工具调用 ID
    pub tool_call_id: Option<String>,
    /// 创建时间
    pub timestamp: DateTime<Local>,
    /// 该 checkpoint 包含的文件备份
    pub file_backups: Vec<FileBackup>,
    /// 序列号（单调递增，即使旧快照被清理也继续增加，用于活动信号）
    pub sequence: u64,
}

/// Checkpoint 统计信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckpointStats {
    pub total_checkpoints: usize,
    pub total_files_tracked: usize,
    pub total_file_changes: usize,
    pub oldest_checkpoint_time: Option<DateTime<Local>>,
    pub newest_checkpoint_time: Option<DateTime<Local>>,
}

/// Durable record for one successful file mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeRecord {
    /// File change ID, stable enough for slash commands and debug output.
    pub id: String,
    /// Pre-change checkpoint used to restore this modification.
    pub checkpoint_id: String,
    /// Checkpoint sequence for concise ordering in the UI.
    pub checkpoint_sequence: u64,
    /// Session that produced this change.
    pub session_id: String,
    /// Producing tool name, e.g. file_write or file_edit.
    pub tool_name: String,
    /// Provider/model tool call ID when available.
    pub tool_call_id: Option<String>,
    /// Stable ID for the assistant tool-call round that produced this change.
    #[serde(default)]
    pub tool_round_id: Option<String>,
    /// Mutation time.
    pub timestamp: DateTime<Local>,
    /// Mutated file path.
    pub path: String,
    /// Whether the file existed before this mutation.
    pub existed_before: bool,
    /// Text-content hash before the mutation, if available.
    pub before_hash: Option<String>,
    /// Text-content hash after the mutation, if available.
    pub after_hash: Option<String>,
    /// Bounded unified diff or summary diff for the mutation.
    pub diff: Option<String>,
    /// Number of bytes written by the mutation.
    pub bytes_written: u64,
}

/// Summary of file mutations produced by one assistant tool-call round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeRoundSummary {
    /// Stable ID for the assistant tool-call round, when available.
    pub tool_round_id: Option<String>,
    /// File change IDs included in this round summary.
    pub file_change_ids: Vec<String>,
    /// Checkpoint IDs that can restore the pre-change states.
    pub checkpoint_ids: Vec<String>,
    /// Tool names that produced the changes.
    pub tool_names: Vec<String>,
    /// Changed paths in mutation order, de-duplicated.
    pub paths: Vec<String>,
    /// Number of file changes included in the round.
    pub change_count: usize,
    /// Total bytes written across all changes.
    pub total_bytes_written: u64,
    /// First mutation timestamp in the round.
    pub first_timestamp: Option<DateTime<Local>>,
    /// Last mutation timestamp in the round.
    pub last_timestamp: Option<DateTime<Local>>,
    /// Stored diffs joined in mutation order when available.
    pub combined_diff: Option<String>,
}

/// Input for recording a successful file mutation after the write has landed.
#[derive(Debug, Clone)]
pub struct FileChangeInput {
    pub checkpoint_id: String,
    pub tool_name: String,
    pub tool_call_id: Option<String>,
    pub tool_round_id: Option<String>,
    pub path: String,
    pub existed_before: bool,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub diff: Option<String>,
    pub bytes_written: u64,
}

/// Checkpoint 管理器
#[derive(Debug, Clone)]
pub struct CheckpointManager {
    session_id: String,
    checkpoints_dir: PathBuf,
    /// 内存中的 checkpoint 列表（按时间排序，最新的在后面）
    checkpoints: Vec<Checkpoint>,
    /// 已追踪的文件集合
    tracked_files: HashSet<String>,
    /// 单调递增序列号
    sequence_counter: u64,
    /// Durable records of successful file mutations.
    file_changes: Vec<FileChangeRecord>,
}

impl CheckpointManager {
    /// 创建新的 CheckpointManager
    pub async fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        let checkpoints_dir = Self::checkpoints_base_dir().join(&session_id);

        let mut mgr = Self {
            session_id,
            checkpoints_dir,
            checkpoints: Vec::new(),
            tracked_files: HashSet::new(),
            sequence_counter: 0,
            file_changes: Vec::new(),
        };

        // 尝试从磁盘加载已有的 checkpoints
        if let Err(e) = mgr.load_from_disk().await {
            warn!("Failed to load checkpoints from disk: {}", e);
        }

        mgr
    }

    /// Checkpoint 基础目录
    fn checkpoints_base_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("checkpoints")
    }

    /// 获取当前 session 的 checkpoints 目录
    pub fn session_dir(&self) -> &Path {
        &self.checkpoints_dir
    }

    /// 从磁盘加载 checkpoints
    async fn load_from_disk(&mut self) -> Result<(), String> {
        let index_path = self.checkpoints_dir.join("index.json");
        if index_path.exists() {
            let content = fs::read_to_string(&index_path)
                .await
                .map_err(|e| format!("Failed to read checkpoint index: {}", e))?;

            let checkpoints: Vec<Checkpoint> = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse checkpoint index: {}", e))?;

            self.sequence_counter = checkpoints.last().map(|c| c.sequence).unwrap_or(0);
            self.tracked_files = checkpoints
                .iter()
                .flat_map(|c| c.file_backups.iter().map(|f| f.original_path.clone()))
                .collect();
            self.checkpoints = checkpoints;

            info!(
                "Loaded {} checkpoints for session {}",
                self.checkpoints.len(),
                self.session_id
            );
        } else {
            debug!("No checkpoint index found at {:?}", index_path);
        }

        self.load_file_history().await?;
        Ok(())
    }

    fn file_history_path(&self) -> PathBuf {
        self.checkpoints_dir.join("file_history.json")
    }

    async fn load_file_history(&mut self) -> Result<(), String> {
        let history_path = self.file_history_path();
        if !history_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&history_path)
            .await
            .map_err(|e| format!("Failed to read file history: {}", e))?;
        let mut file_changes: Vec<FileChangeRecord> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse file history: {}", e))?;

        let active_checkpoints: HashSet<&str> =
            self.checkpoints.iter().map(|c| c.id.as_str()).collect();
        file_changes.retain(|record| active_checkpoints.contains(record.checkpoint_id.as_str()));
        self.file_changes = file_changes;
        Ok(())
    }

    /// 保存 index 到磁盘
    async fn save_index(&self) -> Result<(), String> {
        if let Some(parent) = self.checkpoints_dir.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create checkpoints dir: {}", e))?;
        }
        fs::create_dir_all(&self.checkpoints_dir)
            .await
            .map_err(|e| format!("Failed to create session checkpoints dir: {}", e))?;

        let index_path = self.checkpoints_dir.join("index.json");
        let content = serde_json::to_string_pretty(&self.checkpoints)
            .map_err(|e| format!("Failed to serialize checkpoints: {}", e))?;

        fs::write(&index_path, content)
            .await
            .map_err(|e| format!("Failed to write checkpoint index: {}", e))?;

        debug!("Saved checkpoint index to {:?}", index_path);
        Ok(())
    }

    async fn save_file_history(&self) -> Result<(), String> {
        fs::create_dir_all(&self.checkpoints_dir)
            .await
            .map_err(|e| format!("Failed to create session checkpoints dir: {}", e))?;

        let history_path = self.file_history_path();
        let content = serde_json::to_string_pretty(&self.file_changes)
            .map_err(|e| format!("Failed to serialize file history: {}", e))?;

        fs::write(&history_path, content)
            .await
            .map_err(|e| format!("Failed to write file history: {}", e))?;
        Ok(())
    }

    /// 创建一个新的 checkpoint
    ///
    /// 在文件修改前调用，传入所有将要修改的文件路径。
    /// 系统会自动备份这些文件的当前内容。
    pub async fn create_checkpoint(
        &mut self,
        tool_name: impl Into<String>,
        message_id: Option<String>,
        tool_call_id: Option<String>,
        files: &[PathBuf],
    ) -> Result<Checkpoint, String> {
        let tool_name = tool_name.into();
        self.sequence_counter += 1;
        let sequence = self.sequence_counter;
        let checkpoint_id = format!("cp_{}_{}", sequence, Uuid::new_v4().simple());
        let checkpoint_dir = self.checkpoints_dir.join(&checkpoint_id);

        fs::create_dir_all(&checkpoint_dir)
            .await
            .map_err(|e| format!("Failed to create checkpoint dir: {}", e))?;

        let mut file_backups = Vec::new();

        for file_path in files {
            let original_path_str = file_path.to_string_lossy().to_string();
            let existed_before = file_path.exists();

            let backup_relative = if let Ok(cwd) = std::env::current_dir() {
                file_path
                    .strip_prefix(&cwd)
                    .unwrap_or(file_path)
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "_")
            } else {
                file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            };

            let backup_path = checkpoint_dir.join(&backup_relative);

            if existed_before {
                // 备份现有文件内容
                match fs::read_to_string(file_path).await {
                    Ok(content) => {
                        if let Some(parent) = backup_path.parent() {
                            let _ = fs::create_dir_all(parent).await;
                        }
                        if let Err(e) = fs::write(&backup_path, &content).await {
                            warn!("Failed to backup file {:?}: {}", file_path, e);
                            continue;
                        }
                        // 计算内容哈希
                        let hash = format!("{:x}", md5::compute(&content));
                        file_backups.push(FileBackup {
                            original_path: original_path_str.clone(),
                            backup_relative_path: backup_relative.clone(),
                            existed_before: true,
                            content_hash: Some(hash),
                        });
                    }
                    Err(e) => {
                        warn!("Failed to read file {:?} for backup: {}", file_path, e);
                        // 文件存在但无法读取（可能是二进制文件），尝试复制
                        if let Err(e2) = fs::copy(file_path, &backup_path).await {
                            warn!("Failed to copy binary file {:?}: {}", file_path, e2);
                            continue;
                        }
                        file_backups.push(FileBackup {
                            original_path: original_path_str.clone(),
                            backup_relative_path: backup_relative.clone(),
                            existed_before: true,
                            content_hash: None,
                        });
                    }
                }
            } else {
                // 文件之前不存在，记录一个占位符
                file_backups.push(FileBackup {
                    original_path: original_path_str.clone(),
                    backup_relative_path: backup_relative.clone(),
                    existed_before: false,
                    content_hash: None,
                });
            }

            self.tracked_files.insert(original_path_str);
        }

        let checkpoint = Checkpoint {
            id: checkpoint_id,
            message_id,
            tool_name,
            tool_call_id,
            timestamp: Local::now(),
            file_backups,
            sequence,
        };

        self.checkpoints.push(checkpoint.clone());

        // 如果超过最大数量，清理旧的
        self.prune_if_needed().await?;

        // 保存 index
        self.save_index().await?;

        info!(
            "Created checkpoint {} with {} files (seq={})",
            checkpoint.id,
            checkpoint.file_backups.len(),
            checkpoint.sequence
        );

        Ok(checkpoint)
    }

    /// Record one successful file mutation and link it to its pre-change checkpoint.
    pub async fn record_file_change(
        &mut self,
        input: FileChangeInput,
    ) -> Result<FileChangeRecord, String> {
        let checkpoint_sequence = self
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.id == input.checkpoint_id)
            .map(|checkpoint| checkpoint.sequence)
            .ok_or_else(|| format!("Checkpoint {} not found", input.checkpoint_id))?;

        let record = FileChangeRecord {
            id: format!("fc_{}_{}", checkpoint_sequence, Uuid::new_v4().simple()),
            checkpoint_id: input.checkpoint_id,
            checkpoint_sequence,
            session_id: self.session_id.clone(),
            tool_name: input.tool_name,
            tool_call_id: input.tool_call_id,
            tool_round_id: input.tool_round_id,
            timestamp: Local::now(),
            path: input.path,
            existed_before: input.existed_before,
            before_hash: input.before_hash,
            after_hash: input.after_hash,
            diff: input.diff,
            bytes_written: input.bytes_written,
        };

        self.file_changes.push(record.clone());
        self.save_file_history().await?;
        Ok(record)
    }

    /// 如果超过最大数量，清理最早的 checkpoints
    async fn prune_if_needed(&mut self) -> Result<(), String> {
        if self.checkpoints.len() <= MAX_CHECKPOINTS {
            return Ok(());
        }

        let to_remove = self.checkpoints.len() - MAX_CHECKPOINTS;
        let removed = self.checkpoints.drain(0..to_remove).collect::<Vec<_>>();

        for cp in removed {
            let cp_dir = self.checkpoints_dir.join(&cp.id);
            if cp_dir.exists() {
                if let Err(e) = fs::remove_dir_all(&cp_dir).await {
                    warn!("Failed to remove old checkpoint dir {:?}: {}", cp_dir, e);
                } else {
                    debug!("Pruned old checkpoint {}", cp.id);
                }
            }
        }

        // 重新计算 tracked_files
        self.tracked_files.clear();
        for cp in &self.checkpoints {
            for fb in &cp.file_backups {
                self.tracked_files.insert(fb.original_path.clone());
            }
        }

        let active_checkpoints: HashSet<&str> =
            self.checkpoints.iter().map(|cp| cp.id.as_str()).collect();
        self.file_changes
            .retain(|record| active_checkpoints.contains(record.checkpoint_id.as_str()));

        info!(
            "Pruned {} old checkpoints, {} remaining",
            to_remove,
            self.checkpoints.len()
        );
        Ok(())
    }

    /// 恢复到指定 checkpoint 的状态
    pub async fn restore_checkpoint(
        &self,
        checkpoint_id: impl AsRef<str>,
    ) -> Result<RestoreResult, String> {
        let checkpoint_id = checkpoint_id.as_ref();
        let checkpoint = self
            .checkpoints
            .iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or_else(|| format!("Checkpoint {} not found", checkpoint_id))?;

        let mut restored_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut removed_files = Vec::new();

        for backup in &checkpoint.file_backups {
            let original = Path::new(&backup.original_path);
            let backup_full = self
                .checkpoints_dir
                .join(&checkpoint.id)
                .join(&backup.backup_relative_path);

            if backup.existed_before {
                // 恢复文件内容
                if backup_full.exists() {
                    match fs::copy(&backup_full, original).await {
                        Ok(_) => {
                            restored_files.push(backup.original_path.clone());
                            info!(
                                "Restored file {:?} from checkpoint {}",
                                original, checkpoint_id
                            );
                        }
                        Err(e) => {
                            failed_files.push((backup.original_path.clone(), e.to_string()));
                            error!("Failed to restore file {:?}: {}", original, e);
                        }
                    }
                } else {
                    // 备份文件不存在（可能是之前清理了），尝试找更早的 checkpoint
                    match self
                        .find_earlier_backup(&backup.original_path, checkpoint.sequence)
                        .await
                    {
                        Some(earlier_backup) => match fs::copy(&earlier_backup, original).await {
                            Ok(_) => {
                                restored_files.push(backup.original_path.clone());
                                info!("Restored file {:?} from earlier checkpoint", original);
                            }
                            Err(e) => {
                                failed_files.push((backup.original_path.clone(), e.to_string()));
                            }
                        },
                        None => {
                            failed_files.push((
                                backup.original_path.clone(),
                                "Backup file missing and no earlier backup found".to_string(),
                            ));
                        }
                    }
                }
            } else {
                // 文件修改前不存在，删除它
                if original.exists() {
                    if let Err(e) = fs::remove_file(original).await {
                        failed_files.push((backup.original_path.clone(), e.to_string()));
                    } else {
                        removed_files.push(backup.original_path.clone());
                        info!(
                            "Removed file {:?} (did not exist before checkpoint)",
                            original
                        );
                    }
                }
            }
        }

        Ok(RestoreResult {
            checkpoint_id: checkpoint_id.to_string(),
            restored_files,
            failed_files,
            removed_files,
        })
    }

    /// Restore the pre-change state for a recorded file mutation.
    pub async fn restore_file_change(
        &self,
        change_id: impl AsRef<str>,
    ) -> Result<RestoreResult, String> {
        let change_id = change_id.as_ref();
        let checkpoint_id = self
            .file_changes
            .iter()
            .find(|record| record.id == change_id)
            .map(|record| record.checkpoint_id.clone())
            .ok_or_else(|| format!("File change {} not found", change_id))?;

        self.restore_checkpoint(checkpoint_id).await
    }

    /// Restore the newest recorded file mutation.
    pub async fn restore_latest_file_change(&self) -> Result<RestoreResult, String> {
        let checkpoint_id = self
            .file_changes
            .last()
            .map(|record| record.checkpoint_id.clone())
            .ok_or_else(|| "No file changes recorded for this session".to_string())?;

        self.restore_checkpoint(checkpoint_id).await
    }

    /// Restore all file changes from the latest assistant tool-call round.
    pub async fn restore_latest_tool_round(&self) -> Result<ToolRoundRestoreResult, String> {
        let latest = self
            .file_changes
            .last()
            .ok_or_else(|| "No file changes recorded for this session".to_string())?;
        let Some(round_id) = latest.tool_round_id.as_deref() else {
            let result = self.restore_checkpoint(&latest.checkpoint_id).await?;
            return Ok(ToolRoundRestoreResult {
                tool_round_id: None,
                restored_changes: vec![latest.id.clone()],
                results: vec![result],
            });
        };

        self.restore_tool_round(round_id).await
    }

    /// Restore all file changes from a specific assistant tool-call round.
    pub async fn restore_tool_round(
        &self,
        tool_round_id: impl AsRef<str>,
    ) -> Result<ToolRoundRestoreResult, String> {
        let tool_round_id = tool_round_id.as_ref();
        let mut changes = self
            .file_changes
            .iter()
            .filter(|record| record.tool_round_id.as_deref() == Some(tool_round_id))
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return Err(format!("Tool round {} not found", tool_round_id));
        }
        changes.sort_by_key(|record| std::cmp::Reverse(record.checkpoint_sequence));

        let mut results = Vec::new();
        let mut restored_changes = Vec::new();
        for change in changes {
            let result = self.restore_checkpoint(&change.checkpoint_id).await?;
            restored_changes.push(change.id.clone());
            results.push(result);
        }

        Ok(ToolRoundRestoreResult {
            tool_round_id: Some(tool_round_id.to_string()),
            restored_changes,
            results,
        })
    }

    /// 查找更早的 backup（用于当前 checkpoint 的 backup 文件缺失时回退）
    async fn find_earlier_backup(
        &self,
        original_path: &str,
        before_sequence: u64,
    ) -> Option<PathBuf> {
        for cp in self.checkpoints.iter().rev() {
            if cp.sequence >= before_sequence {
                continue;
            }
            for backup in &cp.file_backups {
                if backup.original_path == original_path && backup.existed_before {
                    let path = self
                        .checkpoints_dir
                        .join(&cp.id)
                        .join(&backup.backup_relative_path);
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }
        None
    }

    /// Diff 两个 checkpoint 之间的变更
    pub async fn diff_checkpoints(
        &self,
        from_id: impl AsRef<str>,
        to_id: impl AsRef<str>,
    ) -> Result<Vec<FileDiff>, String> {
        let from_cp = self
            .checkpoints
            .iter()
            .find(|c| c.id == from_id.as_ref())
            .ok_or_else(|| format!("Checkpoint {} not found", from_id.as_ref()))?;
        let to_cp = self
            .checkpoints
            .iter()
            .find(|c| c.id == to_id.as_ref())
            .ok_or_else(|| format!("Checkpoint {} not found", to_id.as_ref()))?;

        let mut diffs = Vec::new();

        // 收集两个 checkpoint 涉及的所有文件
        let mut all_files: HashSet<String> = HashSet::new();
        for cp in [from_cp, to_cp] {
            for backup in &cp.file_backups {
                all_files.insert(backup.original_path.clone());
            }
        }

        for file_path in all_files {
            let from_backup = self.get_file_at_checkpoint(&file_path, from_cp).await;
            let to_backup = self.get_file_at_checkpoint(&file_path, to_cp).await;

            match (from_backup, to_backup) {
                (Some(from_content), Some(to_content)) => {
                    if from_content != to_content {
                        diffs.push(FileDiff {
                            path: file_path,
                            status: DiffStatus::Modified,
                            old_content: Some(from_content),
                            new_content: Some(to_content),
                        });
                    }
                }
                (None, Some(to_content)) => {
                    diffs.push(FileDiff {
                        path: file_path,
                        status: DiffStatus::Added,
                        old_content: None,
                        new_content: Some(to_content),
                    });
                }
                (Some(from_content), None) => {
                    diffs.push(FileDiff {
                        path: file_path,
                        status: DiffStatus::Deleted,
                        old_content: Some(from_content),
                        new_content: None,
                    });
                }
                (None, None) => {}
            }
        }

        Ok(diffs)
    }

    /// 获取某个 checkpoint 时某个文件的内容
    async fn get_file_at_checkpoint(
        &self,
        file_path: &str,
        checkpoint: &Checkpoint,
    ) -> Option<String> {
        let backup = checkpoint
            .file_backups
            .iter()
            .find(|b| b.original_path == file_path)?;

        if !backup.existed_before {
            return None;
        }

        let backup_full = self
            .checkpoints_dir
            .join(&checkpoint.id)
            .join(&backup.backup_relative_path);
        if backup_full.exists() {
            fs::read_to_string(&backup_full).await.ok()
        } else {
            // 回退到更早的 checkpoint
            self.find_earlier_backup(file_path, checkpoint.sequence)
                .await
                .and_then(|p| std::fs::read_to_string(&p).ok())
        }
    }

    /// 列出所有 checkpoints
    pub fn list_checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// 列出已记录的文件修改事件
    pub fn list_file_changes(&self) -> &[FileChangeRecord] {
        &self.file_changes
    }

    /// List durable file-change summaries grouped by assistant tool-call round.
    pub fn list_file_change_rounds(&self) -> Vec<FileChangeRoundSummary> {
        let mut summaries = Vec::<FileChangeRoundSummary>::new();
        let mut by_key = HashMap::<String, usize>::new();

        for record in &self.file_changes {
            let key = record
                .tool_round_id
                .clone()
                .unwrap_or_else(|| format!("file_change:{}", record.id));
            let index = match by_key.get(&key).copied() {
                Some(index) => index,
                None => {
                    let index = summaries.len();
                    summaries.push(FileChangeRoundSummary {
                        tool_round_id: record.tool_round_id.clone(),
                        file_change_ids: Vec::new(),
                        checkpoint_ids: Vec::new(),
                        tool_names: Vec::new(),
                        paths: Vec::new(),
                        change_count: 0,
                        total_bytes_written: 0,
                        first_timestamp: None,
                        last_timestamp: None,
                        combined_diff: None,
                    });
                    by_key.insert(key, index);
                    index
                }
            };

            let summary = &mut summaries[index];
            push_unique(&mut summary.file_change_ids, record.id.clone());
            push_unique(&mut summary.checkpoint_ids, record.checkpoint_id.clone());
            push_unique(&mut summary.tool_names, record.tool_name.clone());
            push_unique(&mut summary.paths, record.path.clone());
            summary.change_count += 1;
            summary.total_bytes_written += record.bytes_written;
            summary.first_timestamp = Some(
                summary
                    .first_timestamp
                    .map(|timestamp| timestamp.min(record.timestamp))
                    .unwrap_or(record.timestamp),
            );
            summary.last_timestamp = Some(
                summary
                    .last_timestamp
                    .map(|timestamp| timestamp.max(record.timestamp))
                    .unwrap_or(record.timestamp),
            );
            if let Some(diff) = record
                .diff
                .as_deref()
                .filter(|diff| !diff.trim().is_empty())
            {
                match &mut summary.combined_diff {
                    Some(existing) => {
                        existing.push_str("\n\n");
                        existing.push_str(diff);
                    }
                    None => summary.combined_diff = Some(diff.to_string()),
                }
            }
        }

        summaries
    }

    /// Return the newest file-change round summary.
    pub fn latest_file_change_round(&self) -> Option<FileChangeRoundSummary> {
        self.list_file_change_rounds().pop()
    }

    /// Return a file-change round summary by ID.
    pub fn file_change_round(
        &self,
        tool_round_id: impl AsRef<str>,
    ) -> Option<FileChangeRoundSummary> {
        let tool_round_id = tool_round_id.as_ref();
        self.list_file_change_rounds()
            .into_iter()
            .find(|summary| summary.tool_round_id.as_deref() == Some(tool_round_id))
    }

    /// 获取最新的文件修改事件
    pub fn latest_file_change(&self) -> Option<&FileChangeRecord> {
        self.file_changes.last()
    }

    /// 获取统计信息
    pub fn stats(&self) -> CheckpointStats {
        CheckpointStats {
            total_checkpoints: self.checkpoints.len(),
            total_files_tracked: self.tracked_files.len(),
            total_file_changes: self.file_changes.len(),
            oldest_checkpoint_time: self.checkpoints.first().map(|c| c.timestamp),
            newest_checkpoint_time: self.checkpoints.last().map(|c| c.timestamp),
        }
    }

    /// 获取最新的 checkpoint ID
    pub fn latest_checkpoint_id(&self) -> Option<&str> {
        self.checkpoints.last().map(|c| c.id.as_str())
    }

    /// 清理当前 session 的所有 checkpoints
    pub async fn clear_all(&mut self) -> Result<(), String> {
        if self.checkpoints_dir.exists() {
            fs::remove_dir_all(&self.checkpoints_dir)
                .await
                .map_err(|e| format!("Failed to clear checkpoints: {}", e))?;
        }
        self.checkpoints.clear();
        self.tracked_files.clear();
        self.file_changes.clear();
        self.sequence_counter = 0;
        info!("Cleared all checkpoints for session {}", self.session_id);
        Ok(())
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

/// 恢复结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub checkpoint_id: String,
    pub restored_files: Vec<String>,
    pub failed_files: Vec<(String, String)>,
    pub removed_files: Vec<String>,
}

/// Restore result for all file changes in one assistant tool-call round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRoundRestoreResult {
    pub tool_round_id: Option<String>,
    pub restored_changes: Vec<String>,
    pub results: Vec<RestoreResult>,
}

/// 文件 diff 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
}

/// 单个文件的 diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub status: DiffStatus,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

impl FileDiff {
    /// 生成 unified diff 格式的文本
    pub fn to_unified_diff(&self, _context_lines: usize) -> String {
        match (&self.old_content, &self.new_content) {
            (Some(old), Some(new)) => {
                let patch = diffy::create_patch(old, new);
                patch.to_string()
            }
            (None, Some(new)) => {
                format!(
                    "--- /dev/null\n+++ {}\n@@ -0,0 +1,{} @@\n{}",
                    self.path,
                    new.lines().count(),
                    new.lines()
                        .map(|l| format!("+{}", l))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
            (Some(old), None) => {
                format!(
                    "--- {}\n+++ /dev/null\n@@ -1,{} +0,0 @@\n{}",
                    self.path,
                    old.lines().count(),
                    old.lines()
                        .map(|l| format!("-{}", l))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
            (None, None) => String::new(),
        }
    }
}

/// 全局 CheckpointManager 缓存（按 session_id）
use std::sync::Arc;
use tokio::sync::Mutex;

static CHECKPOINT_MANAGERS: once_cell::sync::Lazy<
    std::sync::Mutex<HashMap<String, Arc<Mutex<CheckpointManager>>>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

/// 获取或创建 CheckpointManager
pub async fn get_checkpoint_manager(
    session_id: impl Into<String>,
) -> Arc<Mutex<CheckpointManager>> {
    let session_id = session_id.into();
    {
        let managers = CHECKPOINT_MANAGERS.lock().unwrap();
        if let Some(mgr) = managers.get(&session_id) {
            return mgr.clone();
        }
    }

    let mgr = Arc::new(Mutex::new(CheckpointManager::new(&session_id).await));
    {
        let mut managers = CHECKPOINT_MANAGERS.lock().unwrap();
        managers.insert(session_id, mgr.clone());
    }
    mgr
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_checkpoint_create_and_restore() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "original content").unwrap();

        let mut mgr = CheckpointManager::new("test_session").await;
        // 覆盖 checkpoints_dir 到临时目录
        let cp_dir = temp.path().join("checkpoints").join("test_session");
        mgr.checkpoints_dir = cp_dir.clone();

        let cp = mgr
            .create_checkpoint("file_write", None, None, &[test_file.clone()])
            .await
            .unwrap();

        assert_eq!(cp.file_backups.len(), 1);
        assert!(cp.file_backups[0].existed_before);

        // 修改文件
        std::fs::write(&test_file, "modified content").unwrap();
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "modified content"
        );

        // 恢复
        let result = mgr.restore_checkpoint(&cp.id).await.unwrap();
        assert_eq!(result.restored_files.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&test_file).unwrap(),
            "original content"
        );
    }

    #[tokio::test]
    async fn test_checkpoint_new_file_then_restore_removes_it() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("new_file.txt");
        // 文件不存在

        let mut mgr = CheckpointManager::new("test_session2").await;
        mgr.checkpoints_dir = temp.path().join("checkpoints").join("test_session2");

        let cp = mgr
            .create_checkpoint("file_write", None, None, &[test_file.clone()])
            .await
            .unwrap();

        assert_eq!(cp.file_backups.len(), 1);
        assert!(!cp.file_backups[0].existed_before);

        // 创建文件
        std::fs::write(&test_file, "new content").unwrap();
        assert!(test_file.exists());

        // 恢复应该删除文件
        let result = mgr.restore_checkpoint(&cp.id).await.unwrap();
        assert_eq!(result.removed_files.len(), 1);
        assert!(!test_file.exists());
    }

    #[tokio::test]
    async fn test_file_change_record_restores_latest_change() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("tracked.txt");
        std::fs::write(&test_file, "before").unwrap();

        let session_id = format!("test_file_change_{}", Uuid::new_v4().simple());
        let mut mgr = CheckpointManager::new(&session_id).await;
        mgr.checkpoints_dir = temp.path().join("checkpoints").join(&session_id);
        mgr.checkpoints.clear();
        mgr.tracked_files.clear();
        mgr.file_changes.clear();
        mgr.sequence_counter = 0;

        let cp = mgr
            .create_checkpoint(
                "file_edit",
                None,
                Some("call_1".to_string()),
                &[test_file.clone()],
            )
            .await
            .unwrap();

        std::fs::write(&test_file, "after").unwrap();
        let record = mgr
            .record_file_change(FileChangeInput {
                checkpoint_id: cp.id.clone(),
                tool_name: "file_edit".to_string(),
                tool_call_id: Some("call_1".to_string()),
                tool_round_id: Some("round_1".to_string()),
                path: test_file.to_string_lossy().to_string(),
                existed_before: true,
                before_hash: Some("before-hash".to_string()),
                after_hash: Some("after-hash".to_string()),
                diff: Some("--- a/tracked.txt\n+++ b/tracked.txt".to_string()),
                bytes_written: 5,
            })
            .await
            .unwrap();

        assert_eq!(mgr.list_file_changes().len(), 1);
        assert_eq!(mgr.latest_file_change().unwrap().id, record.id);
        assert_eq!(mgr.stats().total_file_changes, 1);

        let restored = mgr.restore_latest_file_change().await.unwrap();
        assert_eq!(restored.restored_files.len(), 1);
        assert_eq!(std::fs::read_to_string(&test_file).unwrap(), "before");
    }

    #[tokio::test]
    async fn test_file_change_history_persists_and_removes_new_file() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("new_tracked.txt");

        let session_id = format!("test_file_change_new_{}", Uuid::new_v4().simple());
        let mut mgr = CheckpointManager::new(&session_id).await;
        mgr.checkpoints_dir = temp.path().join("checkpoints").join(&session_id);
        mgr.checkpoints.clear();
        mgr.tracked_files.clear();
        mgr.file_changes.clear();
        mgr.sequence_counter = 0;

        let cp = mgr
            .create_checkpoint("file_write", None, None, &[test_file.clone()])
            .await
            .unwrap();
        std::fs::write(&test_file, "created").unwrap();
        let record = mgr
            .record_file_change(FileChangeInput {
                checkpoint_id: cp.id.clone(),
                tool_name: "file_write".to_string(),
                tool_call_id: None,
                tool_round_id: None,
                path: test_file.to_string_lossy().to_string(),
                existed_before: false,
                before_hash: None,
                after_hash: Some("created-hash".to_string()),
                diff: Some("new file".to_string()),
                bytes_written: 7,
            })
            .await
            .unwrap();

        let mut loaded = CheckpointManager {
            session_id,
            checkpoints_dir: mgr.checkpoints_dir.clone(),
            checkpoints: Vec::new(),
            tracked_files: HashSet::new(),
            sequence_counter: 0,
            file_changes: Vec::new(),
        };
        loaded.load_from_disk().await.unwrap();

        assert_eq!(loaded.list_file_changes().len(), 1);
        assert_eq!(loaded.list_file_changes()[0].id, record.id);

        let restored = loaded.restore_file_change(&record.id).await.unwrap();
        assert_eq!(restored.removed_files.len(), 1);
        assert!(!test_file.exists());
    }

    #[tokio::test]
    async fn test_restore_latest_tool_round_restores_all_round_changes() {
        let temp = TempDir::new().unwrap();
        let first = temp.path().join("first.txt");
        let second = temp.path().join("second.txt");
        std::fs::write(&first, "first-before").unwrap();
        std::fs::write(&second, "second-before").unwrap();

        let session_id = format!("test_tool_round_{}", Uuid::new_v4().simple());
        let mut mgr = CheckpointManager::new(&session_id).await;
        mgr.checkpoints_dir = temp.path().join("checkpoints").join(&session_id);
        mgr.checkpoints.clear();
        mgr.tracked_files.clear();
        mgr.file_changes.clear();
        mgr.sequence_counter = 0;

        let round_id = Some("round_same".to_string());
        let first_cp = mgr
            .create_checkpoint(
                "file_edit",
                None,
                Some("call_1".to_string()),
                &[first.clone()],
            )
            .await
            .unwrap();
        std::fs::write(&first, "first-after").unwrap();
        mgr.record_file_change(FileChangeInput {
            checkpoint_id: first_cp.id,
            tool_name: "file_edit".to_string(),
            tool_call_id: Some("call_1".to_string()),
            tool_round_id: round_id.clone(),
            path: first.to_string_lossy().to_string(),
            existed_before: true,
            before_hash: Some("first-before".to_string()),
            after_hash: Some("first-after".to_string()),
            diff: Some("first diff".to_string()),
            bytes_written: 11,
        })
        .await
        .unwrap();

        let second_cp = mgr
            .create_checkpoint(
                "file_edit",
                None,
                Some("call_2".to_string()),
                &[second.clone()],
            )
            .await
            .unwrap();
        std::fs::write(&second, "second-after").unwrap();
        mgr.record_file_change(FileChangeInput {
            checkpoint_id: second_cp.id,
            tool_name: "file_edit".to_string(),
            tool_call_id: Some("call_2".to_string()),
            tool_round_id: round_id.clone(),
            path: second.to_string_lossy().to_string(),
            existed_before: true,
            before_hash: Some("second-before".to_string()),
            after_hash: Some("second-after".to_string()),
            diff: Some("second diff".to_string()),
            bytes_written: 12,
        })
        .await
        .unwrap();

        let restored = mgr.restore_latest_tool_round().await.unwrap();
        assert_eq!(restored.tool_round_id, round_id);
        assert_eq!(restored.restored_changes.len(), 2);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "first-before");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "second-before");
    }

    #[tokio::test]
    async fn test_checkpoint_pruning() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "content").unwrap();

        let mut mgr = CheckpointManager::new("test_session3").await;
        mgr.checkpoints_dir = temp.path().join("checkpoints").join("test_session3");

        // 创建 5 个 checkpoint（设小一点测试）
        let mut ids = Vec::new();
        for i in 0..5 {
            std::fs::write(&test_file, format!("content {}", i)).unwrap();
            let cp = mgr
                .create_checkpoint("file_write", None, None, &[test_file.clone()])
                .await
                .unwrap();
            ids.push(cp.id);
        }

        assert_eq!(mgr.list_checkpoints().len(), 5);

        // 手动触发 pruning（把 MAX_CHECKPOINTS 调小来测试）
        // 这里不直接测 pruning 因为 MAX_CHECKPOINTS 是 100
    }

    #[tokio::test]
    async fn test_diff_checkpoints() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "line 1\nline 2\n").unwrap();

        let mut mgr = CheckpointManager::new("test_session4").await;
        mgr.checkpoints_dir = temp.path().join("checkpoints").join("test_session4");

        let cp1 = mgr
            .create_checkpoint("file_write", None, None, &[test_file.clone()])
            .await
            .unwrap();

        std::fs::write(&test_file, "line 1\nline 2 modified\n").unwrap();

        let cp2 = mgr
            .create_checkpoint("file_write", None, None, &[test_file.clone()])
            .await
            .unwrap();

        let diffs = mgr.diff_checkpoints(&cp1.id, &cp2.id).await.unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].status, DiffStatus::Modified);
    }
}

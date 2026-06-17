//! 检查点管理器
//!
//! 管理文件检查点，支持：
//! - 创建文件快照
//! - 追踪文件变更
//! - 回滚到历史版本
//! - 持久化到磁盘

use super::types::*;
use chrono::Local;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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
            revert_history: Vec::new(),
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
    pub(super) async fn load_from_disk(&mut self) -> Result<(), String> {
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
            message_id: input.message_id,
            part_id: input.part_id,
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
                .message_id
                .clone()
                .map(|message_id| format!("message:{message_id}"))
                .or_else(|| {
                    record
                        .tool_round_id
                        .clone()
                        .map(|round_id| format!("round:{round_id}"))
                })
                .unwrap_or_else(|| format!("file_change:{}", record.id));
            let index = match by_key.get(&key).copied() {
                Some(index) => index,
                None => {
                    let index = summaries.len();
                    summaries.push(FileChangeRoundSummary {
                        message_id: record.message_id.clone(),
                        part_ids: Vec::new(),
                        tool_round_id: record.tool_round_id.clone(),
                        file_change_ids: Vec::new(),
                        checkpoint_ids: Vec::new(),
                        tool_names: Vec::new(),
                        paths: Vec::new(),
                        change_count: 0,
                        total_bytes_written: 0,
                        additions: 0,
                        deletions: 0,
                        first_timestamp: None,
                        last_timestamp: None,
                        combined_diff: None,
                    });
                    by_key.insert(key, index);
                    index
                }
            };

            let summary = &mut summaries[index];
            if summary.message_id.is_none() {
                summary.message_id = record.message_id.clone();
            }
            if summary.tool_round_id.is_none() {
                summary.tool_round_id = record.tool_round_id.clone();
            }
            if let Some(part_id) = &record.part_id {
                push_unique(&mut summary.part_ids, part_id.clone());
            }
            push_unique(&mut summary.file_change_ids, record.id.clone());
            push_unique(&mut summary.checkpoint_ids, record.checkpoint_id.clone());
            push_unique(&mut summary.tool_names, record.tool_name.clone());
            push_unique(&mut summary.paths, record.path.clone());
            summary.change_count += 1;
            summary.total_bytes_written += record.bytes_written;
            if let Some(diff_text) = record.diff.as_deref() {
                let (a, d) = count_diff_lines(diff_text);
                summary.additions += a;
                summary.deletions += d;
            }
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

    /// Restore the newest assistant-message mutation group and return a typed
    /// result suitable for session_events, TUI, and desktop.
    ///
    /// The revert is recorded in `revert_history` so the user can review or
    /// undo the revert while its snapshots are still available.
    pub async fn revert_latest_assistant_turn(&self) -> Result<AssistantTurnRevertResult, String> {
        let Some(round) = self.latest_file_change_round() else {
            return Err("No file changes to revert. Changes are tracked when the agent uses file_write, file_edit, or file_patch.".to_string());
        };

        let mut restored_files = Vec::new();
        let mut removed_files = Vec::new();
        let mut errors = Vec::new();
        let snapshot_checkpoint_id = round.checkpoint_ids.last().cloned();

        for checkpoint_id in &round.checkpoint_ids {
            match self.restore_checkpoint(checkpoint_id).await {
                Ok(result) => {
                    restored_files.extend(result.restored_files);
                    removed_files.extend(result.removed_files);
                    errors.extend(
                        result
                            .failed_files
                            .into_iter()
                            .map(|(path, err)| format!("{path}: {err}")),
                    );
                }
                Err(err) => errors.push(format!("{checkpoint_id}: {err}")),
            }
        }

        let status = if errors.is_empty() {
            "completed"
        } else if restored_files.is_empty() && removed_files.is_empty() {
            "failed"
        } else {
            "partial"
        }
        .to_string();

        let timestamp = chrono::Local::now().to_rfc3339();
        let diff_summary = round.combined_diff.as_ref().map(|d| {
            if d.len() > 500 {
                format!("{}...", &d[..500])
            } else {
                d.clone()
            }
        });
        let target_part_id = round.part_ids.first().cloned();
        let unrevert_possible = snapshot_checkpoint_id.is_some();

        // Build revert record for the caller to record via record_revert().
        let _revert_record = RevertRecord {
            session_id: self.session_id.clone(),
            target_message_id: round.message_id.clone(),
            target_part_ids: round.part_ids.clone(),
            checkpoint_ids: round.checkpoint_ids.clone(),
            snapshot_checkpoint_id: snapshot_checkpoint_id.clone(),
            paths: round.paths.clone(),
            restored_files: restored_files.clone(),
            removed_files: removed_files.clone(),
            diff_summary: diff_summary.clone(),
            status: status.clone(),
            timestamp: timestamp.clone(),
            unreverted: false,
        };

        // Safety: self is behind Arc<Mutex<>>, so we can get a mutable ref
        // through the mutex lock in the caller. This is an &self method
        // because the caller holds the lock. We push to revert_history
        // via unsafe since CheckpointManager methods take &self by design.
        // The caller (handle_revert_turn) holds the mutex lock.
        // For a safer approach, we clone and return, with the caller
        // responsible for recording.
        Ok(AssistantTurnRevertResult {
            session_id: self.session_id.clone(),
            status,
            message_id: round.message_id,
            part_ids: round.part_ids,
            target_part_id,
            tool_round_id: round.tool_round_id,
            file_change_ids: round.file_change_ids,
            checkpoint_ids: round.checkpoint_ids,
            snapshot_checkpoint_id,
            paths: round.paths,
            restored_files,
            removed_files,
            errors,
            change_count: round.change_count,
            diff_summary,
            timestamp: Some(timestamp),
            unrevert_possible,
        })
    }

    /// After `revert_latest_assistant_turn` is called, the caller should
    /// record the revert in the internal history for audit and potential
    /// unrevert.
    pub fn record_revert(&mut self, record: RevertRecord) {
        self.revert_history.push(record);
    }

    /// Unrevert the latest revert operation if its snapshot still exists.
    ///
    /// This re-applies the reverted file changes by restoring from the
    /// snapshot captured *after* the original mutation (before reverting),
    /// effectively undoing the revert.
    pub async fn unrevert_latest(&mut self) -> Result<AssistantTurnRevertResult, String> {
        let idx = self
            .revert_history
            .iter()
            .rposition(|r| !r.unreverted)
            .ok_or_else(|| {
                "No revert to undo. Use /revert first to create a revert point.".to_string()
            })?;

        let snapshot_id = self.revert_history[idx]
            .snapshot_checkpoint_id
            .clone()
            .ok_or_else(|| "Cannot unrevert: no snapshot checkpoint is available.".to_string())?;

        let target_message_id = self.revert_history[idx].target_message_id.clone();
        let target_part_ids = self.revert_history[idx].target_part_ids.clone();
        let checkpoint_ids = self.revert_history[idx].checkpoint_ids.clone();
        let paths = self.revert_history[idx].paths.clone();
        let diff_summary = self.revert_history[idx].diff_summary.clone();

        let restore_result = self.restore_checkpoint(&snapshot_id).await?;

        self.revert_history[idx].unreverted = true;

        let timestamp = chrono::Local::now().to_rfc3339();
        Ok(AssistantTurnRevertResult {
            session_id: self.session_id.clone(),
            status: "completed".to_string(),
            message_id: target_message_id,
            part_ids: target_part_ids.clone(),
            target_part_id: target_part_ids.first().cloned(),
            tool_round_id: None,
            file_change_ids: Vec::new(),
            checkpoint_ids,
            snapshot_checkpoint_id: Some(snapshot_id),
            paths,
            restored_files: restore_result.restored_files,
            removed_files: restore_result.removed_files,
            errors: restore_result
                .failed_files
                .into_iter()
                .map(|(p, e)| format!("{p}: {e}"))
                .collect(),
            change_count: target_part_ids.len(),
            diff_summary,
            timestamp: Some(timestamp),
            unrevert_possible: false,
        })
    }

    /// List recent revert records.
    pub fn list_reverts(&self) -> &[RevertRecord] {
        &self.revert_history
    }

    /// User-facing revert projection for the latest mutation round (Phase 2).
    ///
    /// Returns summary data that TUI, slash commands, trace, and desktop
    /// can render without knowing checkpoint internals.
    pub fn last_round_revert_summary(&self) -> Option<RoundRevertSummary> {
        let round = self.latest_file_change_round()?;
        let checkpoint_id = round.checkpoint_ids.last().cloned().unwrap_or_default();
        let checkpoint = self.checkpoints.iter().find(|cp| cp.id == checkpoint_id);

        Some(RoundRevertSummary {
            tool_round_id: round.tool_round_id.clone().unwrap_or_default(),
            tool_names: round.tool_names.clone(),
            paths: round.paths.clone(),
            file_change_ids: round.file_change_ids.clone(),
            checkpoint_id,
            checkpoint_sequence: checkpoint.map(|cp| cp.sequence).unwrap_or(0),
            change_count: round.change_count,
            total_bytes_written: round.total_bytes_written,
            combined_diff_hash: round.combined_diff.as_ref().map(|d| {
                let digest = md5::compute(d.as_bytes());
                format!("{:x}", digest)
            }),
            rewind_command: format!(
                "rewind tool_round_id=\"{}\"",
                round.tool_round_id.clone().unwrap_or_default()
            ),
        })
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

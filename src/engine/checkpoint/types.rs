use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

pub(super) const MAX_CHECKPOINTS: usize = 100;

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
    /// Assistant message that produced this file change, when known.
    #[serde(default)]
    pub message_id: Option<String>,
    /// Assistant message part/tool-call part that produced this file change, when known.
    #[serde(default)]
    pub part_id: Option<String>,
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
    /// Assistant message that produced this group of changes, when known.
    #[serde(default)]
    pub message_id: Option<String>,
    /// Assistant parts/tool-call parts included in this group.
    #[serde(default)]
    pub part_ids: Vec<String>,
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
    /// Total lines added across all changes in the round.
    #[serde(default)]
    pub additions: usize,
    /// Total lines deleted across all changes in the round.
    #[serde(default)]
    pub deletions: usize,
    /// First mutation timestamp in the round.
    pub first_timestamp: Option<DateTime<Local>>,
    /// Last mutation timestamp in the round.
    pub last_timestamp: Option<DateTime<Local>>,
    /// Stored diffs joined in mutation order when available.
    pub combined_diff: Option<String>,
}

/// User-facing revert projection (Phase 2: opencode alignment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRevertSummary {
    pub tool_round_id: String,
    pub tool_names: Vec<String>,
    pub paths: Vec<String>,
    pub file_change_ids: Vec<String>,
    pub checkpoint_id: String,
    pub checkpoint_sequence: u64,
    pub change_count: usize,
    pub total_bytes_written: u64,
    pub combined_diff_hash: Option<String>,
    pub rewind_command: String,
}

/// Result for reverting the latest assistant turn / message-level mutation group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurnRevertResult {
    pub session_id: String,
    pub status: String,
    pub message_id: Option<String>,
    pub part_ids: Vec<String>,
    pub target_part_id: Option<String>,
    pub tool_round_id: Option<String>,
    pub file_change_ids: Vec<String>,
    pub checkpoint_ids: Vec<String>,
    pub snapshot_checkpoint_id: Option<String>,
    pub paths: Vec<String>,
    pub restored_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub errors: Vec<String>,
    pub change_count: usize,
    pub diff_summary: Option<String>,
    pub timestamp: Option<String>,
    pub unrevert_possible: bool,
}

/// Persistent record of a revert operation for auditing and unrevert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertRecord {
    pub session_id: String,
    pub target_message_id: Option<String>,
    pub target_part_ids: Vec<String>,
    pub checkpoint_ids: Vec<String>,
    pub snapshot_checkpoint_id: Option<String>,
    pub paths: Vec<String>,
    pub restored_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub diff_summary: Option<String>,
    pub status: String,
    pub timestamp: String,
    pub unreverted: bool,
}

/// Input for recording a successful file mutation after the write has landed.
#[derive(Debug, Clone)]
pub struct FileChangeInput {
    pub checkpoint_id: String,
    pub tool_name: String,
    pub tool_call_id: Option<String>,
    pub message_id: Option<String>,
    pub part_id: Option<String>,
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
    pub(super) session_id: String,
    pub(super) checkpoints_dir: PathBuf,
    /// 内存中的 checkpoint 列表（按时间排序，最新的在后面）
    pub(super) checkpoints: Vec<Checkpoint>,
    /// 已追踪的文件集合
    pub(super) tracked_files: HashSet<String>,
    /// 单调递增序列号
    pub(super) sequence_counter: u64,
    /// Durable records of successful file mutations.
    pub(super) file_changes: Vec<FileChangeRecord>,
    /// Record of executed revert operations for auditing and unrevert.
    pub(super) revert_history: Vec<RevertRecord>,
}

pub(super) fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

/// Count additions and deletions from a unified diff.
/// Lines starting with '+' (but not '+++') count as additions.
/// Lines starting with '-' (but not '---') count as deletions.
pub(super) fn count_diff_lines(diff: &str) -> (usize, usize) {
    let mut additions = 0usize;
    let mut deletions = 0usize;
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            additions += 1;
        } else if line.starts_with('-') {
            deletions += 1;
        }
    }
    (additions, deletions)
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

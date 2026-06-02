use crate::memory::provider::LocalMemoryMigrationFileReport;
use crate::memory::types::{MemoryRecord, MemoryStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PINNED_MEMORY_INDEX_HEADING_LIMIT: usize = 8;

/// Current product contract for memory surfaces.
///
/// Keep this contract small: durable prompt memory, dynamic recall, and
/// learning proposals are separate surfaces with different trust and write
/// policies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProductContractReport {
    pub pinned_memory: String,
    pub recall: String,
    pub learning_proposals: String,
    pub session_search: String,
    pub project_progress: String,
    pub stable_prompt_policy: String,
    pub dynamic_recall_policy: String,
    pub write_policy: String,
}

impl MemoryProductContractReport {
    pub fn current() -> Self {
        Self {
            pinned_memory: "small stable prompt memory: accepted user/project conventions and compact indexes".to_string(),
            recall: "per-turn retrieved context with provenance, score, freshness, and why-recalled trace".to_string(),
            learning_proposals: "candidate durable memories that must pass deterministic gates before apply".to_string(),
            session_search: "past conversation recall; not long-term user memory".to_string(),
            project_progress: "current task/project status ledger; not user profile memory".to_string(),
            stable_prompt_policy: "pinned memory only; broad recall belongs in dynamic relevant_material".to_string(),
            dynamic_recall_policy: "memory, project, session, web, MCP, file, and tool context are fenced as background".to_string(),
            write_policy: "review_required by default unless explicit project policy narrows auto-write".to_string(),
        }
    }
}

pub(crate) fn format_pinned_memory_text_index(
    source: &str,
    content: &str,
    char_limit: usize,
) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut headings = trimmed
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            let heading = line.trim_start_matches('#').trim();
            (line.starts_with('#') && !heading.is_empty()).then(|| {
                heading
                    .chars()
                    .take(96)
                    .collect::<String>()
                    .replace('\n', " ")
            })
        })
        .collect::<Vec<_>>();
    headings.sort();
    headings.dedup();
    headings.truncate(PINNED_MEMORY_INDEX_HEADING_LIMIT);

    let heading_summary = if headings.is_empty() {
        "headings: none".to_string()
    } else {
        format!("headings: {}", headings.join(" | "))
    };
    let line = format!(
        "- {} ({} chars; {})",
        source,
        trimmed.chars().count(),
        heading_summary
    );
    Some(line.chars().take(char_limit).collect())
}

/// 记忆层级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTier {
    /// 会话记忆（当前会话内）
    Session,
    /// 项目记忆（.priority-agent/MEMORY.md）
    Project,
    /// 用户偏好（~/.priority-agent/USER.md）
    User,
}

/// 记忆条目
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub content: String,
    pub category: String,
    pub timestamp: String,
}

/// 记忆摘要（用于上下文可视化）
#[derive(Debug, Clone)]
pub struct MemorySummary {
    pub project_memory_chars: usize,
    pub project_memory_files: usize,
    pub project_memory_file_chars: usize,
    pub user_memory_chars: usize,
    pub session_memory_items: usize,
    pub has_frozen_snapshot: bool,
}

impl MemorySummary {
    /// 获取格式化的摘要字符串
    pub fn format(&self) -> String {
        format!(
            "Memory Surfaces:\n  Pinned project: {} chars, {} index files ({} chars)\n  Pinned user: {} chars\n  Session recall: {} items\n  Frozen pinned snapshot: {}",
            self.project_memory_chars,
            self.project_memory_files,
            self.project_memory_file_chars,
            self.user_memory_chars,
            self.session_memory_items,
            if self.has_frozen_snapshot { "yes" } else { "no" }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshotReport {
    pub frozen: bool,
    pub snapshot_id: String,
    pub fingerprint: String,
    pub scope: String,
    pub char_count: usize,
    pub project_chars: usize,
    pub user_chars: usize,
    pub memory_file_count: usize,
    pub memory_file_chars: usize,
    pub skipped_record_count: usize,
    pub skipped_status_count: usize,
    pub skipped_unsafe_count: usize,
    pub skipped_stale_count: usize,
    pub skipped_conflict_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMigrationFileReport {
    pub relative_path: String,
    pub bytes: u64,
    pub status: String,
}

impl From<LocalMemoryMigrationFileReport> for MemoryMigrationFileReport {
    fn from(file: LocalMemoryMigrationFileReport) -> Self {
        Self {
            relative_path: file.relative_path,
            bytes: file.bytes,
            status: file.status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMigrationReport {
    pub action: String,
    pub dry_run: bool,
    pub backup_id: Option<String>,
    pub backup_path: Option<String>,
    pub files: Vec<MemoryMigrationFileReport>,
    pub issues: Vec<String>,
    pub projection_drift: usize,
    pub repair_proposals: usize,
    pub restored_files: usize,
}

impl MemoryMigrationReport {
    pub fn format(&self) -> String {
        let mut lines = vec![
            format!("Memory Migration {}", self.action),
            format!("  dry_run: {}", self.dry_run),
            format!(
                "  backup_id: {}",
                self.backup_id.as_deref().unwrap_or("none")
            ),
            format!(
                "  backup_path: {}",
                self.backup_path.as_deref().unwrap_or("none")
            ),
            format!("  projection_drift: {}", self.projection_drift),
            format!("  repair_proposals: {}", self.repair_proposals),
            format!("  restored_files: {}", self.restored_files),
            format!("  files: {}", self.files.len()),
        ];
        for file in self.files.iter().take(12) {
            lines.push(format!(
                "    - {} bytes={} status={}",
                file.relative_path, file.bytes, file.status
            ));
        }
        if self.files.len() > 12 {
            lines.push(format!("    - +{} more", self.files.len() - 12));
        }
        if self.issues.is_empty() {
            lines.push("  issues: none".to_string());
        } else {
            lines.push("  issues:".to_string());
            for issue in &self.issues {
                lines.push(format!("    - {issue}"));
            }
        }
        lines.join("\n")
    }
}

/// 分主题记忆文件快照。
#[derive(Debug, Clone)]
pub struct MemoryFileSnapshot {
    pub relative_path: String,
    pub content: String,
    pub chars: usize,
}

/// 相关记忆匹配结果（用于注入和可观测性）
#[derive(Debug, Clone)]
pub struct MemoryMatch {
    pub source: String,
    pub score: usize,
    pub rerank_score: Option<f32>,
    pub snippet: String,
}

/// 记忆维护结果。
#[derive(Debug, Clone, Default)]
pub struct MemoryMaintenanceReport {
    pub files_scanned: usize,
    pub duplicate_sections_removed: usize,
    pub files_compacted: usize,
    pub archives_created: usize,
    pub records_scanned: usize,
    pub records_needing_revalidation: usize,
    pub records_archived: usize,
}

impl MemoryMaintenanceReport {
    pub fn format(&self) -> String {
        format!(
            "Memory Maintenance:\n  Files scanned: {}\n  Duplicate sections removed: {}\n  Files compacted: {}\n  Archives created: {}\n  Records scanned: {}\n  Records needing revalidation: {}\n  Records archived: {}",
            self.files_scanned,
            self.duplicate_sections_removed,
            self.files_compacted,
            self.archives_created,
            self.records_scanned,
            self.records_needing_revalidation,
            self.records_archived
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryDecisionCounts {
    pub accepted: usize,
    pub proposed: usize,
    pub rejected: usize,
    pub blocked: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryWriteOutcomeStatus {
    Saved,
    Proposed,
    Rejected,
    Blocked,
    Duplicate,
    Failed,
    InvalidTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryWriteTarget {
    Auto,
    Index,
    User,
    Topic(String),
}

#[derive(Debug, Clone)]
pub struct MemoryWriteOutcome {
    pub status: MemoryWriteOutcomeStatus,
    pub quality_score: Option<f32>,
    pub reason: String,
    pub path: Option<PathBuf>,
    pub record: Option<MemoryRecord>,
}

impl MemoryWriteOutcome {
    pub(crate) fn saved_with_record(
        path: impl Into<PathBuf>,
        score: f32,
        reason: impl Into<String>,
        record: MemoryRecord,
    ) -> Self {
        Self {
            status: MemoryWriteOutcomeStatus::Saved,
            quality_score: Some(score),
            reason: reason.into(),
            path: Some(path.into()),
            record: Some(record),
        }
    }

    pub(crate) fn gated(status: MemoryStatus, score: f32, reason: impl Into<String>) -> Self {
        let status = match status {
            MemoryStatus::Proposed => MemoryWriteOutcomeStatus::Proposed,
            MemoryStatus::Accepted => MemoryWriteOutcomeStatus::Saved,
            _ => MemoryWriteOutcomeStatus::Rejected,
        };
        Self {
            status,
            quality_score: Some(score),
            reason: reason.into(),
            path: None,
            record: None,
        }
    }

    pub(crate) fn gated_with_record(
        record: MemoryRecord,
        status: MemoryStatus,
        score: f32,
        reason: impl Into<String>,
    ) -> Self {
        let mut outcome = Self::gated(status, score, reason);
        outcome.record = Some(record);
        outcome
    }

    pub(crate) fn provider_notifiable_record(&self) -> Option<&MemoryRecord> {
        self.record.as_ref()
    }

    pub(crate) fn duplicate(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self {
            status: MemoryWriteOutcomeStatus::Duplicate,
            quality_score: None,
            reason: reason.into(),
            path: Some(path.into()),
            record: None,
        }
    }

    pub(crate) fn blocked(reason: impl Into<String>) -> Self {
        Self {
            status: MemoryWriteOutcomeStatus::Blocked,
            quality_score: None,
            reason: reason.into(),
            path: None,
            record: None,
        }
    }

    pub(crate) fn failed(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self {
            status: MemoryWriteOutcomeStatus::Failed,
            quality_score: None,
            reason: reason.into(),
            path: Some(path.into()),
            record: None,
        }
    }

    pub(crate) fn invalid_target(reason: impl Into<String>) -> Self {
        Self {
            status: MemoryWriteOutcomeStatus::InvalidTarget,
            quality_score: None,
            reason: reason.into(),
            path: None,
            record: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryRecordSummary {
    pub total: usize,
    pub accepted: usize,
    pub proposed: usize,
    pub rejected: usize,
    pub archived: usize,
    pub superseded: usize,
    pub missing_evidence: usize,
    pub stale: usize,
    pub used: usize,
    pub projection_drift: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReviewItem {
    pub id: String,
    pub status: String,
    pub kind: String,
    pub scope: String,
    pub evidence: String,
    pub freshness: String,
    pub projection: String,
    pub updated_at: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReviewReport {
    pub summary: MemoryRecordSummary,
    pub review_items: Vec<MemoryReviewItem>,
    pub accepted_items: Vec<MemoryReviewItem>,
    pub stale_items: Vec<MemoryReviewItem>,
    pub proposed_items: Vec<MemoryReviewItem>,
    pub rejected_items: Vec<MemoryReviewItem>,
    pub lifecycle_items: Vec<MemoryReviewItem>,
}

impl MemoryReviewReport {
    pub fn format(&self) -> String {
        let mut lines = vec![
            "Typed records:".to_string(),
            format!(
                "  total={} accepted={} proposed={} rejected={} archived={} superseded={} stale={} missing_evidence={} projection_drift={} used={}",
                self.summary.total,
                self.summary.accepted,
                self.summary.proposed,
                self.summary.rejected,
                self.summary.archived,
                self.summary.superseded,
                self.summary.stale,
                self.summary.missing_evidence,
                self.summary.projection_drift,
                self.summary.used
            ),
            "".to_string(),
            format_review_section("Review queue", &self.review_items),
            format_review_section("Accepted records", &self.accepted_items),
            format_review_section("Stale accepted records", &self.stale_items),
            format_review_section("Proposed records", &self.proposed_items),
            format_review_section("Rejected records", &self.rejected_items),
            format_review_section("Lifecycle records", &self.lifecycle_items),
        ];
        lines.retain(|line| !line.is_empty());
        lines.join("\n")
    }
}

fn format_review_section(title: &str, items: &[MemoryReviewItem]) -> String {
    let mut lines = vec![format!("{title}:")];
    if items.is_empty() {
        lines.push("  none".to_string());
        return lines.join("\n");
    }
    for item in items {
        lines.push(format!(
            "  - {} [{} {}] scope={} evidence={} freshness={} projection={} updated={} :: {}",
            item.id,
            item.status,
            item.kind,
            item.scope,
            item.evidence,
            item.freshness,
            item.projection,
            item.updated_at,
            item.summary
        ));
    }
    lines.join("\n")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryFlushReason {
    SessionEnd,
    Exit,
    Clear,
    ResumeSwitch,
    PreCompress,
    Manual,
}

impl std::fmt::Display for MemoryFlushReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            MemoryFlushReason::SessionEnd => "session_end",
            MemoryFlushReason::Exit => "exit",
            MemoryFlushReason::Clear => "clear",
            MemoryFlushReason::ResumeSwitch => "resume_switch",
            MemoryFlushReason::PreCompress => "pre_compress",
            MemoryFlushReason::Manual => "manual",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryFlushStatus {
    Pending,
    Running,
    Completed,
    Failed,
    SkippedDuplicate,
    SkippedReviewOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFlushRecord {
    pub id: String,
    pub session_id: String,
    pub reason: MemoryFlushReason,
    pub status: MemoryFlushStatus,
    pub attempts: u8,
    pub max_attempts: u8,
    pub message_count: usize,
    pub messages_hash: u64,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryFlushSummary {
    pub pending: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped_duplicate: usize,
    pub skipped_review_only: usize,
    pub total: usize,
}

impl MemoryFlushSummary {
    pub fn format(&self) -> String {
        format!(
            "Memory Flushes:\n  Completed: {}\n  Pending: {}\n  Running: {}\n  Failed: {}\n  Skipped duplicate: {}\n  Skipped review-only: {}\n  Total: {}",
            self.completed,
            self.pending,
            self.running,
            self.failed,
            self.skipped_duplicate,
            self.skipped_review_only,
            self.total
        )
    }
}

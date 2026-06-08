//! 记忆持久化管道
//!
//! 会话结束时的 flush、长期记忆文件维护、typed memory records 生命周期管理。
//! 所有方法都是 `MemoryManager` 的编排层。

use super::files::{collect_memory_file_paths, maintain_memory_file, MemoryFileLock};
use super::manager::{
    memory_flush_records_from_jsonl, memory_messages_hash, record_needs_revalidation,
    MemoryDecisionEvent, MemoryFlushReason, MemoryFlushRecord, MemoryFlushStatus,
    MemoryMaintenanceReport, MemoryManager, MAX_LEARNINGS_PER_SESSION_EXTRACT,
    MEMORY_FLUSH_MAX_ATTEMPTS,
};
use super::types::MemoryStatus;
use crate::memory::extraction::extract_session_learnings;
use crate::services::api::Message;
use std::io::Write;
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// impl MemoryManager 方法
// ---------------------------------------------------------------------------

impl MemoryManager {
    /// 会话结束时批量提取学习内容（同步版本）
    pub fn flush_session(&mut self, messages: &[Message]) {
        if !Self::legacy_auto_memory_write_enabled() {
            debug!("Skipping direct memory flush in review-only default policy");
            return;
        }
        self.flush_session_unchecked(messages);
    }

    fn flush_session_unchecked(&mut self, messages: &[Message]) {
        let session_learnings = extract_session_learnings(messages);
        self.ingest_learnings(session_learnings, MAX_LEARNINGS_PER_SESSION_EXTRACT);

        let pending: Vec<String> = self.pending_learnings.drain(..).collect();
        if !pending.is_empty() {
            info!("Flushing {} learnings from session", pending.len());
            for learning in &pending {
                self.add_auto_learning(learning, "learned");
            }
        }
    }

    /// 会话结束时批量提取学习内容（异步版本）
    pub async fn flush_session_async(&mut self, messages: &[Message]) {
        if !Self::legacy_auto_memory_write_enabled() {
            debug!("Skipping direct async memory flush in review-only default policy");
            return;
        }
        self.flush_session_async_unchecked(messages).await;
    }

    async fn flush_session_async_unchecked(&mut self, messages: &[Message]) {
        let session_learnings = extract_session_learnings(messages);
        self.ingest_learnings(session_learnings, MAX_LEARNINGS_PER_SESSION_EXTRACT);

        let pending: Vec<String> = self.pending_learnings.drain(..).collect();
        if !pending.is_empty() {
            info!("Flushing {} learnings from session (async)", pending.len());
            for learning in &pending {
                self.add_auto_learning_async(learning, "learned").await;
            }
        }
    }

    pub fn flush_session_with_reason(
        &mut self,
        session_id: impl Into<String>,
        reason: MemoryFlushReason,
        messages: &[Message],
    ) -> MemoryFlushRecord {
        let session_id = session_id.into();
        let mut record = self.new_flush_record(session_id, reason, messages);
        if self.has_completed_flush(&record) {
            record.status = MemoryFlushStatus::SkippedDuplicate;
            record.completed_at = Some(chrono::Utc::now().to_rfc3339());
            record.updated_at = record.completed_at.clone().unwrap_or(record.updated_at);
            self.append_flush_record(&record);
            return record;
        }

        if !Self::session_flush_can_persist(reason) {
            record.status = MemoryFlushStatus::SkippedReviewOnly;
            record.completed_at = Some(chrono::Utc::now().to_rfc3339());
            record.updated_at = record.completed_at.clone().unwrap_or(record.updated_at);
            self.append_flush_record(&record);
            return record;
        }

        self.append_flush_record(&record);
        record.status = MemoryFlushStatus::Running;
        record.attempts = 1;
        record.updated_at = chrono::Utc::now().to_rfc3339();
        self.append_flush_record(&record);

        self.flush_session_unchecked(messages);

        record.status = MemoryFlushStatus::Completed;
        record.completed_at = Some(chrono::Utc::now().to_rfc3339());
        record.updated_at = record.completed_at.clone().unwrap_or(record.updated_at);
        self.append_flush_record(&record);
        record
    }

    pub async fn flush_session_with_reason_async(
        &mut self,
        session_id: impl Into<String>,
        reason: MemoryFlushReason,
        messages: &[Message],
    ) -> MemoryFlushRecord {
        let session_id = session_id.into();
        let mut record = self.new_flush_record(session_id, reason, messages);
        if self.has_completed_flush(&record) {
            record.status = MemoryFlushStatus::SkippedDuplicate;
            record.completed_at = Some(chrono::Utc::now().to_rfc3339());
            record.updated_at = record.completed_at.clone().unwrap_or(record.updated_at);
            self.append_flush_record(&record);
            return record;
        }

        if !Self::session_flush_can_persist(reason) {
            record.status = MemoryFlushStatus::SkippedReviewOnly;
            record.completed_at = Some(chrono::Utc::now().to_rfc3339());
            record.updated_at = record.completed_at.clone().unwrap_or(record.updated_at);
            self.append_flush_record(&record);
            return record;
        }

        self.append_flush_record(&record);
        record.status = MemoryFlushStatus::Running;
        record.attempts = 1;
        record.updated_at = chrono::Utc::now().to_rfc3339();
        self.append_flush_record(&record);

        self.flush_session_async_unchecked(messages).await;

        record.status = MemoryFlushStatus::Completed;
        record.completed_at = Some(chrono::Utc::now().to_rfc3339());
        record.updated_at = record.completed_at.clone().unwrap_or(record.updated_at);
        self.append_flush_record(&record);
        record
    }

    fn session_flush_can_persist(reason: MemoryFlushReason) -> bool {
        if matches!(reason, MemoryFlushReason::Manual) {
            return true;
        }
        Self::legacy_auto_memory_write_enabled()
    }

    fn legacy_auto_memory_write_enabled() -> bool {
        matches!(
            std::env::var("PRIORITY_AGENT_AUTO_MEMORY_WRITE")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "legacy" | "unsafe" | "all" | "1" | "true" | "on"
        )
    }

    /// 维护长期记忆文件：去重 section，必要时归档过大的主题文件。
    pub fn maintain_memory(&self) -> MemoryMaintenanceReport {
        let mut report = MemoryMaintenanceReport::default();
        let mut paths = vec![self.memory_path.clone(), self.user_path.clone()];
        paths.extend(collect_memory_file_paths(&self.memory_dir, false));

        for path in paths {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            if content.trim().is_empty() {
                continue;
            }

            report.files_scanned += 1;
            let is_topic_file = path.starts_with(&self.memory_dir)
                && !path
                    .strip_prefix(&self.memory_dir)
                    .map(|p| p.starts_with("archive"))
                    .unwrap_or(false);
            let result = maintain_memory_file(&path, &content, is_topic_file, &self.memory_dir);

            match result {
                Ok(file_report) => {
                    report.duplicate_sections_removed += file_report.duplicates_removed;
                    if file_report.compacted {
                        report.files_compacted += 1;
                    }
                    if file_report.archived {
                        report.archives_created += 1;
                    }
                }
                Err(e) => debug!("Failed to maintain memory file {}: {}", path.display(), e),
            }
        }

        let record_report = self.maintain_memory_records();
        report.records_scanned = record_report.records_scanned;
        report.records_needing_revalidation = record_report.records_needing_revalidation;
        report.records_archived = record_report.records_archived;

        // 矛盾检测：在维护过程中主动发现可能冲突的记忆记录
        let contradictions = self.contradictions(0.3, 10);
        if !contradictions.is_empty() {
            debug!(
                "Memory maintenance: detected {} potential contradictions among records",
                contradictions.len()
            );
            for pair in &contradictions {
                debug!(
                    "  contradiction score={:.2}: {} vs {} (shared: {:?})",
                    pair.contradiction_score, pair.record_a, pair.record_b, pair.shared_keywords
                );
            }
        }

        report
    }

    pub(super) fn maintain_memory_records(&self) -> MemoryMaintenanceReport {
        let mut report = MemoryMaintenanceReport::default();
        let mut records = self.memory_records();
        report.records_scanned = records.len();
        if records.is_empty() {
            return report;
        }

        let now = chrono::Utc::now();
        let archive_cutoff = now - chrono::Duration::days(365);
        let mut changed = false;
        for record in &mut records {
            let previous_stale_after = record.stale_after;
            let previous_expires_at = record.expires_at;
            record.apply_default_lifecycle();
            if record.stale_after != previous_stale_after
                || record.expires_at != previous_expires_at
            {
                record.updated_at = now;
                changed = true;
            }
            if matches!(record.status, MemoryStatus::Accepted) && record.is_expired_at(now) {
                record.status = MemoryStatus::Archived;
                record.updated_at = now;
                report.records_archived += 1;
                changed = true;
                continue;
            }
            if record_needs_revalidation(record) {
                report.records_needing_revalidation += 1;
                if !record.tags.iter().any(|tag| tag == "needs_revalidation") {
                    record.tags.push("needs_revalidation".to_string());
                    record.tags.sort();
                    record.tags.dedup();
                    record.updated_at = now;
                    changed = true;
                }
            }
            if matches!(record.status, MemoryStatus::Accepted)
                && record.use_count == 0
                && record.created_at < archive_cutoff
                && record.confidence < 0.55
            {
                record.status = MemoryStatus::Archived;
                record.updated_at = now;
                report.records_archived += 1;
                changed = true;
            }
        }

        if changed {
            if let Err(error) = self.provider_registry.replace_local_memory_records(
                &records,
                "maintenance",
                "maintain typed memory records",
            ) {
                debug!("Failed to maintain typed memory records: {}", error);
            }
        }
        report
    }

    fn new_flush_record(
        &self,
        session_id: String,
        reason: MemoryFlushReason,
        messages: &[Message],
    ) -> MemoryFlushRecord {
        let now = chrono::Utc::now().to_rfc3339();
        MemoryFlushRecord {
            id: format!("flush_{}", uuid::Uuid::new_v4().simple()),
            session_id,
            reason,
            status: MemoryFlushStatus::Pending,
            attempts: 0,
            max_attempts: MEMORY_FLUSH_MAX_ATTEMPTS,
            message_count: messages.len(),
            messages_hash: memory_messages_hash(messages),
            error: None,
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
        }
    }

    fn has_completed_flush(&self, candidate: &MemoryFlushRecord) -> bool {
        let content = std::fs::read_to_string(&self.flush_log_path).unwrap_or_default();
        memory_flush_records_from_jsonl(&content)
            .values()
            .any(|record| {
                record.session_id == candidate.session_id
                    && record.reason == candidate.reason
                    && record.messages_hash == candidate.messages_hash
                    && matches!(
                        record.status,
                        MemoryFlushStatus::Completed | MemoryFlushStatus::SkippedReviewOnly
                    )
            })
    }

    fn append_flush_record(&self, record: &MemoryFlushRecord) {
        if let Some(parent) = self.flush_log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(line) = serde_json::to_string(record) else {
            return;
        };
        let _guard = MemoryFileLock::acquire(&self.flush_log_path).ok();
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.flush_log_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", line);
            }
            Err(e) => debug!("Failed to record memory flush: {}", e),
        }
    }

    pub(super) fn record_memory_decision_event(&self, event: MemoryDecisionEvent) {
        if let Some(parent) = self.decision_log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(line) = serde_json::to_string(&event) else {
            return;
        };
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.decision_log_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", line);
            }
            Err(e) => debug!("Failed to record memory decision: {}", e),
        }
    }
}

//! Memory manager submit functions.
//!
//! Functions for submitting memory candidates.

use super::helpers::{
    default_candidate_evidence, evidence_status, has_required_evidence, infer_memory_importance,
    kind_label, memory_decision_event, memory_lifecycle_key, memory_scope_label,
    normalized_contains, record_has_verified_evidence, record_needs_revalidation,
    requires_verified_evidence, status_label,
};
use super::MemoryManager;
use crate::memory::extraction::infer_memory_tags;
use crate::memory::files::{infer_learning_topic, topic_memory_path, write_memory_file_atomically};
use crate::memory::provider::{LocalMemoryRecordWriteStatus, MemoryOperationJournalEntry};
use crate::memory::quality::assess_memory_candidate;
use crate::memory::reports::{MemoryWriteOutcome, MemoryWriteScoringTrace, MemoryWriteTarget};
use crate::memory::types::{
    MemoryCandidate, MemoryEvidenceKind, MemoryKind, MemoryProjection, MemoryProvenance,
    MemoryRecord, MemoryStatus,
};
use std::path::{Path, PathBuf};
use tracing::debug;

fn markdown_entry_for_record(record: &MemoryRecord, category: &str) -> String {
    let kind_label = kind_label(record.kind);
    let scope_label = memory_scope_label(&record.scope);
    format!(
        "- [{}] {}\n<!-- memory-id: {}; kind: {}; scope: {}; confidence: {:.2}; importance: {} -->\n",
        category.to_uppercase(),
        record.content,
        record.id,
        kind_label,
        scope_label,
        record.confidence,
        record.importance,
    )
}

fn scoring_trace_for_candidate(
    candidate: &MemoryCandidate,
    status: MemoryStatus,
    score: f32,
    threshold: f32,
    duplication: f32,
    reason: impl Into<String>,
) -> MemoryWriteScoringTrace {
    MemoryWriteScoringTrace {
        candidate_id: candidate.id.clone(),
        kind: kind_label(candidate.kind).to_string(),
        status: status_label(status).to_string(),
        score,
        threshold,
        explicit: candidate.explicit,
        duplication,
        reason: reason.into(),
    }
}

impl MemoryManager {
    pub fn candidate_from_content(
        &self,
        content: &str,
        category: &str,
        source: &str,
    ) -> MemoryCandidate {
        let mut candidate = MemoryCandidate::new(
            content,
            category,
            self.active_scope.clone(),
            MemoryProvenance::local(source),
        )
        .with_tags(infer_memory_tags(content, category))
        .importance(infer_memory_importance(content, category));
        candidate.evidence = default_candidate_evidence(&candidate);
        candidate
    }

    pub fn submit_candidate(
        &self,
        mut candidate: MemoryCandidate,
        target: MemoryWriteTarget,
    ) -> MemoryWriteOutcome {
        if candidate.evidence.is_empty() {
            candidate.evidence = default_candidate_evidence(&candidate);
        }
        if candidate.tags.is_empty() {
            candidate.tags = infer_memory_tags(&candidate.content, &candidate.category);
        }
        candidate.importance = candidate.importance.max(infer_memory_importance(
            &candidate.content,
            &candidate.category,
        ));

        let (path, projection_path) = match self.path_for_candidate(&candidate, target) {
            Ok(path) => path,
            Err(reason) => return MemoryWriteOutcome::invalid_target(reason),
        };
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let assessment = match assess_memory_candidate(
            &candidate.content,
            &candidate.category,
            &existing,
            candidate.explicit,
        ) {
            Ok(assessment) => assessment,
            Err(issue) => {
                let _ = self.provider_registry.record_local_memory_operation(
                    MemoryOperationJournalEntry::new(
                        "unsafe_skip",
                        None,
                        Some(candidate.id.clone()),
                        "blocked",
                        format!("{}: {}", issue.code, issue.message),
                        0,
                    ),
                );
                self.record_memory_decision_event(memory_decision_event(
                    "blocked",
                    &candidate,
                    None,
                    &format!("{}: {}", issue.code, issue.message),
                    "blocked",
                ));
                return MemoryWriteOutcome::blocked(format!("{}: {}", issue.code, issue.message));
            }
        };

        if assessment.duplication >= 0.85 || normalized_contains(&existing, &candidate.content) {
            let scoring_trace = MemoryWriteScoringTrace {
                status: "duplicate".to_string(),
                ..scoring_trace_for_candidate(
                    &candidate,
                    assessment.status,
                    assessment.score,
                    assessment.threshold,
                    assessment.duplication,
                    format!("duplicate memory already exists; {}", assessment.reason),
                )
            };
            self.record_memory_decision_event(memory_decision_event(
                "duplicate",
                &candidate,
                Some(assessment.score),
                &format!("duplicate memory already exists; {}", assessment.reason),
                evidence_status(&candidate),
            ));
            return MemoryWriteOutcome::duplicate(
                path,
                format!("duplicate memory already exists; {}", assessment.reason),
            )
            .with_scoring_trace(scoring_trace);
        }

        let mut status = assessment.status;
        let mut reason = assessment.reason.clone();
        if status == MemoryStatus::Proposed
            && requires_verified_evidence(candidate.kind)
            && has_required_evidence(&candidate)
            && assessment.score >= 0.55
        {
            status = MemoryStatus::Accepted;
            reason = format!(
                "{}; accepted because kind-appropriate evidence verified the candidate",
                reason
            );
        }
        if status == MemoryStatus::Accepted
            && requires_verified_evidence(candidate.kind)
            && !has_required_evidence(&candidate)
        {
            status = MemoryStatus::Proposed;
            reason = format!(
                "{}; {:?} memory requires kind-appropriate evidence before acceptance",
                reason, candidate.kind
            );
        }
        if status == MemoryStatus::Accepted
            && candidate.kind == MemoryKind::UserPreference
            && !candidate.explicit
            && !candidate
                .evidence
                .iter()
                .any(|evidence| matches!(evidence.kind, MemoryEvidenceKind::UserStatement))
        {
            status = MemoryStatus::Proposed;
            reason = format!(
                "{}; user preference memory requires explicit user-statement evidence",
                reason
            );
        }
        let scoring_trace = scoring_trace_for_candidate(
            &candidate,
            status,
            assessment.score,
            assessment.threshold,
            assessment.duplication,
            reason.clone(),
        );

        let mut record = MemoryRecord::from_candidate(
            candidate.clone(),
            status,
            candidate.confidence.max(assessment.score),
            assessment.future_utility,
            assessment.sensitivity,
        );
        record.projection = Some(MemoryProjection {
            path: projection_path.clone(),
            heading: format!("[{}]", candidate.category.to_uppercase()),
        });

        self.apply_record_lifecycle_before_append(&mut record);

        let write_status = match self.provider_registry.append_local_memory_record(
            &record,
            &record.scope,
            "manager_submit_candidate",
            &reason,
        ) {
            Ok(status) => status,
            Err(error) => {
                return MemoryWriteOutcome::failed(
                    self.records_path.clone(),
                    format!("failed to append typed memory record: {error}"),
                )
                .with_scoring_trace(scoring_trace);
            }
        };
        if write_status == LocalMemoryRecordWriteStatus::Duplicate {
            return MemoryWriteOutcome::duplicate(
                self.records_path.clone(),
                "duplicate typed memory record already exists",
            )
            .with_scoring_trace(MemoryWriteScoringTrace {
                status: "duplicate".to_string(),
                ..scoring_trace
            });
        }

        self.record_memory_decision_event(memory_decision_event(
            status_label(status),
            &candidate,
            Some(assessment.score),
            &reason,
            evidence_status(&candidate),
        ));

        if status != MemoryStatus::Accepted {
            return MemoryWriteOutcome::gated_with_record(record, status, assessment.score, reason)
                .with_scoring_trace(scoring_trace);
        }

        let entry = markdown_entry_for_record(&record, &candidate.category);
        let header = if existing.trim().is_empty() {
            if path == self.user_path.as_path() {
                "# User Preferences\n".to_string()
            } else if path.starts_with(&self.memory_dir) {
                "# Priority Agent Topic Memory\n".to_string()
            } else {
                "# Priority Agent Memory\n".to_string()
            }
        } else {
            String::new()
        };
        let new_content = format!("{}{}{}", existing, header, entry);
        if let Err(error) = write_memory_file_atomically(&path, &new_content) {
            return MemoryWriteOutcome::failed(path, error.to_string())
                .with_scoring_trace(scoring_trace);
        }

        MemoryWriteOutcome::saved_with_record(path, assessment.score, reason, record)
            .with_scoring_trace(scoring_trace)
    }

    pub async fn submit_candidate_with_provider_notifications(
        &self,
        candidate: MemoryCandidate,
        target: MemoryWriteTarget,
    ) -> MemoryWriteOutcome {
        let outcome = self.submit_candidate(candidate, target);
        let Some(record) = outcome.provider_notifiable_record() else {
            return outcome;
        };
        let provider_outcomes = self
            .provider_registry
            .on_memory_write_all(record, &record.scope)
            .await;
        for provider_outcome in provider_outcomes {
            if provider_outcome.status != crate::memory::provider::MemoryProviderCallStatus::Ok {
                debug!(
                    "Memory provider write hook {:?}: provider={} record={} error={:?}",
                    provider_outcome.status,
                    provider_outcome.provider,
                    record.id,
                    provider_outcome.error
                );
            }
        }
        outcome
    }

    fn apply_record_lifecycle_before_append(&self, record: &mut MemoryRecord) {
        if !matches!(record.status, MemoryStatus::Accepted) {
            return;
        }
        let mut records = self.memory_records();
        if records.is_empty() {
            return;
        }
        let record_key = memory_lifecycle_key(record);
        if record_key.is_empty() {
            return;
        }

        let mut changed = false;
        let verified = record_has_verified_evidence(record);
        for existing in &mut records {
            if existing.id == record.id
                || !matches!(
                    existing.status,
                    MemoryStatus::Accepted | MemoryStatus::Proposed
                )
                || existing.kind != record.kind
                || memory_lifecycle_key(existing) != record_key
            {
                continue;
            }

            let supersede = match record.kind {
                MemoryKind::ProjectFact | MemoryKind::ToolQuirk => {
                    verified
                        && (!record_has_verified_evidence(existing)
                            || record_needs_revalidation(existing))
                }
                MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => true,
                _ => false,
            };
            if !supersede {
                continue;
            }

            record.failure_count = record.failure_count.saturating_add(existing.failure_count);
            record.success_count = record.success_count.saturating_add(existing.success_count);
            record.supersedes.push(existing.id.clone());
            existing.status = MemoryStatus::Superseded;
            existing.superseded_by = Some(record.id.clone());
            existing.updated_at = chrono::Utc::now();
            changed = true;
        }

        if changed {
            record.supersedes.sort();
            record.supersedes.dedup();
            if let Err(error) = self.provider_registry.replace_local_memory_records(
                &records,
                "lifecycle_supersede",
                "supersede older lifecycle-equivalent records",
            ) {
                debug!("Failed to update superseded memory records: {}", error);
            }
        }
    }

    fn path_for_candidate(
        &self,
        candidate: &MemoryCandidate,
        target: MemoryWriteTarget,
    ) -> Result<(PathBuf, String), String> {
        let path = match target {
            MemoryWriteTarget::User => self.user_path.clone(),
            MemoryWriteTarget::Index => self.memory_path.clone(),
            MemoryWriteTarget::Topic(topic) => topic_memory_path(&self.memory_dir, &topic)
                .ok_or_else(|| format!("invalid topic '{}'", topic))?,
            MemoryWriteTarget::Auto => {
                if matches!(candidate.kind, MemoryKind::UserPreference)
                    || matches!(candidate.category.as_str(), "preference" | "user")
                {
                    self.user_path.clone()
                } else if let Some(topic) =
                    infer_learning_topic(&candidate.content, &candidate.category)
                {
                    topic_memory_path(&self.memory_dir, topic)
                        .ok_or_else(|| format!("invalid inferred topic '{}'", topic))?
                } else {
                    self.memory_path.clone()
                }
            }
        };
        let projection_path = self.projection_path(&path);
        Ok((path, projection_path))
    }

    pub(super) fn projection_path(&self, path: &Path) -> String {
        if path == self.user_path.as_path() {
            return "USER.md".to_string();
        }
        if path == self.memory_path.as_path() {
            return "MEMORY.md".to_string();
        }
        if let Ok(relative) = path.strip_prefix(&self.memory_dir) {
            return format!("memory/{}", relative.to_string_lossy().replace('\\', "/"));
        }
        path.to_string_lossy().to_string()
    }

    pub(super) fn projection_contains_record(
        &self,
        projection: &MemoryProjection,
        record_id: &str,
    ) -> bool {
        self.provider_registry
            .local_projection_contains_record(projection, record_id)
    }

    pub(super) fn append_record_to_projection_with_backup(
        &self,
        record: &MemoryRecord,
        projection: &MemoryProjection,
    ) -> anyhow::Result<()> {
        self.provider_registry
            .append_local_record_to_projection_with_backup(record, projection)
    }
}

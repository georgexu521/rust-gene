//! Memory manager review functions.
//!
//! Functions for memory review reports and projection repair.

use super::helpers::{
    kind_label, log_preview, memory_scope_label, record_needs_revalidation, status_label,
};
use super::MemoryManager;
use crate::engine::task_contract::{
    MemoryProposal, MemoryProposalCandidate, MemoryProposalReviewStore, MemoryProposalStatus,
};
use crate::memory::reports::{
    memory_record_summary_from_records_with_stale, MemoryRecordSummary, MemoryReviewItem,
    MemoryReviewReport,
};
use crate::memory::types::MemoryStatus;

fn memory_evidence_label(record: &crate::memory::types::MemoryRecord) -> &'static str {
    if record.evidence.is_empty() {
        "missing"
    } else if record.evidence.iter().any(|e| e.confidence >= 0.85) {
        "verified"
    } else if record
        .evidence
        .iter()
        .any(|e| matches!(e.kind, crate::memory::types::MemoryEvidenceKind::Inference))
    {
        "inferred"
    } else {
        "inferred"
    }
}

fn memory_freshness_label(record: &crate::memory::types::MemoryRecord) -> &'static str {
    if record_needs_revalidation(record) {
        "stale"
    } else {
        "fresh"
    }
}

fn memory_projection_label(
    record: &crate::memory::types::MemoryRecord,
    projection_drift: bool,
) -> String {
    if let Some(projection) = &record.projection {
        if projection_drift {
            format!("drift:{}", projection.path)
        } else {
            format!("ok:{}", projection.path)
        }
    } else {
        "none".to_string()
    }
}

fn memory_review_item(
    record: &crate::memory::types::MemoryRecord,
    projection_drift: bool,
) -> MemoryReviewItem {
    MemoryReviewItem {
        id: record.id.clone(),
        status: status_label(record.status).to_string(),
        kind: kind_label(record.kind).to_string(),
        scope: memory_scope_label(&record.scope),
        evidence: memory_evidence_label(record).to_string(),
        freshness: memory_freshness_label(record).to_string(),
        projection: memory_projection_label(record, projection_drift),
        updated_at: record.updated_at.to_rfc3339(),
        summary: log_preview(&record.content, 120),
    }
}

fn truncate_review_items(items: &mut Vec<MemoryReviewItem>, limit: usize) {
    if items.len() > limit {
        items.truncate(limit);
    }
}

fn short_record_id(id: &str) -> String {
    if id.len() > 12 {
        id[..12].to_string()
    } else {
        id.to_string()
    }
}

fn memory_proposal_scope_for_record(record: &crate::memory::types::MemoryRecord) -> String {
    memory_scope_label(&record.scope)
}

fn parse_memory_proposal_evidence_value(evidence: &[String], key: &str) -> Option<String> {
    let prefix = format!("{}:", key);
    evidence
        .iter()
        .find(|line| line.starts_with(&prefix))
        .map(|line| line[prefix.len()..].trim().to_string())
}

fn upsert_memory_repair_proposal(proposal: &MemoryProposal) -> anyhow::Result<()> {
    MemoryProposalReviewStore::default().upsert(proposal)
}

impl MemoryManager {
    pub fn memory_record_summary(&self) -> MemoryRecordSummary {
        let records = self.memory_records();
        let mut summary =
            memory_record_summary_from_records_with_stale(&records, record_needs_revalidation);
        summary.projection_drift = records
            .iter()
            .filter(|record| {
                matches!(record.status, MemoryStatus::Accepted)
                    && record.projection.as_ref().is_some_and(|projection| {
                        !self.projection_contains_record(projection, &record.id)
                    })
            })
            .count();
        summary
    }

    pub fn memory_review_report(&self, limit: usize) -> MemoryReviewReport {
        let records = self.memory_records();
        let summary = self.memory_record_summary();
        let mut review_items = Vec::new();
        let mut accepted_items = Vec::new();
        let mut stale_items = Vec::new();
        let mut proposed_items = Vec::new();
        let mut rejected_items = Vec::new();
        let mut lifecycle_items = Vec::new();

        for record in records.iter().rev() {
            let projection_drift = matches!(record.status, MemoryStatus::Accepted)
                && record.projection.as_ref().is_some_and(|projection| {
                    !self.projection_contains_record(projection, &record.id)
                });
            let item = memory_review_item(record, projection_drift);
            let needs_revalidation = record_needs_revalidation(record);
            if matches!(record.status, MemoryStatus::Proposed)
                || projection_drift
                || needs_revalidation
            {
                review_items.push(item.clone());
            }
            if needs_revalidation {
                stale_items.push(item.clone());
            }
            match record.status {
                MemoryStatus::Proposed => proposed_items.push(item),
                MemoryStatus::Rejected => rejected_items.push(item),
                MemoryStatus::Superseded | MemoryStatus::Archived => lifecycle_items.push(item),
                MemoryStatus::Accepted => accepted_items.push(item),
            }
        }

        truncate_review_items(&mut review_items, limit);
        truncate_review_items(&mut accepted_items, limit);
        truncate_review_items(&mut stale_items, limit);
        truncate_review_items(&mut proposed_items, limit);
        truncate_review_items(&mut rejected_items, limit);
        truncate_review_items(&mut lifecycle_items, limit);

        MemoryReviewReport {
            summary,
            review_items,
            accepted_items,
            stale_items,
            proposed_items,
            rejected_items,
            lifecycle_items,
        }
    }

    pub fn projection_repair_proposals(&self, limit: usize) -> Vec<MemoryProposal> {
        self.memory_records()
            .into_iter()
            .filter(|record| matches!(record.status, MemoryStatus::Accepted))
            .filter(|record| {
                record.projection.as_ref().is_some_and(|projection| {
                    !self.projection_contains_record(projection, &record.id)
                })
            })
            .take(limit)
            .map(|record| {
                let projection_path = record
                    .projection
                    .as_ref()
                    .map(|projection| projection.path.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                MemoryProposal {
                    task_id: format!("repair-projection-{}", short_record_id(&record.id)),
                    source: "repair".to_string(),
                    status: MemoryProposalStatus::Proposed,
                    candidates: vec![MemoryProposalCandidate {
                        kind: kind_label(record.kind).to_string(),
                        scope: memory_proposal_scope_for_record(&record),
                        content: format!(
                            "Repair Markdown projection `{}` for canonical memory `{}`: {}",
                            projection_path,
                            short_record_id(&record.id),
                            log_preview(&record.summary, 180)
                        ),
                        evidence: vec![
                            format!("record_id: {}", record.id),
                            format!("projection: {}", projection_path),
                            "canonical_store: records.jsonl".to_string(),
                            "repair_policy: review_required".to_string(),
                        ],
                    }],
                    write_policy: "review_required".to_string(),
                    write_performed: false,
                    reason:
                        "Markdown projection drift detected; canonical JSONL remains source of truth"
                            .to_string(),
                }
            })
            .collect()
    }

    pub fn upsert_projection_repair_proposals(&self, limit: usize) -> usize {
        self.projection_repair_proposals(limit)
            .into_iter()
            .filter(|proposal| upsert_memory_repair_proposal(proposal).is_ok())
            .count()
    }

    pub fn apply_projection_repair_proposal(
        &self,
        proposal: &MemoryProposal,
    ) -> anyhow::Result<usize> {
        if proposal.source != "repair" {
            return Ok(0);
        }
        let records = self.memory_records();
        let mut applied = 0usize;
        for candidate in &proposal.candidates {
            let Some(record_id) =
                parse_memory_proposal_evidence_value(&candidate.evidence, "record_id")
            else {
                continue;
            };
            let Some(record) = records.iter().find(|record| record.id == record_id) else {
                continue;
            };
            let Some(projection) = record.projection.as_ref() else {
                continue;
            };
            if self.projection_contains_record(projection, &record.id) {
                continue;
            }
            self.append_record_to_projection_with_backup(record, projection)?;
            applied += 1;
        }
        Ok(applied)
    }
}

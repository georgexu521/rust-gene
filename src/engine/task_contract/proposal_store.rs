use super::{
    compact_text, current_memory_proposal_project_identity, infer_proposal_active_scope,
    memory_proposal_candidate_evidence_refs, memory_proposal_conflict_groups,
    memory_proposal_review_operation, memory_proposal_status_reason,
    memory_write_target_for_proposal_candidate, proposal_blocking_minimum_evidence_reason,
    proposal_blocking_sensitivity_reason, proposal_gate_report, stable_memory_proposal_id,
    summarize_memory_proposal_conflicts, MemoryProposal, MemoryProposalReviewRecord,
    MemoryProposalStatus, MemoryProposalStatusHistoryEntry,
};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MemoryProposalReviewStore {
    path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalBatchFilter {
    pub source: Option<String>,
    pub scope: Option<String>,
    pub project: Option<String>,
    pub status: Option<MemoryProposalStatus>,
    pub stale_days: Option<i64>,
    pub duplicate_only: bool,
    pub blocked_only: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalBatchUpdate {
    pub matched: usize,
    pub updated: usize,
    pub proposal_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalBatchApply {
    pub matched: usize,
    pub applied: usize,
    pub applied_candidates: usize,
    pub failed: usize,
    pub proposal_ids: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProposalConflictResolution {
    pub kept_id: String,
    pub accepted_keep: bool,
    pub rejected_ids: Vec<String>,
    pub conflict_groups: usize,
}

impl MemoryProposalReviewStore {
    pub fn default_path() -> PathBuf {
        if let Ok(path) = std::env::var("PRIORITY_AGENT_MEMORY_PROPOSALS_PATH") {
            return PathBuf::from(path);
        }
        if let Some(root) = std::env::var_os("PRIORITY_AGENT_MEMORY_ROOT") {
            return PathBuf::from(root).join("memory_proposals.jsonl");
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("memory_proposals.jsonl")
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(&self) -> Vec<MemoryProposal> {
        self.list_records()
            .into_iter()
            .map(|record| record.proposal)
            .collect()
    }

    pub fn list_records(&self) -> Vec<MemoryProposalReviewRecord> {
        let content = std::fs::read_to_string(&self.path).unwrap_or_default();
        let mut latest = HashMap::<String, MemoryProposalReviewRecord>::new();
        for line in content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Ok(record) = serde_json::from_str::<MemoryProposalReviewRecord>(line) else {
                continue;
            };
            let mut record = record;
            if record.id.trim().is_empty() || record.id == record.proposal.task_id {
                record.id = stable_memory_proposal_id(&record.proposal);
            }
            let key = if record.proposal.task_id.trim().is_empty() {
                record.id.clone()
            } else {
                record.proposal.task_id.clone()
            };
            latest.insert(key, record);
        }
        let mut records = latest.into_values().collect::<Vec<_>>();
        let all_records = records.clone();
        for record in &mut records {
            if record.project_id.is_none() && record.project_labels.is_empty() {
                let (project_id, project_labels) = current_memory_proposal_project_identity();
                record.project_id = project_id;
                record.project_labels = project_labels;
            }
            let conflict_groups = memory_proposal_conflict_groups(&record.proposal, &all_records);
            if record.conflict_groups.is_empty() {
                record.conflict_groups = conflict_groups;
            }
            if record.duplicate_conflict_summary.trim().is_empty()
                || record.duplicate_conflict_summary == "not_checked"
            {
                record.duplicate_conflict_summary =
                    summarize_memory_proposal_conflicts(&record.conflict_groups);
            }
            if !record
                .gate_report
                .iter()
                .any(|gate| gate.gate == "duplicate_conflict")
            {
                record.gate_report =
                    proposal_gate_report(&record.proposal, &record.conflict_groups);
            }
        }
        records.sort_by(|a, b| {
            a.proposal
                .task_id
                .cmp(&b.proposal.task_id)
                .then_with(|| a.id.cmp(&b.id))
        });
        records
    }

    pub fn get(&self, id_or_prefix: &str) -> Option<MemoryProposal> {
        self.get_record(id_or_prefix).map(|record| record.proposal)
    }

    pub fn get_record(&self, id_or_prefix: &str) -> Option<MemoryProposalReviewRecord> {
        self.list_records().into_iter().find(|record| {
            record.id == id_or_prefix
                || record.id.starts_with(id_or_prefix)
                || record.proposal.task_id == id_or_prefix
                || record.proposal.task_id.starts_with(id_or_prefix)
        })
    }

    pub fn upsert(&self, proposal: &MemoryProposal) -> anyhow::Result<()> {
        if proposal.status == MemoryProposalStatus::NotApplicable {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let now = chrono::Utc::now().to_rfc3339();
        let previous = self.get_record(&proposal.task_id);
        let created_at = previous
            .as_ref()
            .map(|record| record.created_at.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| now.clone());
        let mut status_history = previous
            .as_ref()
            .map(|record| record.status_history.clone())
            .unwrap_or_default();
        let should_append_history = status_history
            .last()
            .map(|entry| entry.status != proposal.status || entry.reason != proposal.reason)
            .unwrap_or(true);
        if should_append_history {
            status_history.push(MemoryProposalStatusHistoryEntry {
                at: now.clone(),
                status: proposal.status,
                reason: proposal.reason.clone(),
            });
        }
        let sibling_records = self
            .list_records()
            .into_iter()
            .filter(|record| record.proposal.task_id != proposal.task_id)
            .collect::<Vec<_>>();
        let conflict_groups = memory_proposal_conflict_groups(proposal, &sibling_records);
        let duplicate_conflict_summary = summarize_memory_proposal_conflicts(&conflict_groups);
        let record_id = previous
            .as_ref()
            .map(|record| record.id.clone())
            .filter(|id| !id.trim().is_empty() && id != &proposal.task_id)
            .unwrap_or_else(|| stable_memory_proposal_id(proposal));
        let (default_project_id, default_project_labels) =
            current_memory_proposal_project_identity();
        let project_id = previous
            .as_ref()
            .and_then(|record| record.project_id.clone())
            .or(default_project_id);
        let project_labels = previous
            .as_ref()
            .map(|record| record.project_labels.clone())
            .filter(|labels| !labels.is_empty())
            .unwrap_or(default_project_labels);
        let record = MemoryProposalReviewRecord {
            id: record_id,
            proposal: proposal.clone(),
            created_at,
            updated_at: now,
            source_session: std::env::var("PRIORITY_AGENT_SESSION_ID").ok(),
            source_task: proposal.task_id.clone(),
            source: proposal.source.clone(),
            active_scope: infer_proposal_active_scope(proposal),
            project_id,
            project_labels,
            gate_report: proposal_gate_report(proposal, &conflict_groups),
            duplicate_conflict_summary: if duplicate_conflict_summary != "not_checked" {
                duplicate_conflict_summary
            } else {
                previous
                    .and_then(|record| {
                        if record.duplicate_conflict_summary.trim().is_empty() {
                            None
                        } else {
                            Some(record.duplicate_conflict_summary)
                        }
                    })
                    .unwrap_or_else(|| "not_checked".to_string())
            },
            conflict_groups,
            status_history,
        };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(&record)?)?;
        self.record_review_operation(&record)?;
        Ok(())
    }

    fn record_review_operation(&self, record: &MemoryProposalReviewRecord) -> anyhow::Result<()> {
        let Some(base_dir) = self.review_operation_base_dir() else {
            return Ok(());
        };
        let provider = crate::memory::LocalMemoryProvider::with_base_dir(base_dir);
        provider.append_operation_journal_entry(&crate::memory::MemoryOperationJournalEntry::new(
            memory_proposal_review_operation(record),
            Some(record.id.clone()),
            Some(record.proposal.task_id.clone()),
            record.proposal.status.label(),
            record.proposal.reason.clone(),
            record.proposal.candidates.len(),
        ))
    }

    fn review_operation_base_dir(&self) -> Option<PathBuf> {
        let parent = self.path.parent()?;
        if self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "memory_proposals.jsonl")
        {
            return Some(parent.to_path_buf());
        }
        let stem = self
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("memory_proposals");
        Some(parent.join(format!(".{stem}-review")))
    }

    pub fn update_status(
        &self,
        id_or_prefix: &str,
        status: MemoryProposalStatus,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        proposal.status = status;
        proposal.reason = memory_proposal_status_reason(status).to_string();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn update_status_with_reason(
        &self,
        id_or_prefix: &str,
        status: MemoryProposalStatus,
        reason: impl Into<String>,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        proposal.status = status;
        proposal.reason = reason.into();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn batch_update_status(
        &self,
        filter: MemoryProposalBatchFilter,
        status: MemoryProposalStatus,
        reason: impl Into<String>,
    ) -> anyhow::Result<MemoryProposalBatchUpdate> {
        let reason = reason.into();
        let mut result = MemoryProposalBatchUpdate::default();
        for record in self.list_records() {
            if !memory_proposal_record_matches_filter(&record, &filter) {
                continue;
            }
            result.matched += 1;
            if record.proposal.status == status && record.proposal.reason == reason {
                continue;
            }
            let mut proposal = record.proposal;
            proposal.status = status;
            proposal.reason = reason.clone();
            self.upsert(&proposal)?;
            result.updated += 1;
            result.proposal_ids.push(proposal.task_id);
        }
        Ok(result)
    }

    pub fn batch_apply(
        &self,
        filter: MemoryProposalBatchFilter,
        memory: &mut crate::memory::MemoryManager,
    ) -> anyhow::Result<MemoryProposalBatchApply> {
        let mut filter = filter;
        if filter.status.is_none() {
            filter.status = Some(MemoryProposalStatus::Accepted);
        }
        let mut result = MemoryProposalBatchApply::default();
        for record in self.list_records() {
            if !memory_proposal_record_matches_filter(&record, &filter) {
                continue;
            }
            result.matched += 1;
            match self.apply(&record.proposal.task_id, memory) {
                Ok(Some((proposal, applied_candidates))) => {
                    result.applied += 1;
                    result.applied_candidates += applied_candidates;
                    result.proposal_ids.push(proposal.task_id);
                }
                Ok(None) => {
                    result.failed += 1;
                    result
                        .failures
                        .push(format!("{}: not found", record.proposal.task_id));
                }
                Err(error) => {
                    result.failed += 1;
                    result
                        .failures
                        .push(format!("{}: {}", record.proposal.task_id, error));
                }
            }
        }
        Ok(result)
    }

    pub fn supersede(
        &self,
        old_id_or_prefix: &str,
        new_id_or_prefix: &str,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(new_proposal) = self.get(new_id_or_prefix) else {
            anyhow::bail!(
                "replacement memory proposal '{}' was not found",
                new_id_or_prefix
            );
        };
        self.update_status_with_reason(
            old_id_or_prefix,
            MemoryProposalStatus::Rejected,
            format!("superseded by memory proposal {}", new_proposal.task_id),
        )
    }

    pub fn resolve_conflict_keep(
        &self,
        keep_id_or_prefix: &str,
    ) -> anyhow::Result<Option<MemoryProposalConflictResolution>> {
        let Some(keep_record) = self.get_record(keep_id_or_prefix) else {
            return Ok(None);
        };
        let keep_id = keep_record.id.clone();
        let mut peer_ids = std::collections::BTreeSet::<String>::new();
        for group in &keep_record.conflict_groups {
            for matched in &group.matches {
                if matched.proposal_id != keep_id {
                    peer_ids.insert(matched.proposal_id.clone());
                }
            }
        }

        let mut accepted_keep = false;
        if keep_record.proposal.status != MemoryProposalStatus::Applied
            && keep_record.proposal.status != MemoryProposalStatus::Accepted
        {
            let mut keep = keep_record.proposal.clone();
            keep.status = MemoryProposalStatus::Accepted;
            keep.reason =
                "accepted as the kept memory proposal for duplicate/conflict resolution; apply separately"
                    .to_string();
            self.upsert(&keep)?;
            accepted_keep = true;
        }

        let mut rejected_ids = Vec::new();
        for peer_id in peer_ids {
            let Some(peer) = self.get(&peer_id) else {
                continue;
            };
            if matches!(
                peer.status,
                MemoryProposalStatus::Applied
                    | MemoryProposalStatus::Rejected
                    | MemoryProposalStatus::NotApplicable
            ) {
                continue;
            }
            if let Some(updated) = self.update_status_with_reason(
                &peer_id,
                MemoryProposalStatus::Rejected,
                format!("resolved duplicate/conflict by keeping memory proposal {keep_id}"),
            )? {
                rejected_ids.push(updated.task_id);
            }
        }

        Ok(Some(MemoryProposalConflictResolution {
            kept_id: keep_id,
            accepted_keep,
            rejected_ids,
            conflict_groups: keep_record.conflict_groups.len(),
        }))
    }

    pub fn edit_first_candidate(
        &self,
        id_or_prefix: &str,
        content: impl Into<String>,
    ) -> anyhow::Result<Option<MemoryProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        if proposal.status == MemoryProposalStatus::Applied {
            anyhow::bail!(
                "memory proposal {} is already applied; create a new proposal instead",
                proposal.task_id
            );
        }
        let Some(candidate) = proposal.candidates.first_mut() else {
            anyhow::bail!(
                "memory proposal {} has no editable candidates",
                proposal.task_id
            );
        };
        candidate.content = content.into();
        proposal.status = MemoryProposalStatus::Proposed;
        proposal.write_performed = false;
        proposal.reason =
            "edited candidate content; review and accept again before apply".to_string();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn edit_and_apply(
        &self,
        id_or_prefix: &str,
        content: impl Into<String>,
        memory: &mut crate::memory::MemoryManager,
    ) -> anyhow::Result<Option<(MemoryProposal, usize)>> {
        let Some(mut proposal) = self.edit_first_candidate(id_or_prefix, content)? else {
            return Ok(None);
        };
        proposal.status = MemoryProposalStatus::Accepted;
        proposal.reason = "edited candidate content and accepted for memory apply".to_string();
        self.upsert(&proposal)?;
        self.apply(&proposal.task_id, memory)
    }

    pub fn apply(
        &self,
        id_or_prefix: &str,
        memory: &mut crate::memory::MemoryManager,
    ) -> anyhow::Result<Option<(MemoryProposal, usize)>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        if proposal.status != MemoryProposalStatus::Accepted {
            anyhow::bail!(
                "memory proposal {} is {}; accept it before apply",
                proposal.task_id,
                proposal.status.label()
            );
        }
        if let Some(reason) = proposal_blocking_sensitivity_reason(&proposal) {
            anyhow::bail!(
                "memory proposal {} cannot be applied because sensitivity gate blocked it: {}",
                proposal.task_id,
                reason
            );
        }
        if let Some(reason) = proposal_blocking_minimum_evidence_reason(&proposal) {
            anyhow::bail!(
                "memory proposal {} cannot be applied because minimum evidence gate blocked it: {}",
                proposal.task_id,
                reason
            );
        }
        if proposal.source != "repair" {
            if let Some(reason) = self.proposal_blocking_unresolved_conflict_reason(&proposal) {
                anyhow::bail!(
                    "memory proposal {} cannot be applied because conflict review is unresolved: {}",
                    proposal.task_id,
                    reason
                );
            }
        }
        if proposal.source == "repair" {
            let applied = memory.apply_projection_repair_proposal(&proposal)?;
            proposal.status = MemoryProposalStatus::Applied;
            proposal.write_performed = applied > 0;
            proposal.reason = format!("applied {} projection repair(s)", applied);
            self.upsert(&proposal)?;
            return Ok(Some((proposal, applied)));
        }
        let mut applied = 0usize;
        for candidate in &proposal.candidates {
            let mut memory_candidate = memory
                .candidate_from_content(
                    &candidate.content,
                    &candidate.kind,
                    "memory_proposal_review",
                )
                .explicit(true);
            memory_candidate.evidence = memory_proposal_candidate_evidence_refs(
                &proposal.task_id,
                &proposal.source,
                candidate,
            );
            let target = memory_write_target_for_proposal_candidate(candidate);
            let outcome = memory.submit_candidate(memory_candidate, target);
            if matches!(
                outcome.status,
                crate::memory::manager::MemoryWriteOutcomeStatus::Saved
                    | crate::memory::manager::MemoryWriteOutcomeStatus::Duplicate
            ) {
                applied += 1;
            }
        }
        proposal.status = MemoryProposalStatus::Applied;
        proposal.write_performed = applied > 0;
        proposal.reason = format!("applied {} candidate(s) to long-term memory", applied);
        self.upsert(&proposal)?;
        Ok(Some((proposal, applied)))
    }

    fn proposal_blocking_unresolved_conflict_reason(
        &self,
        proposal: &MemoryProposal,
    ) -> Option<String> {
        let record = self.get_record(&proposal.task_id)?;
        let proposal_id = record.id.clone();
        let blockers = record
            .conflict_groups
            .iter()
            .filter(|group| group.group_type == "conflict")
            .flat_map(|group| {
                group
                    .matches
                    .iter()
                    .filter(|matched| {
                        matched.proposal_id != proposal_id
                            && matches!(
                                matched.status,
                                MemoryProposalStatus::Proposed | MemoryProposalStatus::Accepted
                            )
                    })
                    .map(|matched| {
                        format!(
                            "{}#{}:{}:{}={}",
                            matched.proposal_id,
                            matched.candidate_index + 1,
                            matched.status.label(),
                            group.key,
                            compact_text(&matched.value, 80)
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .take(6)
            .collect::<Vec<_>>();
        if blockers.is_empty() {
            return None;
        }
        Some(format!(
            "{}; review with /memory-proposals conflicts, then run /memory-proposals resolve-conflict {} or reject/edit the conflicting proposal",
            blockers.join(", "),
            proposal.task_id
        ))
    }
}

impl Default for MemoryProposalReviewStore {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

pub(super) fn memory_proposal_record_matches_filter(
    record: &MemoryProposalReviewRecord,
    filter: &MemoryProposalBatchFilter,
) -> bool {
    if let Some(source) = filter.source.as_deref() {
        if record.source != source {
            return false;
        }
    }
    if let Some(scope) = filter.scope.as_deref() {
        let has_scope = record
            .proposal
            .candidates
            .iter()
            .any(|candidate| candidate.scope == scope)
            || record
                .active_scope
                .split(',')
                .any(|item| item.trim() == scope);
        if !has_scope {
            return false;
        }
    }
    if let Some(project) = filter.project.as_deref() {
        if !memory_proposal_record_matches_project(record, project) {
            return false;
        }
    }
    if let Some(status) = filter.status {
        if record.proposal.status != status {
            return false;
        }
    }
    if let Some(days) = filter.stale_days {
        let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(&record.created_at) else {
            return false;
        };
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days.max(0));
        if created_at.with_timezone(&chrono::Utc) > cutoff {
            return false;
        }
    }
    if filter.duplicate_only && !memory_proposal_record_looks_duplicate(record) {
        return false;
    }
    if filter.blocked_only && !memory_proposal_record_is_blocked(record) {
        return false;
    }
    true
}

fn memory_proposal_record_matches_project(
    record: &MemoryProposalReviewRecord,
    project: &str,
) -> bool {
    let needle = project.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }
    record
        .project_id
        .as_deref()
        .map(|id| id.to_ascii_lowercase().contains(&needle))
        .unwrap_or(false)
        || record
            .project_labels
            .iter()
            .any(|label| label.to_ascii_lowercase().contains(&needle))
}

fn memory_proposal_record_is_blocked(record: &MemoryProposalReviewRecord) -> bool {
    record
        .gate_report
        .iter()
        .any(|gate| matches!(gate.status.as_str(), "blocked" | "missing"))
}

fn memory_proposal_record_looks_duplicate(record: &MemoryProposalReviewRecord) -> bool {
    let duplicate_summary = record.duplicate_conflict_summary.to_ascii_lowercase();
    if !duplicate_summary.trim().is_empty()
        && duplicate_summary != "not_checked"
        && (duplicate_summary.contains("duplicate") || duplicate_summary.contains("conflict"))
    {
        return true;
    }
    let reason = record.proposal.reason.to_ascii_lowercase();
    if reason.contains("duplicate") {
        return true;
    }
    record.proposal.candidates.iter().any(|candidate| {
        candidate
            .evidence
            .iter()
            .any(|evidence| evidence.to_ascii_lowercase().contains("duplicate"))
            || candidate
                .content
                .to_ascii_lowercase()
                .contains("duplicate memory")
    })
}

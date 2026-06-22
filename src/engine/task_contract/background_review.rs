//! Task contract support.
//!
//! Models memory proposals, background review, conflicts, and gates as runtime records instead of prompt-only instructions.

use super::{
    compact_text, ExecutionReport, MemoryProposal, MemoryProposalCandidate, MemoryProposalStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundReviewPacket {
    pub transcript_excerpt_ids: Vec<String>,
    pub closeout_summary: String,
    pub tool_result_summaries: Vec<String>,
    pub existing_memory_digest: String,
    pub recent_rejected_proposals: Vec<String>,
    pub active_scope: String,
    pub source_task: String,
    pub max_candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundRejectedObservation {
    pub observation: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundMemoryReviewOutput {
    pub candidates: Vec<MemoryProposalCandidate>,
    pub no_op_reason: Option<String>,
    pub rejected_observations: Vec<BackgroundRejectedObservation>,
}

pub struct BackgroundMemoryReviewWorker;

impl BackgroundReviewPacket {
    pub fn from_execution_report(report: &ExecutionReport, recent: &[MemoryProposal]) -> Self {
        let rejected = recent
            .iter()
            .filter(|proposal| proposal.status == MemoryProposalStatus::Rejected)
            .take(8)
            .map(|proposal| {
                format!(
                    "{}:{}:{}",
                    proposal.task_id,
                    proposal.source,
                    compact_text(&proposal.reason, 140)
                )
            })
            .collect::<Vec<_>>();
        Self {
            transcript_excerpt_ids: vec![report.task_id.clone()],
            closeout_summary: format!(
                "status={} objective={} changed_files={} validation={} risks={} next_steps={}",
                report.status.label(),
                compact_text(&report.objective, 180),
                report.changed_files.len(),
                report.validation_evidence.len(),
                report.risks.len(),
                report.next_steps.len()
            ),
            tool_result_summaries: report
                .validation_evidence
                .iter()
                .take(8)
                .map(|item| compact_text(item, 220))
                .collect(),
            existing_memory_digest: recent
                .iter()
                .take(8)
                .map(|proposal| {
                    format!(
                        "{}:{}:{}",
                        proposal.task_id,
                        proposal.status.label(),
                        proposal.candidate_kinds().join("+")
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
            recent_rejected_proposals: rejected,
            active_scope: "project".to_string(),
            source_task: report.task_id.clone(),
            max_candidate_count: 3,
        }
    }
}

impl BackgroundMemoryReviewOutput {
    pub fn strict_from_json(text: &str) -> anyhow::Result<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            anyhow::bail!("background memory review output is empty");
        }
        let output: Self = serde_json::from_str(trimmed)?;
        output.validate()?;
        Ok(output)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.candidates.is_empty()
            && self
                .no_op_reason
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
        {
            anyhow::bail!(
                "background memory review output must include candidates or no_op_reason"
            );
        }
        for candidate in &self.candidates {
            if candidate.kind.trim().is_empty()
                || candidate.scope.trim().is_empty()
                || candidate.content.trim().is_empty()
            {
                anyhow::bail!(
                    "background memory review candidate must include kind, scope, and content"
                );
            }
            if candidate.evidence.is_empty() {
                anyhow::bail!("background memory review candidate must include evidence");
            }
        }
        Ok(())
    }
}

impl BackgroundMemoryReviewWorker {
    pub fn review_execution_report(
        packet: &BackgroundReviewPacket,
        report: &ExecutionReport,
    ) -> BackgroundMemoryReviewOutput {
        let mut candidates = Vec::new();
        let mut rejected_observations = Vec::new();

        for next_step in report.next_steps.iter().take(packet.max_candidate_count) {
            candidates.push(MemoryProposalCandidate {
                kind: "next_step".to_string(),
                scope: packet.active_scope.clone(),
                content: format!(
                    "Next step after `{}`: {}",
                    compact_text(&report.objective, 140),
                    compact_text(next_step, 220)
                ),
                evidence: vec![
                    format!("source_task: {}", packet.source_task),
                    format!("closeout: {}", packet.closeout_summary),
                    format!("next_step: {}", compact_text(next_step, 220)),
                ],
            });
        }

        for risk in report
            .risks
            .iter()
            .filter(|risk| risk.as_str() != "none recorded")
            .take(packet.max_candidate_count.saturating_sub(candidates.len()))
        {
            candidates.push(MemoryProposalCandidate {
                kind: "open_risk".to_string(),
                scope: packet.active_scope.clone(),
                content: format!(
                    "Open risk after `{}`: {}",
                    compact_text(&report.objective, 140),
                    compact_text(risk, 220)
                ),
                evidence: vec![
                    format!("source_task: {}", packet.source_task),
                    format!("closeout: {}", packet.closeout_summary),
                    format!("risk: {}", compact_text(risk, 220)),
                ],
            });
        }

        if report.validation_evidence.is_empty() {
            rejected_observations.push(BackgroundRejectedObservation {
                observation: "validation_baseline".to_string(),
                reason: "no validation evidence in closeout packet".to_string(),
            });
        } else if candidates.len() < packet.max_candidate_count {
            candidates.push(MemoryProposalCandidate {
                kind: "validation_baseline".to_string(),
                scope: packet.active_scope.clone(),
                content: format!(
                    "Validation baseline for `{}`: {}",
                    compact_text(&report.objective, 140),
                    compact_text(&report.validation_evidence.join("; "), 260)
                ),
                evidence: std::iter::once(format!("source_task: {}", packet.source_task))
                    .chain(std::iter::once(format!(
                        "closeout: {}",
                        packet.closeout_summary
                    )))
                    .chain(packet.tool_result_summaries.clone())
                    .collect(),
            });
        }

        let no_op_reason = if candidates.is_empty() {
            Some("closeout packet did not contain durable project progress candidates".to_string())
        } else {
            None
        };
        BackgroundMemoryReviewOutput {
            candidates,
            no_op_reason,
            rejected_observations,
        }
    }

    pub fn proposal_from_output(
        packet: &BackgroundReviewPacket,
        output: BackgroundMemoryReviewOutput,
    ) -> MemoryProposal {
        let status = if output.candidates.is_empty() {
            MemoryProposalStatus::NotApplicable
        } else {
            MemoryProposalStatus::Proposed
        };
        let reason = if let Some(no_op) = output.no_op_reason {
            no_op
        } else {
            "background review produced review-required memory proposal candidates".to_string()
        };
        MemoryProposal {
            task_id: format!("background-{}", packet.source_task),
            source: "background".to_string(),
            status,
            candidates: output.candidates,
            write_policy: "review_required".to_string(),
            write_performed: false,
            reason,
        }
    }
}

//! Memory evaluation workflow support.
//!
//! Runs memory review scenarios without mutating the production memory store directly.

use super::{fail, pass, temp_eval_dir, MemoryEvalFailureOwner, MemoryEvalResult};
use crate::engine::task_contract::{
    BackgroundMemoryReviewWorker, BackgroundReviewPacket, ExecutionReport, ExecutionReportStatus,
    MemoryProposalReviewStore, MemoryProposalStatus,
};

pub(super) fn eval_background_review_proposal_only() -> MemoryEvalResult {
    let report = ExecutionReport {
        task_id: "memory-eval-background".to_string(),
        objective: "verify background memory review".to_string(),
        status: ExecutionReportStatus::Success,
        changed_files: vec!["src/memory/eval.rs".to_string()],
        validation_evidence: vec!["cargo test -q memory_eval passed".to_string()],
        risks: Vec::new(),
        next_steps: vec!["review proposal queue".to_string()],
        assumptions: Vec::new(),
    };
    let packet = BackgroundReviewPacket::from_execution_report(&report, &[]);
    let output = BackgroundMemoryReviewWorker::review_execution_report(&packet, &report);
    let proposal = BackgroundMemoryReviewWorker::proposal_from_output(&packet, output);
    if proposal.source == "background"
        && proposal.status == MemoryProposalStatus::Proposed
        && proposal.write_policy == "review_required"
        && !proposal.write_performed
        && !proposal.candidates.is_empty()
    {
        pass(
            "background_review_proposal_only",
            "background_review",
            "background review produced review-required proposal without durable write",
        )
    } else {
        fail(
            "background_review_proposal_only",
            "background_review",
            MemoryEvalFailureOwner::Framework,
            "background review did not preserve proposal-only write boundary",
        )
    }
}

pub(super) fn eval_background_review_multi_session_quality() -> MemoryEvalResult {
    let reports = vec![
        ExecutionReport {
            task_id: "memory-eval-session-a".to_string(),
            objective: "harden memory doctor observability".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/tools/memory_tool/mod.rs".to_string()],
            validation_evidence: vec![
                "cargo test -q memory_doctor passed".to_string(),
                "cargo clippy --all-features -- -D warnings passed".to_string(),
            ],
            risks: vec!["doctor output still needs real multi-session eval coverage".to_string()],
            next_steps: vec!["add background review multi-session fixture".to_string()],
            assumptions: Vec::new(),
        },
        ExecutionReport {
            task_id: "memory-eval-session-b".to_string(),
            objective: "add project progress retrieval eval".to_string(),
            status: ExecutionReportStatus::Partial,
            changed_files: vec!["src/memory/eval.rs".to_string()],
            validation_evidence: vec!["cargo test -q memory_eval passed".to_string()],
            risks: Vec::new(),
            next_steps: vec!["rerun full cargo test before closeout".to_string()],
            assumptions: Vec::new(),
        },
        ExecutionReport {
            task_id: "memory-eval-session-c".to_string(),
            objective: "investigate memory migration failure".to_string(),
            status: ExecutionReportStatus::NotVerified,
            changed_files: Vec::new(),
            validation_evidence: Vec::new(),
            risks: vec!["migration rollback command has not been verified".to_string()],
            next_steps: Vec::new(),
            assumptions: Vec::new(),
        },
        ExecutionReport {
            task_id: "memory-eval-session-d".to_string(),
            objective: "answer a one-off memory question".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: Vec::new(),
            validation_evidence: Vec::new(),
            risks: Vec::new(),
            next_steps: Vec::new(),
            assumptions: Vec::new(),
        },
    ];
    let base = temp_eval_dir("background-multi-session");
    let store = MemoryProposalReviewStore::new(base.join("memory_proposals.jsonl"));
    let mut recent = Vec::new();
    let mut proposals = Vec::new();
    let mut no_validation_rejected = false;
    let mut no_op_seen = false;

    for report in &reports {
        let packet = BackgroundReviewPacket::from_execution_report(report, &recent);
        let output = BackgroundMemoryReviewWorker::review_execution_report(&packet, report);
        if report.validation_evidence.is_empty()
            && output.rejected_observations.iter().any(|observation| {
                observation.observation == "validation_baseline"
                    && observation.reason.contains("no validation evidence")
            })
        {
            no_validation_rejected = true;
        }
        if output.candidates.is_empty() && output.no_op_reason.is_some() {
            no_op_seen = true;
        }
        let proposal = BackgroundMemoryReviewWorker::proposal_from_output(&packet, output);
        if let Err(error) = store.upsert(&proposal) {
            let _ = std::fs::remove_dir_all(&base);
            return fail(
                "background_review_multi_session_quality",
                "background_review",
                MemoryEvalFailureOwner::TestHarness,
                format!("failed to write proposal fixture: {error}"),
            );
        }
        if proposal.status != MemoryProposalStatus::NotApplicable {
            recent.push(proposal.clone());
            proposals.push(proposal);
        }
    }

    let records = store.list_records();
    let _ = std::fs::remove_dir_all(&base);
    let proposal_ids = proposals
        .iter()
        .map(|proposal| proposal.task_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let candidate_kinds = proposals
        .iter()
        .flat_map(|proposal| {
            proposal
                .candidates
                .iter()
                .map(|candidate| candidate.kind.as_str())
        })
        .collect::<std::collections::HashSet<_>>();
    let proposal_only = proposals.iter().all(|proposal| {
        proposal.source == "background"
            && proposal.status == MemoryProposalStatus::Proposed
            && proposal.write_policy == "review_required"
            && !proposal.write_performed
            && proposal.candidates.len() <= 3
    });
    let evidence_bound =
        proposals.iter().all(|proposal| {
            let source_task = proposal
                .task_id
                .strip_prefix("background-")
                .unwrap_or(proposal.task_id.as_str());
            proposal.candidates.iter().all(|candidate| {
                candidate.evidence.iter().any(|evidence| {
                    evidence.contains("source_task:") && evidence.contains(source_task)
                }) && candidate
                    .evidence
                    .iter()
                    .any(|evidence| evidence.contains("closeout:"))
            })
        });
    let store_preserved_all = records.len() == proposals.len()
        && records
            .iter()
            .all(|record| record.source == "background" && record.proposal.source == "background");

    if proposals.len() == 3
        && proposal_ids.len() == proposals.len()
        && candidate_kinds.contains("next_step")
        && candidate_kinds.contains("open_risk")
        && candidate_kinds.contains("validation_baseline")
        && proposal_only
        && evidence_bound
        && no_validation_rejected
        && no_op_seen
        && store_preserved_all
    {
        pass(
            "background_review_multi_session_quality",
            "background_review",
            "multi-session fixture keeps background review proposal-only, evidence-bound, unique, and no-op aware",
        )
    } else {
        fail(
            "background_review_multi_session_quality",
            "background_review",
            MemoryEvalFailureOwner::Framework,
            format!(
                "proposals={} unique_ids={} kinds={:?} proposal_only={proposal_only} evidence_bound={evidence_bound} no_validation_rejected={no_validation_rejected} no_op_seen={no_op_seen} store_preserved_all={store_preserved_all}",
                proposals.len(),
                proposal_ids.len(),
                candidate_kinds
            ),
        )
    }
}

//! JSON rendering helpers for memory diagnostics.
//!
//! The JSON form is used by tooling and tests that need stable fields instead
//! of the compact human-readable report.

use super::doctor_types::{
    MemoryCalibrationReportJson, MemoryDecisionCountsJson, MemoryDoctorDiagnostics,
    MemoryDoctorDocumentsJson, MemoryDoctorJson, MemoryFlushCountsJson, MemoryMaintenanceJson,
    MemoryProviderLifecyclePanelJson, MemoryQualityGatesJson, MemoryRecordSummaryJson,
};
use super::paths::memory_root;
use super::{load_memory_doctor_diagnostics, memory_maintenance_decisions, MemoryDocument};

pub(super) fn memory_doctor_json(
    docs: &[MemoryDocument],
    conflicts: &[String],
    provider_lifecycle: &MemoryProviderLifecyclePanelJson,
    snapshot: &crate::memory::MemorySnapshotReport,
) -> serde_json::Value {
    memory_doctor_json_with_reports(
        docs,
        conflicts,
        provider_lifecycle,
        snapshot,
        crate::memory::run_memory_calibration_samples(),
        crate::memory::run_memory_eval_suite(),
        load_memory_doctor_diagnostics(),
    )
}

pub(super) fn memory_doctor_json_with_reports(
    docs: &[MemoryDocument],
    conflicts: &[String],
    provider_lifecycle: &MemoryProviderLifecyclePanelJson,
    snapshot: &crate::memory::MemorySnapshotReport,
    calibration: Vec<crate::memory::MemoryCalibrationResult>,
    eval_suite: crate::memory::MemoryEvalReport,
    diagnostics: MemoryDoctorDiagnostics,
) -> serde_json::Value {
    let MemoryDoctorDiagnostics {
        counts,
        flushes,
        operation_journal,
        proposal_queue,
        last_background_review,
        last_retrieval_trace,
        record_summary,
        store_paths,
    } = diagnostics;
    let calibration_passed = calibration.iter().filter(|result| result.passed).count();
    let total_chars: usize = docs.iter().map(|doc| doc.content.chars().count()).sum();
    let topic_count = docs.iter().filter(|doc| doc.namespace == "topic").count();
    let agent_count = docs
        .iter()
        .filter(|doc| doc.namespace.starts_with("agent"))
        .count();
    let maintenance = memory_maintenance_decisions(docs, conflicts)
        .into_iter()
        .map(|(path, decision)| MemoryMaintenanceJson {
            path,
            score: decision.score,
            action: format!("{:?}", decision.action),
            reason: decision.reason,
        })
        .collect();
    let report = MemoryDoctorJson {
        root: memory_root().display().to_string(),
        contract: crate::memory::MemoryProductContractReport::current(),
        store_paths,
        documents: MemoryDoctorDocumentsJson {
            total: docs.len(),
            topic: topic_count,
            agent: agent_count,
            chars: total_chars,
        },
        snapshot: snapshot.clone(),
        records: MemoryRecordSummaryJson {
            total: record_summary.total,
            accepted: record_summary.accepted,
            proposed: record_summary.proposed,
            rejected: record_summary.rejected,
            archived: record_summary.archived,
            superseded: record_summary.superseded,
            missing_evidence: record_summary.missing_evidence,
            stale: record_summary.stale,
            used: record_summary.used,
            projection_drift: record_summary.projection_drift,
        },
        proposal_queue,
        last_background_review,
        last_retrieval_trace,
        operation_journal,
        provider_lifecycle: provider_lifecycle.clone(),
        decisions: MemoryDecisionCountsJson {
            accepted: counts.accepted,
            proposed: counts.proposed,
            rejected: counts.rejected,
            blocked: counts.blocked,
        },
        flushes: MemoryFlushCountsJson {
            completed: flushes.completed,
            pending: flushes.pending,
            running: flushes.running,
            failed: flushes.failed,
            skipped_duplicate: flushes.skipped_duplicate,
            skipped_review_only: flushes.skipped_review_only,
            total: flushes.total,
        },
        quality_gates: MemoryQualityGatesJson {
            accept_threshold: 0.65,
            propose_threshold: 0.45,
            explicit_override_threshold: 0.60,
            hard_stops: vec!["unsafe_content", "secret_like_content", "duplicate_memory"],
        },
        calibration: MemoryCalibrationReportJson {
            passed: calibration_passed,
            total: calibration.len(),
            results: calibration,
        },
        eval_suite,
        conflicts: conflicts.to_vec(),
        maintenance,
    };
    serde_json::to_value(report).unwrap_or_else(|_| serde_json::json!({}))
}

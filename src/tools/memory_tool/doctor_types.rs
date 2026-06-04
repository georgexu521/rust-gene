use super::paths::MemoryStorePathsJson;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct MemoryDecisionCounts {
    pub(super) accepted: usize,
    pub(super) proposed: usize,
    pub(super) rejected: usize,
    pub(super) blocked: usize,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryDoctorJson {
    pub(super) root: String,
    pub(super) contract: crate::memory::MemoryProductContractReport,
    pub(super) store_paths: MemoryStorePathsJson,
    pub(super) documents: MemoryDoctorDocumentsJson,
    pub(super) snapshot: crate::memory::MemorySnapshotReport,
    pub(super) records: MemoryRecordSummaryJson,
    pub(super) proposal_queue: MemoryProposalQueueJson,
    pub(super) last_background_review: Option<MemoryLastBackgroundReviewJson>,
    pub(super) last_retrieval_trace: Option<MemoryLastRetrievalTraceJson>,
    pub(super) operation_journal: Vec<MemoryOperationJournalJson>,
    pub(super) provider_lifecycle: MemoryProviderLifecyclePanelJson,
    pub(super) decisions: MemoryDecisionCountsJson,
    pub(super) flushes: MemoryFlushCountsJson,
    pub(super) quality_gates: MemoryQualityGatesJson,
    pub(super) calibration: MemoryCalibrationReportJson,
    pub(super) eval_suite: crate::memory::MemoryEvalReport,
    pub(super) conflicts: Vec<String>,
    pub(super) maintenance: Vec<MemoryMaintenanceJson>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryDoctorDocumentsJson {
    pub(super) total: usize,
    pub(super) topic: usize,
    pub(super) agent: usize,
    pub(super) chars: usize,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryRecordSummaryJson {
    pub(super) total: usize,
    pub(super) accepted: usize,
    pub(super) proposed: usize,
    pub(super) rejected: usize,
    pub(super) archived: usize,
    pub(super) superseded: usize,
    pub(super) missing_evidence: usize,
    pub(super) stale: usize,
    pub(super) used: usize,
    pub(super) projection_drift: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct MemoryProposalQueueJson {
    pub(super) total: usize,
    pub(super) proposed: usize,
    pub(super) accepted: usize,
    pub(super) rejected: usize,
    pub(super) applied: usize,
    pub(super) background: usize,
    pub(super) closeout: usize,
    pub(super) conflict_groups: usize,
    pub(super) recent: Vec<MemoryProposalQueueItemJson>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct MemoryProposalQueueItemJson {
    pub(super) id: String,
    pub(super) task_id: String,
    pub(super) status: String,
    pub(super) source: String,
    pub(super) project_id: Option<String>,
    pub(super) candidates: usize,
    pub(super) conflict_groups: usize,
    pub(super) updated_at: String,
    pub(super) reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct MemoryOperationJournalJson {
    pub(super) id: String,
    pub(super) created_at: String,
    pub(super) operation: String,
    pub(super) record_id: Option<String>,
    pub(super) candidate_id: Option<String>,
    pub(super) status: String,
    pub(super) reason: String,
    pub(super) record_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct MemoryLastBackgroundReviewJson {
    pub(super) id: String,
    pub(super) task_id: String,
    pub(super) status: String,
    pub(super) candidates: usize,
    pub(super) candidate_kinds: Vec<String>,
    pub(super) write_policy: String,
    pub(super) write_performed: bool,
    pub(super) conflict_groups: usize,
    pub(super) updated_at: String,
    pub(super) reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct MemoryLastRetrievalTraceJson {
    pub(super) updated_at: String,
    pub(super) created_at: String,
    pub(super) query: String,
    pub(super) policy: crate::engine::intent_router::RetrievalPolicy,
    pub(super) item_count: usize,
    pub(super) token_estimate: usize,
    pub(super) selected_records: usize,
    pub(super) selected_chars: usize,
    pub(super) max_chars: usize,
    pub(super) skipped_unrelated: usize,
    pub(super) skipped_unsafe: usize,
    pub(super) skipped_stale_conflict: usize,
    pub(super) skipped_budget: usize,
    pub(super) skipped_duplicate: usize,
    pub(super) per_scope: Vec<crate::engine::retrieval_context::MemoryRetrievalScopeTrace>,
    pub(super) decisions: Vec<MemoryLastRetrievalDecisionJson>,
    pub(super) selected_items: Vec<MemoryLastRetrievalItemJson>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct MemoryLastRetrievalDecisionJson {
    pub(super) source: String,
    pub(super) scope: String,
    pub(super) action: String,
    pub(super) reason: String,
    pub(super) score: usize,
    pub(super) chars: usize,
    pub(super) score_explanation:
        Option<crate::engine::retrieval_context::MemoryRetrievalScoreExplanation>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct MemoryLastRetrievalItemJson {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) source: crate::engine::retrieval_context::RetrievalSource,
    pub(super) score: f32,
    pub(super) trust: crate::engine::retrieval_context::TrustLevel,
    pub(super) conflict: bool,
    pub(super) reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct MemoryProviderLifecyclePanelJson {
    pub(super) active_scope: String,
    pub(super) providers: Vec<crate::memory::MemoryProviderLifecycleEntry>,
    pub(super) external_provider: Option<String>,
    pub(super) external_mode: String,
    pub(super) lifecycle_hooks: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryDecisionCountsJson {
    pub(super) accepted: usize,
    pub(super) proposed: usize,
    pub(super) rejected: usize,
    pub(super) blocked: usize,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryFlushCountsJson {
    pub(super) completed: usize,
    pub(super) pending: usize,
    pub(super) running: usize,
    pub(super) failed: usize,
    pub(super) skipped_duplicate: usize,
    pub(super) skipped_review_only: usize,
    pub(super) total: usize,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryQualityGatesJson {
    pub(super) accept_threshold: f32,
    pub(super) propose_threshold: f32,
    pub(super) explicit_override_threshold: f32,
    pub(super) hard_stops: Vec<&'static str>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryCalibrationReportJson {
    pub(super) passed: usize,
    pub(super) total: usize,
    pub(super) results: Vec<crate::memory::MemoryCalibrationResult>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct MemoryMaintenanceJson {
    pub(super) path: String,
    pub(super) score: f32,
    pub(super) action: String,
    pub(super) reason: String,
}

#[derive(Debug, Clone)]
pub(super) struct MemoryDoctorDiagnostics {
    pub(super) counts: MemoryDecisionCounts,
    pub(super) flushes: crate::memory::MemoryFlushSummary,
    pub(super) operation_journal: Vec<MemoryOperationJournalJson>,
    pub(super) proposal_queue: MemoryProposalQueueJson,
    pub(super) last_background_review: Option<MemoryLastBackgroundReviewJson>,
    pub(super) last_retrieval_trace: Option<MemoryLastRetrievalTraceJson>,
    pub(super) record_summary: crate::memory::MemoryRecordSummary,
    pub(super) store_paths: MemoryStorePathsJson,
}

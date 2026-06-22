//! Human-readable rendering helpers for memory diagnostics.
//!
//! The rendered text is shown to the model and CLI users, so this module keeps
//! summaries compact while preserving the evidence needed to debug retrieval,
//! proposal, and flush behavior.

use super::doctor_types::{
    MemoryDoctorDiagnostics, MemoryLastBackgroundReviewJson, MemoryLastRetrievalTraceJson,
    MemoryProviderLifecyclePanelJson,
};
use super::paths::{format_memory_store_paths, memory_root};
use super::{
    compact_line, load_memory_doctor_diagnostics, memory_maintenance_decisions, MemoryDocument,
};

pub(super) fn format_last_memory_retrieval_trace(
    trace: Option<&MemoryLastRetrievalTraceJson>,
) -> String {
    let Some(trace) = trace else {
        return "  Last retrieval trace: none\n".to_string();
    };

    let mut out = format!(
        "  Last retrieval trace: query={} · policy={:?} · items={} · selected={} · chars={}/{} · skipped unrelated={} unsafe={} stale_conflict={} budget={} duplicate={} · updated={}\n",
        compact_line(&trace.query, 96),
        trace.policy,
        trace.item_count,
        trace.selected_records,
        trace.selected_chars,
        trace.max_chars,
        trace.skipped_unrelated,
        trace.skipped_unsafe,
        trace.skipped_stale_conflict,
        trace.skipped_budget,
        trace.skipped_duplicate,
        trace.updated_at
    );
    for decision in trace.decisions.iter().take(3) {
        let score = decision
            .score_explanation
            .as_ref()
            .map(|explanation| {
                format!(
                    " final={:.2} scope_match={:.2} pinned_bonus={:.2}",
                    explanation.final_score, explanation.scope_match, explanation.user_pinned_bonus
                )
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "    {} {} scope={} score={} chars={}{} reason={}\n",
            decision.action,
            decision.source,
            decision.scope,
            decision.score,
            decision.chars,
            score,
            compact_line(&decision.reason, 96)
        ));
    }
    out
}

pub(super) fn format_memory_snapshot(snapshot: &crate::memory::MemorySnapshotReport) -> String {
    format!(
        "Pinned Memory Snapshot\n  Status: {}\n  Snapshot id: {}\n  Fingerprint: {}\n  Scope: {}\n  Pinned prompt chars: {}\n  Project chars: {}\n  User chars: {}\n  Memory index files: {} ({} chars)\n  Pinned sources: {}\n  Skipped records: {} (status={} unsafe={} stale={} conflicts={})",
        if snapshot.frozen { "frozen" } else { "live/not frozen" },
        snapshot.snapshot_id,
        snapshot.fingerprint,
        snapshot.scope,
        snapshot.char_count,
        snapshot.project_chars,
        snapshot.user_chars,
        snapshot.memory_file_count,
        snapshot.memory_file_chars,
        format_pinned_sources(&snapshot.pinned_sources),
        snapshot.skipped_record_count,
        snapshot.skipped_status_count,
        snapshot.skipped_unsafe_count,
        snapshot.skipped_stale_count,
        snapshot.skipped_conflict_count
    )
}

fn format_pinned_sources(sources: &[String]) -> String {
    if sources.is_empty() {
        "none".to_string()
    } else {
        sources.join(", ")
    }
}

pub(super) fn format_last_background_review(
    review: Option<&MemoryLastBackgroundReviewJson>,
) -> String {
    let Some(review) = review else {
        return "  Last background review: none\n".to_string();
    };
    format!(
        "  Last background review: {} [{}] candidates={} kinds={} conflicts={} write_policy={} write_performed={} updated={} reason={}\n",
        review.task_id,
        review.status,
        review.candidates,
        if review.candidate_kinds.is_empty() {
            "none".to_string()
        } else {
            review.candidate_kinds.join("+")
        },
        review.conflict_groups,
        review.write_policy,
        review.write_performed,
        review.updated_at,
        review.reason
    )
}

pub(super) fn format_memory_doctor(
    docs: &[MemoryDocument],
    conflicts: &[String],
    provider_lifecycle: &MemoryProviderLifecyclePanelJson,
    snapshot: &crate::memory::MemorySnapshotReport,
) -> String {
    format_memory_doctor_with_reports(
        docs,
        conflicts,
        provider_lifecycle,
        snapshot,
        crate::memory::run_memory_calibration_samples(),
        crate::memory::run_memory_eval_suite(),
        load_memory_doctor_diagnostics(),
    )
}

pub(super) fn format_memory_doctor_with_reports(
    docs: &[MemoryDocument],
    conflicts: &[String],
    provider_lifecycle: &MemoryProviderLifecyclePanelJson,
    snapshot: &crate::memory::MemorySnapshotReport,
    calibration: Vec<crate::memory::MemoryCalibrationResult>,
    eval_suite: crate::memory::MemoryEvalReport,
    diagnostics: MemoryDoctorDiagnostics,
) -> String {
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

    let mut out = String::new();
    out.push_str("Memory Doctor\n");
    out.push_str(&format!("  Root: {}\n", memory_root().display()));
    let contract = crate::memory::MemoryProductContractReport::current();
    out.push_str("  Surfaces:\n");
    out.push_str(&format!(
        "    - pinned_memory: {}\n",
        contract.pinned_memory
    ));
    out.push_str(&format!("    - recall: {}\n", contract.recall));
    out.push_str(&format!(
        "    - learning_proposals: {}\n",
        contract.learning_proposals
    ));
    out.push_str(&format!("    - write_policy: {}\n", contract.write_policy));
    out.push_str(&format_memory_store_paths(&store_paths));
    out.push_str(&format!(
        "  Documents: {} total · {} topic · {} agent · {} chars\n",
        docs.len(),
        topic_count,
        agent_count,
        total_chars
    ));
    out.push_str(&format!(
        "  Pinned snapshot: {} · fingerprint={} · scope={} · {} chars · {} index files · sources={} · skipped_records={} status={} unsafe={} stale={} conflicts={}\n",
        if snapshot.frozen {
            "frozen"
        } else {
            "live/not frozen"
        },
        snapshot.fingerprint,
        snapshot.scope,
        snapshot.char_count,
        snapshot.memory_file_count,
        format_pinned_sources(&snapshot.pinned_sources),
        snapshot.skipped_record_count,
        snapshot.skipped_status_count,
        snapshot.skipped_unsafe_count,
        snapshot.skipped_stale_count,
        snapshot.skipped_conflict_count
    ));
    out.push_str(&format!(
        "  Decisions: {} accepted · {} proposed · {} rejected · {} blocked\n",
        counts.accepted, counts.proposed, counts.rejected, counts.blocked
    ));
    out.push_str(&format!(
        "  Records: {} total · {} accepted · {} proposed · {} missing evidence · {} stale · {} used · {} projection drift\n",
        record_summary.total,
        record_summary.accepted,
        record_summary.proposed,
        record_summary.missing_evidence,
        record_summary.stale,
        record_summary.used,
        record_summary.projection_drift
    ));
    out.push_str(&format!(
        "  Pending memory candidates: {} proposed · {} accepted · {} rejected · {} applied · {} background · {} closeout · {} conflict groups\n",
        proposal_queue.proposed,
        proposal_queue.accepted,
        proposal_queue.rejected,
        proposal_queue.applied,
        proposal_queue.background,
        proposal_queue.closeout,
        proposal_queue.conflict_groups
    ));
    if !proposal_queue.recent.is_empty() {
        out.push_str("  Recent memory candidates:\n");
        for item in &proposal_queue.recent {
            out.push_str(&format!(
                "    {} [{}] source={} candidates={} conflicts={} reason={}\n",
                item.id,
                item.status,
                item.source,
                item.candidates,
                item.conflict_groups,
                item.reason
            ));
        }
    }
    out.push_str(&format_last_background_review(
        last_background_review.as_ref(),
    ));
    out.push_str(&format_last_memory_retrieval_trace(
        last_retrieval_trace.as_ref(),
    ));
    out.push_str(&format!(
        "  Flushes: {} completed · {} pending · {} running · {} failed · {} duplicate-skipped · {} review-skipped\n",
        flushes.completed,
        flushes.pending,
        flushes.running,
        flushes.failed,
        flushes.skipped_duplicate,
        flushes.skipped_review_only
    ));
    if operation_journal.is_empty() {
        out.push_str("  Operation journal: none\n");
    } else {
        out.push_str("  Operation journal:\n");
        for entry in &operation_journal {
            let target = entry
                .record_id
                .as_deref()
                .or(entry.candidate_id.as_deref())
                .unwrap_or("n/a");
            out.push_str(&format!(
                "    {} {} target={} count={} reason={}\n",
                entry.operation,
                entry.status,
                target,
                entry.record_count,
                compact_line(&entry.reason, 96)
            ));
        }
    }
    out.push_str(&format!(
        "  Providers: {} total · external={} · mode={} · active_scope={}\n",
        provider_lifecycle.providers.len(),
        provider_lifecycle
            .external_provider
            .as_deref()
            .unwrap_or("none"),
        provider_lifecycle.external_mode,
        provider_lifecycle.active_scope
    ));
    out.push_str(&format!(
        "  Lifecycle: {}\n",
        provider_lifecycle.lifecycle_hooks.join(" -> ")
    ));
    for provider in &provider_lifecycle.providers {
        out.push_str(&format!(
            "    {} ({}) available={} hooks={} capabilities={}\n",
            provider.name,
            provider.kind,
            provider.available,
            provider.hooks.len(),
            format_provider_capabilities(provider.capabilities)
        ));
    }
    out.push_str("  Quality gates: accept>=0.65 · propose>=0.45 · explicit>=0.60 with safety/duplicate hard stops\n");
    out.push_str(&format!(
        "  Calibration: {}/{} passed\n",
        calibration_passed,
        calibration.len()
    ));
    out.push_str(&format!(
        "  Memory evals: {}/{} passed\n",
        eval_suite.passed, eval_suite.total
    ));
    for result in eval_suite
        .results
        .iter()
        .filter(|result| !result.passed)
        .take(5)
    {
        out.push_str(&format!(
            "    FAIL {} owner={} reason={}\n",
            result.id,
            result.failure_owner.label(),
            compact_line(&result.reason, 120)
        ));
    }
    for result in calibration.iter().filter(|result| !result.passed).take(5) {
        let score = result
            .score
            .map(|score| format!("{score:.2}"))
            .unwrap_or_else(|| "n/a".to_string());
        out.push_str(&format!(
            "    FAIL {} expected={} actual={} score={} reason={}\n",
            result.id,
            result.expected.label(),
            result.actual.label(),
            score,
            compact_line(&result.reason, 120)
        ));
    }
    if conflicts.is_empty() {
        out.push_str("  Conflicts: none\n");
    } else {
        out.push_str(&format!("  Conflicts: {}\n", conflicts.len()));
        for conflict in conflicts.iter().take(5) {
            out.push_str("    ");
            out.push_str(conflict.trim_start_matches("- "));
            out.push('\n');
        }
    }
    let maintenance = memory_maintenance_decisions(docs, conflicts);
    if !maintenance.is_empty() {
        out.push_str("  Maintenance scores:\n");
        for (path, decision) in maintenance.iter().take(5) {
            out.push_str(&format!(
                "    {}: {:.2} {:?}\n",
                path, decision.score, decision.action
            ));
        }
    }
    out
}

fn format_provider_capabilities(capabilities: crate::memory::MemoryProviderCapabilities) -> String {
    let mut labels = Vec::new();
    if capabilities.prompt_block {
        labels.push("prompt");
    }
    if capabilities.prefetch {
        labels.push("prefetch");
    }
    if capabilities.search {
        labels.push("search");
    }
    if capabilities.queue_prefetch {
        labels.push("queue");
    }
    if capabilities.sync_turn {
        labels.push("sync");
    }
    if capabilities.session_end {
        labels.push("session_end");
    }
    if capabilities.pre_compress {
        labels.push("pre_compress");
    }
    if capabilities.write_mirror {
        labels.push("write_mirror");
    }
    if capabilities.tools {
        labels.push("tools");
    }
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join(",")
    }
}

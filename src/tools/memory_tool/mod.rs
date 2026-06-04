use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::Path;

mod doctor_types;
mod paths;

use doctor_types::*;
use paths::*;

#[derive(Debug, Clone)]
struct MemoryDocument {
    namespace: String,
    path: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentMemoryJsonEntry {
    key: String,
    value: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct MemoryKeyValue {
    namespace: String,
    key: String,
    value: String,
}

fn load_memory_dir_files() -> Vec<(String, String)> {
    let root = memory_dir();
    let mut files = Vec::new();
    collect_memory_dir_files(&root, &root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn load_memory_documents() -> Vec<MemoryDocument> {
    let mut docs = Vec::new();
    push_text_document(&mut docs, "project", "MEMORY.md", &memory_path());
    push_text_document(&mut docs, "user", "USER.md", &user_path());

    for (path, content) in load_memory_dir_files() {
        docs.push(MemoryDocument {
            namespace: "topic".to_string(),
            path: format!("memory/{}", path),
            content,
        });
    }

    collect_agent_memory_documents(&memory_dir().join("agents"), "agent", &mut docs);
    collect_agent_memory_documents(&legacy_agent_memory_dir(), "agent_legacy", &mut docs);
    docs.sort_by(|a, b| {
        a.namespace
            .cmp(&b.namespace)
            .then_with(|| a.path.cmp(&b.path))
    });
    docs
}

fn push_text_document(docs: &mut Vec<MemoryDocument>, namespace: &str, label: &str, path: &Path) {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return,
    };
    if content.trim().is_empty() {
        return;
    }
    docs.push(MemoryDocument {
        namespace: namespace.to_string(),
        path: label.to_string(),
        content,
    });
}

fn collect_agent_memory_documents(dir: &Path, namespace: &str, docs: &mut Vec<MemoryDocument>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }
        let display_content = format_agent_memory_content(&content);
        if display_content.trim().is_empty() {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.json");
        docs.push(MemoryDocument {
            namespace: namespace.to_string(),
            path: format!("memory/agents/{}", file_name),
            content: display_content,
        });
    }
}

fn format_agent_memory_content(content: &str) -> String {
    match serde_json::from_str::<Vec<AgentMemoryJsonEntry>>(content) {
        Ok(entries) => entries
            .into_iter()
            .map(|entry| {
                let tags = if entry.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", entry.tags.join(","))
                };
                format!("{}: {}{}", entry.key, entry.value, tags)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(_) => content.to_string(),
    }
}

fn collect_memory_dir_files(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_memory_dir_files(root, path.as_path(), files);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        files.push((relative, content));
    }
}

#[cfg(test)]
fn infer_topic(content: &str, category: &str) -> Option<&'static str> {
    let lower = content.to_lowercase();
    let category = category.to_lowercase();

    if category == "preference" || lower.contains("user preference") || lower.contains("偏好") {
        return None;
    }
    if contains_any(
        &lower,
        &[
            "tui", "terminal", "ui", "claude", "scroll", "界面", "设计", "滚动",
        ],
    ) {
        return Some("tui-design");
    }
    if contains_any(
        &lower,
        &[
            "context",
            "prompt",
            "token",
            "memory",
            "compression",
            "上下文",
            "提示词",
            "记忆",
        ],
    ) {
        return Some("context-management");
    }
    if contains_any(
        &lower,
        &["permission", "approval", "allow", "deny", "权限", "授权"],
    ) {
        return Some("permissions");
    }
    if contains_any(&lower, &["tool", "bash", "mcp", "工具"]) {
        return Some("tools");
    }
    if contains_any(&lower, &["rust", "cargo", ".rs", "crate"]) {
        return Some("rust-workflow");
    }
    if category == "decision" {
        return Some("decisions");
    }
    if category == "convention" {
        return Some("conventions");
    }
    None
}

#[cfg(test)]
fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn search_memory_documents(docs: &[MemoryDocument], query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    let mut matching = Vec::new();

    for doc in docs {
        for line in doc.content.lines() {
            if line.to_lowercase().contains(&query_lower) {
                matching.push(format!("[{}:{}] {}", doc.namespace, doc.path, line.trim()));
            }
        }
    }

    matching
}

fn memory_conflicts(docs: &[MemoryDocument], max_conflicts: usize) -> Vec<String> {
    let mut by_key: HashMap<String, Vec<MemoryKeyValue>> = HashMap::new();
    for doc in docs {
        for entry in extract_key_values(doc) {
            by_key
                .entry(entry.key.to_lowercase())
                .or_default()
                .push(entry);
        }
    }

    let mut conflicts = by_key
        .into_iter()
        .filter_map(|(key, entries)| {
            if entries.len() < 2 {
                return None;
            }
            let mut values = entries
                .iter()
                .map(|entry| normalize_value(&entry.value))
                .collect::<Vec<_>>();
            values.sort();
            values.dedup();
            if values.len() < 2 {
                return None;
            }
            let locations = entries
                .iter()
                .take(4)
                .map(|entry| {
                    format!(
                        "{}={} ({})",
                        entry.namespace,
                        compact_line(&entry.value, 70),
                        entry.key
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            Some(format!(
                "- key '{}' has conflicting values: {}",
                key, locations
            ))
        })
        .collect::<Vec<_>>();

    conflicts.sort();
    conflicts.truncate(max_conflicts);
    conflicts
}

fn load_memory_doctor_diagnostics() -> MemoryDoctorDiagnostics {
    MemoryDoctorDiagnostics {
        counts: load_memory_decision_counts(),
        flushes: load_memory_flush_summary(),
        operation_journal: load_memory_operation_journal(5),
        proposal_queue: load_memory_proposal_queue(),
        last_background_review: load_last_background_review(),
        last_retrieval_trace: load_last_memory_retrieval_trace(),
        record_summary: crate::memory::MemoryManager::new().memory_record_summary(),
        store_paths: memory_store_paths(),
    }
}

fn load_memory_decision_counts() -> MemoryDecisionCounts {
    let content = std::fs::read_to_string(memory_decision_log_path()).unwrap_or_default();
    memory_decision_counts_from_jsonl(&content)
}

fn load_memory_flush_summary() -> crate::memory::MemoryFlushSummary {
    let content = std::fs::read_to_string(memory_flush_log_path()).unwrap_or_default();
    let mut latest = std::collections::HashMap::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(record) = serde_json::from_str::<crate::memory::MemoryFlushRecord>(line) else {
            continue;
        };
        latest.insert(record.id.clone(), record);
    }

    let mut summary = crate::memory::MemoryFlushSummary {
        total: latest.len(),
        ..Default::default()
    };
    for record in latest.values() {
        match record.status {
            crate::memory::MemoryFlushStatus::Pending => summary.pending += 1,
            crate::memory::MemoryFlushStatus::Running => summary.running += 1,
            crate::memory::MemoryFlushStatus::Completed => summary.completed += 1,
            crate::memory::MemoryFlushStatus::Failed => summary.failed += 1,
            crate::memory::MemoryFlushStatus::SkippedDuplicate => summary.skipped_duplicate += 1,
            crate::memory::MemoryFlushStatus::SkippedReviewOnly => summary.skipped_review_only += 1,
        }
    }
    summary
}

fn load_memory_operation_journal(limit: usize) -> Vec<MemoryOperationJournalJson> {
    let mut entries = crate::memory::MemoryManager::new().memory_operation_journal();
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    entries
        .into_iter()
        .take(limit)
        .map(|entry| MemoryOperationJournalJson {
            id: entry.id,
            created_at: entry.created_at,
            operation: entry.operation,
            record_id: entry.record_id,
            candidate_id: entry.candidate_id,
            status: entry.status,
            reason: entry.reason,
            record_count: entry.record_count,
        })
        .collect()
}

pub(crate) fn record_last_memory_retrieval_trace(
    ctx: &crate::engine::retrieval_context::RetrievalContext,
) -> std::io::Result<()> {
    write_last_memory_retrieval_trace_to_path(&memory_retrieval_trace_path(), ctx)
}

fn write_last_memory_retrieval_trace_to_path(
    path: &Path,
    ctx: &crate::engine::retrieval_context::RetrievalContext,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = last_memory_retrieval_trace_from_context(ctx);
    let data = serde_json::to_vec_pretty(&payload).map_err(std::io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, data)?;
    std::fs::rename(tmp_path, path)
}

fn load_last_memory_retrieval_trace() -> Option<MemoryLastRetrievalTraceJson> {
    load_last_memory_retrieval_trace_from_path(&memory_retrieval_trace_path())
}

fn load_last_memory_retrieval_trace_from_path(path: &Path) -> Option<MemoryLastRetrievalTraceJson> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn last_memory_retrieval_trace_from_context(
    ctx: &crate::engine::retrieval_context::RetrievalContext,
) -> MemoryLastRetrievalTraceJson {
    let trace = ctx.memory_trace.as_ref();
    MemoryLastRetrievalTraceJson {
        updated_at: chrono::Utc::now().to_rfc3339(),
        created_at: ctx.created_at.to_rfc3339(),
        query: ctx.query.clone(),
        policy: ctx.policy,
        item_count: ctx.items.len(),
        token_estimate: ctx.token_estimate,
        selected_records: trace.map(|trace| trace.selected_records).unwrap_or(0),
        selected_chars: trace.map(|trace| trace.selected_chars).unwrap_or(0),
        max_chars: trace.map(|trace| trace.max_chars).unwrap_or(0),
        skipped_unrelated: trace.map(|trace| trace.skipped_unrelated).unwrap_or(0),
        skipped_unsafe: trace.map(|trace| trace.skipped_unsafe).unwrap_or(0),
        skipped_stale_conflict: trace.map(|trace| trace.skipped_stale_conflict).unwrap_or(0),
        skipped_budget: trace.map(|trace| trace.skipped_budget).unwrap_or(0),
        skipped_duplicate: trace.map(|trace| trace.skipped_duplicate).unwrap_or(0),
        per_scope: trace
            .map(|trace| trace.per_scope.clone())
            .unwrap_or_default(),
        decisions: trace
            .map(|trace| {
                trace
                    .decisions
                    .iter()
                    .take(8)
                    .map(|decision| MemoryLastRetrievalDecisionJson {
                        source: decision.source.clone(),
                        scope: decision.scope.clone(),
                        action: decision.action.clone(),
                        reason: decision.reason.clone(),
                        score: decision.score,
                        chars: decision.chars,
                        score_explanation: decision.score_explanation.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        selected_items: ctx
            .items
            .iter()
            .take(8)
            .map(|item| MemoryLastRetrievalItemJson {
                id: item.id.clone(),
                title: item.title.clone(),
                source: item.source,
                score: item.score,
                trust: item.trust,
                conflict: item.conflict,
                reason: item.reason.clone(),
            })
            .collect(),
    }
}

fn format_last_memory_retrieval_trace(trace: Option<&MemoryLastRetrievalTraceJson>) -> String {
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

fn memory_decision_counts_from_jsonl(content: &str) -> MemoryDecisionCounts {
    let mut counts = MemoryDecisionCounts::default();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match value["status"].as_str().unwrap_or("") {
            "accepted" => counts.accepted += 1,
            "proposed" => counts.proposed += 1,
            "rejected" => counts.rejected += 1,
            "blocked" => counts.blocked += 1,
            _ => {}
        }
    }
    counts
}

async fn memory_provider_lifecycle_panel(
    context: &ToolContext,
) -> MemoryProviderLifecyclePanelJson {
    if let Some(memory_manager) = context.memory_manager.as_ref() {
        let manager = memory_manager.lock().await;
        let report = manager.memory_provider_lifecycle_report();
        return MemoryProviderLifecyclePanelJson {
            active_scope: memory_scope_label_for_tool(&manager.active_scope()),
            providers: report.providers,
            external_provider: report.external_provider,
            external_mode: current_external_memory_provider_mode(),
            lifecycle_hooks: report.lifecycle_hooks,
        };
    }

    default_memory_provider_lifecycle_panel()
}

async fn memory_snapshot_report_panel(
    context: &ToolContext,
) -> crate::memory::MemorySnapshotReport {
    if let Some(memory_manager) = context.memory_manager.as_ref() {
        let manager = memory_manager.lock().await;
        return manager.memory_snapshot_report();
    }

    crate::memory::MemoryManager::new().memory_snapshot_report()
}

fn default_memory_provider_lifecycle_panel() -> MemoryProviderLifecyclePanelJson {
    let manager = crate::memory::MemoryManager::new();
    let report = manager.memory_provider_lifecycle_report();
    MemoryProviderLifecyclePanelJson {
        active_scope: memory_scope_label_for_tool(&manager.active_scope()),
        providers: report.providers,
        external_provider: report.external_provider,
        external_mode: current_external_memory_provider_mode(),
        lifecycle_hooks: report.lifecycle_hooks,
    }
}

fn current_external_memory_provider_mode() -> String {
    crate::services::config::AppConfig::load()
        .map(|config| config.memory.external_provider.effective_mode())
        .unwrap_or_else(|_| "off".to_string())
}

fn memory_scope_label_for_tool(scope: &crate::memory::MemoryScope) -> String {
    scope.identity_label()
}

fn format_memory_snapshot(snapshot: &crate::memory::MemorySnapshotReport) -> String {
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

fn load_memory_proposal_queue() -> MemoryProposalQueueJson {
    use crate::engine::task_contract::{MemoryProposalReviewStore, MemoryProposalStatus};

    let records = MemoryProposalReviewStore::default().list_records();
    let mut queue = MemoryProposalQueueJson {
        total: records.len(),
        proposed: 0,
        accepted: 0,
        rejected: 0,
        applied: 0,
        background: 0,
        closeout: 0,
        conflict_groups: 0,
        recent: Vec::new(),
    };
    for record in &records {
        queue.conflict_groups += record.conflict_groups.len();
        match record.proposal.status {
            MemoryProposalStatus::Proposed => queue.proposed += 1,
            MemoryProposalStatus::Accepted => queue.accepted += 1,
            MemoryProposalStatus::Rejected => queue.rejected += 1,
            MemoryProposalStatus::Applied => queue.applied += 1,
            MemoryProposalStatus::NotApplicable => {}
        }
        match record.source.as_str() {
            "background" => queue.background += 1,
            "closeout" => queue.closeout += 1,
            _ => {}
        }
    }
    let mut recent = records;
    recent.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    queue.recent = recent
        .into_iter()
        .take(5)
        .map(|record| MemoryProposalQueueItemJson {
            id: record.id,
            task_id: record.proposal.task_id.clone(),
            status: record.proposal.status.label().to_string(),
            source: record.source,
            project_id: record.project_id,
            candidates: record.proposal.candidates.len(),
            conflict_groups: record.conflict_groups.len(),
            updated_at: record.updated_at,
            reason: compact_line(&record.proposal.reason, 120),
        })
        .collect();
    queue
}

fn load_last_background_review() -> Option<MemoryLastBackgroundReviewJson> {
    let records = crate::engine::task_contract::MemoryProposalReviewStore::default().list_records();
    last_background_review_from_records(records)
}

fn last_background_review_from_records(
    mut records: Vec<crate::engine::task_contract::MemoryProposalReviewRecord>,
) -> Option<MemoryLastBackgroundReviewJson> {
    records.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    records
        .into_iter()
        .find(|record| record.source == "background" || record.proposal.source == "background")
        .map(|record| MemoryLastBackgroundReviewJson {
            id: record.id,
            task_id: record.proposal.task_id.clone(),
            status: record.proposal.status.label().to_string(),
            candidates: record.proposal.candidates.len(),
            candidate_kinds: record.proposal.candidate_kinds(),
            write_policy: record.proposal.write_policy,
            write_performed: record.proposal.write_performed,
            conflict_groups: record.conflict_groups.len(),
            updated_at: record.updated_at,
            reason: compact_line(&record.proposal.reason, 140),
        })
}

fn format_last_background_review(review: Option<&MemoryLastBackgroundReviewJson>) -> String {
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

fn format_memory_doctor(
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

fn format_memory_doctor_with_reports(
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

fn memory_doctor_json(
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

fn memory_doctor_json_with_reports(
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

fn memory_maintenance_decisions(
    docs: &[MemoryDocument],
    conflicts: &[String],
) -> Vec<(String, crate::memory::MemoryKeepDecision)> {
    let mut decisions = docs
        .iter()
        .map(|doc| {
            let redundancy = repeated_line_ratio(&doc.content);
            let has_conflict = document_has_conflict(doc, conflicts);
            let factors = crate::memory::memory_keep_factors_from_document(
                &doc.namespace,
                &doc.content,
                has_conflict,
                redundancy,
            );
            (doc.path.clone(), crate::memory::score_memory_keep(factors))
        })
        .collect::<Vec<_>>();
    decisions.sort_by(|a, b| a.1.score.total_cmp(&b.1.score));
    decisions
}

fn document_has_conflict(doc: &MemoryDocument, conflicts: &[String]) -> bool {
    if conflicts.is_empty() {
        return false;
    }
    let lower_path = doc.path.to_lowercase();
    let lower_namespace = doc.namespace.to_lowercase();
    conflicts.iter().any(|conflict| {
        let lower = conflict.to_lowercase();
        lower.contains(&lower_path) || lower.contains(&lower_namespace)
    })
}

fn repeated_line_ratio(content: &str) -> f32 {
    let mut total = 0usize;
    let mut unique = HashSet::new();
    for line in content.lines().map(str::trim) {
        if line.len() < 12 {
            continue;
        }
        total += 1;
        unique.insert(line.to_lowercase());
    }
    if total == 0 {
        return 0.0;
    }
    ((total - unique.len()) as f32 / total as f32).clamp(0.0, 1.0)
}

fn extract_key_values(doc: &MemoryDocument) -> Vec<MemoryKeyValue> {
    doc.content
        .lines()
        .filter_map(|line| {
            let trimmed = line
                .trim()
                .trim_start_matches("- ")
                .trim_start_matches("* ");
            let (key, value) = trimmed.split_once(':')?;
            let key = key.trim().trim_matches('`');
            let value = value.trim();
            if key.is_empty()
                || value.is_empty()
                || key.starts_with('#')
                || key.chars().count() > 80
                || key.contains("://")
            {
                return None;
            }
            Some(MemoryKeyValue {
                namespace: format!("{}:{}", doc.namespace, doc.path),
                key: key.to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

fn normalize_value(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('.')
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact_line(text: &str, max_chars: usize) -> String {
    let mut value = text.replace('\n', " ");
    if value.chars().count() > max_chars {
        value = value.chars().take(max_chars).collect::<String>();
        value.push_str("...");
    }
    value
}

fn sanitize_topic(topic: &str) -> Option<String> {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in topic.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() || ch == '_' {
            output.push(ch);
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    let output = output
        .trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>();
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// Memory Save 工具 - 保存信息到持久记忆
pub struct MemorySaveTool;

#[async_trait]
impl Tool for MemorySaveTool {
    fn name(&self) -> &str {
        "memory_save"
    }

    fn description(&self) -> &str {
        "Save durable facts, preferences, decisions, and stable quirks to persistent memory. Do not save task progress, command history, or repeatable procedures; procedures belong in skills. By default it auto-routes to USER.md or memory/<topic>.md; use target=index to force MEMORY.md."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Durable information to save. Exclude task progress, command history, and step-by-step procedures; procedures belong in skills."
                },
                "category": {
                    "type": "string",
                    "description": "Category: preference, convention, decision, note",
                    "enum": ["preference", "convention", "decision", "note"],
                    "default": "note"
                },
                "target": {
                    "type": "string",
                    "description": "Optional target: auto infers destination, index writes MEMORY.md, user writes USER.md, topic writes memory/<topic>.md",
                    "enum": ["auto", "index", "user", "topic"],
                    "default": "auto"
                },
                "topic": {
                    "type": "string",
                    "description": "Optional topic filename for memory/<topic>.md. Example: tui-design, context-management, rust-workflow"
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let content = params["content"].as_str().unwrap_or("");
        if content.is_empty() {
            return ToolResult::error("Content cannot be empty");
        }

        let category = params["category"].as_str().unwrap_or("note");
        let target = params["target"].as_str().unwrap_or("auto");
        let topic = params["topic"].as_str().unwrap_or("").trim();

        let mut candidate = crate::memory::MemoryCandidate::new(
            content,
            category,
            crate::memory::MemoryScope {
                project_root: Some(context.working_dir.clone()),
                session_id: context.session_id.clone(),
                platform: "tool".to_string(),
                ..Default::default()
            },
            crate::memory::MemoryProvenance {
                source: "memory_save_tool".to_string(),
                session_id: Some(context.session_id.clone()),
                turn_index: None,
                tool_name: Some("memory_save".to_string()),
            },
        )
        .explicit(true);
        candidate
            .evidence
            .push(crate::memory::MemoryEvidenceRef::new(
                crate::memory::MemoryEvidenceKind::ToolOutput,
                "memory_save_tool",
                "explicit memory_save tool call",
                0.85,
            ));

        let write_target = if target == "user" || category == "preference" {
            crate::memory::MemoryWriteTarget::User
        } else if target == "topic" || !topic.is_empty() {
            let topic = if topic.is_empty() { category } else { topic };
            if sanitize_topic(topic).is_none() {
                return ToolResult::error("Topic must contain at least one valid character");
            }
            crate::memory::MemoryWriteTarget::Topic(topic.to_string())
        } else if target == "index" {
            crate::memory::MemoryWriteTarget::Index
        } else {
            crate::memory::MemoryWriteTarget::Auto
        };

        let outcome = if let Some(memory_manager) = context.memory_manager.as_ref() {
            let manager = memory_manager.lock().await;
            manager
                .submit_candidate_with_provider_notifications(candidate, write_target)
                .await
        } else {
            let manager = crate::memory::MemoryManager::new();
            manager
                .submit_candidate_with_provider_notifications(candidate, write_target)
                .await
        };
        let path = outcome
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| memory_root().display().to_string());
        let score = outcome
            .quality_score
            .map(|score| format!("{score:.2}"))
            .unwrap_or_else(|| "n/a".to_string());

        match outcome.status {
            crate::memory::manager::MemoryWriteOutcomeStatus::Saved => ToolResult::success(
                format!("Saved to {} (quality {}): [{}] {}", path, score, category, content),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Duplicate => ToolResult::success(
                format!(
                    "Memory already exists in {} (quality {}): [{}] {}",
                    path, score, category, content
                ),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Proposed => ToolResult::success(
                format!(
                    "Memory proposed for review, not injected as accepted memory yet (quality {}). Reason: {}",
                    score, outcome.reason
                ),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Rejected => ToolResult::success(
                format!(
                    "Memory not saved: quality gate rejected it (quality {}). Reason: {}",
                    score, outcome.reason
                ),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Blocked => ToolResult::error(
                format!("Blocked unsafe memory: {}", outcome.reason),
            ),
            crate::memory::manager::MemoryWriteOutcomeStatus::Failed => {
                ToolResult::error(format!("Failed to save memory: {}", outcome.reason))
            }
            crate::memory::manager::MemoryWriteOutcomeStatus::InvalidTarget => {
                ToolResult::error(format!("Invalid memory target: {}", outcome.reason))
            }
        }
    }
}

/// Memory Load 工具 - 读取持久记忆
pub struct MemoryLoadTool;

#[async_trait]
impl Tool for MemoryLoadTool {
    fn name(&self) -> &str {
        "memory_load"
    }

    fn description(&self) -> &str {
        "Load or search the agent's persistent memory across tiers: \
         MEMORY.md (project facts/decisions), USER.md (user preferences), \
         memory/*.md (topic files), and typed memory records. \
         \
         Actions: load (full content), search (filter by query), doctor (health \
         summary), conflicts (find contradictory entries across tiers), \
         review (recent decisions, flushes, conflicts). \
         \
         Memory is frozen at session start for cache consistency — recent writes \
         may not appear until next session. Use memory_save to persist new facts."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "load returns memory content, search filters by query, doctor summarizes health, doctor_json returns machine-readable health, snapshot reports the frozen prompt memory snapshot, eval runs deterministic memory lifecycle evals, conflicts lists conflicts, review summarizes decisions/flushes/conflicts, repair_proposals creates review-required proposals for projection drift, migrate_dry_run/migrate_backup/migrate_rollback manage conservative memory backups, explain/why shows why a matching memory was retrieved.",
                    "enum": ["load", "search", "doctor", "doctor_json", "snapshot", "eval", "conflicts", "review", "repair_proposals", "migrate_dry_run", "migrate_backup", "migrate_rollback", "explain", "why"],
                    "default": "load"
                },
                "query": {
                    "type": "string",
                    "description": "Optional: search query to filter memories. If empty, returns all memories."
                },
                "include_conflicts": {
                    "type": "boolean",
                    "description": "Whether to include duplicate/conflicting key hints across memory namespaces.",
                    "default": true
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum repair proposals to create for repair_proposals.",
                    "default": 20
                },
                "backup_id": {
                    "type": "string",
                    "description": "Backup id for migrate_rollback."
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let docs = load_memory_documents();
        let include_conflicts = params["include_conflicts"].as_bool().unwrap_or(true);
        let action = params["action"].as_str().unwrap_or("load");
        let provider_lifecycle = if matches!(action, "doctor" | "doctor_json" | "review") {
            Some(memory_provider_lifecycle_panel(&context).await)
        } else {
            None
        };
        let snapshot_report = if matches!(action, "doctor" | "doctor_json" | "review" | "snapshot")
        {
            Some(memory_snapshot_report_panel(&context).await)
        } else {
            None
        };

        if action == "snapshot" {
            let snapshot = snapshot_report
                .as_ref()
                .expect("snapshot report is loaded for snapshot action");
            return ToolResult::success(format_memory_snapshot(snapshot));
        }

        if action == "eval" {
            return ToolResult::success(crate::memory::run_memory_eval_suite().format());
        }

        if action == "repair_proposals" {
            let limit = params["limit"]
                .as_u64()
                .map(|value| value as usize)
                .unwrap_or(20)
                .clamp(1, 200);
            let created =
                crate::memory::MemoryManager::new().upsert_projection_repair_proposals(limit);
            return ToolResult::success(format!(
                "Memory repair proposal scan complete\n- projection drift proposals: {}\n- review: /memory-proposals list --source repair",
                created
            ));
        }

        if action == "migrate_dry_run" {
            return ToolResult::success(
                crate::memory::MemoryManager::new()
                    .memory_migration_dry_run()
                    .format(),
            );
        }

        if action == "migrate_backup" {
            return match crate::memory::MemoryManager::new().memory_migration_backup() {
                Ok(report) => ToolResult::success(report.format()),
                Err(error) => ToolResult::error(format!("memory migration backup failed: {error}")),
            };
        }

        if action == "migrate_rollback" {
            let backup_id = params["backup_id"].as_str().unwrap_or("");
            if backup_id.trim().is_empty() {
                return ToolResult::error("backup_id is required for migrate_rollback");
            }
            return match crate::memory::MemoryManager::new().memory_migration_rollback(backup_id) {
                Ok(report) => ToolResult::success(report.format()),
                Err(error) => {
                    ToolResult::error(format!("memory migration rollback failed: {error}"))
                }
            };
        }

        if docs.is_empty() {
            if action == "doctor_json" {
                let provider_lifecycle = provider_lifecycle
                    .as_ref()
                    .expect("provider lifecycle is loaded for doctor_json");
                let snapshot = snapshot_report
                    .as_ref()
                    .expect("snapshot report is loaded for doctor_json");
                return ToolResult::success(
                    memory_doctor_json(&docs, &[], provider_lifecycle, snapshot).to_string(),
                );
            }
            if matches!(action, "doctor" | "review") {
                let provider_lifecycle = provider_lifecycle
                    .as_ref()
                    .expect("provider lifecycle is loaded for doctor/review");
                let snapshot = snapshot_report
                    .as_ref()
                    .expect("snapshot report is loaded for doctor/review");
                return ToolResult::success(format_memory_doctor(
                    &docs,
                    &[],
                    provider_lifecycle,
                    snapshot,
                ));
            }
            return ToolResult::success("Memory is empty.");
        }

        let query = params["query"].as_str().unwrap_or("");
        let conflicts = if include_conflicts {
            memory_conflicts(&docs, 8)
        } else {
            Vec::new()
        };

        if action == "doctor" {
            let provider_lifecycle = provider_lifecycle
                .as_ref()
                .expect("provider lifecycle is loaded for doctor");
            let snapshot = snapshot_report
                .as_ref()
                .expect("snapshot report is loaded for doctor");
            return ToolResult::success(format_memory_doctor(
                &docs,
                &conflicts,
                provider_lifecycle,
                snapshot,
            ));
        }

        if action == "doctor_json" {
            let provider_lifecycle = provider_lifecycle
                .as_ref()
                .expect("provider lifecycle is loaded for doctor_json");
            let snapshot = snapshot_report
                .as_ref()
                .expect("snapshot report is loaded for doctor_json");
            return ToolResult::success(
                memory_doctor_json(&docs, &conflicts, provider_lifecycle, snapshot).to_string(),
            );
        }

        if action == "conflicts" {
            return if conflicts.is_empty() {
                ToolResult::success("Memory conflicts: none")
            } else {
                ToolResult::success(format!("Memory Conflicts\n{}", conflicts.join("\n")))
            };
        }

        if action == "review" {
            let provider_lifecycle = provider_lifecycle
                .as_ref()
                .expect("provider lifecycle is loaded for review");
            let snapshot = snapshot_report
                .as_ref()
                .expect("snapshot report is loaded for review");
            return ToolResult::success(format_memory_doctor(
                &docs,
                &conflicts,
                provider_lifecycle,
                snapshot,
            ));
        }

        if matches!(action, "explain" | "why") {
            if query.trim().is_empty() {
                return ToolResult::error("query is required for memory why");
            }
            let matching = search_memory_documents(&docs, query);
            return if matching.is_empty() {
                ToolResult::success(format!("No memories matching '{}'", query))
            } else {
                ToolResult::success(format!(
                    "Memory Explain\nselector: {}\nreason: matched memory namespace/path/content text. Use /memory search for retrieval ids in the interactive CLI.\n\n{}",
                    query,
                    matching.join("\n")
                ))
            };
        }

        if action == "search" || !query.is_empty() {
            let mut matching = search_memory_documents(&docs, query);

            if matching.is_empty() {
                ToolResult::success(format!("No memories matching '{}'", query))
            } else {
                if !conflicts.is_empty() {
                    matching.push(String::new());
                    matching.push("Conflicts:".to_string());
                    matching.extend(conflicts);
                }
                let result = matching.join("\n");
                let truncated: String = result.chars().take(3000).collect();
                ToolResult::success(truncated)
            }
        } else {
            // 返回全部（限制大小）
            let mut output = String::new();
            for doc in &docs {
                output.push_str(&format!("# [{}] {}\n", doc.namespace, doc.path));
                output.push_str(doc.content.trim());
                output.push_str("\n\n");
            }
            if !conflicts.is_empty() {
                output.push_str("# Conflicts\n");
                output.push_str(&conflicts.join("\n"));
                output.push('\n');
            }
            let truncated: String = output.chars().take(5000).collect();
            ToolResult::success(truncated)
        }
    }
}

/// Memory Clear 工具 - 清空记忆
pub struct MemoryClearTool;

#[async_trait]
impl Tool for MemoryClearTool {
    fn name(&self) -> &str {
        "memory_clear"
    }

    fn description(&self) -> &str {
        "Clear all persistent memory. Use with caution - this will delete all saved preferences and notes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "confirm": {
                    "type": "boolean",
                    "description": "Must be true to confirm deletion"
                }
            },
            "required": ["confirm"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        if !params["confirm"].as_bool().unwrap_or(false) {
            return ToolResult::error("Set confirm=true to clear memory");
        }

        let path = memory_path();
        let memory_dir = memory_dir();
        let write_result = std::fs::write(&path, "# Priority Agent Memory\n");
        if memory_dir.exists() {
            let _ = std::fs::remove_dir_all(&memory_dir);
        }
        let _ = std::fs::create_dir_all(&memory_dir);

        match write_result {
            Ok(_) => ToolResult::success("Memory cleared"),
            Err(e) => ToolResult::error(format!("Failed to clear memory: {}", e)),
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn confirmation_prompt(&self, _params: &serde_json::Value) -> Option<String> {
        Some("This will delete all saved memory. Continue?".to_string())
    }
}

#[cfg(test)]
mod tests;

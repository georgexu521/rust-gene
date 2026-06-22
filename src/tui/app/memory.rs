//! TUI application state support.
//!
//! Keeps runtime state, memory panels, slash commands, and status tools separate from rendering.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MemorySaveTarget {
    Auto,
    User,
    Topic,
}

pub(super) fn parse_memory_save_args(args: &str) -> (MemorySaveTarget, Option<&str>, &str) {
    let trimmed = args.trim();
    if let Some(rest) = trimmed.strip_prefix("--user ") {
        return (MemorySaveTarget::User, None, rest.trim());
    }
    if let Some(rest) = trimmed.strip_prefix("--topic=") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let topic = parts.next().filter(|part| !part.trim().is_empty());
        let content = parts.next().unwrap_or("").trim();
        return (MemorySaveTarget::Topic, topic, content);
    }
    if let Some(rest) = trimmed.strip_prefix("--topic ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let topic = parts.next().filter(|part| !part.trim().is_empty());
        let content = parts.next().unwrap_or("").trim();
        return (MemorySaveTarget::Topic, topic, content);
    }
    (MemorySaveTarget::Auto, None, trimmed)
}

pub(super) fn format_memory_write_outcome(
    content: &str,
    outcome: &crate::memory::manager::MemoryWriteOutcome,
) -> String {
    use crate::memory::manager::MemoryWriteOutcomeStatus;

    let score = outcome
        .quality_score
        .map(|score| format!("quality {:.2}", score))
        .unwrap_or_else(|| "quality n/a".to_string());
    let path = outcome
        .path
        .as_ref()
        .map(|path| format!("\nPath: {}", path.display()))
        .unwrap_or_default();
    match outcome.status {
        MemoryWriteOutcomeStatus::Saved => {
            format!("Saved memory: {}\n{}{}", content, score, path)
        }
        MemoryWriteOutcomeStatus::Duplicate => {
            format!(
                "Memory already exists; not saved again: {}\nReason: {}{}",
                content, outcome.reason, path
            )
        }
        MemoryWriteOutcomeStatus::Proposed => {
            format!(
                "Memory was not saved to long-term memory yet: quality gate proposed review.\n{}; reason: {}",
                score, outcome.reason
            )
        }
        MemoryWriteOutcomeStatus::Rejected => {
            format!(
                "Memory was not saved: quality gate rejected it.\n{}; reason: {}",
                score, outcome.reason
            )
        }
        MemoryWriteOutcomeStatus::Blocked => {
            format!("Memory was blocked for safety: {}", outcome.reason)
        }
        MemoryWriteOutcomeStatus::Failed => {
            format!("Memory save failed: {}{}", outcome.reason, path)
        }
        MemoryWriteOutcomeStatus::InvalidTarget => {
            format!("Memory save target is invalid: {}", outcome.reason)
        }
    }
}

pub(super) fn dedupe_palette_commands(commands: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for command in commands {
        if seen.insert(command.clone()) {
            deduped.push(command);
        }
    }
    deduped
}

pub(super) fn format_memory_retrieval_context(
    ctx: &crate::engine::retrieval_context::RetrievalContext,
) -> String {
    let mut lines = vec![
        "Memory Search".to_string(),
        format!(
            "Query: {} · items: {} · tokens~{} · conflicts: {}",
            ctx.query,
            ctx.items.len(),
            ctx.token_estimate,
            ctx.conflict_count()
        ),
        String::new(),
    ];
    if let Some(trace) = &ctx.memory_trace {
        lines.push(format!(
            "Trace: selected={} chars={}/{} skipped unrelated={} unsafe={} stale_conflict={} budget={}",
            trace.selected_records,
            trace.selected_chars,
            trace.max_chars,
            trace.skipped_unrelated,
            trace.skipped_unsafe,
            trace.skipped_stale_conflict,
            trace.skipped_budget
        ));
        for scope in &trace.per_scope {
            lines.push(format!(
                "  scope {}: selected={} skipped={} cap={}",
                scope.scope, scope.selected, scope.skipped, scope.cap
            ));
        }
        for decision in trace.decisions.iter().take(6) {
            let score = decision
                .score_explanation
                .as_ref()
                .map(|explanation| {
                    format!(
                        " lexical={:.2} recency={:.2} scope_match={:.2} confidence={:.2} status={} conflict_penalty={:.2} pinned_bonus={:.2} final={:.2}",
                        explanation.lexical_match,
                        explanation.recency,
                        explanation.scope_match,
                        explanation.confidence,
                        explanation.status,
                        explanation.conflict_penalty,
                        explanation.user_pinned_bonus,
                        explanation.final_score
                    )
                })
                .unwrap_or_default();
            lines.push(format!(
                "  decision {} {} scope={} score={} chars={}{} reason={}",
                decision.action,
                decision.source,
                decision.scope,
                decision.score,
                decision.chars,
                score,
                decision.reason
            ));
        }
        lines.push(String::new());
    }
    for item in &ctx.items {
        let preview = item
            .content_preview
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!(
            "- {} · {} · score {:.2} · {:?}{}",
            item.id,
            item.title,
            item.score,
            item.trust,
            if item.conflict { " · conflict" } else { "" }
        ));
        lines.push(format!("  reason: {}", item.reason));
        lines.push(format!("  provenance: {}", item.provenance));
        lines.push(format!(
            "  {}",
            preview.chars().take(240).collect::<String>()
        ));
    }
    lines.join("\n")
}

pub(super) fn format_memory_snapshot_report(
    snapshot: &crate::memory::MemorySnapshotReport,
) -> String {
    let pinned_sources = if snapshot.pinned_sources.is_empty() {
        "none".to_string()
    } else {
        snapshot.pinned_sources.join(", ")
    };
    format!(
        "Pinned Memory Snapshot\n- Status: {}\n- Snapshot id: {}\n- Fingerprint: {}\n- Scope: {}\n- Pinned prompt chars: {}\n- Project chars: {}\n- User chars: {}\n- Memory index files: {} ({} chars)\n- Pinned sources: {}\n- Skipped records: {} (status={} unsafe={} stale={} conflicts={})",
        if snapshot.frozen {
            "frozen"
        } else {
            "live/not frozen"
        },
        snapshot.snapshot_id,
        snapshot.fingerprint,
        snapshot.scope,
        snapshot.char_count,
        snapshot.project_chars,
        snapshot.user_chars,
        snapshot.memory_file_count,
        snapshot.memory_file_chars,
        pinned_sources,
        snapshot.skipped_record_count,
        snapshot.skipped_status_count,
        snapshot.skipped_unsafe_count,
        snapshot.skipped_stale_count,
        snapshot.skipped_conflict_count
    )
}

pub(super) fn format_memory_migration_command(
    mem: &crate::memory::MemoryManager,
    args: &str,
) -> String {
    let mut parts = args.split_whitespace();
    match parts.next().unwrap_or("--dry-run") {
        "--dry-run" | "dry-run" | "status" => mem.memory_migration_dry_run().format(),
        "--backup" | "backup" => match mem.memory_migration_backup() {
            Ok(report) => report.format(),
            Err(error) => format!("Memory migration backup failed: {}", error),
        },
        "--rollback" | "rollback" => {
            let Some(backup_id) = parts.next() else {
                return "Usage: /memory migrate --rollback <backup_id>".to_string();
            };
            match mem.memory_migration_rollback(backup_id) {
                Ok(report) => report.format(),
                Err(error) => format!("Memory migration rollback failed: {}", error),
            }
        }
        _ => "Usage: /memory migrate [--dry-run|--backup|--rollback <backup_id>]".to_string(),
    }
}

pub(super) fn format_memory_records(records: &[crate::memory::MemoryRecord], args: &str) -> String {
    let scope_filter = parse_memory_scope_filter(args);
    let mut filtered = records
        .iter()
        .filter(|record| {
            scope_filter
                .as_deref()
                .map(|scope| memory_record_scope_label(record).contains(scope))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    filtered.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    if filtered.is_empty() {
        return match scope_filter {
            Some(scope) => format!("Memory Records\n- none for scope '{}'", scope),
            None => "Memory Records\n- none".to_string(),
        };
    }
    let mut lines = vec![format!(
        "Memory Records ({} shown{})",
        filtered.len().min(30),
        scope_filter
            .as_deref()
            .map(|scope| format!(" · scope={scope}"))
            .unwrap_or_default()
    )];
    for record in filtered.into_iter().take(30) {
        lines.push(format!(
            "- {} [{} {:?}] scope={} confidence={:.2} utility={:.2} evidence={} used={} updated={}",
            record.id,
            memory_status_label(record.status),
            record.kind,
            memory_record_scope_label(record),
            record.confidence,
            record.utility,
            record.evidence.len(),
            record.use_count,
            record.updated_at.to_rfc3339()
        ));
        lines.push(format!(
            "  {}",
            record
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(260)
                .collect::<String>()
        ));
    }
    lines.join("\n")
}

fn parse_memory_scope_filter(args: &str) -> Option<String> {
    let parts = args.split_whitespace().collect::<Vec<_>>();
    for (idx, part) in parts.iter().enumerate() {
        if let Some(scope) = part.strip_prefix("--scope=") {
            return Some(scope.to_ascii_lowercase());
        }
        if *part == "--scope" {
            return parts.get(idx + 1).map(|scope| scope.to_ascii_lowercase());
        }
    }
    parts
        .first()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn memory_record_scope_label(record: &crate::memory::MemoryRecord) -> String {
    if matches!(record.kind, crate::memory::MemoryKind::UserPreference) {
        return "user".to_string();
    }
    if record.scope.project_root.is_some() {
        return "project".to_string();
    }
    if !record.scope.session_id.trim().is_empty() {
        return "session".to_string();
    }
    record.scope.platform.clone()
}

fn memory_status_label(status: crate::memory::MemoryStatus) -> &'static str {
    match status {
        crate::memory::MemoryStatus::Proposed => "proposed",
        crate::memory::MemoryStatus::Accepted => "accepted",
        crate::memory::MemoryStatus::Rejected => "rejected",
        crate::memory::MemoryStatus::Superseded => "superseded",
        crate::memory::MemoryStatus::Archived => "archived",
    }
}

pub(super) fn format_memory_proposal_queue() -> String {
    let queue = memory_proposal_queue_json();
    let mut lines = vec![format!(
        "Pending memory candidates\n- Proposed: {} · accepted: {} · rejected: {} · applied: {} · background: {} · closeout: {}",
        queue["proposed"].as_u64().unwrap_or(0),
        queue["accepted"].as_u64().unwrap_or(0),
        queue["rejected"].as_u64().unwrap_or(0),
        queue["applied"].as_u64().unwrap_or(0),
        queue["background"].as_u64().unwrap_or(0),
        queue["closeout"].as_u64().unwrap_or(0)
    )];
    if let Some(recent) = queue["recent"].as_array() {
        for item in recent.iter().take(5) {
            lines.push(format!(
                "- {} [{}] source={} candidates={} reason={}",
                item["id"].as_str().unwrap_or("unknown"),
                item["status"].as_str().unwrap_or("unknown"),
                item["source"].as_str().unwrap_or("unknown"),
                item["candidates"].as_u64().unwrap_or(0),
                item["reason"].as_str().unwrap_or("")
            ));
        }
    }
    lines.join("\n")
}

pub(super) fn memory_proposal_queue_json() -> serde_json::Value {
    use crate::engine::task_contract::{MemoryProposalReviewStore, MemoryProposalStatus};

    let mut records = MemoryProposalReviewStore::default().list_records();
    let mut proposed = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut applied = 0usize;
    let mut background = 0usize;
    let mut closeout = 0usize;
    for record in &records {
        match record.proposal.status {
            MemoryProposalStatus::Proposed => proposed += 1,
            MemoryProposalStatus::Accepted => accepted += 1,
            MemoryProposalStatus::Rejected => rejected += 1,
            MemoryProposalStatus::Applied => applied += 1,
            MemoryProposalStatus::NotApplicable => {}
        }
        match record.source.as_str() {
            "background" => background += 1,
            "closeout" => closeout += 1,
            _ => {}
        }
    }
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let recent = records
        .into_iter()
        .take(5)
        .map(|record| {
            serde_json::json!({
                "id": record.proposal.task_id,
                "status": record.proposal.status.label(),
                "source": record.source,
                "candidates": record.proposal.candidates.len(),
                "updated_at": record.updated_at,
                "reason": record.proposal.reason.chars().take(120).collect::<String>(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "total": proposed + accepted + rejected + applied,
        "proposed": proposed,
        "accepted": accepted,
        "rejected": rejected,
        "applied": applied,
        "background": background,
        "closeout": closeout,
        "recent": recent,
    })
}

pub(super) fn explain_memory_retrieval_item(
    ctx: &crate::engine::retrieval_context::RetrievalContext,
    selector: &str,
) -> String {
    let selector = selector.to_lowercase();
    let Some(item) = ctx.items.iter().find(|item| {
        item.id.to_lowercase().contains(&selector)
            || item.title.to_lowercase().contains(&selector)
            || item.provenance.to_lowercase().contains(&selector)
    }) else {
        return format!(
            "No retrieval item matching '{}'. Run /memory search <query> to see ids.",
            selector
        );
    };
    format!(
        "Memory Retrieval Explanation\n\nid: {}\nsource: {:?}\ntitle: {}\nscore: {:.2}\ntrust: {:?}\nconflict: {}\nprovenance: {}\nreason: {}\n\n{}",
        item.id,
        item.source,
        item.title,
        item.score,
        item.trust,
        item.conflict,
        item.provenance,
        item.reason,
        item.content_preview
    )
}

pub(super) fn parse_memory_why_args<'a>(
    args: &'a str,
    latest_user_message: &'a str,
) -> Option<(&'a str, Option<&'a str>, bool)> {
    let args = args.trim();
    if args.is_empty() {
        return None;
    }
    let last_turn = args.contains("--last-turn");
    // Handle --last-turn first
    if last_turn {
        // Try to find --item in the args
        if let Some(selector) = args.strip_prefix("--item ") {
            let query = latest_user_message.trim();
            if query.is_empty() {
                return None;
            }
            return Some((query, Some(selector.trim()), last_turn));
        }
        if let Some((_query, selector)) = args.split_once(" --item ") {
            let selector = selector.trim();
            let query = latest_user_message.trim();
            if query.is_empty() {
                return None;
            }
            return Some((query, Some(selector), last_turn));
        }
        // No --item, use latest_user_message
        let query = latest_user_message.trim();
        if query.is_empty() {
            return None;
        }
        return Some((query, None, last_turn));
    }
    // No --last-turn
    if let Some(selector) = args.strip_prefix("--item ") {
        let query = latest_user_message.trim();
        if query.is_empty() {
            return None;
        }
        return Some((query, Some(selector.trim()), false));
    }
    if let Some((query, selector)) = args.split_once(" --item ") {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }
        return Some((query, Some(selector.trim()), false));
    }
    Some((args, None, false))
}

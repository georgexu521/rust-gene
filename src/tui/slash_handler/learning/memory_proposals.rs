use super::*;

/// /memory-proposals - Review closeout-generated memory candidates
pub fn handle_memory_proposals(app: &mut TuiApp, args: &str) -> String {
    use crate::engine::task_contract::MemoryProposalReviewStore;

    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");
    let store = MemoryProposalReviewStore::default();

    match action {
        "list" | "" => {
            let remaining = parts.collect::<Vec<_>>();
            let filter = parse_memory_proposal_batch_filter(&remaining);
            let mut records = store.list_records();
            if let Some(source) = filter.source.as_deref() {
                records.retain(|record| record.source == source);
            }
            if let Some(scope) = filter.scope.as_deref() {
                records.retain(|record| {
                    record
                        .proposal
                        .candidates
                        .iter()
                        .any(|candidate| candidate.scope == scope)
                        || record.active_scope.split(',').any(|item| item.trim() == scope)
                });
            }
            if let Some(status) = filter.status {
                records.retain(|record| record.proposal.status == status);
            }
            if records.is_empty() {
                return "Memory Proposals\n- none yet".to_string();
            }
            let mut lines = vec![format!("Memory Proposals ({} total)", records.len())];
            for record in records.iter().take(20) {
                lines.push(format_memory_proposal_record_line(record));
            }
            lines.join("\n")
        }
        "show" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals show <task-id>".to_string();
            };
            match store.get_record(id) {
                Some(record) => format_memory_proposal_detail(&record),
                None => format!("No memory proposal matching '{}'.", id),
            }
        }
        "conflicts" | "conflict-groups" => {
            let records = store.list_records();
            format_memory_proposal_conflict_panel(&records)
        }
        "resolve-conflict" | "resolve" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals resolve-conflict <keep-task-id>".to_string();
            };
            match store.resolve_conflict_keep(id) {
                Ok(Some(result)) => format_memory_proposal_conflict_resolution(&result),
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(error) => format!("Failed to resolve memory proposal conflict: {}", error),
            }
        }
        "accept" | "reject" => {
            let Some(id) = parts.next() else {
                return format!("Usage: /memory-proposals {} <task-id>", action);
            };
            let status = if action == "accept" {
                MemoryProposalStatus::Accepted
            } else {
                MemoryProposalStatus::Rejected
            };
            match store.update_status(id, status) {
                Ok(Some(proposal)) => {
                    format!("Updated memory proposal\n{}", format_memory_proposal_line(&proposal))
                }
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to update memory proposal: {}", e),
            }
        }
        "batch-accept" | "accept-batch" => {
            let remaining = parts.collect::<Vec<_>>();
            let mut filter = parse_memory_proposal_batch_filter(&remaining);
            if filter.status.is_none() {
                filter.status = Some(MemoryProposalStatus::Proposed);
            }
            match store.batch_update_status(
                filter,
                MemoryProposalStatus::Accepted,
                "batch accepted for memory apply",
            ) {
                Ok(result) => format_memory_proposal_batch_result("Batch accepted", &result),
                Err(error) => format!("Failed to batch accept memory proposals: {}", error),
            }
        }
        "batch-reject" | "reject-batch" => {
            let remaining = parts.collect::<Vec<_>>();
            let mut filter = parse_memory_proposal_batch_filter(&remaining);
            let mut reason = "batch rejected by review".to_string();
            if remaining.iter().any(|part| *part == "duplicate" || *part == "--duplicate") {
                filter.duplicate_only = true;
                reason = "batch rejected as duplicate/conflicting".to_string();
            }
            if filter.status.is_none() {
                filter.status = Some(MemoryProposalStatus::Proposed);
            }
            match store.batch_update_status(filter, MemoryProposalStatus::Rejected, reason) {
                Ok(result) => format_memory_proposal_batch_result("Batch rejected", &result),
                Err(error) => format!("Failed to batch reject memory proposals: {}", error),
            }
        }
        "cleanup-stale" => {
            let remaining = parts.collect::<Vec<_>>();
            let mut filter = parse_memory_proposal_batch_filter(&remaining);
            if filter.stale_days.is_none() {
                filter.stale_days = Some(30);
            }
            if filter.status.is_none() {
                filter.status = Some(MemoryProposalStatus::Proposed);
            }
            match store.batch_update_status(
                filter,
                MemoryProposalStatus::Rejected,
                "batch rejected as stale proposal",
            ) {
                Ok(result) => format_memory_proposal_batch_result("Stale cleanup", &result),
                Err(error) => format!("Failed to cleanup stale memory proposals: {}", error),
            }
        }
        "supersede" => {
            let Some(old_id) = parts.next() else {
                return "Usage: /memory-proposals supersede <old-id> <new-id>".to_string();
            };
            let Some(new_id) = parts.next() else {
                return "Usage: /memory-proposals supersede <old-id> <new-id>".to_string();
            };
            match store.supersede(old_id, new_id) {
                Ok(Some(proposal)) => format!(
                    "Superseded memory proposal\n{}",
                    format_memory_proposal_line(&proposal)
                ),
                Ok(None) => format!("No memory proposal matching '{}'.", old_id),
                Err(error) => format!("Failed to supersede memory proposal: {}", error),
            }
        }
        "edit" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals edit <task-id> <content>".to_string();
            };
            let content = parts.collect::<Vec<_>>().join(" ");
            if content.trim().is_empty() {
                return "Usage: /memory-proposals edit <task-id> <content>".to_string();
            }
            match store.edit_first_candidate(id, content) {
                Ok(Some(proposal)) => {
                    format!("Edited memory proposal\n{}", format_memory_proposal_line(&proposal))
                }
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to edit memory proposal: {}", e),
            }
        }
        "apply" => {
            let remaining = parts.collect::<Vec<_>>();
            if remaining.iter().any(|part| {
                matches!(
                    *part,
                    "--accepted"
                        | "accepted"
                        | "--scope"
                        | "--source"
                        | "--status"
                        | "--pending"
                        | "--rejected"
                        | "--applied"
                ) || part.starts_with("--scope=")
                    || part.starts_with("--source=")
                    || part.starts_with("--status=")
            }) {
                let mut filter = parse_memory_proposal_batch_filter(&remaining);
                if filter.status.is_none() {
                    filter.status = Some(MemoryProposalStatus::Accepted);
                }
                let mut memory = crate::memory::MemoryManager::new();
                return match store.batch_apply(filter, &mut memory) {
                    Ok(result) => format_memory_proposal_batch_apply_result(&result),
                    Err(e) => format!("Failed to batch apply memory proposals: {}", e),
                };
            }
            let Some(id) = remaining.first().copied() else {
                return "Usage: /memory-proposals apply <task-id>".to_string();
            };
            let mut memory = crate::memory::MemoryManager::new();
            match store.apply(id, &mut memory) {
                Ok(Some((proposal, applied))) => format!(
                    "Applied memory proposal {}\n- candidates applied: {}\n{}",
                    proposal.task_id,
                    applied,
                    format_memory_proposal_line(&proposal)
                ),
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to apply memory proposal: {}", e),
            }
        }
        "edit-and-apply" | "edit-apply" => {
            let Some(id) = parts.next() else {
                return "Usage: /memory-proposals edit-and-apply <task-id> <content>".to_string();
            };
            let content = parts.collect::<Vec<_>>().join(" ");
            if content.trim().is_empty() {
                return "Usage: /memory-proposals edit-and-apply <task-id> <content>".to_string();
            }
            let mut memory = crate::memory::MemoryManager::new();
            match store.edit_and_apply(id, content, &mut memory) {
                Ok(Some((proposal, applied))) => format!(
                    "Edited and applied memory proposal {}\n- candidates applied: {}\n{}",
                    proposal.task_id,
                    applied,
                    format_memory_proposal_line(&proposal)
                ),
                Ok(None) => format!("No memory proposal matching '{}'.", id),
                Err(e) => format!("Failed to edit and apply memory proposal: {}", e),
            }
        }
        "repair-drift" | "repair-proposals" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(20)
                .clamp(1, 200);
            let created = if let Some(manager) = app
                .streaming_engine
                .as_ref()
                .and_then(|engine| engine.memory_manager_or_init())
            {
                match manager.try_lock() {
                    Ok(memory) => memory.upsert_projection_repair_proposals(limit),
                    Err(_) => {
                        return "Memory manager is busy; retry repair proposal scan later."
                            .to_string();
                    }
                }
            } else {
                crate::memory::MemoryManager::new().upsert_projection_repair_proposals(limit)
            };
            format!(
                "Memory repair proposal scan complete\n- projection drift proposals: {}\n- review: /memory-proposals list --source repair",
                created
            )
        }
        _ => {
            "Usage: /memory-proposals [list [--status proposed|accepted|rejected|applied] [--blocked] [--scope user|project|topic] [--project <id|label>] [--source background|repair]|show <task-id>|conflicts|resolve-conflict <keep-task-id>|accept <task-id>|reject <task-id>|batch-accept [filters]|batch-reject [duplicate] [filters]|cleanup-stale [--days N]|supersede <old> <new>|edit <task-id> <content>|apply <task-id>|apply --accepted [--scope project|user|topic] [--project <id|label>] [--source closeout|background|repair]|edit-and-apply <task-id> <content>|repair-drift [limit]]"
                .to_string()
        }
    }
}

pub(super) fn parse_memory_proposal_batch_filter(parts: &[&str]) -> MemoryProposalBatchFilter {
    let mut filter = MemoryProposalBatchFilter::default();
    for (idx, part) in parts.iter().enumerate() {
        if let Some(source) = part.strip_prefix("--source=") {
            filter.source = Some(source.to_string());
        } else if *part == "--source" {
            filter.source = parts.get(idx + 1).map(|value| (*value).to_string());
        } else if let Some(scope) = part.strip_prefix("--scope=") {
            filter.scope = Some(scope.to_string());
        } else if *part == "--scope" {
            filter.scope = parts.get(idx + 1).map(|value| (*value).to_string());
        } else if let Some(project) = part.strip_prefix("--project=") {
            filter.project = Some(project.to_string());
        } else if *part == "--project" {
            filter.project = parts.get(idx + 1).map(|value| (*value).to_string());
        } else if let Some(status) = part.strip_prefix("--status=") {
            filter.status = parse_memory_proposal_status(status);
        } else if *part == "--status" {
            filter.status = parts
                .get(idx + 1)
                .and_then(|value| parse_memory_proposal_status(value));
        } else if *part == "--pending" || *part == "pending" {
            filter.status = Some(MemoryProposalStatus::Proposed);
        } else if *part == "--accepted" || *part == "accepted" {
            filter.status = Some(MemoryProposalStatus::Accepted);
        } else if *part == "--rejected" || *part == "rejected" {
            filter.status = Some(MemoryProposalStatus::Rejected);
        } else if *part == "--applied" || *part == "applied" {
            filter.status = Some(MemoryProposalStatus::Applied);
        } else if *part == "--blocked" || *part == "blocked" {
            filter.blocked_only = true;
        } else if let Some(days) = part.strip_prefix("--days=") {
            filter.stale_days = days.parse::<i64>().ok();
        } else if *part == "--days" {
            filter.stale_days = parts
                .get(idx + 1)
                .and_then(|value| value.parse::<i64>().ok());
        } else if *part == "--duplicate" || *part == "duplicate" {
            filter.duplicate_only = true;
        }
    }
    filter
}

fn parse_memory_proposal_status(value: &str) -> Option<MemoryProposalStatus> {
    match value {
        "pending" | "proposed" => Some(MemoryProposalStatus::Proposed),
        "accepted" => Some(MemoryProposalStatus::Accepted),
        "rejected" => Some(MemoryProposalStatus::Rejected),
        "applied" => Some(MemoryProposalStatus::Applied),
        "not_applicable" => Some(MemoryProposalStatus::NotApplicable),
        _ => None,
    }
}

fn format_memory_proposal_batch_result(
    title: &str,
    result: &crate::engine::task_contract::MemoryProposalBatchUpdate,
) -> String {
    let ids = if result.proposal_ids.is_empty() {
        "none".to_string()
    } else {
        result
            .proposal_ids
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{}\n- matched: {}\n- updated: {}\n- ids: {}",
        title, result.matched, result.updated, ids
    )
}

pub(super) fn format_memory_proposal_batch_apply_result(
    result: &crate::engine::task_contract::MemoryProposalBatchApply,
) -> String {
    let ids = if result.proposal_ids.is_empty() {
        "none".to_string()
    } else {
        result
            .proposal_ids
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let failures = if result.failures.is_empty() {
        "none".to_string()
    } else {
        result
            .failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ")
    };
    format!(
        "Batch applied memory proposals\n- matched: {}\n- applied: {}\n- candidates applied: {}\n- failed: {}\n- ids: {}\n- failures: {}",
        result.matched, result.applied, result.applied_candidates, result.failed, ids, failures
    )
}

fn format_memory_proposal_conflict_resolution(
    result: &crate::engine::task_contract::MemoryProposalConflictResolution,
) -> String {
    let rejected = if result.rejected_ids.is_empty() {
        "none".to_string()
    } else {
        result.rejected_ids.join(", ")
    };
    format!(
        "Memory proposal conflict resolved\n- kept: {}\n- accepted kept proposal: {}\n- conflict groups: {}\n- rejected: {}\n- next: /memory-proposals apply {}",
        result.kept_id, result.accepted_keep, result.conflict_groups, rejected, result.kept_id
    )
}

pub(super) fn format_memory_proposal_conflict_panel(
    records: &[crate::engine::task_contract::MemoryProposalReviewRecord],
) -> String {
    let mut lines = vec!["Memory Proposal Conflicts".to_string()];
    let mut seen = std::collections::HashSet::<String>::new();
    let records_by_id = records
        .iter()
        .map(|record| (record.proposal.task_id.as_str(), record))
        .collect::<std::collections::HashMap<_, _>>();
    for record in records {
        for group in &record.conflict_groups {
            let key = format!(
                "{}:{}:{}:{}:{}",
                group.group_type,
                group.scope,
                group.kind,
                group.key,
                group
                    .matches
                    .iter()
                    .map(|item| format!("{}#{}", item.proposal_id, item.candidate_index))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            if !seen.insert(key) {
                continue;
            }
            let ids = group
                .matches
                .iter()
                .map(|item| {
                    format!(
                        "{}#{}:{}",
                        item.proposal_id,
                        item.candidate_index + 1,
                        item.status.label()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "- {} scope={} kind={} key={} proposals={} hint={}",
                group.group_type, group.scope, group.kind, group.key, ids, group.resolution_hint
            ));
            for matched in group.matches.iter().take(6) {
                let evidence = records_by_id
                    .get(matched.proposal_id.as_str())
                    .and_then(|matched_record| {
                        matched_record
                            .proposal
                            .candidates
                            .get(matched.candidate_index)
                    })
                    .map(|candidate| candidate.evidence.len())
                    .unwrap_or(0);
                lines.push(format!(
                    "  - {}#{} status={} source={} evidence={} value={} content={}",
                    matched.proposal_id,
                    matched.candidate_index + 1,
                    matched.status.label(),
                    matched.source,
                    evidence,
                    compact_inline(&matched.value, 80),
                    compact_inline(&matched.content, 140)
                ));
            }
            lines.push(format!(
                "  next: keep one with /memory-proposals resolve-conflict <task-id>; inspect with /memory-proposals show {}",
                record.proposal.task_id
            ));
        }
    }
    if lines.len() == 1 {
        lines.push("- none".to_string());
    }
    lines.push("Resolve: /memory-proposals resolve-conflict <keep-task-id>".to_string());
    lines.join("\n")
}

fn format_memory_proposal_line(proposal: &crate::engine::task_contract::MemoryProposal) -> String {
    format!(
        "- task={} [{}] source={} candidates={} kinds={} evidence={} wrote={} reason={}",
        proposal.task_id,
        proposal.status.label(),
        proposal.source,
        proposal.candidates.len(),
        if proposal.candidates.is_empty() {
            "none".to_string()
        } else {
            proposal.candidate_kinds().join("+")
        },
        proposal.evidence_items(),
        proposal.write_performed,
        compact_inline(&proposal.reason, 80)
    )
}

fn format_memory_proposal_record_line(
    record: &crate::engine::task_contract::MemoryProposalReviewRecord,
) -> String {
    format!(
        "- id={} task={} [{}] source={} project={} candidates={} kinds={} evidence={} conflicts={} wrote={} reason={}",
        record.id,
        record.proposal.task_id,
        record.proposal.status.label(),
        record.source,
        record.project_id.as_deref().unwrap_or("unknown"),
        record.proposal.candidates.len(),
        if record.proposal.candidates.is_empty() {
            "none".to_string()
        } else {
            record.proposal.candidate_kinds().join("+")
        },
        record.proposal.evidence_items(),
        record.conflict_groups.len(),
        record.proposal.write_performed,
        compact_inline(&record.proposal.reason, 80)
    )
}

pub(super) fn format_memory_proposal_detail(
    record: &crate::engine::task_contract::MemoryProposalReviewRecord,
) -> String {
    let proposal = &record.proposal;
    let readiness = memory_proposal_review_readiness(record);
    let candidates = if proposal.candidates.is_empty() {
        "- none".to_string()
    } else {
        proposal
            .candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| {
                let evidence = if candidate.evidence.is_empty() {
                    "   evidence: none".to_string()
                } else {
                    candidate
                        .evidence
                        .iter()
                        .enumerate()
                        .map(|(evidence_idx, evidence)| {
                            format!(
                                "   evidence {}: {}",
                                evidence_idx + 1,
                                compact_inline(evidence, 180)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                format!(
                    "{}. kind={} scope={} evidence={}\n   {}\n{}",
                    idx + 1,
                    candidate.kind,
                    candidate.scope,
                    candidate.evidence.len(),
                    compact_inline(&candidate.content, 220),
                    evidence
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let gates = if record.gate_report.is_empty() {
        "- none".to_string()
    } else {
        record
            .gate_report
            .iter()
            .map(|gate| {
                let target = gate
                    .candidate_index
                    .map(|idx| format!("candidate {}", idx + 1))
                    .unwrap_or_else(|| "proposal".to_string());
                format!(
                    "- {} [{}]: {} ({})",
                    gate.gate, target, gate.status, gate.reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let history = if record.status_history.is_empty() {
        "- none".to_string()
    } else {
        record
            .status_history
            .iter()
            .map(|entry| {
                format!(
                    "- {}: {} ({})",
                    entry.at,
                    entry.status.label(),
                    compact_inline(&entry.reason, 120)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let conflicts = if record.conflict_groups.is_empty() {
        "- none".to_string()
    } else {
        record
            .conflict_groups
            .iter()
            .map(|group| {
                let matches = group
                    .matches
                    .iter()
                    .map(|item| {
                        format!(
                            "  - {}#{} [{} source={}] value={} content={}",
                            item.proposal_id,
                            item.candidate_index + 1,
                            item.status.label(),
                            item.source,
                            compact_inline(&item.value, 120),
                            compact_inline(&item.content, 160)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "- {} key={} scope={} kind={} hint={}\n{}",
                    group.group_type,
                    group.key,
                    group.scope,
                    group.kind,
                    group.resolution_hint,
                    matches
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "Memory Proposal {}\nID: {}\nStatus: {}\nReview state: {}\nAffects future sessions: {}\nWhy this was suggested: {}\nCreated: {}\nUpdated: {}\nSource session: {}\nSource task: {}\nSource: {}\nActive scope: {}\nProject: {}\nProject labels: {}\nWrite policy: {}\nWrite performed: {}\nReason: {}\nDuplicate/conflict: {}\n\nConflict groups:\n{}\n\nGate report:\n{}\n\nStatus history:\n{}\n\nCandidates:\n{}",
        proposal.task_id,
        record.id,
        proposal.status.label(),
        readiness,
        if proposal.status == crate::engine::task_contract::MemoryProposalStatus::Applied {
            "yes"
        } else {
            "after accept/apply only"
        },
        compact_inline(&proposal.reason, 180),
        record.created_at,
        record.updated_at,
        record.source_session.as_deref().unwrap_or("unknown"),
        record.source_task,
        record.source,
        record.active_scope,
        record.project_id.as_deref().unwrap_or("unknown"),
        if record.project_labels.is_empty() {
            "none".to_string()
        } else {
            record.project_labels.join(", ")
        },
        proposal.write_policy,
        proposal.write_performed,
        proposal.reason,
        record.duplicate_conflict_summary,
        conflicts,
        gates,
        history,
        candidates
    )
}

fn memory_proposal_review_readiness(
    record: &crate::engine::task_contract::MemoryProposalReviewRecord,
) -> String {
    use crate::engine::task_contract::MemoryProposalStatus;

    match record.proposal.status {
        MemoryProposalStatus::Applied => "already applied".to_string(),
        MemoryProposalStatus::Rejected => "rejected; preserved for audit".to_string(),
        MemoryProposalStatus::NotApplicable => "not applicable".to_string(),
        MemoryProposalStatus::Accepted => {
            if let Some(blocking) = record
                .gate_report
                .iter()
                .find(|gate| gate.status == "blocked" || gate.status == "missing")
            {
                return format!("blocked by {}: {}", blocking.gate, blocking.reason);
            }
            if let Some(review) = record
                .gate_report
                .iter()
                .find(|gate| gate.status == "review_required")
            {
                return format!(
                    "accepted, review needed for {}: {}",
                    review.gate, review.reason
                );
            }
            "accepted; ready to apply".to_string()
        }
        MemoryProposalStatus::Proposed => {
            if let Some(blocking) = record
                .gate_report
                .iter()
                .find(|gate| gate.status == "blocked" || gate.status == "missing")
            {
                return format!("not ready; {} says {}", blocking.gate, blocking.reason);
            }
            "pending user review; accept before apply".to_string()
        }
    }
}

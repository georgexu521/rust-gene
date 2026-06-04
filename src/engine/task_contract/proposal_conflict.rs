use super::{
    compact_text, MemoryProposal, MemoryProposalCandidate, MemoryProposalConflictGroup,
    MemoryProposalConflictMatch, MemoryProposalReviewRecord, MemoryProposalStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryProposalCandidateSignal {
    proposal_id: String,
    candidate_index: usize,
    status: MemoryProposalStatus,
    source: String,
    scope: String,
    kind: String,
    key: String,
    value: String,
    has_explicit_key: bool,
    normalized_value: String,
    normalized_content: String,
    content: String,
}

pub(crate) fn memory_proposal_conflict_groups(
    proposal: &MemoryProposal,
    peer_records: &[MemoryProposalReviewRecord],
) -> Vec<MemoryProposalConflictGroup> {
    let current = memory_proposal_candidate_signals(proposal);
    if current.is_empty() {
        return Vec::new();
    }
    let mut peers = peer_records
        .iter()
        .filter(|record| {
            !matches!(
                record.proposal.status,
                MemoryProposalStatus::Rejected | MemoryProposalStatus::NotApplicable
            )
        })
        .flat_map(|record| memory_proposal_candidate_signals(&record.proposal))
        .collect::<Vec<_>>();
    peers.extend(current.clone());

    let mut groups = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for signal in current {
        let duplicate_matches = peers
            .iter()
            .filter(|peer| {
                peer.proposal_id != signal.proposal_id
                    && peer.normalized_content == signal.normalized_content
            })
            .cloned()
            .collect::<Vec<_>>();
        if !duplicate_matches.is_empty() {
            let mut matches = vec![signal.clone()];
            matches.extend(duplicate_matches);
            push_memory_proposal_conflict_group(
                &mut groups,
                &mut seen,
                "duplicate",
                &signal,
                matches,
            );
        }

        let conflict_matches = peers
            .iter()
            .filter(|peer| {
                peer.proposal_id != signal.proposal_id
                    && peer.scope == signal.scope
                    && peer.kind == signal.kind
                    && peer.key == signal.key
                    && peer.has_explicit_key
                    && signal.has_explicit_key
                    && peer.normalized_value != signal.normalized_value
            })
            .cloned()
            .collect::<Vec<_>>();
        if !conflict_matches.is_empty() {
            let mut matches = vec![signal.clone()];
            matches.extend(conflict_matches);
            push_memory_proposal_conflict_group(
                &mut groups,
                &mut seen,
                "conflict",
                &signal,
                matches,
            );
        }
    }
    groups.sort_by(|a, b| {
        a.group_type
            .cmp(&b.group_type)
            .then_with(|| a.scope.cmp(&b.scope))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.key.cmp(&b.key))
    });
    groups
}

fn memory_proposal_candidate_signals(
    proposal: &MemoryProposal,
) -> Vec<MemoryProposalCandidateSignal> {
    proposal
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| memory_proposal_candidate_signal(proposal, idx, candidate))
        .collect()
}

fn memory_proposal_candidate_signal(
    proposal: &MemoryProposal,
    candidate_index: usize,
    candidate: &MemoryProposalCandidate,
) -> Option<MemoryProposalCandidateSignal> {
    let content = candidate.content.trim();
    if content.is_empty() {
        return None;
    }
    let explicit_pair = content
        .lines()
        .map(str::trim)
        .find_map(|line| line.split_once(':'));
    let (raw_key, raw_value, has_explicit_key) = explicit_pair
        .map(|(key, value)| (key.trim(), value.trim(), true))
        .unwrap_or((candidate.kind.as_str(), content, false));
    let key = normalize_memory_proposal_key(raw_key, &candidate.kind);
    let value = raw_value.trim().trim_matches(['`', '"', '\'']).to_string();
    let normalized_value = normalize_memory_proposal_text(&value);
    let normalized_content = normalize_memory_proposal_text(content);
    if key.is_empty() || normalized_value.is_empty() || normalized_content.is_empty() {
        return None;
    }
    Some(MemoryProposalCandidateSignal {
        proposal_id: stable_memory_proposal_id(proposal),
        candidate_index,
        status: proposal.status,
        source: proposal.source.clone(),
        scope: candidate.scope.trim().to_ascii_lowercase(),
        kind: candidate.kind.trim().to_ascii_lowercase(),
        key,
        value,
        has_explicit_key,
        normalized_value,
        normalized_content,
        content: content.to_string(),
    })
}

fn normalize_memory_proposal_key(raw_key: &str, kind: &str) -> String {
    let key = normalize_memory_proposal_text(raw_key);
    match key.as_str() {
        "" | "memory" | "note" | "preference" | "user preference" | "project" | "project fact"
        | "project convention" | "convention" => normalize_memory_proposal_text(kind),
        _ => key,
    }
}

fn normalize_memory_proposal_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_memory_proposal_conflict_group(
    groups: &mut Vec<MemoryProposalConflictGroup>,
    seen: &mut std::collections::HashSet<String>,
    group_type: &str,
    signal: &MemoryProposalCandidateSignal,
    mut matches: Vec<MemoryProposalCandidateSignal>,
) {
    matches.sort_by(|a, b| {
        a.proposal_id
            .cmp(&b.proposal_id)
            .then_with(|| a.candidate_index.cmp(&b.candidate_index))
    });
    matches
        .dedup_by(|a, b| a.proposal_id == b.proposal_id && a.candidate_index == b.candidate_index);
    let identity = format!(
        "{}:{}:{}:{}:{}",
        group_type,
        signal.scope,
        signal.kind,
        signal.key,
        matches
            .iter()
            .map(|item| format!("{}#{}", item.proposal_id, item.candidate_index))
            .collect::<Vec<_>>()
            .join(",")
    );
    if !seen.insert(identity) {
        return;
    }
    groups.push(MemoryProposalConflictGroup {
        group_type: group_type.to_string(),
        key: signal.key.clone(),
        scope: signal.scope.clone(),
        kind: signal.kind.clone(),
        matches: matches
            .into_iter()
            .map(|item| MemoryProposalConflictMatch {
                proposal_id: item.proposal_id,
                candidate_index: item.candidate_index,
                status: item.status,
                source: item.source,
                value: compact_text(&item.value, 160),
                content: compact_text(&item.content, 220),
            })
            .collect(),
        resolution_hint: if group_type == "duplicate" {
            "reject duplicate or keep one proposal before apply".to_string()
        } else if signal.kind == "user_preference" || signal.kind == "preference" {
            "prefer newer explicit user correction; reject or edit the older preference".to_string()
        } else {
            "accept one candidate, reject/edit the conflicting candidate, or supersede explicitly"
                .to_string()
        },
    });
}

pub(crate) fn summarize_memory_proposal_conflicts(
    groups: &[MemoryProposalConflictGroup],
) -> String {
    if groups.is_empty() {
        return "not_checked".to_string();
    }
    let duplicates = groups
        .iter()
        .filter(|group| group.group_type == "duplicate")
        .count();
    let conflicts = groups
        .iter()
        .filter(|group| group.group_type == "conflict")
        .count();
    let keys = groups
        .iter()
        .take(4)
        .map(|group| format!("{}:{}:{}", group.scope, group.kind, group.key))
        .collect::<Vec<_>>()
        .join(", ");
    format!("duplicates={duplicates} conflicts={conflicts} keys={keys}")
}
pub(crate) fn stable_memory_proposal_id(proposal: &MemoryProposal) -> String {
    let seed = if proposal.task_id.trim().is_empty() {
        let candidates = proposal
            .candidates
            .iter()
            .map(|candidate| {
                format!(
                    "{}\x1f{}\x1f{}",
                    candidate.kind, candidate.scope, candidate.content
                )
            })
            .collect::<Vec<_>>()
            .join("\x1e");
        format!("v1\x1f{}\x1f{}", proposal.source, candidates)
    } else {
        format!("v1\x1f{}\x1f{}", proposal.source, proposal.task_id)
    };
    let digest = format!("{:x}", md5::compute(seed));
    format!("mp-{}", &digest[..16])
}

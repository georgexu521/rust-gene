use super::{RetrievalItem, RetrievalSource, TrustLevel};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

pub fn estimate_tokens(text: &str) -> usize {
    // Good enough for budgeting and trace display. CJK text often maps closer
    // to one token per character, so this intentionally stays conservative.
    text.chars().count().div_ceil(4).max(1)
}

pub(super) fn preview(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in text.chars().take(max_chars) {
        out.push(ch);
    }
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

pub(super) fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) fn retrieval_item_id(
    source: RetrievalSource,
    title: &str,
    provenance: &str,
    content: &str,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    title.hash(&mut hasher);
    provenance.hash(&mut hasher);
    content.hash(&mut hasher);
    format!("ret_{:016x}", hasher.finish())
}

pub(super) fn retrieval_item_dedupe_key(item: &RetrievalItem) -> String {
    let content = normalized_fact_key(&item.content_preview);
    if content.chars().count() >= 12 {
        return format!("content:{content}");
    }
    format!(
        "title:{}:{}",
        source_rank(item.source),
        normalized_fact_key(&item.title)
    )
}

fn normalized_fact_key(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '"' | '\'' | '`' | ',' | '.' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            })
            .to_ascii_lowercase()
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn compare_retrieval_items(left: &RetrievalItem, right: &RetrievalItem) -> Ordering {
    score_key(right.score)
        .cmp(&score_key(left.score))
        .then_with(|| left.conflict.cmp(&right.conflict))
        .then_with(|| trust_rank(right.trust).cmp(&trust_rank(left.trust)))
        .then_with(|| freshness_rank(right).cmp(&freshness_rank(left)))
        .then_with(|| source_rank(right.source).cmp(&source_rank(left.source)))
        .then_with(|| normalized_fact_key(&left.title).cmp(&normalized_fact_key(&right.title)))
        .then_with(|| left.provenance.cmp(&right.provenance))
        .then_with(|| left.id.cmp(&right.id))
}

fn score_key(score: f32) -> i32 {
    (score.clamp(0.0, 1.0) * 1000.0).round() as i32
}

fn trust_rank(trust: TrustLevel) -> u8 {
    match trust {
        TrustLevel::High => 3,
        TrustLevel::Medium => 2,
        TrustLevel::Low => 1,
    }
}

fn source_rank(source: RetrievalSource) -> u8 {
    match source {
        RetrievalSource::Project => 7,
        RetrievalSource::File => 6,
        RetrievalSource::Tool => 5,
        RetrievalSource::Session => 4,
        RetrievalSource::Memory => 3,
        RetrievalSource::Mcp => 2,
        RetrievalSource::Web => 1,
    }
}

fn freshness_rank(item: &RetrievalItem) -> (u8, String) {
    let freshness = item
        .freshness
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    match freshness {
        Some(value) => (1, value),
        None => (0, String::new()),
    }
}

pub(super) fn merge_duplicate_retrieval_items(
    left: RetrievalItem,
    right: RetrievalItem,
) -> RetrievalItem {
    let (mut primary, secondary) = if compare_retrieval_items(&right, &left) == Ordering::Less {
        (right, left)
    } else {
        (left, right)
    };
    primary.provenance = merged_provenance(&primary, &secondary);
    primary.reason = merged_reason(&primary, &secondary);
    primary.conflict = primary.conflict && secondary.conflict;
    primary
}

fn merged_provenance(primary: &RetrievalItem, secondary: &RetrievalItem) -> String {
    let primary_entry = primary_provenance_entry(primary);
    let mut alternates = Vec::new();
    for entry in provenance_entries(primary)
        .into_iter()
        .chain(provenance_entries(secondary))
    {
        if entry != primary_entry && !alternates.contains(&entry) {
            alternates.push(entry);
        }
    }
    alternates.sort();
    if alternates.is_empty() {
        return primary.provenance.clone();
    }
    let mut parts = vec![format!("primary={primary_entry}")];
    parts.extend(alternates.into_iter().map(|entry| format!("also={entry}")));
    parts.join("; ")
}

fn primary_provenance_entry(item: &RetrievalItem) -> String {
    provenance_entries(item)
        .into_iter()
        .next()
        .unwrap_or_else(|| provenance_entry(item))
}

fn provenance_entries(item: &RetrievalItem) -> Vec<String> {
    let provenance = item.provenance.trim();
    if provenance.starts_with("primary=") {
        return provenance
            .split(';')
            .filter_map(|part| {
                part.trim()
                    .strip_prefix("primary=")
                    .or_else(|| part.trim().strip_prefix("also="))
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
            })
            .collect();
    }
    vec![provenance_entry(item)]
}

fn provenance_entry(item: &RetrievalItem) -> String {
    format!("{:?}:{}", item.source, item.provenance.trim())
}

fn merged_reason(primary: &RetrievalItem, secondary: &RetrievalItem) -> String {
    let mut reasons = vec![primary.reason.trim().to_string()];
    let secondary_reason = secondary.reason.trim().to_string();
    if !secondary_reason.is_empty() && !reasons.contains(&secondary_reason) {
        reasons.push(secondary_reason);
    }
    if reasons.len() == 1 {
        return reasons[0].clone();
    }
    format!(
        "{}; corroborated_by={}",
        reasons[0],
        reasons[1..].join(" | ")
    )
}

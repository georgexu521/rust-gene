//! Memory manager helper functions.
//!
//! Pure helper functions used by the memory manager.

use crate::memory::types::{
    MemoryCandidate, MemoryEvidenceKind, MemoryEvidenceRef, MemoryKind, MemoryRecord, MemoryScope,
    MemoryStatus,
};
use std::time::Duration;

pub const MAX_LEARNINGS_PER_TURN: usize = 3;
pub const MAX_LEARNINGS_PER_SESSION_EXTRACT: usize = 6;
pub const MEMORY_DIR_NAME: &str = "memory";
pub const MEMORY_FLUSH_LOG_FILE: &str = "flush_queue.jsonl";
pub const MEMORY_RECORDS_FILE: &str = "records.jsonl";
pub const MEMORY_FLUSH_MAX_ATTEMPTS: u8 = 3;

pub fn memory_llm_timeout() -> Duration {
    let secs = std::env::var("PRIORITY_AGENT_MEMORY_LLM_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60)
        .clamp(10, 300);
    Duration::from_secs(secs)
}

pub fn log_preview(content: &str, max_chars: usize) -> String {
    content.chars().take(max_chars).collect()
}

pub fn kind_label(kind: MemoryKind) -> &'static str {
    match kind {
        MemoryKind::UserPreference => "user_preference",
        MemoryKind::ProjectFact => "project_fact",
        MemoryKind::WorkflowConvention => "workflow_convention",
        MemoryKind::ToolQuirk => "tool_quirk",
        MemoryKind::FailurePattern => "failure_pattern",
        MemoryKind::SuccessfulFix => "successful_fix",
        MemoryKind::Decision => "decision",
        MemoryKind::SkillCandidate => "skill_candidate",
        MemoryKind::Note => "note",
    }
}

pub fn status_label(status: MemoryStatus) -> &'static str {
    match status {
        MemoryStatus::Proposed => "proposed",
        MemoryStatus::Accepted => "accepted",
        MemoryStatus::Rejected => "rejected",
        MemoryStatus::Superseded => "superseded",
        MemoryStatus::Archived => "archived",
    }
}

pub fn normalized_contains(existing: &str, candidate: &str) -> bool {
    let normalized_existing = normalize_for_duplicate(existing);
    let normalized_candidate = normalize_for_duplicate(candidate);
    !normalized_candidate.is_empty() && normalized_existing.contains(&normalized_candidate)
}

pub fn normalize_for_duplicate(content: &str) -> String {
    content
        .to_lowercase()
        .replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "")
}

pub fn default_candidate_evidence(candidate: &MemoryCandidate) -> Vec<MemoryEvidenceRef> {
    let source = candidate.provenance.source.clone();
    let summary = format!(
        "memory candidate submitted from {}",
        candidate.provenance.source
    );
    let kind = if matches!(candidate.kind, MemoryKind::UserPreference) {
        MemoryEvidenceKind::UserStatement
    } else if source.contains("tool")
        || source.contains("memory_save")
        || candidate.provenance.tool_name.is_some()
    {
        MemoryEvidenceKind::ToolOutput
    } else if source.contains("trace") {
        MemoryEvidenceKind::Trace
    } else if source.contains("learning_event") || source.contains("experience") {
        MemoryEvidenceKind::LearningEvent
    } else if source.contains("observer")
        || source.contains("stop")
        || source.contains("recovery")
        || source.contains("runtime")
    {
        MemoryEvidenceKind::RuntimeObservation
    } else {
        MemoryEvidenceKind::Inference
    };
    let confidence = if matches!(kind, MemoryEvidenceKind::Inference) {
        0.45
    } else {
        0.75
    };
    vec![MemoryEvidenceRef::new(kind, source, summary, confidence)]
}

pub fn evidence_status(candidate: &MemoryCandidate) -> &'static str {
    if candidate.evidence.is_empty() {
        "missing"
    } else if candidate
        .evidence
        .iter()
        .any(|evidence| !matches!(evidence.kind, MemoryEvidenceKind::Inference))
    {
        "verified"
    } else {
        "inferred"
    }
}

pub fn has_required_evidence(candidate: &MemoryCandidate) -> bool {
    match candidate.kind {
        MemoryKind::ProjectFact | MemoryKind::ToolQuirk => {
            candidate.evidence.iter().any(|evidence| {
                matches!(
                    evidence.kind,
                    MemoryEvidenceKind::File
                        | MemoryEvidenceKind::ToolOutput
                        | MemoryEvidenceKind::Trace
                        | MemoryEvidenceKind::RuntimeObservation
                )
            })
        }
        MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => {
            candidate.evidence.iter().any(|evidence| {
                matches!(
                    evidence.kind,
                    MemoryEvidenceKind::ToolOutput
                        | MemoryEvidenceKind::Trace
                        | MemoryEvidenceKind::RuntimeObservation
                )
            })
        }
        _ => true,
    }
}

pub fn requires_verified_evidence(kind: MemoryKind) -> bool {
    matches!(
        kind,
        MemoryKind::ProjectFact
            | MemoryKind::ToolQuirk
            | MemoryKind::FailurePattern
            | MemoryKind::SuccessfulFix
    )
}

pub fn infer_memory_importance(content: &str, category: &str) -> u8 {
    let base: u8 = match category {
        "preference" => 5,
        "workflow" => 3,
        "failure_pattern" => 3,
        "successful_fix" => 3,
        "project_fact" => 3,
        "tool_quirk" => 2,
        _ => 2,
    };
    let length_bonus = (content.len() / 80).min(1) as u8;
    base.saturating_add(length_bonus).min(5).max(1)
}

pub fn memory_scope_label(scope: &MemoryScope) -> String {
    scope.identity_label()
}

pub fn record_has_verified_evidence(record: &MemoryRecord) -> bool {
    record.evidence.iter().any(|evidence| {
        matches!(
            evidence.kind,
            MemoryEvidenceKind::UserStatement
                | MemoryEvidenceKind::ToolOutput
                | MemoryEvidenceKind::Trace
        )
    })
}

pub fn memory_lifecycle_key(record: &MemoryRecord) -> String {
    match record.kind {
        MemoryKind::ProjectFact | MemoryKind::ToolQuirk => normalize_lifecycle_key(&record.content),
        MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => {
            normalize_lifecycle_key(&record.content)
        }
        _ => String::new(),
    }
}

fn normalize_lifecycle_key(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .take(5)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn is_safe_memory_backup_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

pub fn record_needs_revalidation(record: &MemoryRecord) -> bool {
    let now = chrono::Utc::now();
    let created = record.created_at;
    let age = now.signed_duration_since(created);
    if age.num_days() > 90 {
        return true;
    }
    if let Some(stale_after) = record.stale_after {
        if now > stale_after {
            return true;
        }
    }
    if let Some(last_verified) = record.last_verified_at {
        let stale = now.signed_duration_since(last_verified);
        if stale.num_days() > 90 {
            return true;
        }
    }
    if let Some(last_used) = record.last_used_at {
        let stale = now.signed_duration_since(last_used);
        if stale.num_days() > 60 {
            return true;
        }
    }
    false
}

pub fn memory_messages_hash(messages: &[crate::services::api::Message]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for msg in messages {
        match msg {
            crate::services::api::Message::User { content } => {
                "user".hash(&mut hasher);
                content.hash(&mut hasher);
            }
            crate::services::api::Message::Assistant { content, .. } => {
                "assistant".hash(&mut hasher);
                content.hash(&mut hasher);
            }
            _ => {}
        }
    }
    hasher.finish()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryDecisionEvent {
    pub decision: String,
    pub kind: String,
    pub scope: String,
    pub score: Option<f32>,
    pub reason: String,
    pub evidence_status: String,
    pub content_preview: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub candidate_id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub scope_detail: Option<String>,
    #[serde(default)]
    pub kind_detail: Option<String>,
    #[serde(default)]
    pub score_detail: Option<f32>,
    #[serde(default)]
    pub evidence_status_detail: Option<String>,
    #[serde(default)]
    pub safety_status_detail: Option<String>,
}

pub fn memory_decision_event(
    decision: &str,
    candidate: &MemoryCandidate,
    score: Option<f32>,
    reason: &str,
    evidence_status: &str,
) -> MemoryDecisionEvent {
    MemoryDecisionEvent {
        decision: decision.to_string(),
        kind: kind_label(candidate.kind).to_string(),
        scope: memory_scope_label(&candidate.scope),
        score,
        reason: reason.to_string(),
        evidence_status: evidence_status.to_string(),
        content_preview: log_preview(&candidate.content, 120),
        created_at: chrono::Utc::now().to_rfc3339(),
        candidate_id: Some(candidate.id.clone()),
        source: Some(candidate.provenance.source.clone()),
        scope_detail: Some(memory_scope_label(&candidate.scope)),
        kind_detail: Some(kind_label(candidate.kind).to_string()),
        score_detail: score,
        evidence_status_detail: Some(evidence_status.to_string()),
        safety_status_detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind_label() {
        assert_eq!(kind_label(MemoryKind::UserPreference), "user_preference");
        assert_eq!(kind_label(MemoryKind::ProjectFact), "project_fact");
    }

    #[test]
    fn test_status_label() {
        assert_eq!(status_label(MemoryStatus::Accepted), "accepted");
        assert_eq!(status_label(MemoryStatus::Proposed), "proposed");
    }

    #[test]
    fn test_normalized_contains() {
        assert!(normalized_contains("Hello World", "hello"));
        assert!(!normalized_contains("Hello World", "xyz"));
    }
}

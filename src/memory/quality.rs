use crate::memory::safety::{scan_memory_content, MemorySafetyIssue};
use crate::memory::scoring::{
    memory_write_factors_from_signals, score_memory_write, MemoryWriteFactors,
};
use crate::memory::types::{MemoryKind, MemoryStatus, SensitivityLevel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQualityAssessment {
    pub stable_fact: f32,
    pub future_utility: f32,
    pub specificity: f32,
    pub relevance: f32,
    pub volatility: f32,
    pub sensitivity_risk: f32,
    pub duplication: f32,
    pub write_factors: MemoryWriteFactors,
    pub score: f32,
    pub threshold: f32,
    pub status: MemoryStatus,
    pub sensitivity: SensitivityLevel,
    pub reason: String,
}

impl MemoryQualityAssessment {
    pub fn accepted(&self) -> bool {
        self.status == MemoryStatus::Accepted
    }
}

pub fn assess_memory_candidate(
    content: &str,
    category: &str,
    existing_content: &str,
    explicit: bool,
) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
    let sensitivity = scan_memory_content(content)?;
    let kind = MemoryKind::from_category(category, content);
    let lower = content.to_lowercase();
    let char_count = content.chars().count();

    let stable_fact = match kind {
        MemoryKind::UserPreference
        | MemoryKind::WorkflowConvention
        | MemoryKind::Decision
        | MemoryKind::ProjectFact => 0.85,
        MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
        MemoryKind::SkillCandidate => 0.8,
        MemoryKind::Note => {
            if contains_any(&lower, &["remember", "记住", "always", "never"]) {
                0.65
            } else {
                0.45
            }
        }
    };

    let future_utility = if matches!(
        kind,
        MemoryKind::UserPreference
            | MemoryKind::ProjectFact
            | MemoryKind::WorkflowConvention
            | MemoryKind::Decision
            | MemoryKind::FailurePattern
            | MemoryKind::SuccessfulFix
            | MemoryKind::ToolQuirk
            | MemoryKind::SkillCandidate
    ) {
        0.80
    } else if contains_any(
        &lower,
        &[
            "prefer",
            "preference",
            "always",
            "never",
            "convention",
            "decision",
            "fix",
            "error",
            "path",
            "command",
            "workflow",
            "偏好",
            "习惯",
            "约定",
            "决策",
            "修复",
            "错误",
            "路径",
        ],
    ) {
        0.85
    } else if explicit {
        0.65
    } else {
        0.45
    };

    let specificity = if char_count < 12 {
        0.15
    } else if contains_any(
        &lower,
        &["/users/", ".rs", ".md", "cargo ", "npm ", "make "],
    ) || content.contains(':')
        || content.contains('`')
        || char_count >= 48
    {
        0.8
    } else if matches!(
        kind,
        MemoryKind::UserPreference
            | MemoryKind::ProjectFact
            | MemoryKind::WorkflowConvention
            | MemoryKind::Decision
            | MemoryKind::FailurePattern
            | MemoryKind::SuccessfulFix
            | MemoryKind::ToolQuirk
            | MemoryKind::SkillCandidate
    ) {
        0.65
    } else {
        0.55
    };

    let relevance = match kind {
        MemoryKind::UserPreference | MemoryKind::ProjectFact | MemoryKind::WorkflowConvention => {
            0.85
        }
        MemoryKind::Decision | MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 0.8,
        MemoryKind::ToolQuirk | MemoryKind::SkillCandidate => 0.75,
        MemoryKind::Note => {
            if explicit {
                0.65
            } else {
                0.45
            }
        }
    };

    let volatility = if contains_any(
        &lower,
        &[
            "today",
            "temporary",
            "for now",
            "maybe",
            "might",
            "今天",
            "临时",
            "暂时",
            "可能",
        ],
    ) {
        0.7
    } else {
        0.2
    };

    let sensitivity_risk = match sensitivity {
        SensitivityLevel::Public => 0.0,
        SensitivityLevel::LocalOnly => 0.15,
        SensitivityLevel::SecretLike => 0.85,
        SensitivityLevel::Unsafe => 1.0,
    };

    let duplication = duplicate_score(existing_content, content);
    let write_factors = memory_write_factors_from_signals(
        kind,
        content,
        stable_fact,
        future_utility,
        relevance,
        volatility,
        sensitivity_risk,
        duplication,
        explicit,
    );
    let write_decision = score_memory_write(write_factors, sensitivity, duplication, explicit);
    let score = write_decision.score;
    let threshold = write_decision.threshold;
    let status = write_decision.status;

    let reason = format!(
        "{}, kind={kind:?}, stable={stable_fact:.2}, utility={future_utility:.2}, specificity={specificity:.2}, volatility={volatility:.2}, duplication={duplication:.2}",
        write_decision.reason
    );

    Ok(MemoryQualityAssessment {
        stable_fact,
        future_utility,
        specificity,
        relevance,
        volatility,
        sensitivity_risk,
        duplication,
        write_factors,
        score,
        threshold,
        status,
        sensitivity,
        reason,
    })
}

fn duplicate_score(existing: &str, candidate: &str) -> f32 {
    let normalized_existing = normalize(existing);
    let normalized_candidate = normalize(candidate);
    if normalized_candidate.is_empty() {
        return 1.0;
    }
    if normalized_existing.contains(&normalized_candidate) {
        return 1.0;
    }

    let words = normalized_candidate.split_whitespace().collect::<Vec<_>>();
    if words.len() < 4 {
        return 0.0;
    }
    let hits = words
        .iter()
        .filter(|word| normalized_existing.contains(**word))
        .count();
    (hits as f32 / words.len() as f32).min(0.95)
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace(|c: char| c.is_ascii_punctuation(), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_specific_project_convention() {
        let assessment = assess_memory_candidate(
            "Project convention: run cargo test --quiet before committing Rust workflow changes.",
            "convention",
            "",
            false,
        )
        .unwrap();
        assert_eq!(assessment.status, MemoryStatus::Accepted);
        assert!(assessment.score >= 0.70);
    }

    #[test]
    fn proposes_vague_automatic_note() {
        let assessment =
            assess_memory_candidate("This might be useful later", "note", "", false).unwrap();
        assert!(matches!(
            assessment.status,
            MemoryStatus::Proposed | MemoryStatus::Rejected
        ));
    }

    #[test]
    fn explicit_does_not_accept_low_quality_note() {
        let assessment =
            assess_memory_candidate("This might be useful later", "note", "", true).unwrap();
        assert_ne!(assessment.status, MemoryStatus::Accepted);
        assert!(assessment.score < 0.60);
    }

    #[test]
    fn explicit_does_not_accept_duplicate_memory() {
        let existing =
            "Project convention: run cargo test --quiet before committing Rust workflow changes.";
        let assessment = assess_memory_candidate(existing, "convention", existing, true).unwrap();
        assert_ne!(assessment.status, MemoryStatus::Accepted);
        assert!(assessment.duplication >= 0.85);
    }

    #[test]
    fn blocks_secret_candidate() {
        let err = assess_memory_candidate(
            "The API token is sk-123456789012345678901234",
            "note",
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }

    #[test]
    fn explicit_save_cannot_override_secret_candidate() {
        let err = assess_memory_candidate(
            "password = sk-123456789012345678901234",
            "preference",
            "",
            true,
        )
        .unwrap_err();
        assert_eq!(err.code, "secret_like_content");
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }
}

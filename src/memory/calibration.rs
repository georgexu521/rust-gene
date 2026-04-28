use crate::memory::quality::assess_memory_candidate;
use crate::memory::types::MemoryStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCalibrationExpectation {
    Accepted,
    Proposed,
    Rejected,
    Blocked,
    NotAccepted,
}

impl MemoryCalibrationExpectation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Proposed => "proposed",
            Self::Rejected => "rejected",
            Self::Blocked => "blocked",
            Self::NotAccepted => "not_accepted",
        }
    }

    fn matches(self, actual: MemoryCalibrationActual) -> bool {
        match self {
            Self::Accepted => actual == MemoryCalibrationActual::Accepted,
            Self::Proposed => actual == MemoryCalibrationActual::Proposed,
            Self::Rejected => actual == MemoryCalibrationActual::Rejected,
            Self::Blocked => actual == MemoryCalibrationActual::Blocked,
            Self::NotAccepted => matches!(
                actual,
                MemoryCalibrationActual::Proposed
                    | MemoryCalibrationActual::Rejected
                    | MemoryCalibrationActual::Blocked
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCalibrationActual {
    Accepted,
    Proposed,
    Rejected,
    Blocked,
}

impl MemoryCalibrationActual {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Proposed => "proposed",
            Self::Rejected => "rejected",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCalibrationSample {
    pub id: &'static str,
    pub content: &'static str,
    pub category: &'static str,
    pub explicit: bool,
    pub existing_content: &'static str,
    pub expected: MemoryCalibrationExpectation,
    pub rationale: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCalibrationResult {
    pub id: String,
    pub expected: MemoryCalibrationExpectation,
    pub actual: MemoryCalibrationActual,
    pub score: Option<f32>,
    pub passed: bool,
    pub reason: String,
    pub rationale: String,
}

pub fn built_in_memory_calibration_samples() -> Vec<MemoryCalibrationSample> {
    vec![
        MemoryCalibrationSample {
            id: "project_test_command",
            content: "For this Rust project, run `cargo test --quiet -- --test-threads=1` before finishing workflow changes.",
            category: "convention",
            explicit: false,
            existing_content: "",
            expected: MemoryCalibrationExpectation::Accepted,
            rationale: "Stable, reusable project convention with direct risk reduction.",
        },
        MemoryCalibrationSample {
            id: "user_language_preference",
            content: "User prefers concise Chinese progress updates during coding tasks.",
            category: "preference",
            explicit: false,
            existing_content: "",
            expected: MemoryCalibrationExpectation::Accepted,
            rationale: "Stable user preference that should be injected across sessions.",
        },
        MemoryCalibrationSample {
            id: "vague_temporary_note",
            content: "This might be useful later today.",
            category: "note",
            explicit: false,
            existing_content: "",
            expected: MemoryCalibrationExpectation::NotAccepted,
            rationale: "Low specificity and low future utility should not enter long-term memory.",
        },
        MemoryCalibrationSample {
            id: "explicit_low_value_note",
            content: "Remember that this was interesting for now.",
            category: "note",
            explicit: true,
            existing_content: "",
            expected: MemoryCalibrationExpectation::NotAccepted,
            rationale: "Explicit save can lower friction but must not bypass quality gates.",
        },
        MemoryCalibrationSample {
            id: "duplicate_project_fact",
            content: "Use cargo test before committing this project.",
            category: "convention",
            explicit: true,
            existing_content: "Use cargo test before committing this project.",
            expected: MemoryCalibrationExpectation::Rejected,
            rationale: "Duplicate memories should be rejected even when explicit.",
        },
        MemoryCalibrationSample {
            id: "secret_like_token",
            content: "OPENAI_API_KEY=sk-123456789012345678901234",
            category: "note",
            explicit: true,
            existing_content: "",
            expected: MemoryCalibrationExpectation::Blocked,
            rationale: "Secret-like content must be blocked before scoring.",
        },
    ]
}

pub fn run_memory_calibration_samples() -> Vec<MemoryCalibrationResult> {
    built_in_memory_calibration_samples()
        .into_iter()
        .map(run_memory_calibration_sample)
        .collect()
}

fn run_memory_calibration_sample(sample: MemoryCalibrationSample) -> MemoryCalibrationResult {
    match assess_memory_candidate(
        sample.content,
        sample.category,
        sample.existing_content,
        sample.explicit,
    ) {
        Ok(assessment) => {
            let actual = match assessment.status {
                MemoryStatus::Accepted => MemoryCalibrationActual::Accepted,
                MemoryStatus::Proposed => MemoryCalibrationActual::Proposed,
                _ => MemoryCalibrationActual::Rejected,
            };
            MemoryCalibrationResult {
                id: sample.id.to_string(),
                expected: sample.expected,
                actual,
                score: Some(assessment.score),
                passed: sample.expected.matches(actual),
                reason: assessment.reason,
                rationale: sample.rationale.to_string(),
            }
        }
        Err(issue) => {
            let actual = MemoryCalibrationActual::Blocked;
            MemoryCalibrationResult {
                id: sample.id.to_string(),
                expected: sample.expected,
                actual,
                score: None,
                passed: sample.expected.matches(actual),
                reason: format!("[{}] {}", issue.code, issue.message),
                rationale: sample.rationale.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_calibration_samples_pass() {
        let results = run_memory_calibration_samples();
        assert!(!results.is_empty());
        let failed: Vec<_> = results.iter().filter(|result| !result.passed).collect();
        assert!(
            failed.is_empty(),
            "failed calibration samples: {:?}",
            failed
        );
    }

    #[test]
    fn calibration_includes_secret_and_explicit_gate_samples() {
        let samples = built_in_memory_calibration_samples();
        assert!(samples
            .iter()
            .any(|sample| sample.id == "secret_like_token"));
        assert!(samples
            .iter()
            .any(|sample| sample.id == "explicit_low_value_note" && sample.explicit));
    }
}

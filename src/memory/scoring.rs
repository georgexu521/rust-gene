use crate::memory::types::{MemoryKind, MemoryStatus, SensitivityLevel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MemoryWriteFactors {
    pub relevance: f32,
    pub reuse_probability: f32,
    pub stability: f32,
    pub trust: f32,
    pub novelty: f32,
    pub risk_reduction: f32,
    pub token_cost: f32,
    pub sensitivity_risk: f32,
}

impl MemoryWriteFactors {
    pub fn clamped(self) -> Self {
        Self {
            relevance: clamp01(self.relevance),
            reuse_probability: clamp01(self.reuse_probability),
            stability: clamp01(self.stability),
            trust: clamp01(self.trust),
            novelty: clamp01(self.novelty),
            risk_reduction: clamp01(self.risk_reduction),
            token_cost: clamp01(self.token_cost),
            sensitivity_risk: clamp01(self.sensitivity_risk),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWriteDecision {
    pub factors: MemoryWriteFactors,
    pub score: f32,
    pub status: MemoryStatus,
    pub threshold: f32,
    pub explicit_override: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMaintenanceAction {
    KeepActive,
    CompressOrDemote,
    ArchiveCandidate,
    ReviewConflict,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MemoryKeepFactors {
    pub recent_use: f32,
    pub historical_usefulness: f32,
    pub trust: f32,
    pub stability: f32,
    pub scope_importance: f32,
    pub contradiction_risk: f32,
    pub redundancy: f32,
}

impl MemoryKeepFactors {
    pub fn clamped(self) -> Self {
        Self {
            recent_use: clamp01(self.recent_use),
            historical_usefulness: clamp01(self.historical_usefulness),
            trust: clamp01(self.trust),
            stability: clamp01(self.stability),
            scope_importance: clamp01(self.scope_importance),
            contradiction_risk: clamp01(self.contradiction_risk),
            redundancy: clamp01(self.redundancy),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryKeepDecision {
    pub factors: MemoryKeepFactors,
    pub score: f32,
    pub action: MemoryMaintenanceAction,
    pub reason: String,
}

pub fn score_memory_write(
    factors: MemoryWriteFactors,
    sensitivity: SensitivityLevel,
    duplication: f32,
    explicit: bool,
) -> MemoryWriteDecision {
    let factors = factors.clamped();
    let score = (factors.relevance * 0.25
        + factors.reuse_probability * 0.20
        + factors.stability * 0.15
        + factors.trust * 0.15
        + factors.novelty * 0.10
        + factors.risk_reduction * 0.10
        - factors.token_cost * 0.15
        - factors.sensitivity_risk * 0.20)
        .clamp(0.0, 1.0);

    let explicit_override =
        explicit && score >= 0.60 && duplication < 0.85 && factors.sensitivity_risk < 0.50;
    let status = if matches!(
        sensitivity,
        SensitivityLevel::Unsafe | SensitivityLevel::SecretLike
    ) {
        MemoryStatus::Rejected
    } else if duplication >= 0.85 {
        MemoryStatus::Rejected
    } else if score >= 0.65 || explicit_override {
        MemoryStatus::Accepted
    } else if score >= 0.45 {
        MemoryStatus::Proposed
    } else {
        MemoryStatus::Rejected
    };

    let threshold = match status {
        MemoryStatus::Accepted => 0.65,
        MemoryStatus::Proposed => 0.45,
        _ => 0.0,
    };
    let reason = format!(
        "write_score={score:.2}, status={status:?}, relevance={:.2}, reuse={:.2}, stability={:.2}, trust={:.2}, novelty={:.2}, risk_reduction={:.2}, token_cost={:.2}, sensitivity_risk={:.2}",
        factors.relevance,
        factors.reuse_probability,
        factors.stability,
        factors.trust,
        factors.novelty,
        factors.risk_reduction,
        factors.token_cost,
        factors.sensitivity_risk
    );

    MemoryWriteDecision {
        factors,
        score,
        status,
        threshold,
        explicit_override,
        reason,
    }
}

pub fn score_memory_keep(factors: MemoryKeepFactors) -> MemoryKeepDecision {
    let factors = factors.clamped();
    let score = (factors.recent_use * 0.25
        + factors.historical_usefulness * 0.25
        + factors.trust * 0.20
        + factors.stability * 0.15
        + factors.scope_importance * 0.15
        - factors.contradiction_risk * 0.20
        - factors.redundancy * 0.15)
        .clamp(0.0, 1.0);

    let action = if factors.contradiction_risk >= 0.70 {
        MemoryMaintenanceAction::ReviewConflict
    } else if score >= 0.65 {
        MemoryMaintenanceAction::KeepActive
    } else if score >= 0.40 {
        MemoryMaintenanceAction::CompressOrDemote
    } else {
        MemoryMaintenanceAction::ArchiveCandidate
    };

    let reason = format!(
        "keep_score={score:.2}, action={action:?}, recent_use={:.2}, usefulness={:.2}, trust={:.2}, stability={:.2}, scope={:.2}, contradiction={:.2}, redundancy={:.2}",
        factors.recent_use,
        factors.historical_usefulness,
        factors.trust,
        factors.stability,
        factors.scope_importance,
        factors.contradiction_risk,
        factors.redundancy
    );

    MemoryKeepDecision {
        factors,
        score,
        action,
        reason,
    }
}

pub fn memory_keep_factors_from_document(
    namespace: &str,
    content: &str,
    has_conflict: bool,
    redundancy: f32,
) -> MemoryKeepFactors {
    let lower = content.to_lowercase();
    let content_chars = content.chars().count();
    let useful_markers = [
        "convention",
        "decision",
        "workflow",
        "tested",
        "verified",
        "failed",
        "fix",
        "project",
        "preference",
        "约定",
        "决策",
        "流程",
        "测试",
        "验证",
        "失败",
        "修复",
        "项目",
        "偏好",
    ];
    let marker_hits = useful_markers
        .iter()
        .filter(|marker| lower.contains(**marker))
        .count() as f32;
    let historical_usefulness = (0.35 + marker_hits * 0.08).min(0.90);
    let scope_importance = match namespace {
        "project" | "user" => 0.85,
        "topic" => 0.70,
        ns if ns.starts_with("agent") => 0.55,
        _ => 0.50,
    };
    let recent_use = match namespace {
        "project" | "user" => 0.70,
        "topic" => 0.55,
        _ => 0.45,
    };
    let stability = if contains_any(
        &lower,
        &["temporary", "for now", "today", "临时", "暂时", "今天"],
    ) {
        0.35
    } else {
        0.72
    };
    let trust = if contains_any(
        &lower,
        &["verified", "tested", "confirmed", "验证", "测试", "确认"],
    ) {
        0.82
    } else {
        0.62
    };
    let size_pressure = if content_chars > 20_000 {
        0.20
    } else if content_chars > 8_000 {
        0.10
    } else {
        0.0
    };

    MemoryKeepFactors {
        recent_use,
        historical_usefulness,
        trust,
        stability,
        scope_importance,
        contradiction_risk: if has_conflict { 0.85 } else { 0.0 },
        redundancy: (redundancy + size_pressure).clamp(0.0, 1.0),
    }
}

pub fn memory_write_factors_from_signals(
    kind: MemoryKind,
    content: &str,
    stable_fact: f32,
    future_utility: f32,
    relevance: f32,
    volatility: f32,
    sensitivity_risk: f32,
    duplication: f32,
    explicit: bool,
) -> MemoryWriteFactors {
    let lower = content.to_lowercase();
    let stability = (stable_fact - volatility * 0.45).clamp(0.0, 1.0);
    let trust = evidence_trust(&lower, kind, explicit);
    let novelty = (1.0 - duplication).clamp(0.0, 1.0);
    let risk_reduction = risk_reduction_value(&lower, kind);
    let token_cost = token_cost(content);

    let vague_short_penalty = if content.chars().count() < 12 {
        0.45
    } else {
        1.0
    };

    MemoryWriteFactors {
        relevance: relevance * vague_short_penalty,
        reuse_probability: future_utility * vague_short_penalty,
        stability,
        trust: trust * vague_short_penalty,
        novelty,
        risk_reduction: risk_reduction * vague_short_penalty,
        token_cost,
        sensitivity_risk,
    }
}

fn evidence_trust(lower: &str, kind: MemoryKind, explicit: bool) -> f32 {
    let evidence_boost = contains_any(
        lower,
        &[
            "tested",
            "verified",
            "observed",
            "confirmed",
            "passed",
            "failed",
            "测试",
            "验证",
            "确认",
            "通过",
            "失败",
        ],
    );
    if evidence_boost {
        0.85
    } else if matches!(
        kind,
        MemoryKind::WorkflowConvention
            | MemoryKind::Decision
            | MemoryKind::FailurePattern
            | MemoryKind::SuccessfulFix
            | MemoryKind::ToolQuirk
    ) {
        0.72
    } else if explicit {
        0.70
    } else {
        0.58
    }
}

fn risk_reduction_value(lower: &str, kind: MemoryKind) -> f32 {
    if matches!(
        kind,
        MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk
    ) || contains_any(
        lower,
        &[
            "avoid", "prevent", "error", "failed", "panic", "rollback", "避免", "防止", "错误",
            "失败", "回滚",
        ],
    ) {
        0.85
    } else if matches!(kind, MemoryKind::WorkflowConvention | MemoryKind::Decision) {
        0.65
    } else if matches!(kind, MemoryKind::ProjectFact | MemoryKind::SkillCandidate) {
        0.55
    } else {
        0.25
    }
}

fn token_cost(content: &str) -> f32 {
    let approx_tokens = content.chars().count() as f32 / 4.0;
    (approx_tokens / 220.0).clamp(0.0, 1.0)
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_high_value_memory_write() {
        let decision = score_memory_write(
            MemoryWriteFactors {
                relevance: 0.9,
                reuse_probability: 0.85,
                stability: 0.8,
                trust: 0.8,
                novelty: 1.0,
                risk_reduction: 0.7,
                token_cost: 0.05,
                sensitivity_risk: 0.0,
            },
            SensitivityLevel::Public,
            0.0,
            false,
        );
        assert_eq!(decision.status, MemoryStatus::Accepted);
        assert!(decision.score >= 0.65);
    }

    #[test]
    fn duplicate_memory_write_is_rejected() {
        let decision = score_memory_write(
            MemoryWriteFactors {
                relevance: 1.0,
                reuse_probability: 1.0,
                stability: 1.0,
                trust: 1.0,
                novelty: 0.0,
                risk_reduction: 1.0,
                token_cost: 0.0,
                sensitivity_risk: 0.0,
            },
            SensitivityLevel::Public,
            0.9,
            true,
        );
        assert_eq!(decision.status, MemoryStatus::Rejected);
    }

    #[test]
    fn memory_keep_conflict_requires_review() {
        let decision = score_memory_keep(MemoryKeepFactors {
            recent_use: 0.9,
            historical_usefulness: 0.9,
            trust: 0.9,
            stability: 0.9,
            scope_importance: 0.9,
            contradiction_risk: 0.9,
            redundancy: 0.0,
        });
        assert_eq!(decision.action, MemoryMaintenanceAction::ReviewConflict);
    }

    #[test]
    fn memory_keep_low_value_archive_candidate() {
        let decision = score_memory_keep(MemoryKeepFactors {
            recent_use: 0.0,
            historical_usefulness: 0.1,
            trust: 0.2,
            stability: 0.2,
            scope_importance: 0.2,
            contradiction_risk: 0.0,
            redundancy: 1.0,
        });
        assert_eq!(decision.action, MemoryMaintenanceAction::ArchiveCandidate);
    }
}

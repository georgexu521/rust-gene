use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallDecision {
    Inject,
    Available,
    Omit,
    ConflictCapped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RecallFactors {
    pub match_quality: f32,
    pub scope_match: f32,
    pub recency: f32,
    pub trust: f32,
    pub prior_usefulness: f32,
    pub task_criticality: f32,
    pub token_cost: f32,
}

impl RecallFactors {
    pub fn clamped(self) -> Self {
        Self {
            match_quality: self.match_quality.clamp(0.0, 1.0),
            scope_match: self.scope_match.clamp(0.0, 1.0),
            recency: self.recency.clamp(0.0, 1.0),
            trust: self.trust.clamp(0.0, 1.0),
            prior_usefulness: self.prior_usefulness.clamp(0.0, 1.0),
            task_criticality: self.task_criticality.clamp(0.0, 1.0),
            token_cost: self.token_cost.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallScore {
    pub factors: RecallFactors,
    pub score: f32,
    pub decision: RecallDecision,
    pub reason: String,
}

pub fn score_recall(factors: RecallFactors, conflict: bool) -> RecallScore {
    let factors = factors.clamped();
    let base = (factors.match_quality * 0.30
        + factors.scope_match * 0.20
        + factors.recency * 0.15
        + factors.trust * 0.15
        + factors.prior_usefulness * 0.10
        + factors.task_criticality * 0.10
        - factors.token_cost * 0.15)
        .clamp(0.0, 1.0);
    let score = if conflict {
        (base * 0.55).min(0.49)
    } else {
        base
    };
    let decision = if conflict {
        RecallDecision::ConflictCapped
    } else if score >= 0.70 {
        RecallDecision::Inject
    } else if score >= 0.50 {
        RecallDecision::Available
    } else {
        RecallDecision::Omit
    };
    let reason = format!(
        "recall_score={score:.2}, decision={decision:?}, match_quality={:.2}, scope={:.2}, recency={:.2}, trust={:.2}, usefulness={:.2}, criticality={:.2}, token_cost={:.2}",
        factors.match_quality,
        factors.scope_match,
        factors.recency,
        factors.trust,
        factors.prior_usefulness,
        factors.task_criticality,
        factors.token_cost
    );
    RecallScore {
        factors,
        score,
        decision,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_caps_recall_below_inject_range() {
        let score = score_recall(
            RecallFactors {
                match_quality: 1.0,
                scope_match: 1.0,
                recency: 1.0,
                trust: 1.0,
                prior_usefulness: 1.0,
                task_criticality: 1.0,
                token_cost: 0.0,
            },
            true,
        );
        assert_eq!(score.decision, RecallDecision::ConflictCapped);
        assert!(score.score < 0.50);
    }
}

//! Learning-to-planning feedback.
//!
//! Runtime learning and high-confidence retrieval should not silently replace
//! model judgment. This module applies bounded, auditable factor adjustments to
//! an existing model-led workflow judgment.

use crate::engine::retrieval_context::{RetrievalContext, RetrievalSource, TrustLevel};
use crate::engine::workflow_contract::{
    normalize_weight_shares, recompute_step_weight, ProgrammingWorkflowJudgment, WeightFactors,
};
use crate::session_store::LearningEventRecord;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPlanningAudit {
    pub applied: bool,
    pub explanation: String,
    pub before_top_step: Option<String>,
    pub after_top_step: Option<String>,
    pub before_plan: Vec<serde_json::Value>,
    pub after_plan: Vec<serde_json::Value>,
    pub adjustments: Vec<LearningPlanningAdjustment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPlanningAdjustment {
    pub step_id: Option<String>,
    pub step_description: String,
    pub source: String,
    pub kind: String,
    pub reason: String,
    pub factor_delta: serde_json::Value,
}

#[derive(Debug, Default)]
struct LearningPlanningSignals {
    failed_tools: Vec<String>,
    failed_workflows: usize,
    recovery_plans: usize,
    repeated_success_patterns: Vec<String>,
    high_confidence_memories: Vec<MemoryPlanningSignal>,
    memory_conflicts: usize,
}

#[derive(Debug, Clone)]
struct MemoryPlanningSignal {
    title: String,
    content: String,
    score: f32,
    trust: TrustLevel,
}

pub fn apply_learning_to_workflow_judgment(
    judgment: &mut ProgrammingWorkflowJudgment,
    events: &[LearningEventRecord],
    retrieval_context: Option<&RetrievalContext>,
) -> LearningPlanningAudit {
    let before_plan = judgment.weighted_plan_summary();
    let before_top_step = judgment.top_plan_step().map(|step| step.description);
    let signals = LearningPlanningSignals::from_inputs(events, retrieval_context);
    let mut adjustments = Vec::new();

    if signals.is_empty() || judgment.plan.is_empty() {
        return LearningPlanningAudit {
            applied: false,
            explanation: "No relevant learning or memory signals for weighted planning."
                .to_string(),
            before_top_step,
            after_top_step: before_plan
                .first()
                .and_then(|item| item.get("description"))
                .and_then(|value| value.as_str())
                .map(ToString::to_string),
            before_plan: before_plan.clone(),
            after_plan: before_plan,
            adjustments,
        };
    }

    for step in &mut judgment.plan {
        let mut factors = step
            .factors
            .unwrap_or_else(|| WeightFactors::from_priority(step.priority));
        let before = factors;
        let mut reasons = Vec::new();

        if signals.failed_workflows > 0 || signals.recovery_plans > 0 {
            let strength = ((signals.failed_workflows + signals.recovery_plans) as f32 * 0.08)
                .clamp(0.08, 0.24);
            if is_verification_or_recovery_step(step.description.as_str(), step.reason.as_str()) {
                factors.risk_reduction += strength;
                factors.uncertainty_reduction += strength * 0.75;
                factors.blocking += strength * 0.60;
                reasons.push(format!(
                    "past failed workflow/recovery signal raised verification priority ({:.2})",
                    strength
                ));
            }
        }

        for tool in &signals.failed_tools {
            if step_mentions(step.description.as_str(), step.reason.as_str(), tool)
                || is_verification_or_recovery_step(step.description.as_str(), step.reason.as_str())
            {
                factors.risk_reduction += 0.08;
                factors.uncertainty_reduction += 0.10;
                factors.blocking += 0.06;
                reasons.push(format!(
                    "recent {} failure increased recovery/validation weight",
                    tool
                ));
            }
        }

        if !signals.repeated_success_patterns.is_empty()
            && is_exploration_step(step.description.as_str(), step.reason.as_str())
            && repeated_success_matches(
                &signals.repeated_success_patterns,
                step.description.as_str(),
                step.reason.as_str(),
            )
        {
            factors.uncertainty_reduction -= 0.10;
            factors.blocking -= 0.08;
            factors.cost += 0.05;
            reasons.push(
                "repeated successful pattern reduced unnecessary exploration weight".to_string(),
            );
        }

        if !signals.high_confidence_memories.is_empty() {
            let relevance = memory_step_relevance(
                &signals.high_confidence_memories,
                step.description.as_str(),
                step.reason.as_str(),
            );
            if relevance >= 0.25
                && is_memory_sensitive_step(step.description.as_str(), step.reason.as_str())
            {
                let strength = (relevance * 0.12).clamp(0.03, 0.12);
                factors.dependency += strength;
                factors.uncertainty_reduction += strength;
                reasons.push(format!(
                    "relevant high-confidence memory raised context-sensitive planning weight ({:.2}, relevance {:.2})",
                    strength, relevance
                ));
            }
        }

        if signals.memory_conflicts > 0 {
            let strength = (signals.memory_conflicts as f32 * 0.07).clamp(0.07, 0.18);
            if is_memory_sensitive_step(step.description.as_str(), step.reason.as_str())
                || is_verification_or_recovery_step(step.description.as_str(), step.reason.as_str())
            {
                factors.uncertainty_reduction += strength;
                factors.risk_reduction += strength * 0.70;
                factors.blocking += strength * 0.50;
                reasons.push(format!(
                    "conflicting memory increased clarification/verification weight ({:.2})",
                    strength
                ));
            }
        }

        if reasons.is_empty() {
            continue;
        }

        step.factors = Some(factors);
        recompute_step_weight(step);
        let after = step.factors.unwrap_or(factors);
        adjustments.push(LearningPlanningAdjustment {
            step_id: step.id.clone(),
            step_description: step.description.clone(),
            source: "learning_to_planning".to_string(),
            kind: "factor_adjustment".to_string(),
            reason: reasons.join("; "),
            factor_delta: factor_delta_json(before, after),
        });
    }

    normalize_weight_shares(&mut judgment.plan);
    let after_plan = judgment.weighted_plan_summary();
    let after_top_step = judgment.top_plan_step().map(|step| step.description);
    let applied = !adjustments.is_empty();

    LearningPlanningAudit {
        applied,
        explanation: if applied {
            format!(
                "Applied {} learning/memory planning adjustment(s).",
                adjustments.len()
            )
        } else {
            "Learning and memory signals were present but did not match any plan step.".to_string()
        },
        before_top_step,
        after_top_step,
        before_plan,
        after_plan,
        adjustments,
    }
}

impl LearningPlanningSignals {
    fn from_inputs(
        events: &[LearningEventRecord],
        retrieval_context: Option<&RetrievalContext>,
    ) -> Self {
        let mut signals = Self::default();
        let mut tool_failures: HashMap<String, usize> = HashMap::new();
        let mut success_patterns: HashMap<String, usize> = HashMap::new();

        for event in events.iter().take(50) {
            match event.kind.as_str() {
                "tool_outcome" => {
                    let tool = event
                        .payload
                        .get("tool")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    let success = event
                        .payload
                        .get("success")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false);
                    if !tool.is_empty() && !success {
                        *tool_failures.entry(tool.to_string()).or_default() += 1;
                    }
                }
                "workflow_outcome" | "turn_outcome" => {
                    let success = event
                        .payload
                        .get("success")
                        .and_then(|value| value.as_bool())
                        .or_else(|| {
                            event.payload.get("status").and_then(|value| {
                                value.as_str().map(|status| {
                                    matches!(status, "Completed" | "completed" | "success")
                                })
                            })
                        })
                        .unwrap_or(false);
                    if success {
                        if let Some(pattern) = procedure_pattern(event) {
                            *success_patterns.entry(pattern).or_default() += 1;
                        }
                    } else {
                        signals.failed_workflows += 1;
                    }
                }
                "recovery_plan" => signals.recovery_plans += 1,
                _ => {}
            }
        }

        signals.failed_tools = tool_failures
            .into_iter()
            .filter(|(_, count)| *count >= 1)
            .map(|(tool, _)| tool)
            .collect();
        signals.failed_tools.sort();
        signals.failed_tools.truncate(5);
        signals.repeated_success_patterns = success_patterns
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(pattern, _)| pattern)
            .collect();
        signals.repeated_success_patterns.sort();
        signals.repeated_success_patterns.truncate(5);

        if let Some(ctx) = retrieval_context {
            for item in &ctx.items {
                if item.source != RetrievalSource::Memory {
                    continue;
                }
                if item.conflict {
                    signals.memory_conflicts += 1;
                }
                if item.score >= 0.70 && matches!(item.trust, TrustLevel::High | TrustLevel::Medium)
                {
                    signals.high_confidence_memories.push(MemoryPlanningSignal {
                        title: item.title.clone(),
                        content: item.content_preview.clone(),
                        score: item.score,
                        trust: item.trust,
                    });
                }
            }
        }

        signals
    }

    fn is_empty(&self) -> bool {
        self.failed_tools.is_empty()
            && self.failed_workflows == 0
            && self.recovery_plans == 0
            && self.repeated_success_patterns.is_empty()
            && self.high_confidence_memories.is_empty()
            && self.memory_conflicts == 0
    }
}

fn procedure_pattern(event: &LearningEventRecord) -> Option<String> {
    for key in ["procedure", "workflow", "pattern", "task_type"] {
        if let Some(value) = event.payload.get(key).and_then(|value| value.as_str()) {
            let normalized = normalize_text(value);
            if !normalized.is_empty() {
                return Some(normalized);
            }
        }
    }
    let normalized = normalize_text(&event.summary);
    (!normalized.is_empty()).then_some(normalized)
}

fn repeated_success_matches(patterns: &[String], description: &str, reason: &str) -> bool {
    let step_tokens = informative_tokens(&format!("{} {}", description, reason));
    if step_tokens.is_empty() {
        return false;
    }
    patterns.iter().any(|pattern| {
        let pattern_tokens = informative_tokens(pattern);
        token_sets_related(&pattern_tokens, &step_tokens)
    })
}

fn memory_step_relevance(
    memories: &[MemoryPlanningSignal],
    description: &str,
    reason: &str,
) -> f32 {
    let step_tokens = informative_tokens(&format!("{} {}", description, reason));
    if step_tokens.is_empty() {
        return 0.0;
    }

    memories
        .iter()
        .map(|memory| {
            let memory_tokens = informative_tokens(&format!("{} {}", memory.title, memory.content));
            let overlap = overlap_count(&memory_tokens, &step_tokens);
            if overlap == 0 {
                return 0.0;
            }
            let union = memory_tokens.union(&step_tokens).count().max(1) as f32;
            let jaccard = overlap as f32 / union;
            let trust_factor = match memory.trust {
                TrustLevel::High => 1.0,
                TrustLevel::Medium => 0.82,
                TrustLevel::Low => 0.45,
            };
            let overlap_factor = if overlap >= 2 { 1.0 } else { 0.45 };
            (jaccard * 0.60 + memory.score * 0.40) * trust_factor * overlap_factor
        })
        .fold(0.0, f32::max)
        .clamp(0.0, 1.0)
}

fn token_sets_related(left: &HashSet<String>, right: &HashSet<String>) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let overlap = overlap_count(left, right);
    if overlap >= 2 {
        return true;
    }
    if overlap == 0 {
        return false;
    }
    let union = left.union(right).count().max(1) as f32;
    let jaccard = overlap as f32 / union;
    jaccard >= 0.45 && left.len().min(right.len()) <= 2
}

fn overlap_count(left: &HashSet<String>, right: &HashSet<String>) -> usize {
    left.intersection(right).count()
}

fn step_mentions(description: &str, reason: &str, needle: &str) -> bool {
    let haystack = format!("{} {}", description, reason).to_lowercase();
    haystack.contains(&needle.to_lowercase())
}

fn is_verification_or_recovery_step(description: &str, reason: &str) -> bool {
    contains_any(
        &format!("{} {}", description, reason),
        &[
            "verify", "test", "check", "validate", "验收", "测试", "验证", "修复", "recover",
            "debug", "诊断",
        ],
    )
}

fn is_exploration_step(description: &str, reason: &str) -> bool {
    contains_any(
        &format!("{} {}", description, reason),
        &[
            "inspect",
            "explore",
            "search",
            "read",
            "scan",
            "investigate",
            "查看",
            "搜索",
            "读取",
            "分析",
        ],
    )
}

fn is_memory_sensitive_step(description: &str, reason: &str) -> bool {
    contains_any(
        &format!("{} {}", description, reason),
        &[
            "context",
            "memory",
            "preference",
            "requirement",
            "project",
            "上下文",
            "记忆",
            "偏好",
            "需求",
            "项目",
        ],
    )
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn normalize_text(value: &str) -> String {
    value
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|word| word.len() > 2)
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
}

fn informative_tokens(value: &str) -> HashSet<String> {
    normalize_text(value)
        .split_whitespace()
        .filter(|word| word.chars().count() > 3)
        .filter(|word| !is_planning_stopword(word))
        .map(ToString::to_string)
        .collect()
}

fn is_planning_stopword(word: &str) -> bool {
    matches!(
        word,
        "project"
            | "context"
            | "workflow"
            | "task"
            | "tasks"
            | "test"
            | "tests"
            | "step"
            | "steps"
            | "plan"
            | "planning"
            | "check"
            | "verify"
            | "validation"
            | "memory"
            | "agent"
    )
}

fn factor_delta_json(before: WeightFactors, after: WeightFactors) -> serde_json::Value {
    serde_json::json!({
        "dependency": after.dependency - before.dependency,
        "user_value": after.user_value - before.user_value,
        "risk_reduction": after.risk_reduction - before.risk_reduction,
        "uncertainty_reduction": after.uncertainty_reduction - before.uncertainty_reduction,
        "blocking": after.blocking - before.blocking,
        "cost": after.cost - before.cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{RetrievalPolicy, RiskLevel};
    use crate::engine::retrieval_context::{RetrievalItem, RetrievalSource, TrustLevel};
    use crate::engine::workflow_contract::{
        AcceptanceContract, PriorityLabel, TaskComplexity, WorkflowPlanStep,
    };

    fn event(
        id: i64,
        kind: &str,
        summary: &str,
        payload: serde_json::Value,
    ) -> LearningEventRecord {
        LearningEventRecord {
            id,
            session_id: "s1".to_string(),
            kind: kind.to_string(),
            source: "test".to_string(),
            summary: summary.to_string(),
            confidence: 0.9,
            payload,
            created_at: "2026-04-28T00:00:00Z".to_string(),
        }
    }

    fn judgment() -> ProgrammingWorkflowJudgment {
        ProgrammingWorkflowJudgment {
            task_type: "bug_fix".to_string(),
            complexity: TaskComplexity::Medium,
            risk: RiskLevel::Medium,
            requirement_complete_enough: true,
            needs_user_questions: false,
            question_reason: None,
            questions: Vec::new(),
            assumptions: Vec::new(),
            guided_reasoning_required: false,
            guided_reasoning_triggers: Vec::new(),
            plan: vec![
                WorkflowPlanStep {
                    id: Some("inspect".to_string()),
                    description: "Inspect project context".to_string(),
                    priority: PriorityLabel::P1,
                    weight: None,
                    importance_score: None,
                    weight_share: None,
                    factors: Some(WeightFactors {
                        dependency: 0.7,
                        user_value: 0.5,
                        risk_reduction: 0.4,
                        uncertainty_reduction: 0.8,
                        blocking: 0.4,
                        cost: 0.2,
                    }),
                    override_adjustment: None,
                    computation: None,
                    reason: "Need context before editing".to_string(),
                    acceptance_criteria: Vec::new(),
                },
                WorkflowPlanStep {
                    id: Some("verify".to_string()),
                    description: "Run tests and verify fix".to_string(),
                    priority: PriorityLabel::P2,
                    weight: None,
                    importance_score: None,
                    weight_share: None,
                    factors: Some(WeightFactors {
                        dependency: 0.4,
                        user_value: 0.6,
                        risk_reduction: 0.5,
                        uncertainty_reduction: 0.4,
                        blocking: 0.4,
                        cost: 0.2,
                    }),
                    override_adjustment: None,
                    computation: None,
                    reason: "Validate behavior".to_string(),
                    acceptance_criteria: Vec::new(),
                },
            ],
            acceptance: AcceptanceContract::pending("fix bug", vec!["tests pass".into()], vec![]),
        }
    }

    #[test]
    fn past_failures_raise_verification_weight() {
        let mut judgment = judgment();
        for step in &mut judgment.plan {
            recompute_step_weight(step);
        }
        normalize_weight_shares(&mut judgment.plan);
        let before = judgment.plan[1].normalized_weight();
        let events = vec![
            event(
                1,
                "tool_outcome",
                "bash failed",
                serde_json::json!({"tool": "bash", "success": false}),
            ),
            event(
                2,
                "workflow_outcome",
                "workflow failed",
                serde_json::json!({"success": false}),
            ),
        ];

        let audit = apply_learning_to_workflow_judgment(&mut judgment, &events, None);
        assert!(audit.applied);
        assert!(judgment.plan[1].normalized_weight() > before);
        assert!(audit
            .adjustments
            .iter()
            .any(|item| item.step_id.as_deref() == Some("verify")));
    }

    #[test]
    fn repeated_success_reduces_exploration_weight() {
        let mut judgment = judgment();
        judgment.plan[0].description = "Inspect cargo failure context".to_string();
        judgment.plan[0].reason = "Read cargo diagnostics before editing".to_string();
        for step in &mut judgment.plan {
            recompute_step_weight(step);
        }
        normalize_weight_shares(&mut judgment.plan);
        let before = judgment.plan[0].normalized_weight();
        let events = vec![
            event(
                1,
                "workflow_outcome",
                "cargo failure inspection workflow succeeded",
                serde_json::json!({"success": true, "procedure": "cargo failure inspection"}),
            ),
            event(
                2,
                "workflow_outcome",
                "cargo failure inspection workflow succeeded again",
                serde_json::json!({"success": true, "procedure": "cargo failure inspection"}),
            ),
        ];
        let audit = apply_learning_to_workflow_judgment(&mut judgment, &events, None);
        assert!(audit.applied);
        assert!(judgment.plan[0].normalized_weight() < before);
    }

    #[test]
    fn repeated_success_does_not_match_generic_words_only() {
        let events = vec!["project context workflow", "context project workflow"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();

        assert!(!repeated_success_matches(
            &events,
            "Inspect project context",
            "Need context before editing"
        ));
    }

    #[test]
    fn high_confidence_memory_adjusts_context_step() {
        let mut judgment = judgment();
        judgment.plan[0].description = "Inspect cargo diagnostics".to_string();
        judgment.plan[0].reason = "Need compile failure context before editing".to_string();
        for step in &mut judgment.plan {
            recompute_step_weight(step);
        }
        normalize_weight_shares(&mut judgment.plan);
        let before = judgment.plan[0].normalized_weight();
        let mut ctx = RetrievalContext::new("fix bug", RetrievalPolicy::Project);
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Memory,
            "USER.md",
            "Cargo diagnostics should be inspected before editing compile failures",
            0.9,
            "memory.match:USER.md",
            TrustLevel::High,
        ));
        let audit = apply_learning_to_workflow_judgment(&mut judgment, &[], Some(&ctx));
        assert!(audit.applied);
        assert!(judgment.plan[0].normalized_weight() > before);
    }

    #[test]
    fn unrelated_high_confidence_memory_does_not_adjust_context_step() {
        let mut judgment = judgment();
        for step in &mut judgment.plan {
            recompute_step_weight(step);
        }
        normalize_weight_shares(&mut judgment.plan);
        let before = judgment.plan[0].normalized_weight();
        let mut ctx = RetrievalContext::new("fix bug", RetrievalPolicy::Project);
        ctx.add_item(RetrievalItem::new(
            RetrievalSource::Memory,
            "USER.md",
            "User prefers concise Chinese final responses",
            0.95,
            "memory.match:USER.md",
            TrustLevel::High,
        ));

        let audit = apply_learning_to_workflow_judgment(&mut judgment, &[], Some(&ctx));
        assert!(!audit.applied);
        assert_eq!(judgment.plan[0].normalized_weight(), before);
    }
}

//! Typed context assembly zones for model requests.
//!
//! This module is intentionally small: it gives the runtime one place to name,
//! order, fingerprint, and report context zones before later controllers move
//! more material into those zones.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextZoneName {
    StablePrefix,
    TaskState,
    RelevantMaterial,
    RecentObservation,
    CurrentDecisionRequest,
}

impl ContextZoneName {
    pub fn label(self) -> &'static str {
        match self {
            Self::StablePrefix => "stable_prefix",
            Self::TaskState => "task_state",
            Self::RelevantMaterial => "relevant_material",
            Self::RecentObservation => "recent_observation",
            Self::CurrentDecisionRequest => "current_decision_request",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextZone {
    pub name: ContextZoneName,
    pub content: String,
    pub chars: usize,
    pub tokens: u64,
    pub fingerprint: String,
    pub budget_tokens: u64,
    pub overflow_reason: Option<String>,
}

impl ContextZone {
    pub fn new(name: ContextZoneName, content: impl Into<String>) -> Self {
        let content = content.into();
        let tokens = crate::engine::context_compressor::estimate_tokens(&content);
        let budget_tokens = default_zone_budget_tokens(name);
        let overflow_reason = (tokens > budget_tokens).then(|| {
            format!(
                "{} tokens exceed {} token zone budget",
                tokens, budget_tokens
            )
        });
        Self {
            name,
            chars: content.chars().count(),
            tokens,
            fingerprint: crate::engine::prompt_context::stable_fingerprint(&content),
            budget_tokens,
            overflow_reason,
            content,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    pub fn report(&self) -> ContextZoneReport {
        ContextZoneReport {
            name: self.name.label().to_string(),
            chars: self.chars,
            tokens: self.tokens,
            fingerprint: self.fingerprint.clone(),
            budget_tokens: self.budget_tokens,
            overflow_reason: self.overflow_reason.clone(),
            empty: self.is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextZoneReport {
    pub name: String,
    pub chars: usize,
    pub tokens: u64,
    pub fingerprint: String,
    pub budget_tokens: u64,
    pub overflow_reason: Option<String>,
    pub empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextTokenReport {
    pub total_chars: usize,
    pub total_tokens: u64,
    pub stable_prefix_tokens: u64,
    pub dynamic_tail_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCacheReport {
    pub stable_prefix_fingerprint: String,
    pub stable_prefix_tokens: u64,
    pub dynamic_tail_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextAssemblyReport {
    pub zones: Vec<ContextZoneReport>,
    pub token_report: ContextTokenReport,
    pub cache_report: ContextCacheReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextAssemblyPlan {
    pub stable_prefix: ContextZone,
    pub task_state: ContextZone,
    pub relevant_material: ContextZone,
    pub recent_observation: ContextZone,
    pub current_decision_request: ContextZone,
    pub token_report: ContextTokenReport,
    pub cache_report: ContextCacheReport,
}

impl ContextAssemblyPlan {
    pub fn new(input: ContextAssemblyInput) -> Self {
        let stable_prefix = ContextZone::new(ContextZoneName::StablePrefix, input.stable_prefix);
        let task_state = ContextZone::new(ContextZoneName::TaskState, input.task_state);
        let relevant_material =
            ContextZone::new(ContextZoneName::RelevantMaterial, input.relevant_material);
        let recent_observation =
            ContextZone::new(ContextZoneName::RecentObservation, input.recent_observation);
        let current_decision_request = ContextZone::new(
            ContextZoneName::CurrentDecisionRequest,
            input.current_decision_request,
        );

        let dynamic_tail_tokens = task_state
            .tokens
            .saturating_add(relevant_material.tokens)
            .saturating_add(recent_observation.tokens)
            .saturating_add(current_decision_request.tokens);
        let total_tokens = stable_prefix.tokens.saturating_add(dynamic_tail_tokens);
        let dynamic_tail_chars = task_state
            .chars
            .saturating_add(relevant_material.chars)
            .saturating_add(recent_observation.chars)
            .saturating_add(current_decision_request.chars);
        let total_chars = stable_prefix.chars.saturating_add(dynamic_tail_chars);

        let token_report = ContextTokenReport {
            total_chars,
            total_tokens,
            stable_prefix_tokens: stable_prefix.tokens,
            dynamic_tail_tokens,
        };
        let cache_report = ContextCacheReport {
            stable_prefix_fingerprint: stable_prefix.fingerprint.clone(),
            stable_prefix_tokens: stable_prefix.tokens,
            dynamic_tail_tokens,
        };

        Self {
            stable_prefix,
            task_state,
            relevant_material,
            recent_observation,
            current_decision_request,
            token_report,
            cache_report,
        }
    }

    pub fn report(&self) -> ContextAssemblyReport {
        ContextAssemblyReport {
            zones: self.zone_reports(),
            token_report: self.token_report.clone(),
            cache_report: self.cache_report.clone(),
        }
    }

    pub fn zone_reports(&self) -> Vec<ContextZoneReport> {
        self.zones().iter().map(|zone| zone.report()).collect()
    }

    pub fn zones(&self) -> [&ContextZone; 5] {
        [
            &self.stable_prefix,
            &self.task_state,
            &self.relevant_material,
            &self.recent_observation,
            &self.current_decision_request,
        ]
    }

    pub fn render_zoned_context(&self) -> String {
        let mut rendered = String::new();
        for zone in self.zones() {
            if zone.is_empty() {
                continue;
            }
            if !rendered.is_empty() {
                rendered.push_str("\n\n");
            }
            rendered.push_str(&format!(
                "<{}>\n{}\n</{}>",
                zone.name.label(),
                zone.content.trim(),
                zone.name.label()
            ));
        }
        rendered
    }

    pub fn render_legacy_system_prompt(&self) -> String {
        let mut rendered = self.stable_prefix.content.clone();
        if !self.task_state.content.is_empty() {
            rendered.push_str(&self.task_state.content);
        }
        rendered
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextAssemblyInput {
    pub stable_prefix: String,
    pub task_state: String,
    pub relevant_material: String,
    pub recent_observation: String,
    pub current_decision_request: String,
}

fn default_zone_budget_tokens(name: ContextZoneName) -> u64 {
    match name {
        ContextZoneName::StablePrefix => 12_000,
        ContextZoneName::TaskState => 2_000,
        ContextZoneName::RelevantMaterial => 12_000,
        ContextZoneName::RecentObservation => 4_000,
        ContextZoneName::CurrentDecisionRequest => 4_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(task_state: &str, user_request: &str) -> ContextAssemblyPlan {
        ContextAssemblyPlan::new(ContextAssemblyInput {
            stable_prefix: "base rules".to_string(),
            task_state: task_state.to_string(),
            relevant_material: "src/main.rs".to_string(),
            recent_observation: "tests passed".to_string(),
            current_decision_request: user_request.to_string(),
        })
    }

    #[test]
    fn reports_zones_in_runtime_order() {
        let plan = plan("stage=understand", "fix bug");

        let names = plan
            .zone_reports()
            .into_iter()
            .map(|zone| zone.name)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "stable_prefix",
                "task_state",
                "relevant_material",
                "recent_observation",
                "current_decision_request"
            ]
        );
    }

    #[test]
    fn stable_prefix_fingerprint_ignores_dynamic_tail() {
        let first = plan("stage=understand", "fix bug");
        let second = plan("stage=validate", "write tests");

        assert_eq!(
            first.cache_report.stable_prefix_fingerprint,
            second.cache_report.stable_prefix_fingerprint
        );
        assert_ne!(
            first.token_report.dynamic_tail_tokens,
            second.token_report.dynamic_tail_tokens
        );
    }

    #[test]
    fn legacy_system_prompt_preserves_stable_prefix_plus_task_state_tail() {
        let plan = ContextAssemblyPlan::new(ContextAssemblyInput {
            stable_prefix: "base".to_string(),
            task_state: "\n\n<task-focus>coding</task-focus>".to_string(),
            relevant_material: "do not render in legacy system prompt".to_string(),
            recent_observation: "do not render in legacy system prompt".to_string(),
            current_decision_request: "do not render in legacy system prompt".to_string(),
        });

        assert_eq!(
            plan.render_legacy_system_prompt(),
            "base\n\n<task-focus>coding</task-focus>"
        );
    }
}

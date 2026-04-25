//! Resource-aware execution policy.
//!
//! The policy is selected after intent routing and gives the runtime a visible
//! budget for latency, cost, reasoning depth, parallelism, tool calls, and
//! context size.

use crate::engine::intent_router::{IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyTarget {
    Fast,
    Balanced,
    Deep,
}

impl LatencyTarget {
    pub fn target_ms(self) -> u64 {
        match self {
            LatencyTarget::Fast => 5_000,
            LatencyTarget::Balanced => 20_000,
            LatencyTarget::Deep => 60_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePolicy {
    pub latency: LatencyTarget,
    pub cost_ceiling_usd: f64,
    pub reasoning: ReasoningPolicy,
    pub parallelism_limit: usize,
    pub max_tool_calls: usize,
    pub context_budget_tokens: usize,
    pub allow_fallback_model: bool,
    pub reason: String,
}

impl ResourcePolicy {
    pub fn from_route(route: &IntentRoute) -> Self {
        let mut policy = match route.reasoning {
            ReasoningPolicy::Low => Self {
                latency: LatencyTarget::Fast,
                cost_ceiling_usd: 0.02,
                reasoning: route.reasoning,
                parallelism_limit: 1,
                max_tool_calls: 4,
                context_budget_tokens: 8_000,
                allow_fallback_model: true,
                reason: "low reasoning route favors fast response".to_string(),
            },
            ReasoningPolicy::Medium => Self {
                latency: LatencyTarget::Balanced,
                cost_ceiling_usd: 0.08,
                reasoning: route.reasoning,
                parallelism_limit: 2,
                max_tool_calls: 12,
                context_budget_tokens: 24_000,
                allow_fallback_model: true,
                reason: "medium reasoning route uses balanced resource budget".to_string(),
            },
            ReasoningPolicy::High => Self {
                latency: LatencyTarget::Deep,
                cost_ceiling_usd: 0.25,
                reasoning: route.reasoning,
                parallelism_limit: 4,
                max_tool_calls: 30,
                context_budget_tokens: 64_000,
                allow_fallback_model: true,
                reason: "high reasoning route allows deeper investigation".to_string(),
            },
        };

        if matches!(
            route.retrieval,
            RetrievalPolicy::Web | RetrievalPolicy::Full
        ) {
            policy.parallelism_limit = policy.parallelism_limit.max(3);
            policy.max_tool_calls = policy.max_tool_calls.max(16);
            policy
                .reason
                .push_str("; retrieval policy needs broader source checks");
        }

        if matches!(route.risk, RiskLevel::High) {
            policy.parallelism_limit = 1;
            policy.max_tool_calls = policy.max_tool_calls.min(12);
            policy
                .reason
                .push_str("; high-risk route limits parallel side effects");
        }

        policy
    }

    pub fn compact_label(&self) -> String {
        format!(
            "{:?} ${:.2} p{} tools{} ctx{}",
            self.latency,
            self.cost_ceiling_usd,
            self.parallelism_limit,
            self.max_tool_calls,
            self.context_budget_tokens
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn direct_route_gets_fast_budget() {
        let route = IntentRouter::new().route("你好");
        let policy = ResourcePolicy::from_route(&route);
        assert_eq!(policy.latency, LatencyTarget::Fast);
        assert_eq!(policy.parallelism_limit, 1);
        assert!(policy.max_tool_calls <= 4);
    }

    #[test]
    fn code_change_gets_deeper_budget() {
        let route = IntentRouter::new().route("继续开发 CLI，优化状态栏");
        let policy = ResourcePolicy::from_route(&route);
        assert_eq!(policy.latency, LatencyTarget::Deep);
        assert!(policy.parallelism_limit >= 4);
        assert!(policy.context_budget_tokens >= 64_000);
    }

    #[test]
    fn high_risk_limits_parallelism() {
        let mut route = IntentRouter::new().route("修改配置");
        route.risk = RiskLevel::High;
        let policy = ResourcePolicy::from_route(&route);
        assert_eq!(policy.parallelism_limit, 1);
    }
}

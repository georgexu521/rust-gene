//! Workflow Policy Layer
//!
//! 将 Gate / Questioning / Weights 的阈值与开关集中管理，避免策略散落。

/// Gate 策略
#[derive(Debug, Clone, Default)]
pub struct GatePolicy {
    pub workflow_enabled: bool,
    pub llm_classifier_enabled: bool,
}

/// 主动提问式深思策略
#[derive(Debug, Clone)]
pub struct SocraticPolicy {
    pub max_rounds: usize,
    pub max_answer_tokens: usize,
    pub max_total_tokens: usize,
    pub max_depth: usize,
}

impl Default for SocraticPolicy {
    fn default() -> Self {
        Self {
            max_rounds: 5,
            max_answer_tokens: 500,
            max_total_tokens: 3750,
            max_depth: 3,
        }
    }
}

/// 权重系数策略
#[derive(Debug, Clone)]
pub struct WeightMultipliers {
    pub risk: f64,
    pub impact: f64,
    pub complexity: f64,
    pub blocker: f64,
    pub dependency: f64,
    pub drift: f64,
    pub historical_failure: f64,
}

impl Default for WeightMultipliers {
    fn default() -> Self {
        Self {
            risk: 1.0,
            impact: 1.0,
            complexity: 1.0,
            blocker: 1.0,
            dependency: 1.0,
            drift: 1.0,
            historical_failure: 1.0,
        }
    }
}

/// Workflow 总策略（Policy Layer）
#[derive(Debug, Clone, Default)]
pub struct WorkflowPolicy {
    pub gate: GatePolicy,
    pub socratic: SocraticPolicy,
    pub weights: WeightMultipliers,
}

impl WorkflowPolicy {
    /// 从环境变量加载策略（单一入口）
    pub fn from_env() -> Self {
        let mut p = Self::default();
        p.gate.workflow_enabled = env_bool_any(
            &[
                "PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED",
                "PRIORITY_AGENT_WORKFLOW_ENABLED",
            ],
            false,
        );
        p.gate.llm_classifier_enabled = env_bool("PRIORITY_AGENT_WORKFLOW_GATE_LLM", false);

        p.socratic.max_rounds = env_usize("PRIORITY_AGENT_SOCRATIC_MAX_ROUNDS", 5);
        p.socratic.max_answer_tokens = env_usize("PRIORITY_AGENT_SOCRATIC_ANSWER_BUDGET", 500);
        p.socratic.max_total_tokens = env_usize("PRIORITY_AGENT_SOCRATIC_TOTAL_BUDGET", 3750);
        p.socratic.max_depth = env_usize("PRIORITY_AGENT_SOCRATIC_MAX_DEPTH", 3);

        p.weights.risk = env_f64("PRIORITY_AGENT_WEIGHT_RISK_MUL", 1.0);
        p.weights.impact = env_f64("PRIORITY_AGENT_WEIGHT_IMPACT_MUL", 1.0);
        p.weights.complexity = env_f64("PRIORITY_AGENT_WEIGHT_COMPLEXITY_MUL", 1.0);
        p.weights.blocker = env_f64("PRIORITY_AGENT_WEIGHT_BLOCKER_MUL", 1.0);
        p.weights.dependency = env_f64("PRIORITY_AGENT_WEIGHT_DEPENDENCY_MUL", 1.0);
        p.weights.drift = env_f64("PRIORITY_AGENT_WEIGHT_DRIFT_MUL", 1.0);
        p.weights.historical_failure = env_f64("PRIORITY_AGENT_WEIGHT_HIST_FAIL_MUL", 1.0);
        p
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(default)
}

fn env_bool_any(names: &[&str], default: bool) -> bool {
    for name in names {
        if let Ok(value) = std::env::var(name) {
            return value != "0" && !value.eq_ignore_ascii_case("false");
        }
    }
    default
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::env_guard::EnvVarGuard;

    #[test]
    fn workflow_policy_disables_legacy_workflow_by_default() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");

        assert!(!WorkflowPolicy::from_env().gate.workflow_enabled);
    }

    #[test]
    fn workflow_policy_accepts_new_and_legacy_env_switches() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED", "1");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        assert!(WorkflowPolicy::from_env().gate.workflow_enabled);

        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.set("PRIORITY_AGENT_WORKFLOW_ENABLED", "1");
        assert!(WorkflowPolicy::from_env().gate.workflow_enabled);
    }
}

//! 质量门禁 (Quality Gates)
//!
//! 在发布前必须通过的质量检查点
//! 对应 PLAN.md 中的 G0-G5 五级门禁

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::Hash;

/// 质量门禁级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateLevel {
    /// G0: 编译通过
    Compiles,
    /// G1: 测试通过
    TestsPass,
    /// G2: Lint 通过
    LintClean,
    /// G3: 文档一致
    DocsAligned,
    /// G4: 回归测试通过
    RegressionPass,
    /// G5: 性能基准达标
    PerformanceMet,
}

impl GateLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            GateLevel::Compiles => "G0: Compiles",
            GateLevel::TestsPass => "G1: Tests Pass",
            GateLevel::LintClean => "G2: Lint Clean",
            GateLevel::DocsAligned => "G3: Docs Aligned",
            GateLevel::RegressionPass => "G4: Regression Pass",
            GateLevel::PerformanceMet => "G5: Performance Met",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            GateLevel::Compiles => "cargo check must pass",
            GateLevel::TestsPass => "cargo test must pass with no new failures",
            GateLevel::LintClean => "cargo clippy must pass with no new warnings",
            GateLevel::DocsAligned => "Documentation matches implementation",
            GateLevel::RegressionPass => "Regression test suite passes",
            GateLevel::PerformanceMet => "Performance benchmarks are within threshold",
        }
    }
}

/// 门禁检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate: GateLevel,
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
}

/// 门禁检查器
#[derive(Debug, Default)]
pub struct QualityGates {
    /// 已通过的门禁
    passed_gates: HashSet<GateLevel>,
}

impl QualityGates {
    pub fn new() -> Self {
        Self {
            passed_gates: HashSet::new(),
        }
    }

    /// 标记门禁为通过
    pub fn pass(&mut self, gate: GateLevel) {
        self.passed_gates.insert(gate);
    }

    /// 标记门禁为失败
    pub fn fail(&mut self, gate: GateLevel) {
        self.passed_gates.remove(&gate);
    }

    /// 检查门禁是否通过
    pub fn is_passed(&self, gate: GateLevel) -> bool {
        self.passed_gates.contains(&gate)
    }

    /// 获取所有已通过的門禁
    pub fn passed(&self) -> Vec<GateLevel> {
        self.passed_gates.iter().copied().collect()
    }

    /// 获取所有未通过的門禁
    pub fn failed(&self, all_gates: &[GateLevel]) -> Vec<GateLevel> {
        all_gates
            .iter()
            .filter(|g| !self.passed_gates.contains(g))
            .copied()
            .collect()
    }

    /// 检查是否可以发布
    pub fn can_release(&self, min_level: GateLevel) -> bool {
        let all_gates = [
            GateLevel::Compiles,
            GateLevel::TestsPass,
            GateLevel::LintClean,
            GateLevel::DocsAligned,
            GateLevel::RegressionPass,
            GateLevel::PerformanceMet,
        ];

        // Find the index of min_level
        let min_idx = all_gates.iter().position(|g| *g == min_level).unwrap_or(0);

        // All gates up to min_level must be passed
        all_gates[..=min_idx].iter().all(|g| self.passed_gates.contains(g))
    }

    /// 生成门禁报告
    pub fn report(&self) -> String {
        let all_gates = [
            GateLevel::Compiles,
            GateLevel::TestsPass,
            GateLevel::LintClean,
            GateLevel::DocsAligned,
            GateLevel::RegressionPass,
            GateLevel::PerformanceMet,
        ];

        let mut lines = vec!["Quality Gates Report".to_string(), "=====================".to_string()];

        for gate in &all_gates {
            let status = if self.passed_gates.contains(gate) {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };
            lines.push(format!("{}: {}", gate.as_str(), status));
            lines.push(format!("  {}", gate.description()));
        }

        lines.join("\n")
    }
}

/// 环境变量检查
pub fn check_env_gates() -> Vec<GateResult> {
    let mut results = vec![];

    // Check PRIORITY_AGENT_* environment variables
    let required_vars = [
        "PRIORITY_AGENT_AUTO_TEST",
        "PRIORITY_AGENT_SMART_EDIT",
    ];

    for var in required_vars {
        let exists = std::env::var(var).is_ok();
        results.push(GateResult {
            gate: GateLevel::Compiles, // Using G0 as placeholder
            passed: exists,
            message: if exists {
                format!("{} is set", var)
            } else {
                format!("{} is not set (optional)", var)
            },
            details: None,
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_gates() {
        let mut gates = QualityGates::new();

        gates.pass(GateLevel::Compiles);
        gates.pass(GateLevel::TestsPass);

        assert!(gates.is_passed(GateLevel::Compiles));
        assert!(!gates.is_passed(GateLevel::LintClean));
        assert!(gates.can_release(GateLevel::TestsPass));
        assert!(!gates.can_release(GateLevel::LintClean));
    }
}

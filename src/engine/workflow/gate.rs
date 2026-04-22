//! Workflow 闸门 — 判定请求走 Direct Mode 还是 Workflow
//!
//! 三层判定架构：
//! 1. Fast Lane（硬规则，O(1)）
//! 2. Heuristic（关键词，O(n)）
//! 3. LLM Classifier（可选，M1 中暂不提供默认实现）

use std::sync::LazyLock;

/// 闸门判定结果
#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    /// 走现有直接对话模式
    Direct { reason: String },
    /// 进入 Workflow 结构化流程
    Workflow { reason: String, confidence: f64 },
}

impl GateDecision {
    pub fn is_workflow(&self) -> bool {
        matches!(self, GateDecision::Workflow { .. })
    }

    pub fn reason(&self) -> &str {
        match self {
            GateDecision::Direct { reason } => reason,
            GateDecision::Workflow { reason, .. } => reason,
        }
    }
}

// ============================================================================
// Fast Lane — 硬规则短路
// ============================================================================

/// 快速通道匹配规则
static FAST_LANE_PATTERNS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    vec![
        // 帮助类命令
        (r"^/(help|clear|status|doctor|quit|memory|save|load|cost|token|model|tools)\b", "help_cmd"),
        // 只读系统查询
        (r"^(git status|ls|cat|echo|pwd|whoami|uname|date)\b", "readonly_query"),
        // 问候闲聊
        (r"^(你好|在吗|谢谢|再见|hi\b|hello\b|thanks\b|hey\b)\b", "greeting"),
    ]
});

fn fast_lane_check(input: &str) -> Option<GateDecision> {
    let trimmed = input.trim();
    for (pattern, category) in FAST_LANE_PATTERNS.iter() {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(trimmed) {
                return Some(GateDecision::Direct {
                    reason: format!("Fast lane: {}", category),
                });
            }
        }
    }
    None
}

// ============================================================================
// Heuristic — 关键词扫描
// ============================================================================

/// 高风险关键词 — 命中即 Workflow
const HIGH_RISK_KEYWORDS: &[&str] = &[
    "重构", "redesign", "architecture", "拆分", "解耦",
    "所有文件", "批量", "全局", "cross-module",
    "新增模块", "实现系统", "添加引擎", "引入框架",
    "删除", "迁移", "升级", "替换底层",
    " redesign ", " refactor ", " restructure ",
    " implement system ", " add engine ",
];

/// 低风险关键词 — 无高风险词时判定为 Direct
const LOW_RISK_KEYWORDS: &[&str] = &[
    "修复", "fix", "改正", "typo", "纠正",
    "查看", "显示", "列出", "grep", "find",
    "改", "调参数", "开关", "更新版本", "修改",
];

fn heuristic_scan(input: &str) -> Option<GateDecision> {
    let lower = input.to_lowercase();
    let has_high = HIGH_RISK_KEYWORDS.iter().any(|w| lower.contains(w));
    let has_low = LOW_RISK_KEYWORDS.iter().any(|w| lower.contains(w));

    if has_high {
        return Some(GateDecision::Workflow {
            reason: "Heuristic: high-risk keywords detected".into(),
            confidence: 0.8,
        });
    }

    if has_low && !has_high {
        return Some(GateDecision::Direct {
            reason: "Heuristic: low-risk keywords only".into(),
        });
    }

    None
}

// ============================================================================
// Gate 主结构
// ============================================================================

/// Workflow 闸门
///
/// 组合 Fast Lane 和 Heuristic 判定请求路径。
/// M1 中 LLM Classifier 为可选扩展。
pub struct Gate {
    /// 是否启用 LLM 分类器（M1 默认 false）
    enable_llm_classifier: bool,
}

impl Gate {
    pub fn new() -> Self {
        Self {
            enable_llm_classifier: false,
        }
    }

    pub fn with_llm_classifier(mut self, enabled: bool) -> Self {
        self.enable_llm_classifier = enabled;
        self
    }

    /// 判定输入请求的路径
    ///
    /// 判定顺序：环境变量开关 → Fast Lane → Heuristic → (LLM Classifier，如果启用)
    pub fn decide(&self, input: &str) -> GateDecision {
        // 0. 环境变量全局开关
        if !Self::is_workflow_enabled() {
            return GateDecision::Direct {
                reason: "Workflow disabled by PRIORITY_AGENT_WORKFLOW_ENABLED".into(),
            };
        }

        // 1. Fast Lane 检查
        if let Some(decision) = fast_lane_check(input) {
            return decision;
        }

        // 2. Heuristic 扫描
        if let Some(decision) = heuristic_scan(input) {
            return decision;
        }

        // 3. 默认行为：复杂可能性高，偏向 Workflow
        // M1 中 LLM Classifier 可选，未启用时默认 Workflow
        if self.enable_llm_classifier {
            // TODO: M2 中接入 LLM 轻量分类
            GateDecision::Workflow {
                reason: "LLM classifier not yet implemented in M1, defaulting to Workflow".into(),
                confidence: 0.5,
            }
        } else {
            GateDecision::Workflow {
                reason: "No fast lane or heuristic match, defaulting to Workflow (M1)".into(),
                confidence: 0.5,
            }
        }
    }

    /// 检查 Workflow 是否被环境变量启用（默认启用）
    pub fn is_workflow_enabled() -> bool {
        std::env::var("PRIORITY_AGENT_WORKFLOW_ENABLED")
            .ok()
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true)
    }

    /// 批量判定（用于测试和基准）
    pub fn decide_batch(&self, inputs: &[&str]) -> Vec<GateDecision> {
        inputs.iter().map(|input| self.decide(input)).collect()
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_lane_help() {
        let gate = Gate::new();
        let d = gate.decide("/help");
        assert!(matches!(d, GateDecision::Direct { .. }));
        assert!(d.reason().contains("Fast lane"));
    }

    #[test]
    fn test_fast_lane_git_status() {
        let gate = Gate::new();
        let d = gate.decide("git status");
        assert!(matches!(d, GateDecision::Direct { .. }));
    }

    #[test]
    fn test_fast_lane_greeting() {
        let gate = Gate::new();
        assert!(matches!(gate.decide("你好"), GateDecision::Direct { .. }));
        assert!(matches!(gate.decide("hello"), GateDecision::Direct { .. }));
        assert!(matches!(gate.decide("thanks"), GateDecision::Direct { .. }));
    }

    #[test]
    fn test_heuristic_high_risk() {
        let gate = Gate::new();
        let d = gate.decide("重构整个模块架构");
        assert!(matches!(d, GateDecision::Workflow { .. }));
        assert!(d.reason().contains("high-risk"));
    }

    #[test]
    fn test_heuristic_low_risk() {
        let gate = Gate::new();
        let d = gate.decide("修复一个 typo");
        assert!(matches!(d, GateDecision::Direct { .. }));
        assert!(d.reason().contains("low-risk"));
    }

    #[test]
    fn test_default_to_workflow() {
        let gate = Gate::new();
        // 既不匹配 Fast Lane 也不匹配 Heuristic
        let d = gate.decide("请帮我分析一下这个项目的代码结构");
        assert!(matches!(d, GateDecision::Workflow { .. }));
    }

    #[test]
    fn test_batch_decide() {
        let gate = Gate::new();
        let inputs = vec!["/help", "重构模块", "修复 bug", "分析代码"];
        let results = gate.decide_batch(&inputs);
        assert_eq!(results.len(), 4);
        assert!(matches!(results[0], GateDecision::Direct { .. }));
        assert!(matches!(results[1], GateDecision::Workflow { .. }));
        assert!(matches!(results[2], GateDecision::Direct { .. }));
        assert!(matches!(results[3], GateDecision::Workflow { .. }));
    }

    #[test]
    fn test_complex_task_workflow() {
        let gate = Gate::new();
        let cases = vec![
            "新增一个完整的用户认证系统",
            "实现批量重构工具",
            "迁移数据库到 PostgreSQL",
            "升级底层依赖版本",
        ];
        for case in cases {
            let d = gate.decide(case);
            assert!(
                d.is_workflow(),
                "Expected workflow for: {}, got: {:?}",
                case,
                d
            );
        }
    }

    #[test]
    fn test_simple_task_direct() {
        let gate = Gate::new();
        let cases = vec![
            "更新版本号",
            "查看当前配置",
            "grep TODO",
            "修改环境变量默认值",
        ];
        for case in cases {
            let d = gate.decide(case);
            assert!(
                !d.is_workflow(),
                "Expected direct for: {}, got: {:?}",
                case,
                d
            );
        }
    }

    #[test]
    fn test_workflow_disabled_env_var() {
        use crate::test_utils::env_guard::EnvVarGuard;

        let mut env = EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_WORKFLOW_ENABLED", "0");

        let gate = Gate::new();
        // 即使是高风险任务，也应返回 Direct
        let d = gate.decide("重构整个模块架构");
        assert!(
            matches!(d, GateDecision::Direct { .. }),
            "Expected Direct when workflow disabled, got: {:?}",
            d
        );
        assert!(d.reason().contains("disabled"));

        // 默认不匹配任何规则的任务也应返回 Direct
        let d2 = gate.decide("分析代码结构并优化");
        assert!(
            matches!(d2, GateDecision::Direct { .. }),
            "Expected Direct when workflow disabled, got: {:?}",
            d2
        );
    }

    #[test]
    fn test_workflow_enabled_by_default() {
        use crate::test_utils::env_guard::EnvVarGuard;

        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");

        assert!(Gate::is_workflow_enabled(), "Workflow should be enabled by default");
    }
}

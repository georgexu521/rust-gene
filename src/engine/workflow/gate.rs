//! Workflow 闸门 — 判定请求走 Direct Mode 还是 Workflow
//!
//! 三层判定架构：
//! 1. Fast Lane（硬规则，O(1)）
//! 2. Heuristic（关键词，O(n)）
//! 3. LLM Classifier（可选，M1 中暂不提供默认实现）

use super::policy::GatePolicy;
use crate::services::api::{ChatRequest, LlmProvider, Message};
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
        (
            r"^/(help|clear|status|doctor|quit|memory|save|load|cost|token|model|tools)\b",
            "help_cmd",
        ),
        // 只读系统查询
        (
            r"^(git status|ls|cat|echo|pwd|whoami|uname|date)\b",
            "readonly_query",
        ),
        // 问候闲聊
        (
            r"^(你好|在吗|谢谢|再见|hi\b|hello\b|thanks\b|hey\b)\b",
            "greeting",
        ),
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
    "重构",
    "redesign",
    "architecture",
    "拆分",
    "解耦",
    "所有文件",
    "批量",
    "全局",
    "跨模块",
    "cross-module",
    "新增模块",
    "新增完整",
    "完整子系统",
    "完整的用户认证系统",
    "认证系统",
    "现有架构",
    "实现系统",
    "添加引擎",
    "引入框架",
    "协同策略",
    "发布流程",
    "改进方向",
    "稳定性改造",
    "回退与风控",
    "优化计划",
    "删除",
    "迁移",
    "升级",
    "替换底层",
    " redesign ",
    " refactor ",
    " restructure ",
    " implement system ",
    " add engine ",
];

/// 低风险关键词 — 无高风险词时判定为 Direct
const LOW_RISK_KEYWORDS: &[&str] = &[
    "修复",
    "fix",
    "改正",
    "typo",
    "纠正",
    "查看",
    "显示",
    "列出",
    "grep",
    "find",
    "改",
    "调参数",
    "开关",
    "更新版本",
    "修改",
];

const CODE_WORKFLOW_KEYWORDS: &[&str] = &[
    "bug_fix",
    "regression",
    "回归",
    "测试失败",
    "test failed",
    "cargo test",
    "required_commands",
    "diff_constraints",
    "acceptance",
    "验收",
    "绕过",
    "质量门控",
    "quality gate",
    "src/",
];

fn heuristic_scan(input: &str) -> Option<GateDecision> {
    let lower = input.to_lowercase();
    let has_high = HIGH_RISK_KEYWORDS.iter().any(|w| lower.contains(w));
    let has_low = LOW_RISK_KEYWORDS.iter().any(|w| lower.contains(w));
    let has_code_workflow = CODE_WORKFLOW_KEYWORDS.iter().any(|w| lower.contains(w));

    if has_high {
        return Some(GateDecision::Workflow {
            reason: "Heuristic: high-risk keywords detected".into(),
            confidence: 0.8,
        });
    }

    if has_low && has_code_workflow {
        return Some(GateDecision::Workflow {
            reason: "Heuristic: code regression/test signals require workflow".into(),
            confidence: 0.72,
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
    /// workflow 总开关（由 policy 层统一提供）
    workflow_enabled: bool,
}

impl Gate {
    pub fn new() -> Self {
        Self {
            enable_llm_classifier: false,
            workflow_enabled: false,
        }
    }

    pub fn with_llm_classifier(mut self, enabled: bool) -> Self {
        self.enable_llm_classifier = enabled;
        self
    }

    pub fn with_policy(mut self, policy: GatePolicy) -> Self {
        self.enable_llm_classifier = policy.llm_classifier_enabled;
        self.workflow_enabled = policy.workflow_enabled;
        self
    }

    /// 判定输入请求的路径
    ///
    /// 判定顺序：环境变量开关 → Fast Lane → Heuristic → (LLM Classifier，如果启用)
    pub fn decide(&self, input: &str) -> GateDecision {
        // 0. 环境变量全局开关
        if !self.workflow_enabled {
            return GateDecision::Direct {
                reason: "Legacy workflow disabled; set PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED=1 or PRIORITY_AGENT_WORKFLOW_ENABLED=1 to enable".into(),
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

        // 3. 默认行为：保持 Direct，避免 legacy workflow 抢占正常交互路径。
        let _ = self.enable_llm_classifier;
        GateDecision::Direct {
            reason: "No fast lane or heuristic match; staying direct by default".into(),
        }
    }

    /// 异步判定（可选 LLM 轻量分类）
    ///
    /// 顺序：环境变量开关 → Fast Lane → Heuristic → LLM（可选）→ 默认 Workflow
    pub async fn decide_with_llm(
        &self,
        input: &str,
        provider: &dyn LlmProvider,
        model: &str,
    ) -> GateDecision {
        if !self.workflow_enabled {
            return GateDecision::Direct {
                reason: "Legacy workflow disabled; set PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED=1 or PRIORITY_AGENT_WORKFLOW_ENABLED=1 to enable".into(),
            };
        }

        if let Some(decision) = fast_lane_check(input) {
            return decision;
        }

        if let Some(decision) = heuristic_scan(input) {
            return decision;
        }

        if !self.enable_llm_classifier {
            return GateDecision::Direct {
                reason: "No fast lane or heuristic match; staying direct by default".into(),
            };
        }

        let prompt = format!(
            "你是工作流路由分类器。请判断用户请求应走 direct 还是 workflow。\n\
             规则：\n\
             - 简单查询/闲聊/单点小修复 => direct\n\
             - 多步骤改造/跨模块/高风险操作/架构决策 => workflow\n\
             输出必须是一行 JSON：{{\"decision\":\"direct|workflow\",\"confidence\":0.0-1.0,\"reason\":\"...\"}}\n\
             用户请求：{}",
            input
        );
        let mut request = ChatRequest::new(model)
            .with_messages(vec![
                Message::system("只输出 JSON，不要解释。"),
                Message::user(&prompt),
            ])
            .with_temperature(0.0);
        request.max_tokens = Some(120);

        match provider.chat(request).await {
            Ok(resp) => {
                Self::parse_llm_decision(&resp.content).unwrap_or_else(|| GateDecision::Direct {
                    reason: "LLM classifier parse failed; staying direct by default".into(),
                })
            }
            Err(_) => GateDecision::Direct {
                reason: "LLM classifier failed; staying direct by default".into(),
            },
        }
    }

    /// 批量判定（用于测试和基准）
    pub fn decide_batch(&self, inputs: &[&str]) -> Vec<GateDecision> {
        inputs.iter().map(|input| self.decide(input)).collect()
    }
}

impl Gate {
    fn parse_llm_decision(raw: &str) -> Option<GateDecision> {
        let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
        let decision = v.get("decision")?.as_str()?.to_ascii_lowercase();
        let confidence = v
            .get("confidence")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        let reason = v
            .get("reason")
            .and_then(|x| x.as_str())
            .unwrap_or("LLM classifier decision")
            .to_string();
        match decision.as_str() {
            "direct" => Some(GateDecision::Direct { reason }),
            "workflow" => Some(GateDecision::Workflow { reason, confidence }),
            _ => None,
        }
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
    use crate::test_utils::env_guard::EnvVarGuard;
    use serde::Deserialize;

    fn enabled_gate() -> Gate {
        Gate::new().with_policy(GatePolicy {
            workflow_enabled: true,
            llm_classifier_enabled: false,
        })
    }

    #[test]
    fn test_fast_lane_help() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        let d = gate.decide("/help");
        assert!(matches!(d, GateDecision::Direct { .. }));
        assert!(d.reason().contains("Fast lane"));
    }

    #[test]
    fn test_fast_lane_git_status() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        let d = gate.decide("git status");
        assert!(matches!(d, GateDecision::Direct { .. }));
    }

    #[test]
    fn test_fast_lane_greeting() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        assert!(matches!(gate.decide("你好"), GateDecision::Direct { .. }));
        assert!(matches!(gate.decide("hello"), GateDecision::Direct { .. }));
        assert!(matches!(gate.decide("thanks"), GateDecision::Direct { .. }));
    }

    #[test]
    fn test_heuristic_high_risk() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        let d = gate.decide("重构整个模块架构");
        assert!(matches!(d, GateDecision::Workflow { .. }));
        assert!(d.reason().contains("high-risk"));
    }

    #[test]
    fn test_heuristic_low_risk() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        let d = gate.decide("修复一个 typo");
        assert!(matches!(d, GateDecision::Direct { .. }));
        assert!(d.reason().contains("low-risk"));
    }

    #[test]
    fn test_code_regression_bugfix_routes_to_workflow() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        let d =
            gate.decide("修复 memory_save 绕过记忆质量门控的问题，必须通过 cargo test -q memory");
        assert!(
            matches!(d, GateDecision::Workflow { .. }),
            "expected workflow for code regression, got {:?}",
            d
        );
        assert!(d.reason().contains("code regression"));
    }

    #[test]
    fn test_default_no_match_stays_direct() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        // 既不匹配 Fast Lane 也不匹配 Heuristic
        let d = gate.decide("请帮我分析一下这个项目的代码结构");
        assert!(matches!(d, GateDecision::Direct { .. }));
        assert!(d.reason().contains("staying direct"));
    }

    #[test]
    fn test_batch_decide() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
        let inputs = vec!["/help", "重构模块", "修复 bug", "分析代码"];
        let results = gate.decide_batch(&inputs);
        assert_eq!(results.len(), 4);
        assert!(matches!(results[0], GateDecision::Direct { .. }));
        assert!(matches!(results[1], GateDecision::Workflow { .. }));
        assert!(matches!(results[2], GateDecision::Direct { .. }));
        assert!(matches!(results[3], GateDecision::Direct { .. }));
    }

    #[test]
    fn test_complex_task_workflow() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
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
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED");
        env.remove("PRIORITY_AGENT_WORKFLOW_ENABLED");
        let gate = enabled_gate();
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
        let gate = Gate::new().with_policy(GatePolicy {
            workflow_enabled: false,
            llm_classifier_enabled: false,
        });
        // 即使是高风险任务，也应返回 Direct（策略层关闭）
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
    fn test_legacy_workflow_disabled_by_default() {
        let d = Gate::new().decide("重构整个模块架构");
        assert!(
            !d.is_workflow(),
            "Legacy workflow should not be enabled by default"
        );
        assert!(d.reason().contains("Legacy workflow disabled"));
    }

    #[derive(Debug, Deserialize)]
    struct ReplaySample {
        task_description: String,
        complexity: String,
    }

    fn load_replay_samples() -> (Vec<ReplaySample>, String) {
        let v2 = std::path::Path::new("docs/workflow/gate-replay-samples-v2.json");
        let v1 = std::path::Path::new("docs/workflow/gate-replay-samples.json");
        if v2.exists() {
            let raw = std::fs::read_to_string(v2).expect("read v2 replay samples");
            let samples: Vec<ReplaySample> =
                serde_json::from_str(&raw).expect("valid gate-replay-samples-v2.json");
            return (samples, v2.display().to_string());
        }
        let raw = std::fs::read_to_string(v1).expect("read v1 replay samples");
        let samples: Vec<ReplaySample> =
            serde_json::from_str(&raw).expect("valid gate-replay-samples.json");
        (samples, v1.display().to_string())
    }

    #[test]
    fn test_gate_offline_replay_accuracy() {
        let (samples, source) = load_replay_samples();
        assert!(!samples.is_empty(), "samples should not be empty");

        let gate = enabled_gate();
        let mut hits = 0usize;
        for s in &samples {
            let predicted = gate.decide(&s.task_description);
            let expected_workflow = !s.complexity.eq_ignore_ascii_case("simple");
            if expected_workflow == predicted.is_workflow() {
                hits += 1;
            }
        }

        let acc = hits as f64 / samples.len() as f64;
        let threshold = if samples.len() >= 200 { 0.85 } else { 0.60 };
        eprintln!(
            "[gate replay] source={} accuracy={:.2}% ({}/{}) threshold={:.0}%",
            source,
            acc * 100.0,
            hits,
            samples.len(),
            threshold * 100.0
        );
        assert!(
            acc >= threshold,
            "gate replay accuracy too low: {:.2}% (< {:.0}%)",
            acc * 100.0,
            threshold * 100.0
        );
    }
}

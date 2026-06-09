//! 源码级权重计算引擎（MAINTENANCE-ONLY）
//!
//! 核心原则：权重规则硬编码在 Rust 源码中，不是提示词。
//! LLM 可以参与打分，但最终排序由规则引擎裁决。
//!
//! 🟡 维护状态：仅供 legacy 工作流使用（LegacyWorkflowGateController）。
//! 不添加新功能，不调整系数。核心加权逻辑已迁移至 workflow_contract.rs。
//! 保留原因：legacy 路径活跃（turn_entry_gate_controller.rs:104），不能删除。
//! 参见 docs/WEIGHTING_SYSTEM_AUDIT_2026-06-08.md 第 3 节和第 4 节。
//! 评分模型：
//!   RawScore = Risk + Impact + Complexity + BlockerValue - DependencyPenalty - DriftPenalty
//!   每项范围 [-20, +20]，RawScore 范围 [-120, +120]
//!   经 sigmoid 映射到 [0, 100]

use super::feedback::{FeedbackEngine, HistoricalFailureRule};
use super::policy::WeightMultipliers;

/// 计算权重所需的环境变量系数
fn env_mul(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// 步骤上下文
#[derive(Debug, Clone)]
pub struct StepContext {
    pub description: String,
    pub tool: Option<String>,
    pub step_index: usize,
    pub total_steps: usize,
    pub mainline_goal: String,
    pub completed_steps: Vec<usize>,
    /// 此步骤依赖的其他步骤索引
    pub dependent_steps: Vec<usize>,
    /// 此步骤解锁的后续步骤数
    pub unlocks_count: usize,
}

/// 权重维度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeightDimension {
    Risk,
    Impact,
    Complexity,
    BlockerValue,
    DependencyPenalty,
    DriftPenalty,
}

impl WeightDimension {
    pub fn name(&self) -> &'static str {
        match self {
            WeightDimension::Risk => "Risk",
            WeightDimension::Impact => "Impact",
            WeightDimension::Complexity => "Complexity",
            WeightDimension::BlockerValue => "Blocker",
            WeightDimension::DependencyPenalty => "Dep",
            WeightDimension::DriftPenalty => "Drift",
        }
    }
}

/// 单维度评分结果
#[derive(Debug, Clone)]
pub struct DimensionScore {
    pub dimension: WeightDimension,
    pub raw_score: i32,
    pub weighted_score: f64,
    pub explanation: String,
}

/// 权重计算规则 trait
///
/// 所有规则实现必须是确定性的：相同输入产生相同输出。
pub trait WeightRule: Send + Sync {
    fn dimension(&self) -> WeightDimension;
    fn compute(&self, ctx: &StepContext) -> DimensionScore;
}

// ============================================================================
// 硬编码规则实现
// ============================================================================

/// 风险维度规则
///
/// 涉及文件写入、bash 执行、网络请求、删除操作的风险越高。
pub struct RiskRule {
    multiplier: f64,
}

impl RiskRule {
    pub fn new() -> Self {
        Self {
            multiplier: env_mul("PRIORITY_AGENT_WEIGHT_RISK_MUL", 1.0),
        }
    }

    pub fn with_multiplier(multiplier: f64) -> Self {
        Self { multiplier }
    }

    fn tool_risk_score(tool: &str) -> i32 {
        match tool {
            "file_write" | "file_edit" => 3,
            "bash" | "powershell" => 4,
            "web_fetch" | "web_search" => 2,
            "agent" | "task_create" => 3,
            "mcp" => 2,
            "worktree" => 3,
            "remote_trigger" => 3,
            _ => 0,
        }
    }

    fn desc_risk_score(desc: &str) -> i32 {
        let d = desc.to_lowercase();
        let mut score = 0;
        if d.contains("删除") || d.contains("delete") || d.contains("remove") {
            score += 5;
        }
        if d.contains("迁移") || d.contains("migrate") || d.contains("upgrade") {
            score += 4;
        }
        if d.contains("重写") || d.contains("rewrite") || d.contains("replace") {
            score += 3;
        }
        if d.contains("修复") || d.contains("fix") {
            score += 1;
        }
        score
    }
}

impl Default for RiskRule {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightRule for RiskRule {
    fn dimension(&self) -> WeightDimension {
        WeightDimension::Risk
    }

    fn compute(&self, ctx: &StepContext) -> DimensionScore {
        let tool_score = ctx
            .tool
            .as_ref()
            .map(|t| Self::tool_risk_score(t))
            .unwrap_or(0);
        let desc_score = Self::desc_risk_score(&ctx.description);
        let raw = (tool_score + desc_score).min(20);
        let weighted = raw as f64 * self.multiplier;

        let tool_name = ctx.tool.as_deref().unwrap_or("none");
        let explanation = format!(
            "Risk+{}(tool={}={}, desc={})",
            raw, tool_name, tool_score, desc_score
        );

        DimensionScore {
            dimension: WeightDimension::Risk,
            raw_score: raw,
            weighted_score: weighted,
            explanation,
        }
    }
}

/// 影响面维度规则
///
/// 修改模块数多、公共接口变更、配置文件变更的影响越大。
pub struct ImpactRule {
    multiplier: f64,
}

impl ImpactRule {
    pub fn new() -> Self {
        Self {
            multiplier: env_mul("PRIORITY_AGENT_WEIGHT_IMPACT_MUL", 1.0),
        }
    }

    pub fn with_multiplier(multiplier: f64) -> Self {
        Self { multiplier }
    }

    fn desc_impact_score(desc: &str) -> i32 {
        let d = desc.to_lowercase();
        let mut score = 0;

        // 模块数量暗示
        let module_indicators = ["module", "modules", "模块", "跨模块", "全局", "global"];
        for ind in &module_indicators {
            if d.contains(ind) {
                score += 3;
                break;
            }
        }

        // 公共接口
        if d.contains("public") || d.contains("api") || d.contains("接口") || d.contains("trait")
        {
            score += 4;
        }

        // 配置文件
        if d.contains("config") || d.contains("配置") || d.contains("toml") || d.contains("env") {
            score += 2;
        }

        // 注册表/清单变更（影响面广）
        if d.contains("registry") || d.contains("注册") || d.contains("manifest") {
            score += 3;
        }

        score.min(20)
    }
}

impl Default for ImpactRule {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightRule for ImpactRule {
    fn dimension(&self) -> WeightDimension {
        WeightDimension::Impact
    }

    fn compute(&self, ctx: &StepContext) -> DimensionScore {
        let raw = Self::desc_impact_score(&ctx.description);
        let weighted = raw as f64 * self.multiplier;

        DimensionScore {
            dimension: WeightDimension::Impact,
            raw_score: raw,
            weighted_score: weighted,
            explanation: format!("Impact+{}", raw),
        }
    }
}

/// 复杂度维度规则
///
/// 预估代码行数、文件数量、涉及的概念数量。
pub struct ComplexityRule {
    multiplier: f64,
}

impl ComplexityRule {
    pub fn new() -> Self {
        Self {
            multiplier: env_mul("PRIORITY_AGENT_WEIGHT_COMPLEXITY_MUL", 1.0),
        }
    }

    pub fn with_multiplier(multiplier: f64) -> Self {
        Self { multiplier }
    }

    fn desc_complexity_score(desc: &str) -> i32 {
        let d = desc.to_lowercase();
        let mut score = 0;

        // 代码量暗示
        if d.contains("> 500") || d.contains("大规模") || d.contains("large scale") {
            score += 5;
        } else if d.contains("> 100") || d.contains("大量") {
            score += 2;
        }

        // 文件数量
        if d.contains("文件") || d.contains("files") {
            // 尝试提取数字
            for word in d.split_whitespace() {
                if let Ok(n) = word.parse::<i32>() {
                    if n >= 10 {
                        score += 5;
                    } else if n >= 5 {
                        score += 3;
                    } else if n >= 3 {
                        score += 2;
                    }
                    break;
                }
            }
        }

        // 架构复杂度
        let arch_terms = [
            "递归",
            "状态机",
            "并发",
            "异步",
            "生命周期",
            "借用",
            "泛型",
            "宏",
        ];
        for term in &arch_terms {
            if d.contains(term) {
                score += 2;
            }
        }

        score.min(20)
    }
}

impl Default for ComplexityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightRule for ComplexityRule {
    fn dimension(&self) -> WeightDimension {
        WeightDimension::Complexity
    }

    fn compute(&self, ctx: &StepContext) -> DimensionScore {
        let raw = Self::desc_complexity_score(&ctx.description);
        let weighted = raw as f64 * self.multiplier;

        DimensionScore {
            dimension: WeightDimension::Complexity,
            raw_score: raw,
            weighted_score: weighted,
            explanation: format!("Complexity+{}", raw),
        }
    }
}

/// 阻塞点价值规则
///
/// 解锁后续任务越多，价值越高。
pub struct BlockerValueRule {
    multiplier: f64,
}

impl BlockerValueRule {
    pub fn new() -> Self {
        Self {
            multiplier: env_mul("PRIORITY_AGENT_WEIGHT_BLOCKER_MUL", 1.0),
        }
    }

    pub fn with_multiplier(multiplier: f64) -> Self {
        Self { multiplier }
    }
}

impl Default for BlockerValueRule {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightRule for BlockerValueRule {
    fn dimension(&self) -> WeightDimension {
        WeightDimension::BlockerValue
    }

    fn compute(&self, ctx: &StepContext) -> DimensionScore {
        let raw = (ctx.unlocks_count as i32 * 2).min(20);
        let weighted = raw as f64 * self.multiplier;

        DimensionScore {
            dimension: WeightDimension::BlockerValue,
            raw_score: raw,
            weighted_score: weighted,
            explanation: format!("Blocker+{}(unlocks {})", raw, ctx.unlocks_count),
        }
    }
}

/// 依赖惩罚规则
///
/// 有未完成的先决步骤时降权。
pub struct DependencyPenaltyRule {
    multiplier: f64,
}

impl DependencyPenaltyRule {
    pub fn new() -> Self {
        Self {
            multiplier: env_mul("PRIORITY_AGENT_WEIGHT_DEPENDENCY_MUL", 1.0),
        }
    }

    pub fn with_multiplier(multiplier: f64) -> Self {
        Self { multiplier }
    }
}

impl Default for DependencyPenaltyRule {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightRule for DependencyPenaltyRule {
    fn dimension(&self) -> WeightDimension {
        WeightDimension::DependencyPenalty
    }

    fn compute(&self, ctx: &StepContext) -> DimensionScore {
        let incomplete_deps: Vec<usize> = ctx
            .dependent_steps
            .iter()
            .filter(|dep| !ctx.completed_steps.contains(dep))
            .copied()
            .collect();

        let raw = (incomplete_deps.len() as i32 * -3).max(-20);
        let weighted = raw as f64 * self.multiplier;

        DimensionScore {
            dimension: WeightDimension::DependencyPenalty,
            raw_score: raw,
            weighted_score: weighted,
            explanation: format!("Dep{}(incomplete={})", raw, incomplete_deps.len()),
        }
    }
}

/// 漂移惩罚规则
///
/// 步骤描述与主线目标的偏离程度。
/// M2: 叠加历史漂移惩罚系数（来自 FeedbackEngine）。
pub struct DriftPenaltyRule {
    multiplier: f64,
    feedback: FeedbackEngine,
}

impl DriftPenaltyRule {
    pub fn new() -> Self {
        Self {
            multiplier: env_mul("PRIORITY_AGENT_WEIGHT_DRIFT_MUL", 1.0),
            feedback: FeedbackEngine::load(),
        }
    }

    pub fn with_multiplier(multiplier: f64) -> Self {
        Self {
            multiplier,
            feedback: FeedbackEngine::load(),
        }
    }

    /// 简单关键词匹配判定漂移
    /// 返回 true 表示存在明显漂移
    fn detect_drift(description: &str, mainline: &str) -> (bool, String) {
        let desc_lower = description.to_lowercase();
        let main_lower = mainline.to_lowercase();

        // 智能分词：中文按字符切分，英文按空格切分
        fn tokenize(text: &str) -> Vec<String> {
            let has_chinese = text.chars().any(|c| matches!(c, '\u{4e00}'..='\u{9fff}'));
            if has_chinese {
                // 中文：按字符切分，过滤短字符和标点
                text.chars()
                    .filter(|c| !c.is_ascii_punctuation() && !c.is_whitespace())
                    .map(|c| c.to_string())
                    .collect()
            } else {
                // 英文：按空格切分，过滤短词
                text.split_whitespace()
                    .filter(|w| w.len() > 3)
                    .map(String::from)
                    .collect()
            }
        }

        let main_keywords = tokenize(&main_lower);
        if main_keywords.is_empty() {
            return (false, "no mainline keywords".into());
        }

        let matched = main_keywords
            .iter()
            .filter(|kw| desc_lower.contains(kw.as_str()))
            .count();
        let ratio = matched as f64 / main_keywords.len() as f64;

        if ratio >= 0.5 {
            return (false, format!("aligned({:.0}%)", ratio * 100.0));
        }

        // 检查是否在做明显无关的事情
        let drift_indicators = [
            "文档",
            "注释",
            "doc",
            "comment",
            "格式",
            "format",
            "风格",
            "style",
            "重命名",
            "rename",
            "变量名",
            "naming",
        ];
        let has_drift_indicator = drift_indicators.iter().any(|ind| desc_lower.contains(ind));

        if has_drift_indicator && ratio < 0.3 {
            return (true, format!("drift_indicator({:.0}%)", ratio * 100.0));
        }

        (ratio < 0.2, format!("low_overlap({:.0}%)", ratio * 100.0))
    }
}

impl Default for DriftPenaltyRule {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightRule for DriftPenaltyRule {
    fn dimension(&self) -> WeightDimension {
        WeightDimension::DriftPenalty
    }

    fn compute(&self, ctx: &StepContext) -> DimensionScore {
        let (is_drift, detail) = Self::detect_drift(&ctx.description, &ctx.mainline_goal);
        let raw = if is_drift { -4 } else { 0 };
        let drift_multiplier = self.feedback.get_drift_multiplier();
        let weighted = raw as f64 * self.multiplier * drift_multiplier;

        let mut explanation = format!(
            "Drift{}{}",
            raw,
            if is_drift {
                format!("({})", detail)
            } else {
                String::new()
            }
        );
        if drift_multiplier > 1.01 {
            explanation.push_str(&format!(" [hist_mul={:.2}x]", drift_multiplier));
        }

        DimensionScore {
            dimension: WeightDimension::DriftPenalty,
            raw_score: raw,
            weighted_score: weighted,
            explanation,
        }
    }
}

// ============================================================================
// 权重引擎
// ============================================================================

/// 带权重的步骤结果
#[derive(Debug, Clone)]
pub struct WeightedStep {
    pub step_index: usize,
    /// 原始加权总分（未归一化）
    pub raw_score: f64,
    /// 归一化到 [0, 100] 的分数
    pub normalized_score: u32,
    /// 各维度得分
    pub dimension_scores: Vec<DimensionScore>,
    /// 一句话可解释性输出
    pub explanation: String,
}

/// 权重计算引擎
///
/// 使用硬编码规则计算步骤权重，支持环境变量微调系数。
pub struct WeightEngine {
    rules: Vec<Box<dyn WeightRule>>,
}

impl WeightEngine {
    /// 创建默认引擎（包含所有六维规则 + M2 反馈规则）
    pub fn default_engine() -> Self {
        let rules: Vec<Box<dyn WeightRule>> = vec![
            Box::new(RiskRule::new()),
            Box::new(ImpactRule::new()),
            Box::new(ComplexityRule::new()),
            Box::new(BlockerValueRule::new()),
            Box::new(DependencyPenaltyRule::new()),
            Box::new(DriftPenaltyRule::new()),
            Box::new(HistoricalFailureRule::new()),
        ];
        Self { rules }
    }

    pub fn from_multipliers(multipliers: &WeightMultipliers) -> Self {
        let rules: Vec<Box<dyn WeightRule>> = vec![
            Box::new(RiskRule::with_multiplier(multipliers.risk)),
            Box::new(ImpactRule::with_multiplier(multipliers.impact)),
            Box::new(ComplexityRule::with_multiplier(multipliers.complexity)),
            Box::new(BlockerValueRule::with_multiplier(multipliers.blocker)),
            Box::new(DependencyPenaltyRule::with_multiplier(
                multipliers.dependency,
            )),
            Box::new(DriftPenaltyRule::with_multiplier(multipliers.drift)),
            Box::new(HistoricalFailureRule::with_multiplier(
                multipliers.historical_failure,
            )),
        ];
        Self { rules }
    }

    /// 使用自定义规则创建引擎
    pub fn with_rules(rules: Vec<Box<dyn WeightRule>>) -> Self {
        Self { rules }
    }

    /// 计算单个步骤的权重
    pub fn compute(&self, ctx: &StepContext) -> WeightedStep {
        let mut dimension_scores = Vec::new();
        let mut raw_total = 0.0;

        for rule in &self.rules {
            let score = rule.compute(ctx);
            raw_total += score.weighted_score;
            dimension_scores.push(score);
        }

        let normalized = Self::sigmoid_normalize(raw_total);
        let explanation = Self::format_explanation(&dimension_scores, raw_total, normalized);

        WeightedStep {
            step_index: ctx.step_index,
            raw_score: raw_total,
            normalized_score: normalized,
            dimension_scores,
            explanation,
        }
    }

    /// 批量计算并排序（高权重在前）
    pub fn compute_and_sort(&self, contexts: Vec<StepContext>) -> Vec<WeightedStep> {
        let mut results: Vec<WeightedStep> = contexts.iter().map(|ctx| self.compute(ctx)).collect();
        results.sort_by(|a, b| {
            b.normalized_score.cmp(&a.normalized_score).then_with(|| {
                b.raw_score
                    .partial_cmp(&a.raw_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        results
    }

    /// Sigmoid 映射：将 [-120, +120] 映射到 [0, 100]
    ///
    /// 公式：sigmoid(x) = 100 / (1 + e^(-k * (x - mid) / range))
    /// 其中 k=6，使得在边界处接近饱和，在中段区分度好
    fn sigmoid_normalize(raw: f64) -> u32 {
        const RANGE: f64 = 120.0;
        const K: f64 = 6.0;

        // 硬边界：确保极值精确映射
        if raw >= RANGE {
            return 100;
        }
        if raw <= -RANGE {
            return 0;
        }

        let normalized = (raw + RANGE) / (2.0 * RANGE); // 映射到 [0, 1]
        let clamped = normalized.clamp(0.0, 1.0);
        let sigmoid = 1.0 / (1.0 + (-K * (clamped - 0.5)).exp());
        (sigmoid * 100.0) as u32
    }

    /// 格式化可解释性文本
    fn format_explanation(scores: &[DimensionScore], raw: f64, normalized: u32) -> String {
        let parts: Vec<String> = scores
            .iter()
            .map(|s| format!("{}={:.1}", s.dimension.name(), s.weighted_score))
            .collect();
        format!(
            "{} => Raw={:.1} => Score={}",
            parts.join(", "),
            raw,
            normalized
        )
    }
}

impl Default for WeightEngine {
    fn default() -> Self {
        Self::default_engine()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx(description: &str, tool: Option<&str>) -> StepContext {
        StepContext {
            description: description.into(),
            tool: tool.map(String::from),
            step_index: 0,
            total_steps: 3,
            mainline_goal: "实现用户认证系统".into(),
            completed_steps: vec![],
            dependent_steps: vec![],
            unlocks_count: 2,
        }
    }

    #[test]
    fn test_risk_rule_file_write() {
        let rule = RiskRule::new();
        let ctx = test_ctx("写入配置文件", Some("file_write"));
        let score = rule.compute(&ctx);
        assert_eq!(score.dimension, WeightDimension::Risk);
        assert!(score.raw_score >= 3);
        assert!(score.explanation.contains("file_write"));
    }

    #[test]
    fn test_risk_rule_bash() {
        let rule = RiskRule::new();
        let ctx = test_ctx("执行部署脚本", Some("bash"));
        let score = rule.compute(&ctx);
        assert!(score.raw_score >= 4);
    }

    #[test]
    fn test_impact_rule_public_api() {
        let rule = ImpactRule::new();
        let ctx = test_ctx("修改 public API 签名", None);
        let score = rule.compute(&ctx);
        assert!(score.raw_score >= 4);
    }

    #[test]
    fn test_complexity_rule_recursion() {
        let rule = ComplexityRule::new();
        let ctx = test_ctx("实现递归拆分状态机", None);
        let score = rule.compute(&ctx);
        assert!(score.raw_score >= 2);
    }

    #[test]
    fn test_blocker_value() {
        let rule = BlockerValueRule::new();
        let mut ctx = test_ctx("设计数据库表结构", None);
        ctx.unlocks_count = 5;
        let score = rule.compute(&ctx);
        assert_eq!(score.raw_score, 10); // 5 * 2 = 10
    }

    #[test]
    fn test_dependency_penalty() {
        let rule = DependencyPenaltyRule::new();
        let mut ctx = test_ctx("实现业务逻辑", None);
        ctx.dependent_steps = vec![0, 1];
        ctx.completed_steps = vec![0]; // step 1 未完成
        let score = rule.compute(&ctx);
        assert_eq!(score.raw_score, -3); // 1 个未完成依赖
    }

    #[test]
    fn test_drift_penalty_aligned() {
        let rule = DriftPenaltyRule::new();
        let ctx = test_ctx("实现用户登录接口", None); // 与主线"用户认证"对齐
        let score = rule.compute(&ctx);
        assert_eq!(score.raw_score, 0);
    }

    #[test]
    fn test_drift_penalty_drift() {
        let rule = DriftPenaltyRule::new();
        let ctx = test_ctx("重命名变量提高可读性", None); // 与主线无关
        let score = rule.compute(&ctx);
        assert!(score.raw_score < 0);
    }

    #[test]
    fn test_engine_compute() {
        let engine = WeightEngine::default();
        let ctx = test_ctx("新增 bash 工具支持外部后端", Some("bash"));
        let result = engine.compute(&ctx);

        assert_eq!(result.step_index, 0);
        assert!(result.normalized_score <= 100);
        assert_eq!(result.dimension_scores.len(), 7);
        assert!(!result.explanation.is_empty());
    }

    #[test]
    fn test_engine_sorting() {
        let engine = WeightEngine::default();

        let ctxs = vec![
            StepContext {
                description: "修复 typo".into(),
                tool: Some("file_edit".into()),
                step_index: 0,
                total_steps: 3,
                mainline_goal: "实现认证".into(),
                completed_steps: vec![],
                dependent_steps: vec![],
                unlocks_count: 0,
            },
            StepContext {
                description: "设计数据库 schema（解锁后续所有步骤）".into(),
                tool: None,
                step_index: 1,
                total_steps: 3,
                mainline_goal: "实现认证".into(),
                completed_steps: vec![],
                dependent_steps: vec![],
                unlocks_count: 5,
            },
            StepContext {
                description: "执行危险的数据库迁移".into(),
                tool: Some("bash".into()),
                step_index: 2,
                total_steps: 3,
                mainline_goal: "实现认证".into(),
                completed_steps: vec![],
                dependent_steps: vec![1], // 依赖步骤 1
                unlocks_count: 0,
            },
        ];

        let sorted = engine.compute_and_sort(ctxs);

        // 步骤 1（高 BlockerValue）应该排在最前
        assert_eq!(sorted[0].step_index, 1);
        assert!(sorted[0].normalized_score > sorted[1].normalized_score);
    }

    #[test]
    fn test_sigmoid_boundary() {
        // 极高分数应该接近 100
        assert_eq!(WeightEngine::sigmoid_normalize(120.0), 100);
        // 极低分数应该接近 0
        assert_eq!(WeightEngine::sigmoid_normalize(-120.0), 0);
        // 中间分数应该在 50 左右
        let mid = WeightEngine::sigmoid_normalize(0.0);
        assert!((45..=55).contains(&mid));
    }

    #[test]
    fn test_explanation_format() {
        let engine = WeightEngine::default();
        let ctx = test_ctx("测试", None);
        let result = engine.compute(&ctx);

        // 解释应包含维度名称和分数
        assert!(result.explanation.contains("Risk="));
        assert!(result.explanation.contains("Score="));
    }

    #[test]
    fn test_env_multiplier() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        env.set("PRIORITY_AGENT_WEIGHT_RISK_MUL", "2.0");
        let rule = RiskRule::new();
        let ctx = test_ctx("删除旧数据", Some("bash"));
        let score = rule.compute(&ctx);
        // bash(4) + delete(5) = 9 raw, * 2.0 = 18 weighted
        assert!(score.weighted_score >= 18.0);
        // env guard auto-restores on drop
    }

    #[test]
    fn test_dependency_with_completion() {
        let rule = DependencyPenaltyRule::new();
        let mut ctx = test_ctx("实现功能", None);
        ctx.dependent_steps = vec![0, 1];
        ctx.completed_steps = vec![0, 1]; // 所有依赖已完成
        let score = rule.compute(&ctx);
        assert_eq!(score.raw_score, 0); // 无惩罚
    }
}

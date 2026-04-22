//! 执行反馈引擎 — M2 质量增强
//!
//! 核心能力：
//! - 记录历史执行失败模式（步骤类型 → 失败次数）
//! - 在权重计算中给曾失败的步骤类型加权（历史失败惩罚）
//! - 动态调整 Drift Penalty（根据历史"跑偏"频率）
//!
//! 持久化：~/.priority-agent/workflow_feedback.json

use super::executor::{ExecutionOutcome, ExecutionRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 单步骤类型的历史统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepTypeHistory {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub needs_refactor: usize,
    /// 最近一次失败的时间戳（RFC3339）
    pub last_failure_at: Option<String>,
}

impl StepTypeHistory {
    /// 失败率（0.0 ~ 1.0）
    pub fn failure_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.failed + self.needs_refactor) as f64 / self.total as f64
        }
    }

    /// 是否需要应用历史惩罚（失败率 > 30% 且总次数 >= 2）
    pub fn is_problematic(&self) -> bool {
        self.total >= 2 && self.failure_rate() > 0.3
    }
}

/// 漂移历史统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriftHistory {
    /// 总步骤数
    pub total_steps: usize,
    /// 被判定为"偏离主线"的次数
    pub drift_count: usize,
    /// 最近一次漂移的时间戳
    pub last_drift_at: Option<String>,
}

impl DriftHistory {
    /// 漂移率
    pub fn drift_rate(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            self.drift_count as f64 / self.total_steps as f64
        }
    }

    /// 动态漂移惩罚系数：基础值 1.0 + 漂移率 * 2.0（最高 3.0）
    pub fn penalty_multiplier(&self) -> f64 {
        (1.0 + self.drift_rate() * 2.0).min(3.0)
    }
}

/// 反馈数据持久化结构
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedbackData {
    /// 按步骤描述关键词分组的历史统计
    pub step_histories: HashMap<String, StepTypeHistory>,
    /// 漂移历史
    pub drift_history: DriftHistory,
    /// 数据版本（便于未来迁移）
    pub version: u32,
}

impl FeedbackData {
    pub fn new() -> Self {
        Self {
            version: 1,
            ..Default::default()
        }
    }
}

/// 执行反馈引擎
pub struct FeedbackEngine {
    data: FeedbackData,
    path: std::path::PathBuf,
    /// 是否启用（默认 true，可通过环境变量关闭）
    enabled: bool,
}

impl FeedbackEngine {
    /// 从默认路径加载反馈数据
    pub fn load() -> Self {
        let enabled = std::env::var("PRIORITY_AGENT_WORKFLOW_FEEDBACK")
            .ok()
            .map(|v| v != "0")
            .unwrap_or(true);

        let path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".priority-agent")
            .join("workflow_feedback.json");

        let data = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(FeedbackData::new)
        } else {
            FeedbackData::new()
        };

        Self {
            data,
            path,
            enabled,
        }
    }

    /// 从执行记录更新历史统计
    pub fn record_execution(&mut self, records: &[ExecutionRecord]) {
        if !self.enabled {
            return;
        }

        let now = chrono::Local::now().to_rfc3339();

        for record in records {
            // 用描述的前 20 个字符作为关键词分组键（去空格）
            let key = extract_step_key(&record.description);
            let hist = self.data.step_histories.entry(key).or_default();

            hist.total += 1;
            match &record.outcome {
                ExecutionOutcome::Success(_) => hist.success += 1,
                ExecutionOutcome::Failed(_) => {
                    hist.failed += 1;
                    hist.last_failure_at = Some(now.clone());
                }
                ExecutionOutcome::NeedsRefactor(_) => {
                    hist.needs_refactor += 1;
                    hist.last_failure_at = Some(now.clone());
                }
                ExecutionOutcome::Skipped(_) => {}
            }
        }

        // 更新漂移历史：NeedsRefactor 视为漂移
        let drift_count = records
            .iter()
            .filter(|r| matches!(r.outcome, ExecutionOutcome::NeedsRefactor(_)))
            .count();

        self.data.drift_history.total_steps += records.len();
        self.data.drift_history.drift_count += drift_count;
        if drift_count > 0 {
            self.data.drift_history.last_drift_at = Some(now);
        }

        self.save();
    }

    /// 获取某步骤类型的历史惩罚（返回应减去的 raw score）
    /// 范围：0.0 ~ 15.0，失败率越高惩罚越大
    pub fn get_historical_penalty(&self, step_description: &str) -> f64 {
        if !self.enabled {
            return 0.0;
        }

        let key = extract_step_key(step_description);
        let Some(hist) = self.data.step_histories.get(&key) else {
            return 0.0;
        };

        if !hist.is_problematic() {
            return 0.0;
        }

        // 惩罚 = 失败率 * 15.0，最高 15.0
        let penalty = hist.failure_rate() * 15.0;
        penalty.min(15.0)
    }

    /// 获取动态漂移惩罚系数
    pub fn get_drift_multiplier(&self) -> f64 {
        if !self.enabled {
            return 1.0;
        }
        self.data.drift_history.penalty_multiplier()
    }

    /// 获取漂移历史摘要（用于报告）
    pub fn drift_summary(&self) -> String {
        let h = &self.data.drift_history;
        format!(
            "漂移历史: {} 步中漂移 {} 次（率 {:.1}%），当前惩罚系数 {:.2}x",
            h.total_steps,
            h.drift_count,
            h.drift_rate() * 100.0,
            h.penalty_multiplier()
        )
    }

    /// 持久化到磁盘
    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.data) {
            let _ = std::fs::create_dir_all(self.path.parent().unwrap_or(std::path::Path::new(".")));
            let _ = std::fs::write(&self.path, json);
        }
    }

    /// 清除所有历史数据（用于测试）
    pub fn clear(&mut self) {
        self.data = FeedbackData::new();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 提取步骤关键词（前 20 字符，去空格，转小写）
fn extract_step_key(description: &str) -> String {
    let normalized: String = description
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(20)
        .collect();
    if normalized.is_empty() {
        "(empty)".to_string()
    } else {
        normalized
    }
}

// ============================================================================
// 权重规则：历史失败惩罚
// ============================================================================

use super::weights::{DimensionScore, StepContext, WeightDimension, WeightRule};

/// 历史失败惩罚规则
///
/// 如果某类步骤在历史上失败率较高，则降低其权重（优先做高风险但高价值的步骤，
/// 或者把容易失败的步骤提前以留出更多重试时间）。
pub struct HistoricalFailureRule {
    feedback: FeedbackEngine,
}

impl HistoricalFailureRule {
    pub fn new() -> Self {
        Self {
            feedback: FeedbackEngine::load(),
        }
    }
}

impl WeightRule for HistoricalFailureRule {
    fn dimension(&self) -> WeightDimension {
        // 使用 Risk 维度作为载体（历史失败本质上是风险的一种）
        WeightDimension::Risk
    }

    fn compute(&self, ctx: &StepContext) -> DimensionScore {
        let penalty = self.feedback.get_historical_penalty(&ctx.description);

        let raw_score = -(penalty as i32);
        let explanation = if penalty > 0.0 {
            format!("历史失败惩罚: -{:.1}", penalty)
        } else {
            "无历史失败记录".to_string()
        };

        DimensionScore {
            dimension: self.dimension(),
            raw_score,
            weighted_score: raw_score as f64,
            explanation,
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(description: &str, outcome: ExecutionOutcome) -> ExecutionRecord {
        ExecutionRecord {
            step_index: 0,
            description: description.into(),
            tool: None,
            outcome,
            duration_ms: 100,
            retry_count: 0,
        }
    }

    #[test]
    fn test_feedback_record_and_penalty() {
        let mut engine = FeedbackEngine::load();
        engine.clear();

        // 模拟 3 次执行：2 次失败，1 次成功
        let records = vec![
            make_record("设计数据库表", ExecutionOutcome::Failed("err".into())),
            make_record("设计数据库表", ExecutionOutcome::Failed("err2".into())),
            make_record("设计数据库表", ExecutionOutcome::Success("ok".into())),
        ];
        engine.record_execution(&records);

        let penalty = engine.get_historical_penalty("设计数据库表");
        // 失败率 = 2/3 = 0.667, 惩罚 = 0.667 * 15 = 10.0
        assert!(penalty > 5.0, "Expected significant penalty, got {}", penalty);
    }

    #[test]
    fn test_drift_multiplier() {
        let mut engine = FeedbackEngine::load();
        engine.clear();

        // 初始无历史，系数为 1.0
        assert_eq!(engine.get_drift_multiplier(), 1.0);

        // 10 步中 5 次漂移 → 系数 = 1.0 + 0.5 * 2.0 = 2.0
        let records: Vec<_> = (0..10)
            .map(|i| {
                let outcome = if i < 5 {
                    ExecutionOutcome::NeedsRefactor("drift".into())
                } else {
                    ExecutionOutcome::Success("ok".into())
                };
                make_record(&format!("步骤 {}", i), outcome)
            })
            .collect();
        engine.record_execution(&records);

        let mult = engine.get_drift_multiplier();
        assert!((mult - 2.0).abs() < 0.1, "Expected ~2.0, got {}", mult);
    }

    #[test]
    fn test_no_penalty_for_new_step_type() {
        let mut engine = FeedbackEngine::load();
        engine.clear();

        // 从未执行过的步骤类型不应有惩罚
        let penalty = engine.get_historical_penalty("全新的步骤");
        assert_eq!(penalty, 0.0);
    }

    #[test]
    fn test_step_type_history_failure_rate() {
        let mut h = StepTypeHistory::default();
        h.total = 10;
        h.failed = 3;
        h.needs_refactor = 2;
        assert!((h.failure_rate() - 0.5).abs() < 0.01);
        assert!(h.is_problematic());
    }

    #[test]
    fn test_feedback_disabled() {
        // 设置环境变量禁用反馈
        std::env::set_var("PRIORITY_AGENT_WORKFLOW_FEEDBACK", "0");
        let engine = FeedbackEngine::load();
        assert_eq!(engine.get_drift_multiplier(), 1.0);
        assert_eq!(engine.get_historical_penalty("anything"), 0.0);
        std::env::remove_var("PRIORITY_AGENT_WORKFLOW_FEEDBACK");
    }
}

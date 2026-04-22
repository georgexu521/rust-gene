//! Workflow 执行指标聚合器
//!
//! M1 范围：轻量级执行统计，不持久化。
//! 从 ExecutionRecord 聚合成功/失败/重构等计数。

use super::executor::{ExecutionOutcome, ExecutionRecord};

/// 单步类型统计
#[derive(Debug, Clone, Default)]
pub struct StepTypeStats {
    pub count: usize,
    pub success: usize,
    pub failed: usize,
    pub needs_refactor: usize,
    pub skipped: usize,
    pub total_duration_ms: u64,
    pub total_retries: usize,
}

impl StepTypeStats {
    pub fn avg_duration_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.count as f64
        }
    }
}

/// Workflow 执行指标
#[derive(Debug, Clone, Default)]
pub struct WorkflowMetrics {
    pub total_steps: usize,
    pub success: usize,
    pub failed: usize,
    pub needs_refactor: usize,
    pub skipped: usize,
    pub total_duration_ms: u64,
    pub total_retries: usize,
    /// 按工具名称分组的统计（None 表示无工具）
    pub by_tool: std::collections::HashMap<String, StepTypeStats>,
}

impl WorkflowMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从执行记录列表聚合指标
    pub fn from_records(records: &[ExecutionRecord]) -> Self {
        let mut metrics = Self::new();
        for record in records {
            metrics.record(record);
        }
        metrics
    }

    /// 记录单步执行结果
    pub fn record(&mut self, record: &ExecutionRecord) {
        self.total_steps += 1;
        self.total_duration_ms += record.duration_ms;
        self.total_retries += record.retry_count;

        match &record.outcome {
            ExecutionOutcome::Success(_) => self.success += 1,
            ExecutionOutcome::Failed(_) => self.failed += 1,
            ExecutionOutcome::NeedsRefactor(_) => self.needs_refactor += 1,
            ExecutionOutcome::Skipped(_) => self.skipped += 1,
        }

        let tool_key = record.tool.clone().unwrap_or_else(|| "(none)".to_string());
        let stats = self.by_tool.entry(tool_key).or_default();
        stats.count += 1;
        stats.total_duration_ms += record.duration_ms;
        stats.total_retries += record.retry_count;
        match &record.outcome {
            ExecutionOutcome::Success(_) => stats.success += 1,
            ExecutionOutcome::Failed(_) => stats.failed += 1,
            ExecutionOutcome::NeedsRefactor(_) => stats.needs_refactor += 1,
            ExecutionOutcome::Skipped(_) => stats.skipped += 1,
        }
    }

    /// 成功率（百分比）
    pub fn success_rate(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            (self.success as f64 / self.total_steps as f64) * 100.0
        }
    }

    /// 重构率（百分比）
    pub fn refactor_rate(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            (self.needs_refactor as f64 / self.total_steps as f64) * 100.0
        }
    }

    /// 平均步骤耗时（毫秒）
    pub fn avg_duration_ms(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_steps as f64
        }
    }

    /// Markdown 格式摘要
    pub fn summary(&self) -> String {
        let mut output = String::new();
        output.push_str("## 执行指标\n\n");
        output.push_str(&format!(
            "- 总步骤: {} | 成功: {} | 失败: {} | 需重构: {} | 跳过: {}\n",
            self.total_steps, self.success, self.failed, self.needs_refactor, self.skipped
        ));
        output.push_str(&format!(
            "- 成功率: {:.1}% | 重构率: {:.1}%\n",
            self.success_rate(),
            self.refactor_rate()
        ));
        output.push_str(&format!(
            "- 总耗时: {}ms | 平均: {:.1}ms/步 | 总重试: {}\n",
            self.total_duration_ms,
            self.avg_duration_ms(),
            self.total_retries
        ));

        if !self.by_tool.is_empty() {
            output.push_str("\n### 按工具统计\n\n");
            let mut tools: Vec<_> = self.by_tool.iter().collect();
            tools.sort_by_key(|(k, _)| *k);
            for (tool, stats) in tools {
                output.push_str(&format!(
                    "- `{}`: {} 步（成功 {} / 失败 {} / 重构 {} / 跳过 {}），平均 {:.1}ms\n",
                    tool,
                    stats.count,
                    stats.success,
                    stats.failed,
                    stats.needs_refactor,
                    stats.skipped,
                    stats.avg_duration_ms()
                ));
            }
        }

        output
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(
        tool: Option<&str>,
        outcome: ExecutionOutcome,
        duration_ms: u64,
        retry_count: usize,
    ) -> ExecutionRecord {
        ExecutionRecord {
            step_index: 0,
            description: "test".into(),
            tool: tool.map(String::from),
            outcome,
            duration_ms,
            retry_count,
        }
    }

    #[test]
    fn test_metrics_from_records() {
        let records = vec![
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 100, 0),
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 200, 0),
            make_record(Some("file_edit"), ExecutionOutcome::NeedsRefactor("err".into()), 300, 1),
            make_record(None, ExecutionOutcome::Skipped("skip".into()), 50, 0),
        ];
        let m = WorkflowMetrics::from_records(&records);
        assert_eq!(m.total_steps, 4);
        assert_eq!(m.success, 2);
        assert_eq!(m.needs_refactor, 1);
        assert_eq!(m.skipped, 1);
        assert_eq!(m.total_duration_ms, 650);
        assert_eq!(m.total_retries, 1);
        assert!((m.success_rate() - 50.0).abs() < 0.1);
        assert!((m.refactor_rate() - 25.0).abs() < 0.1);
        assert!((m.avg_duration_ms() - 162.5).abs() < 0.1);
    }

    #[test]
    fn test_by_tool_grouping() {
        let records = vec![
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 100, 0),
            make_record(Some("bash"), ExecutionOutcome::Failed("err".into()), 200, 0),
            make_record(Some("grep"), ExecutionOutcome::Success("ok".into()), 50, 0),
        ];
        let m = WorkflowMetrics::from_records(&records);
        assert_eq!(m.by_tool.len(), 2);
        let bash = m.by_tool.get("bash").unwrap();
        assert_eq!(bash.count, 2);
        assert_eq!(bash.success, 1);
        assert_eq!(bash.failed, 1);
        assert!((bash.avg_duration_ms() - 150.0).abs() < 0.1);
    }

    #[test]
    fn test_empty_records() {
        let m = WorkflowMetrics::from_records(&[]);
        assert_eq!(m.total_steps, 0);
        assert_eq!(m.success_rate(), 0.0);
        assert_eq!(m.refactor_rate(), 0.0);
        assert_eq!(m.avg_duration_ms(), 0.0);
    }

    #[test]
    fn test_summary_contains_key_stats() {
        let records = vec![
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 100, 0),
            make_record(Some("file_edit"), ExecutionOutcome::NeedsRefactor("err".into()), 200, 1),
        ];
        let m = WorkflowMetrics::from_records(&records);
        let s = m.summary();
        assert!(s.contains("总步骤: 2"));
        assert!(s.contains("成功: 1"));
        assert!(s.contains("需重构: 1"));
        assert!(s.contains("成功率: 50.0%"));
        assert!(s.contains("bash"));
        assert!(s.contains("file_edit"));
    }
}

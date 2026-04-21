//! SLO (Service Level Objective) 看板
//!
//! 追踪稳定性、延迟、成本、错误率等关键指标

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// SLO 指标类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SloMetric {
    /// 可用性 (Availability)
    Availability,
    /// 延迟 (Latency)
    Latency,
    /// 成本 (Cost)
    Cost,
    /// 错误率 (Error Rate)
    ErrorRate,
}

/// SLO 目标定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloTarget {
    /// 指标类型
    pub metric: SloMetric,
    /// 目标值
    pub target: f64,
    /// 窗口大小（分钟）
    pub window_minutes: u32,
}

impl SloTarget {
    pub fn availability(target_value: f64, window_minutes: u32) -> Self {
        Self {
            metric: SloMetric::Availability,
            target: target_value,
            window_minutes,
        }
    }

    pub fn latency(target_value_ms: f64, window_minutes: u32) -> Self {
        Self {
            metric: SloMetric::Latency,
            target: target_value_ms,
            window_minutes,
        }
    }

    pub fn cost(target_value_usd: f64, window_minutes: u32) -> Self {
        Self {
            metric: SloMetric::Cost,
            target: target_value_usd,
            window_minutes,
        }
    }

    pub fn error_rate(target_value: f64, window_minutes: u32) -> Self {
        Self {
            metric: SloMetric::ErrorRate,
            target: target_value,
            window_minutes,
        }
    }
}

/// 单个指标数据点（不可序列化，仅用于内存追踪）
#[derive(Debug, Clone)]
pub struct MetricSample {
    pub timestamp: Instant,
    pub value: f64,
}

/// SLO 追踪器
#[derive(Debug)]
pub struct SloTracker {
    /// 指标样本（滑动窗口）
    samples: VecDeque<MetricSample>,
    /// 窗口大小
    window: Duration,
    /// 目标
    target: SloTarget,
}

impl SloTracker {
    pub fn new(target: SloTarget) -> Self {
        let window = Duration::from_secs(target.window_minutes as u64 * 60);
        Self {
            samples: VecDeque::new(),
            window,
            target,
        }
    }

    /// 记录一个样本
    pub fn record(&mut self, value: f64) {
        let now = Instant::now();
        self.samples.push_back(MetricSample { timestamp: now, value });
        self.cleanup(now);
    }

    /// 清理过期样本
    fn cleanup(&mut self, now: Instant) {
        while let Some(oldest) = self.samples.front() {
            if now.duration_since(oldest.timestamp) > self.window {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    /// 获取当前值（平均值）
    pub fn current_value(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.samples.iter().map(|s| s.value).sum();
        sum / self.samples.len() as f64
    }

    /// 获取窗口内样本数
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// 检查 SLO 是否达标
    pub fn is_slo_met(&self) -> bool {
        let current = self.current_value();
        match self.target.metric {
            SloMetric::Availability | SloMetric::Cost => current <= self.target.target,
            SloMetric::Latency | SloMetric::ErrorRate => current <= self.target.target,
        }
    }

    /// 获取达标状态
    pub fn status(&self) -> SloStatus {
        let current = self.current_value();
        let ratio = current / self.target.target;

        if ratio <= 0.8 {
            SloStatus::Healthy
        } else if ratio <= 1.0 {
            SloStatus::AtRisk
        } else {
            SloStatus::Breached
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SloStatus {
    /// 达标
    Healthy,
    /// 接近阈值
    AtRisk,
    /// 超出阈值
    Breached,
}

/// SLO 看板
#[derive(Debug)]
pub struct SloBoard {
    /// 各指标追踪器
    trackers: Vec<SloTracker>,
}

impl SloBoard {
    pub fn new() -> Self {
        let targets = vec![
            SloTarget::availability(99.5, 60),   // 99.5% 可用性
            SloTarget::latency(500.0, 15),        // P500 延迟 < 500ms
            SloTarget::cost(10.0, 60),            // 每小时成本 < $10
            SloTarget::error_rate(1.0, 30),      // 错误率 < 1%
        ];

        Self {
            trackers: targets.into_iter().map(SloTracker::new).collect(),
        }
    }

    /// 记录可用性样本
    pub fn record_availability(&mut self, value: f64) {
        if let Some(tracker) = self.trackers.iter_mut().find(|t| t.target.metric == SloMetric::Availability) {
            tracker.record(value);
        }
    }

    /// 记录延迟样本
    pub fn record_latency(&mut self, value_ms: f64) {
        if let Some(tracker) = self.trackers.iter_mut().find(|t| t.target.metric == SloMetric::Latency) {
            tracker.record(value_ms);
        }
    }

    /// 记录成本样本
    pub fn record_cost(&mut self, value_usd: f64) {
        if let Some(tracker) = self.trackers.iter_mut().find(|t| t.target.metric == SloMetric::Cost) {
            tracker.record(value_usd);
        }
    }

    /// 记录错误样本
    pub fn record_error(&mut self) {
        if let Some(tracker) = self.trackers.iter_mut().find(|t| t.target.metric == SloMetric::ErrorRate) {
            tracker.record(1.0);
        }
    }

    /// 获取所有 SLO 状态
    pub fn get_statuses(&self) -> Vec<(SloMetric, SloStatus, f64, f64)> {
        self.trackers
            .iter()
            .map(|t| {
                (
                    t.target.metric,
                    t.status(),
                    t.current_value(),
                    t.target.target,
                )
            })
            .collect()
    }

    /// 生成 SLO 报告
    pub fn report(&self) -> String {
        let mut lines = vec!["SLO Dashboard".to_string(), "=============".to_string()];

        for (metric, status, current, target) in self.get_statuses() {
            let status_str = match status {
                SloStatus::Healthy => "OK",
                SloStatus::AtRisk => "AT RISK",
                SloStatus::Breached => "BREACHED",
            };

            let metric_name = match metric {
                SloMetric::Availability => "Availability",
                SloMetric::Latency => "Latency",
                SloMetric::Cost => "Cost",
                SloMetric::ErrorRate => "Error Rate",
            };

            let unit = match metric {
                SloMetric::Availability => "%",
                SloMetric::Latency => "ms",
                SloMetric::Cost => "$",
                SloMetric::ErrorRate => "%",
            };

            lines.push(format!(
                "{}: {:.2}{} / {:.2}{} [{}]",
                metric_name, current, unit, target, unit, status_str
            ));
        }

        lines.join("\n")
    }
}

impl Default for SloBoard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slo_tracker() {
        let target = SloTarget::latency(500.0, 60);
        let mut tracker = SloTracker::new(target);

        tracker.record(100.0);
        tracker.record(200.0);
        tracker.record(300.0);

        assert_eq!(tracker.current_value(), 200.0);
        assert!(tracker.is_slo_met());
    }

    #[test]
    fn test_slo_board() {
        let mut board = SloBoard::new();
        board.record_availability(99.9);
        board.record_latency(250.0);
        board.record_cost(5.0);

        let statuses = board.get_statuses();
        assert_eq!(statuses.len(), 4);
    }
}

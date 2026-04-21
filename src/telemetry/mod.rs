//! Telemetry 与性能追踪
//!
//! 在用户同意的前提下，收集并持久化跨会话的性能指标。
//! 数据存储在 ~/.priority-agent/telemetry.json

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// 用户同意状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TelemetryConsent {
    /// 未设置（默认不收集）
    #[default]
    Unset,
    /// 用户同意收集
    Enabled,
    /// 用户拒绝收集
    Disabled,
}

impl TelemetryConsent {
    pub fn from_env() -> Self {
        let raw = std::env::var("PRIORITY_AGENT_TELEMETRY").unwrap_or_default();
        Self::from_value(&raw)
    }

    pub fn is_enabled(&self) -> bool {
        *self == TelemetryConsent::Enabled
    }

    fn from_value(raw: &str) -> Self {
        match raw.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" | "enabled" => TelemetryConsent::Enabled,
            "0" | "false" | "no" | "off" | "disabled" => TelemetryConsent::Disabled,
            _ => TelemetryConsent::Unset,
        }
    }
}

/// 单条会话记录
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionTelemetry {
    pub session_id: String,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub total_requests: u64,
    pub total_tokens: u64,
    pub tool_calls: u64,
    pub tool_success: u64,
    pub tool_failed: u64,
    pub estimated_cost_usd: f64,
    /// 工具耗时分布（工具名 -> 平均耗时 ms）
    pub tool_durations: HashMap<String, u64>,
    /// 崩溃/错误记录
    pub errors: Vec<String>,
    /// 编程质量指标（会话级）
    pub coding_rounds: u64,
    pub first_pass_successes: u64,
    pub verify_failures: u64,
    pub repair_cycles: u64,
}

/// 全局 telemetry 数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryData {
    pub first_recorded_at_ms: u64,
    pub last_updated_at_ms: u64,
    pub total_sessions: u64,
    pub sessions: Vec<SessionTelemetry>,
    /// 聚合工具成功率
    pub aggregated_tool_stats: HashMap<String, ToolAggregateStats>,
    /// 聚合编程质量指标
    pub aggregated_coding_rounds: u64,
    pub aggregated_first_pass_successes: u64,
    pub aggregated_verify_failures: u64,
    pub aggregated_repair_cycles: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolAggregateStats {
    pub calls: u64,
    pub success: u64,
    pub failed: u64,
    pub avg_duration_ms: u64,
}

/// Telemetry 收集器
pub struct TelemetryCollector {
    consent: TelemetryConsent,
    data_path: PathBuf,
    inner: Arc<Mutex<TelemetryData>>,
}

impl TelemetryCollector {
    pub fn new() -> Self {
        let consent = TelemetryConsent::from_env();
        let data_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("telemetry.json");
        let data = Self::load(&data_path);
        Self {
            consent,
            data_path,
            inner: Arc::new(Mutex::new(data)),
        }
    }

    /// 获取当前同意状态
    pub fn consent(&self) -> TelemetryConsent {
        self.consent
    }

    /// 设置同意状态并持久化
    pub fn set_consent(&mut self, consent: TelemetryConsent) {
        self.consent = consent;
        // 写回环境变量（仅当前进程）
        std::env::set_var(
            "PRIORITY_AGENT_TELEMETRY",
            match consent {
                TelemetryConsent::Enabled => "enabled",
                TelemetryConsent::Disabled => "disabled",
                TelemetryConsent::Unset => "",
            },
        );
    }

    /// 是否允许收集
    pub fn is_enabled(&self) -> bool {
        self.consent.is_enabled()
    }

    /// 记录会话结束数据
    pub fn record_session(&self, session: SessionTelemetry) {
        if !self.is_enabled() {
            return;
        }

        let mut data = self.inner.lock().unwrap();
        data.total_sessions += 1;
        data.last_updated_at_ms = now_ms();

        // 更新聚合统计
        let tool_calls = session.tool_calls.max(1);
        for (tool, dur) in &session.tool_durations {
            let agg = data.aggregated_tool_stats.entry(tool.clone()).or_default();
            agg.calls += 1;
            agg.success += session.tool_success / tool_calls;
            agg.failed += session.tool_failed / tool_calls;
            // 简单滑动平均，避免历史被完全覆盖
            agg.avg_duration_ms = if agg.avg_duration_ms == 0 {
                *dur
            } else {
                (agg.avg_duration_ms * 3 + dur) / 4
            };
        }
        data.aggregated_coding_rounds += session.coding_rounds;
        data.aggregated_first_pass_successes += session.first_pass_successes;
        data.aggregated_verify_failures += session.verify_failures;
        data.aggregated_repair_cycles += session.repair_cycles;

        data.sessions.push(session);
        // 只保留最近 100 条会话
        let session_count = data.sessions.len();
        if session_count > 100 {
            data.sessions = data.sessions.split_off(session_count - 100);
        }

        // 克隆数据以在释放锁后保存
        let data_clone = data.clone();
        drop(data);
        let _ = self.save(&data_clone);
    }

    /// 记录错误/崩溃
    pub fn record_error(&self, error: &str) {
        if !self.is_enabled() {
            return;
        }
        debug!("Telemetry recording error: {}", error);
        // 错误记录在当前会话中，由 record_session 统一持久化
    }

    /// 获取汇总报告
    pub fn summary(&self) -> TelemetryData {
        self.inner.lock().unwrap().clone()
    }

    /// 导出为 JSON 字符串
    pub fn export_json(&self) -> anyhow::Result<String> {
        let data = self.inner.lock().unwrap().clone();
        Ok(serde_json::to_string_pretty(&data)?)
    }

    fn load(path: &PathBuf) -> TelemetryData {
        if !path.exists() {
            return TelemetryData {
                first_recorded_at_ms: now_ms(),
                ..Default::default()
            };
        }
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                warn!("Failed to parse telemetry data: {}", e);
                TelemetryData {
                    first_recorded_at_ms: now_ms(),
                    ..Default::default()
                }
            }),
            Err(e) => {
                warn!("Failed to read telemetry data: {}", e);
                TelemetryData {
                    first_recorded_at_ms: now_ms(),
                    ..Default::default()
                }
            }
        }
    }

    fn save(&self, data: &TelemetryData) -> anyhow::Result<()> {
        if let Some(parent) = self.data_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(&self.data_path, json)?;
        Ok(())
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::env_guard::EnvVarGuard;

    #[test]
    fn test_telemetry_consent_from_env() {
        assert!(matches!(
            TelemetryConsent::from_value("enabled"),
            TelemetryConsent::Enabled
        ));
        assert!(matches!(
            TelemetryConsent::from_value("true"),
            TelemetryConsent::Enabled
        ));
        assert!(matches!(
            TelemetryConsent::from_value("disabled"),
            TelemetryConsent::Disabled
        ));
        assert!(matches!(
            TelemetryConsent::from_value(""),
            TelemetryConsent::Unset
        ));
    }

    #[test]
    fn test_telemetry_collector_disabled_by_default() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_TELEMETRY");
        let collector = TelemetryCollector::new();
        assert!(!collector.is_enabled());
    }

    #[test]
    fn test_session_telemetry_serde() {
        let session = SessionTelemetry {
            session_id: "test-123".to_string(),
            started_at_ms: 1000,
            ended_at_ms: Some(2000),
            total_requests: 5,
            total_tokens: 1000,
            tool_calls: 3,
            tool_success: 3,
            tool_failed: 0,
            estimated_cost_usd: 0.001,
            tool_durations: [("bash".to_string(), 500)].into(),
            errors: vec![],
            coding_rounds: 2,
            first_pass_successes: 1,
            verify_failures: 1,
            repair_cycles: 1,
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("test-123"));
    }

    #[test]
    fn test_record_session_aggregates_coding_quality() {
        let collector = TelemetryCollector::new();
        let session = SessionTelemetry {
            session_id: "test-coding-quality".to_string(),
            started_at_ms: 1000,
            ended_at_ms: Some(2000),
            total_requests: 1,
            total_tokens: 100,
            tool_calls: 1,
            tool_success: 1,
            tool_failed: 0,
            estimated_cost_usd: 0.0,
            tool_durations: [("bash".to_string(), 123)].into(),
            errors: vec![],
            coding_rounds: 3,
            first_pass_successes: 2,
            verify_failures: 1,
            repair_cycles: 1,
        };
        let mut collector = collector;
        collector.set_consent(TelemetryConsent::Enabled);
        collector.record_session(session);
        let data = collector.summary();
        assert!(data.aggregated_coding_rounds >= 3);
        assert!(data.aggregated_first_pass_successes >= 2);
        assert!(data.aggregated_verify_failures >= 1);
        assert!(data.aggregated_repair_cycles >= 1);
    }
}

//! 成本追踪器
//!
//! 追踪 API 调用成本和 token 使用情况

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// 成本追踪器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTracker {
    /// 会话开始时间
    pub session_start: SystemTime,
    /// 总请求数
    pub total_requests: u64,
    /// 总 token 数
    pub total_tokens: TokenCount,
    /// 估算成本（美元）
    pub estimated_cost_usd: f64,
    /// 模型使用统计
    pub model_usage: HashMap<String, ModelStats>,
    /// 工具调用统计
    pub tool_usage: HashMap<String, u64>,
    /// 工具执行指标（耗时/成功率/失败原因）
    pub tool_metrics: HashMap<String, ToolExecStats>,
    /// 最近工具调用明细（用于审计回放）
    pub recent_tool_events: Vec<ToolExecEvent>,
    /// 编程质量指标（一次通过率/修复轮次）
    pub coding_quality: CodingQualityStats,
}

/// Token 计数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenCount {
    pub prompt: u64,
    pub completion: u64,
    pub total: u64,
}

/// 模型统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelStats {
    pub requests: u64,
    pub tokens: TokenCount,
    pub estimated_cost: f64,
}

/// 工具执行统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolExecStats {
    pub calls: u64,
    pub success: u64,
    pub failed: u64,
    pub total_duration_ms: u64,
    pub max_duration_ms: u64,
    pub failure_reasons: HashMap<String, u64>,
    /// 重试次数
    pub retries: u64,
    /// 用户反馈：赞（ thumbs up）
    pub user_thumbs_up: u64,
    /// 用户反馈：踩（thumbs down）
    pub user_thumbs_down: u64,
}

impl ToolExecStats {
    /// 计算成功率（0.0 - 1.0）
    pub fn success_rate(&self) -> f64 {
        if self.calls == 0 {
            return 1.0;
        }
        self.success as f64 / self.calls as f64
    }

    /// 计算平均执行时间（毫秒）
    pub fn avg_duration_ms(&self) -> f64 {
        if self.calls == 0 {
            return 0.0;
        }
        self.total_duration_ms as f64 / self.calls as f64
    }

    /// 计算重试率（0.0 - 1.0）
    pub fn retry_rate(&self) -> f64 {
        if self.calls == 0 {
            return 0.0;
        }
        self.retries as f64 / self.calls as f64
    }

    /// 计算用户满意度（0.0 - 1.0）
    pub fn user_satisfaction(&self) -> f64 {
        let total_feedback = self.user_thumbs_up + self.user_thumbs_down;
        if total_feedback == 0 {
            return 0.5; // 无反馈时返回中性
        }
        self.user_thumbs_up as f64 / total_feedback as f64
    }

    /// 计算综合质量分数（0.0 - 100.0）
    /// 综合考虑成功率、平均耗时、用户满意度
    pub fn quality_score(&self) -> f64 {
        let success_weight = 0.5;
        let latency_weight = 0.2;
        let satisfaction_weight = 0.3;

        // 成功率分数（0-100）
        let success_score = self.success_rate() * 100.0;

        // 延迟分数（基于平均耗时，越低越好）
        // 假设 1000ms 以内为满分，10000ms 以上为0分
        let avg_ms = self.avg_duration_ms();
        let latency_score = if avg_ms <= 100.0 {
            100.0
        } else if avg_ms >= 10000.0 {
            0.0
        } else {
            100.0 - ((avg_ms - 100.0) / 9900.0 * 100.0)
        };

        // 用户满意度分数（0-100）
        let satisfaction_score = self.user_satisfaction() * 100.0;

        // 加权求和
        success_weight * success_score
            + latency_weight * latency_score
            + satisfaction_weight * satisfaction_score
    }
}

/// 工具调用审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecEvent {
    pub timestamp_ms: u64,
    pub tool_name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub failure_reason: Option<String>,
}

/// 编程质量指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodingQualityStats {
    /// 发生代码改动并触发验证的轮次
    pub file_change_rounds: u64,
    /// 首次验证即通过
    pub first_pass_successes: u64,
    /// 验证失败次数
    pub verify_failures: u64,
    /// 失败后再次修复并通过的轮次
    pub repair_cycles: u64,
    /// 当前是否处于失败链路中（内部状态）
    pub pending_failure_chain: bool,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self {
            session_start: SystemTime::now(),
            total_requests: 0,
            total_tokens: TokenCount::default(),
            estimated_cost_usd: 0.0,
            model_usage: HashMap::new(),
            tool_usage: HashMap::new(),
            tool_metrics: HashMap::new(),
            recent_tool_events: Vec::new(),
            coding_quality: CodingQualityStats::default(),
        }
    }
}

impl CostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录 API 调用
    pub fn record_api_call(&mut self, model: &str, prompt_tokens: u64, completion_tokens: u64) {
        self.total_requests += 1;
        self.total_tokens.prompt += prompt_tokens;
        self.total_tokens.completion += completion_tokens;
        self.total_tokens.total += prompt_tokens + completion_tokens;

        // 计算成本（使用 Kimi K2.5 定价作为参考）
        let cost = calculate_cost(model, prompt_tokens, completion_tokens);
        self.estimated_cost_usd += cost;

        // 更新模型统计
        let stats = self.model_usage.entry(model.to_string()).or_default();
        stats.requests += 1;
        stats.tokens.prompt += prompt_tokens;
        stats.tokens.completion += completion_tokens;
        stats.tokens.total += prompt_tokens + completion_tokens;
        stats.estimated_cost += cost;
    }

    /// 记录工具调用
    pub fn record_tool_call(&mut self, tool_name: &str) {
        *self.tool_usage.entry(tool_name.to_string()).or_insert(0) += 1;
    }

    /// 记录工具执行（含成功率、耗时、失败原因）
    pub fn record_tool_execution(
        &mut self,
        tool_name: &str,
        success: bool,
        duration_ms: u64,
        error: Option<&str>,
    ) {
        self.record_tool_call(tool_name);
        let stats = self.tool_metrics.entry(tool_name.to_string()).or_default();
        stats.calls += 1;
        stats.total_duration_ms += duration_ms;
        stats.max_duration_ms = stats.max_duration_ms.max(duration_ms);
        if success {
            stats.success += 1;
        } else {
            stats.failed += 1;
            let reason = classify_tool_failure_reason(error);
            *stats.failure_reasons.entry(reason).or_insert(0) += 1;
        }

        let failure_reason = if success {
            None
        } else {
            Some(classify_tool_failure_reason(error))
        };
        self.recent_tool_events.push(ToolExecEvent {
            timestamp_ms: now_epoch_ms(),
            tool_name: tool_name.to_string(),
            success,
            duration_ms,
            failure_reason,
        });
        // ring buffer: 保留最近 500 条
        const MAX_RECENT_EVENTS: usize = 500;
        if self.recent_tool_events.len() > MAX_RECENT_EVENTS {
            let drop_n = self.recent_tool_events.len() - MAX_RECENT_EVENTS;
            self.recent_tool_events.drain(0..drop_n);
        }
    }

    /// 记录工具重试
    pub fn record_tool_retry(&mut self, tool_name: &str) {
        let stats = self.tool_metrics.entry(tool_name.to_string()).or_default();
        stats.retries += 1;
    }

    /// 记录用户对工具结果的反馈
    pub fn record_tool_feedback(&mut self, tool_name: &str, thumbs_up: bool) {
        let stats = self.tool_metrics.entry(tool_name.to_string()).or_default();
        if thumbs_up {
            stats.user_thumbs_up += 1;
        } else {
            stats.user_thumbs_down += 1;
        }
    }

    /// 获取工具质量分数（用于质量排行）
    pub fn tool_quality_scores(&self) -> Vec<(String, f64)> {
        self.tool_metrics
            .iter()
            .filter(|(_, stats)| stats.calls > 0)
            .map(|(name, stats)| (name.clone(), stats.quality_score()))
            .collect()
    }

    /// 获取工具质量报告
    pub fn tool_quality_report(&self, limit: usize) -> String {
        let mut scores = self.tool_quality_scores();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if scores.is_empty() {
            return "tool_quality: (no data)".to_string();
        }

        let items: Vec<String> = scores
            .iter()
            .take(limit)
            .map(|(name, score)| format!("{}:{:.1}", name, score))
            .collect();

        format!("tool_quality: {}", items.join(", "))
    }

    /// 汇总工具诊断信息（用于 /doctor）
    pub fn tool_diagnostics_line(&self) -> String {
        let mut total_calls = 0_u64;
        let mut total_failed = 0_u64;
        let mut total_duration = 0_u64;
        for s in self.tool_metrics.values() {
            total_calls += s.calls;
            total_failed += s.failed;
            total_duration += s.total_duration_ms;
        }
        let avg_ms = if total_calls == 0 {
            0.0
        } else {
            total_duration as f64 / total_calls as f64
        };
        let failure_rate = if total_calls == 0 {
            0.0
        } else {
            (total_failed as f64 / total_calls as f64) * 100.0
        };
        format!(
            "tool_metrics: calls={} failed={} fail_rate={:.1}% avg_ms={:.1}",
            total_calls, total_failed, failure_rate, avg_ms
        )
    }

    /// 最慢工具摘要（用于 /doctor）
    pub fn slowest_tools_line(&self, limit: usize) -> String {
        let mut rows: Vec<(String, f64)> = self
            .tool_metrics
            .iter()
            .filter(|(_, s)| s.calls > 0)
            .map(|(name, s)| (name.clone(), s.total_duration_ms as f64 / s.calls as f64))
            .collect();
        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let items: Vec<String> = rows
            .into_iter()
            .take(limit)
            .map(|(name, avg)| format!("{}:{:.1}ms", name, avg))
            .collect();
        if items.is_empty() {
            "tool_slowest: (none)".to_string()
        } else {
            format!("tool_slowest: {}", items.join(", "))
        }
    }

    /// 失败原因摘要（用于 /doctor）
    pub fn top_failure_reasons_line(&self, limit: usize) -> String {
        let mut agg: HashMap<String, u64> = HashMap::new();
        for s in self.tool_metrics.values() {
            for (reason, cnt) in &s.failure_reasons {
                *agg.entry(reason.clone()).or_insert(0) += cnt;
            }
        }
        let mut rows: Vec<(String, u64)> = agg.into_iter().collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        let items: Vec<String> = rows
            .into_iter()
            .take(limit)
            .map(|(reason, cnt)| format!("{}:{}", reason, cnt))
            .collect();
        if items.is_empty() {
            "tool_fail_reasons: (none)".to_string()
        } else {
            format!("tool_fail_reasons: {}", items.join(", "))
        }
    }

    /// 最近工具调用明细（最新在前）
    pub fn recent_tool_events(&self, limit: usize) -> Vec<ToolExecEvent> {
        self.recent_tool_events
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn recent_tool_event_count(&self) -> usize {
        self.recent_tool_events.len()
    }

    /// 记录一次“修改代码后验证”结果
    pub fn record_coding_round(&mut self, verification_passed: bool) {
        self.coding_quality.file_change_rounds += 1;
        if verification_passed {
            if self.coding_quality.pending_failure_chain {
                self.coding_quality.repair_cycles += 1;
                self.coding_quality.pending_failure_chain = false;
            } else {
                self.coding_quality.first_pass_successes += 1;
            }
        } else {
            self.coding_quality.verify_failures += 1;
            self.coding_quality.pending_failure_chain = true;
        }
    }

    /// 编程质量摘要（一次通过率/修复轮次）
    pub fn coding_quality_line(&self) -> String {
        let rounds = self.coding_quality.file_change_rounds;
        if rounds == 0 {
            return "coding_quality: rounds=0".to_string();
        }
        let first_pass_rate =
            (self.coding_quality.first_pass_successes as f64 / rounds as f64) * 100.0;
        format!(
            "coding_quality: rounds={} first_pass={} ({:.1}%) verify_failures={} repairs={}",
            rounds,
            self.coding_quality.first_pass_successes,
            first_pass_rate,
            self.coding_quality.verify_failures,
            self.coding_quality.repair_cycles
        )
    }

    /// 导出审计快照 JSON（包含汇总和最近事件）
    pub fn export_audit_snapshot_json(
        &self,
        session_id: Option<&str>,
        recent_limit: usize,
    ) -> String {
        let snapshot = json!({
            "session_id": session_id,
            "exported_at_ms": now_epoch_ms(),
            "summary": {
                "total_requests": self.total_requests,
                "total_tokens": self.total_tokens,
                "estimated_cost_usd": self.estimated_cost_usd,
                "tool_usage": self.tool_usage,
                "tool_metrics": self.tool_metrics,
                "coding_quality": self.coding_quality,
            },
            "recent_events": self.recent_tool_events(recent_limit),
        });
        serde_json::to_string_pretty(&snapshot).unwrap_or_else(|_| "{}".to_string())
    }

    /// 获取会话持续时间
    pub fn session_duration(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.session_start)
            .unwrap_or_default()
    }

    /// 生成报告
    pub fn generate_report(&self) -> String {
        let duration = self.session_duration();
        let minutes = duration.as_secs() / 60;

        format!(
            r#"Cost Report
============
Session Duration: {}m {}s
Total Requests: {}
Total Tokens: {} (prompt: {}, completion: {})
Estimated Cost: ${:.4}

Model Usage:
{}

Tool Usage:
{}"#,
            minutes,
            duration.as_secs() % 60,
            self.total_requests,
            self.total_tokens.total,
            self.total_tokens.prompt,
            self.total_tokens.completion,
            self.estimated_cost_usd,
            self.format_model_usage(),
            self.format_tool_usage()
        )
    }

    fn format_model_usage(&self) -> String {
        if self.model_usage.is_empty() {
            return "  (no usage)".to_string();
        }

        self.model_usage
            .iter()
            .map(|(model, stats)| {
                format!(
                    "  {}: {} requests, {} tokens, ${:.4}",
                    model, stats.requests, stats.tokens.total, stats.estimated_cost
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_tool_usage(&self) -> String {
        if self.tool_usage.is_empty() {
            return "  (no usage)".to_string();
        }

        self.tool_usage
            .iter()
            .map(|(tool, count)| format!("  {}: {}", tool, count))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn classify_tool_failure_reason(error: Option<&str>) -> String {
    let e = error.unwrap_or("").to_ascii_lowercase();
    if e.contains("timed out") || e.contains("timeout") {
        "timeout".to_string()
    } else if e.contains("permission denied") {
        "permission".to_string()
    } else if e.contains("not found") {
        "not_found".to_string()
    } else if e.contains("blocked by pre-tool hook") {
        "hook_blocked".to_string()
    } else if e.contains("dangerous command") {
        "dangerous_command".to_string()
    } else if e.is_empty() {
        "unknown".to_string()
    } else {
        "other".to_string()
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 计算成本（基于 Kimi API 定价）
fn calculate_cost(model: &str, prompt_tokens: u64, completion_tokens: u64) -> f64 {
    // 价格（每 1K tokens）
    let (prompt_price, completion_price) = match model {
        "kimi-k2.5" => (0.0015, 0.0060), // $1.5 / $6.0 per 1M tokens
        "kimi-k2-turbo" => (0.0010, 0.0040),
        _ => (0.0015, 0.0060), // 默认
    };

    let prompt_cost = (prompt_tokens as f64 / 1000.0) * prompt_price;
    let completion_cost = (completion_tokens as f64 / 1000.0) * completion_price;

    prompt_cost + completion_cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_tracker() {
        let mut tracker = CostTracker::new();

        tracker.record_api_call("kimi-k2.5", 1000, 500);
        tracker.record_tool_call("file_read");
        tracker.record_tool_execution(
            "bash",
            false,
            1200,
            Some("Command timed out after 60 seconds"),
        );

        assert_eq!(tracker.total_requests, 1);
        assert_eq!(tracker.total_tokens.total, 1500);
        assert!(tracker.estimated_cost_usd > 0.0);
        assert_eq!(tracker.tool_metrics.get("bash").map(|s| s.failed), Some(1));
        assert_eq!(tracker.recent_tool_event_count(), 1);
    }

    #[test]
    fn test_cost_calculation() {
        let cost = calculate_cost("kimi-k2.5", 1000, 500);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_classify_tool_failure_reason() {
        assert_eq!(
            classify_tool_failure_reason(Some("Command timed out")),
            "timeout"
        );
        assert_eq!(
            classify_tool_failure_reason(Some("Permission denied")),
            "permission"
        );
        assert_eq!(
            classify_tool_failure_reason(Some("Tool 'x' not found")),
            "not_found"
        );
    }

    #[test]
    fn test_export_audit_snapshot_json() {
        let mut tracker = CostTracker::new();
        tracker.record_tool_execution("grep", true, 15, None);
        tracker.record_coding_round(true);
        let json = tracker.export_audit_snapshot_json(Some("sess_1"), 10);
        assert!(json.contains("\"session_id\""));
        assert!(json.contains("\"recent_events\""));
        assert!(json.contains("\"grep\""));
        assert!(json.contains("\"coding_quality\""));
    }

    #[test]
    fn test_coding_quality_stats() {
        let mut tracker = CostTracker::new();
        tracker.record_coding_round(false); // 首次失败
        tracker.record_coding_round(true); // 修复成功
        tracker.record_coding_round(true); // 一次通过

        assert_eq!(tracker.coding_quality.file_change_rounds, 3);
        assert_eq!(tracker.coding_quality.verify_failures, 1);
        assert_eq!(tracker.coding_quality.repair_cycles, 1);
        assert_eq!(tracker.coding_quality.first_pass_successes, 1);
        assert!(tracker.coding_quality_line().contains("first_pass=1"));
    }
}

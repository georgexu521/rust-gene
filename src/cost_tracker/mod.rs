//! 成本追踪器
//!
//! 追踪 API 调用成本和 token 使用情况

mod prompt_cache;

pub use prompt_cache::PromptCacheDiagnosticEntry;
use prompt_cache::{
    build_prompt_cache_diagnostic, render_prompt_cache_miss_report, trim_prompt_cache_diagnostics,
};
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
    /// Recent prompt-cache diagnostic entries inferred from stable request shape.
    #[serde(default)]
    pub prompt_cache_diagnostics: Vec<PromptCacheDiagnosticEntry>,
}

/// Token 计数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenCount {
    pub prompt: u64,
    pub completion: u64,
    pub total: u64,
    /// Cached prompt tokens (prefix cache hits from provider)
    #[serde(default)]
    pub cached: u64,
    /// Prompt tokens charged as cache misses by the provider.
    #[serde(default)]
    pub cache_miss: u64,
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
            prompt_cache_diagnostics: Vec::new(),
        }
    }
}

impl CostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录 API 调用（含缓存 token）
    pub fn record_api_call(
        &mut self,
        model: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_tokens: Option<u64>,
    ) {
        self.record_api_call_with_cache_shape(
            model,
            prompt_tokens,
            completion_tokens,
            cached_tokens,
            None,
        );
    }

    pub fn record_api_call_with_cache_shape(
        &mut self,
        model: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
        cached_tokens: Option<u64>,
        cache_shape: Option<crate::engine::cache_stability::CacheDiagnosticShape>,
    ) {
        self.total_requests += 1;
        let cache_usage =
            crate::engine::cache_stability::prompt_cache_usage(prompt_tokens, cached_tokens);
        self.total_tokens.prompt += prompt_tokens;
        self.total_tokens.completion += completion_tokens;
        self.total_tokens.total += prompt_tokens + completion_tokens;
        self.total_tokens.cached += cache_usage.cached_tokens;
        self.total_tokens.cache_miss += cache_usage.cache_miss_tokens;

        let cost = calculate_cost(
            model,
            prompt_tokens,
            completion_tokens,
            cache_usage.cached_tokens,
        );
        self.estimated_cost_usd += cost;

        // 更新模型统计
        let stats = self.model_usage.entry(model.to_string()).or_default();
        stats.requests += 1;
        stats.tokens.prompt += prompt_tokens;
        stats.tokens.completion += completion_tokens;
        stats.tokens.total += prompt_tokens + completion_tokens;
        stats.tokens.cached += cache_usage.cached_tokens;
        stats.tokens.cache_miss += cache_usage.cache_miss_tokens;
        stats.estimated_cost += cost;

        if let Some(cache_shape) = cache_shape {
            self.record_prompt_cache_diagnostic(model, cache_usage, cost, cache_shape);
        }
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

    /// 工具延迟百分位数（基于最近调用明细）
    /// 返回 Vec<(tool_name, p50_ms, p95_ms, p99_ms, sample_count)>
    pub fn tool_latency_percentiles(&self, limit: usize) -> Vec<(String, f64, f64, f64, usize)> {
        let mut by_tool: HashMap<String, Vec<u64>> = HashMap::new();
        for ev in &self.recent_tool_events {
            by_tool
                .entry(ev.tool_name.clone())
                .or_default()
                .push(ev.duration_ms);
        }

        let mut result: Vec<(String, f64, f64, f64, usize)> = Vec::new();
        for (name, mut durations) in by_tool {
            if durations.is_empty() {
                continue;
            }
            durations.sort_unstable();
            let n = durations.len();
            let p50 = percentile_sorted(&durations, 0.50);
            let p95 = percentile_sorted(&durations, 0.95);
            let p99 = percentile_sorted(&durations, 0.99);
            result.push((name, p50, p95, p99, n));
        }
        result.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        result.into_iter().take(limit).collect()
    }

    /// 模型使用概览（一行摘要）
    pub fn model_usage_summary(&self) -> String {
        if self.model_usage.is_empty() {
            return "model_usage: (no data)".to_string();
        }
        let items: Vec<String> = self
            .model_usage
            .iter()
            .map(|(model, stats)| {
                format!(
                    "{}: {}req {}tok ${:.3}",
                    model, stats.requests, stats.tokens.total, stats.estimated_cost
                )
            })
            .collect();
        format!("model_usage: {}", items.join(", "))
    }

    /// Token 使用概览
    pub fn token_summary(&self) -> String {
        if self.total_tokens.total == 0 {
            return "tokens: (no data)".to_string();
        }
        let prompt = self.total_tokens.prompt;
        let completion = self.total_tokens.completion;
        let total = self.total_tokens.total;
        let cached = self.total_tokens.cached;
        let cache_miss = self.total_tokens.cache_miss;
        let prompt_pct = (prompt as f64 / total as f64) * 100.0;
        let cached_info = if cached > 0 {
            format!(
                " cached={} miss={} hit_rate={:.1}%",
                cached,
                cache_miss,
                prompt_cache_hit_rate_percent(prompt, cached)
            )
        } else {
            String::new()
        };
        format!(
            "tokens: total={} prompt={} ({:.1}%) completion={} ({:.1}%){}",
            total,
            prompt,
            prompt_pct,
            completion,
            100.0 - prompt_pct,
            cached_info
        )
    }

    /// 编程质量详细报告
    pub fn coding_quality_detail(&self) -> String {
        let q = &self.coding_quality;
        if q.file_change_rounds == 0 {
            return "coding_quality: no code changes yet".to_string();
        }
        let rounds = q.file_change_rounds;
        let first_pass_rate = (q.first_pass_successes as f64 / rounds as f64) * 100.0;
        let repair_rate = (q.repair_cycles as f64 / rounds as f64) * 100.0;
        let fail_rate = (q.verify_failures as f64 / rounds as f64) * 100.0;
        let health = if first_pass_rate >= 70.0 {
            "healthy"
        } else if first_pass_rate >= 40.0 {
            "needs_improvement"
        } else {
            "concerning"
        };
        format!(
            "coding_quality: rounds={} first_pass={:.1}% repairs={:.1}% failures={:.1}% status={}",
            rounds, first_pass_rate, repair_rate, fail_rate, health
        )
    }

    /// 工具质量分数排行摘要
    pub fn tool_quality_ranking(&self, limit: usize) -> String {
        let mut scores = self.tool_quality_scores();
        if scores.is_empty() {
            return "tool_quality: (no data)".to_string();
        }
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let items: Vec<String> = scores
            .into_iter()
            .take(limit)
            .map(|(name, score)| format!("{}:{:.0}", name, score))
            .collect();
        format!("tool_quality: {}", items.join(", "))
    }

    /// 导出性能体检面板数据（结构化 JSON）
    pub fn performance_panel_json(&self) -> serde_json::Value {
        let latencies = self
            .tool_latency_percentiles(10)
            .into_iter()
            .map(|(name, p50, p95, p99, n)| {
                serde_json::json!({
                    "tool": name,
                    "p50_ms": p50,
                    "p95_ms": p95,
                    "p99_ms": p99,
                    "samples": n
                })
            })
            .collect::<Vec<_>>();

        serde_json::json!({
            "session_duration_secs": self.session_duration().as_secs(),
            "total_requests": self.total_requests,
            "tokens": self.total_tokens,
            "prompt_cache": {
                "prompt_tokens": self.total_tokens.prompt,
                "cached_tokens": self.total_tokens.cached,
                "cache_miss_tokens": self.total_tokens.cache_miss,
                "hit_rate": prompt_cache_hit_rate(self.total_tokens.prompt, self.total_tokens.cached),
            },
            "estimated_cost_usd": self.estimated_cost_usd,
            "model_usage": self.model_usage,
            "tool_metrics_summary": {
                "total_calls": self.tool_metrics.values().map(|s| s.calls).sum::<u64>(),
                "total_success": self.tool_metrics.values().map(|s| s.success).sum::<u64>(),
                "total_failed": self.tool_metrics.values().map(|s| s.failed).sum::<u64>(),
            },
            "tool_latency_percentiles": latencies,
            "coding_quality": self.coding_quality,
        })
    }

    /// 记录一次"修改代码后验证"结果
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
                "prompt_cache": {
                    "prompt_tokens": self.total_tokens.prompt,
                    "cached_tokens": self.total_tokens.cached,
                    "cache_miss_tokens": self.total_tokens.cache_miss,
                    "hit_rate": prompt_cache_hit_rate(self.total_tokens.prompt, self.total_tokens.cached),
                },
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
Prompt Cache: cached {} / miss {} / hit_rate {:.1}%
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
            self.total_tokens.cached,
            self.total_tokens.cache_miss,
            prompt_cache_hit_rate_percent(self.total_tokens.prompt, self.total_tokens.cached),
            self.estimated_cost_usd,
            self.format_model_usage(),
            self.format_tool_usage()
        )
    }

    pub fn prompt_cache_report(&self) -> String {
        let mut lines = vec![
            "Prompt Cache Report".to_string(),
            "===================".to_string(),
            format!("Requests: {}", self.total_requests),
            format!("Prompt Tokens: {}", self.total_tokens.prompt),
            format!("Cached Tokens: {}", self.total_tokens.cached),
            format!("Cache Miss Tokens: {}", self.total_tokens.cache_miss),
            format!(
                "Hit Rate: {:.1}%",
                prompt_cache_hit_rate_percent(self.total_tokens.prompt, self.total_tokens.cached)
            ),
        ];

        if self.model_usage.is_empty() {
            lines.push("Model Usage: (no data)".to_string());
            return lines.join("\n");
        }

        lines.push("".to_string());
        lines.push("By Model:".to_string());
        let mut rows = self.model_usage.iter().collect::<Vec<_>>();
        rows.sort_by(|left, right| left.0.cmp(right.0));
        for (model, stats) in rows {
            lines.push(format!(
                "  {}: {} requests, prompt={}, cached={}, miss={}, hit_rate={:.1}%",
                model,
                stats.requests,
                stats.tokens.prompt,
                stats.tokens.cached,
                stats.tokens.cache_miss,
                prompt_cache_hit_rate_percent(stats.tokens.prompt, stats.tokens.cached)
            ));
        }

        lines.join("\n")
    }

    pub fn prompt_cache_miss_report(&self) -> String {
        render_prompt_cache_miss_report(&self.prompt_cache_diagnostics)
    }

    fn record_prompt_cache_diagnostic(
        &mut self,
        model: &str,
        cache_usage: crate::engine::cache_stability::PromptCacheUsage,
        estimated_cost_usd: f64,
        cache_shape: crate::engine::cache_stability::CacheDiagnosticShape,
    ) {
        let entry = build_prompt_cache_diagnostic(
            model,
            self.total_requests,
            cache_usage,
            estimated_cost_usd,
            cache_shape,
            self.prompt_cache_diagnostics.last(),
        );
        self.prompt_cache_diagnostics.push(entry);
        trim_prompt_cache_diagnostics(&mut self.prompt_cache_diagnostics);
    }

    fn format_model_usage(&self) -> String {
        if self.model_usage.is_empty() {
            return "  (no usage)".to_string();
        }

        self.model_usage
            .iter()
            .map(|(model, stats)| {
                let cached_info = if stats.tokens.cached > 0 || stats.tokens.cache_miss > 0 {
                    format!(
                        " (cached: {}, miss: {}, hit_rate: {:.1}%)",
                        stats.tokens.cached,
                        stats.tokens.cache_miss,
                        prompt_cache_hit_rate_percent(stats.tokens.prompt, stats.tokens.cached)
                    )
                } else {
                    String::new()
                };
                format!(
                    "  {}: {} requests, {} tokens{}, ${:.4}",
                    model, stats.requests, stats.tokens.total, cached_info, stats.estimated_cost
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

fn prompt_cache_hit_rate(prompt_tokens: u64, cached_tokens: u64) -> f64 {
    if prompt_tokens == 0 {
        0.0
    } else {
        cached_tokens.min(prompt_tokens) as f64 / prompt_tokens as f64
    }
}

fn prompt_cache_hit_rate_percent(prompt_tokens: u64, cached_tokens: u64) -> f64 {
    prompt_cache_hit_rate(prompt_tokens, cached_tokens) * 100.0
}

fn percentile_sorted(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len();
    if n == 1 {
        return sorted[0] as f64;
    }
    let idx = (p * (n - 1) as f64).floor() as usize;
    let frac = p * (n - 1) as f64 - idx as f64;
    let lower = sorted[idx.min(n - 1)] as f64;
    let upper = sorted[(idx + 1).min(n - 1)] as f64;
    lower + frac * (upper - lower)
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 计算成本（基于 Kimi API 定价）
fn calculate_cost(
    model: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
    cached_tokens: u64,
) -> f64 {
    // 价格（每 1K tokens）
    let (prompt_price, completion_price) = match model {
        "kimi-k2.5" => (0.0015, 0.0060), // $1.5 / $6.0 per 1M tokens
        "kimi-k2-turbo" => (0.0010, 0.0040),
        _ => (0.0015, 0.0060), // 默认
    };

    // Cached tokens discount: 25% of normal prompt price (75% savings)
    let cached_discount = 0.25;
    let uncached_prompt = prompt_tokens.saturating_sub(cached_tokens);
    let prompt_cost = (uncached_prompt as f64 / 1000.0) * prompt_price
        + (cached_tokens as f64 / 1000.0) * prompt_price * cached_discount;
    let completion_cost = (completion_tokens as f64 / 1000.0) * completion_price;

    prompt_cost + completion_cost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::{Message, Tool};

    fn cache_shape(
        user_text: &str,
        tools: &[Tool],
    ) -> crate::engine::cache_stability::CacheDiagnosticShape {
        crate::engine::cache_stability::request_cache_diagnostic_shape(
            &[Message::system("stable system"), Message::user(user_text)],
            tools,
        )
    }

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            description: format!("{name} description"),
            parameters: serde_json::json!({"type": "object"}),
            strict_schema: false,
        }
    }

    #[test]
    fn test_cost_tracker() {
        let mut tracker = CostTracker::new();

        tracker.record_api_call("kimi-k2.5", 1000, 500, None);
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
        let cost = calculate_cost("kimi-k2.5", 1000, 500, 0);
        assert!(cost > 0.0);

        // Test cached token discount
        let cost_cached = calculate_cost("kimi-k2.5", 1000, 500, 800);
        assert!(cost_cached < cost, "Cached tokens should reduce cost");
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

    #[test]
    fn test_percentile_sorted() {
        let data = vec![1u64, 2, 3, 4, 5];
        assert_eq!(percentile_sorted(&data, 0.0), 1.0);
        assert_eq!(percentile_sorted(&data, 0.5), 3.0);
        assert_eq!(percentile_sorted(&data, 1.0), 5.0);

        let data2 = vec![10u64, 20, 30];
        assert_eq!(percentile_sorted(&data2, 0.5), 20.0);
    }

    #[test]
    fn test_tool_latency_percentiles() {
        let mut tracker = CostTracker::new();
        for ms in [10u64, 20, 30, 40, 50, 60, 70, 80, 90, 100] {
            tracker.record_tool_execution("bash", true, ms, None);
        }
        let pcts = tracker.tool_latency_percentiles(5);
        assert_eq!(pcts.len(), 1);
        let (_name, p50, p95, p99, n) = &pcts[0];
        assert_eq!(*n, 10);
        assert!(*p50 >= 50.0 && *p50 <= 60.0); // median ~55
        assert!(*p95 >= 90.0 && *p95 <= 100.0);
        assert!(*p99 >= 95.0);
    }

    #[test]
    fn test_token_summary() {
        let mut tracker = CostTracker::new();
        tracker.record_api_call("kimi-k2.5", 1000, 500, None);
        let summary = tracker.token_summary();
        assert!(summary.contains("total=1500"));
        assert!(summary.contains("prompt=1000"));
        assert!(summary.contains("completion=500"));
    }

    #[test]
    fn test_prompt_cache_miss_tracking_and_report() {
        let mut tracker = CostTracker::new();
        tracker.record_api_call("kimi-k2.5", 1000, 500, Some(800));

        assert_eq!(tracker.total_tokens.cached, 800);
        assert_eq!(tracker.total_tokens.cache_miss, 200);
        let summary = tracker.token_summary();
        assert!(summary.contains("cached=800"));
        assert!(summary.contains("miss=200"));
        assert!(summary.contains("hit_rate=80.0%"));

        let report = tracker.prompt_cache_report();
        assert!(report.contains("Cache Miss Tokens: 200"));
        assert!(report.contains("Hit Rate: 80.0%"));
        assert!(report.contains("kimi-k2.5"));
    }

    #[test]
    fn prompt_cache_miss_report_infers_reason_from_request_shape() {
        let mut tracker = CostTracker::new();
        let alpha = tool("alpha");
        tracker.record_api_call_with_cache_shape(
            "kimi-k2.5",
            1000,
            100,
            Some(0),
            Some(cache_shape("one", std::slice::from_ref(&alpha))),
        );
        tracker.record_api_call_with_cache_shape(
            "kimi-k2.5",
            1200,
            100,
            Some(900),
            Some(cache_shape("one", &[alpha, tool("beta")])),
        );

        assert_eq!(tracker.prompt_cache_diagnostics.len(), 2);
        let report = tracker.prompt_cache_miss_report();
        assert!(report.contains("Prompt Cache Miss Report"));
        assert!(report.contains("reason=cold-start"));
        assert!(report.contains("reason=tool-list-changed"));
        assert!(report.contains("Estimated Saved Cost"));
    }

    #[test]
    fn test_model_usage_summary() {
        let mut tracker = CostTracker::new();
        tracker.record_api_call("kimi-k2.5", 1000, 500, None);
        let summary = tracker.model_usage_summary();
        assert!(summary.contains("kimi-k2.5"));
        assert!(summary.contains("1req"));
    }

    #[test]
    fn test_coding_quality_detail() {
        let mut tracker = CostTracker::new();
        assert!(tracker.coding_quality_detail().contains("no code changes"));
        tracker.record_coding_round(true);
        assert!(tracker.coding_quality_detail().contains("status=healthy"));
    }

    #[test]
    fn test_tool_quality_ranking() {
        let mut tracker = CostTracker::new();
        tracker.record_tool_execution("grep", true, 10, None);
        tracker.record_tool_execution("grep", true, 20, None);
        let ranking = tracker.tool_quality_ranking(5);
        assert!(ranking.contains("grep"));
    }

    #[test]
    fn test_performance_panel_json() {
        let mut tracker = CostTracker::new();
        tracker.record_api_call("kimi-k2.5", 1000, 500, None);
        tracker.record_tool_execution("grep", true, 15, None);
        let json = tracker.performance_panel_json();
        assert!(json.get("session_duration_secs").is_some());
        assert!(json.get("tool_latency_percentiles").is_some());
    }
}

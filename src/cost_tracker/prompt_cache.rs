use serde::{Deserialize, Serialize};
use std::time::SystemTime;

const MAX_PROMPT_CACHE_DIAGNOSTICS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCacheDiagnosticEntry {
    pub timestamp_ms: u64,
    pub turn: u64,
    pub model: String,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub cache_miss_tokens: u64,
    pub hit_rate: f64,
    pub estimated_cost_usd: f64,
    pub saved_cost_usd: f64,
    pub miss_reason: String,
    pub miss_reason_detail: String,
    pub prefix_fingerprint: String,
    pub system_fingerprint: String,
    pub tool_schema_fingerprint: String,
    pub few_shots_fingerprint: String,
    pub dynamic_tail_fingerprint: String,
    pub tool_count: usize,
    pub tool_names: Vec<String>,
    pub dynamic_zone_messages: usize,
    pub dynamic_zones_before_last_user: usize,
    pub inferred: bool,
}

pub(super) fn build_prompt_cache_diagnostic(
    model: &str,
    turn: u64,
    cache_usage: crate::engine::cache_stability::PromptCacheUsage,
    estimated_cost_usd: f64,
    cache_shape: crate::engine::cache_stability::CacheDiagnosticShape,
    previous: Option<&PromptCacheDiagnosticEntry>,
) -> PromptCacheDiagnosticEntry {
    let previous_shape =
        previous.map(
            |entry| crate::engine::cache_stability::CacheDiagnosticShape {
                prefix_fingerprint: entry.prefix_fingerprint.clone(),
                system_fingerprint: entry.system_fingerprint.clone(),
                tool_schema_fingerprint: entry.tool_schema_fingerprint.clone(),
                few_shots_fingerprint: entry.few_shots_fingerprint.clone(),
                dynamic_tail_fingerprint: entry.dynamic_tail_fingerprint.clone(),
                tool_count: entry.tool_count,
                tool_names: entry.tool_names.clone(),
                message_count: 0,
                dynamic_zone_messages: entry.dynamic_zone_messages,
                dynamic_zones_before_last_user: entry.dynamic_zones_before_last_user,
            },
        );
    let inference = crate::engine::cache_stability::infer_cache_miss_reason(
        previous_shape.as_ref(),
        &cache_shape,
        cache_usage,
    );
    PromptCacheDiagnosticEntry {
        timestamp_ms: now_epoch_ms(),
        turn,
        model: model.to_string(),
        prompt_tokens: cache_usage.prompt_tokens,
        cached_tokens: cache_usage.cached_tokens,
        cache_miss_tokens: cache_usage.cache_miss_tokens,
        hit_rate: cache_usage.hit_ratio,
        estimated_cost_usd,
        saved_cost_usd: prompt_cache_savings_usd(model, cache_usage.cached_tokens),
        miss_reason: inference.reason.label().to_string(),
        miss_reason_detail: inference.detail,
        prefix_fingerprint: cache_shape.prefix_fingerprint,
        system_fingerprint: cache_shape.system_fingerprint,
        tool_schema_fingerprint: cache_shape.tool_schema_fingerprint,
        few_shots_fingerprint: cache_shape.few_shots_fingerprint,
        dynamic_tail_fingerprint: cache_shape.dynamic_tail_fingerprint,
        tool_count: cache_shape.tool_count,
        tool_names: cache_shape.tool_names,
        dynamic_zone_messages: cache_shape.dynamic_zone_messages,
        dynamic_zones_before_last_user: cache_shape.dynamic_zones_before_last_user,
        inferred: true,
    }
}

pub(super) fn trim_prompt_cache_diagnostics(entries: &mut Vec<PromptCacheDiagnosticEntry>) {
    if entries.len() <= MAX_PROMPT_CACHE_DIAGNOSTICS {
        return;
    }
    let drop_count = entries.len().saturating_sub(MAX_PROMPT_CACHE_DIAGNOSTICS);
    entries.drain(0..drop_count);
}

pub(super) fn render_prompt_cache_miss_report(entries: &[PromptCacheDiagnosticEntry]) -> String {
    if entries.is_empty() {
        return [
            "Prompt Cache Miss Report",
            "========================",
            "No per-turn prompt-cache diagnostics recorded for this session yet.",
            "Run one model turn first. Providers return cached/miss token counts; this report infers miss reasons from stable-prefix evidence.",
        ]
        .join("\n");
    }

    let prompt_tokens: u64 = entries.iter().map(|entry| entry.prompt_tokens).sum();
    let cached_tokens: u64 = entries.iter().map(|entry| entry.cached_tokens).sum();
    let cache_miss_tokens: u64 = entries.iter().map(|entry| entry.cache_miss_tokens).sum();
    let saved_cost_usd: f64 = entries.iter().map(|entry| entry.saved_cost_usd).sum();

    let mut lines = vec![
        "Prompt Cache Miss Report".to_string(),
        "========================".to_string(),
        format!("Recorded Turns: {}", entries.len()),
        format!("Prompt Tokens: {}", prompt_tokens),
        format!("Cached Tokens: {}", cached_tokens),
        format!("Cache Miss Tokens: {}", cache_miss_tokens),
        format!(
            "Hit Rate: {:.1}%",
            prompt_cache_hit_rate_percent(prompt_tokens, cached_tokens)
        ),
        format!("Estimated Saved Cost: ${saved_cost_usd:.4}"),
        "Note: miss reasons are inferred locally from stable request-shape hashes.".to_string(),
        "".to_string(),
        "Recent Turns:".to_string(),
    ];

    let recent = entries.iter().rev().take(8).collect::<Vec<_>>();
    for entry in recent.into_iter().rev() {
        lines.push(format!(
            "  #{} {} prompt={} cached={} miss={} hit_rate={:.1}% reason={}",
            entry.turn,
            entry.model,
            entry.prompt_tokens,
            entry.cached_tokens,
            entry.cache_miss_tokens,
            entry.hit_rate * 100.0,
            entry.miss_reason
        ));
        lines.push(format!("    detail: {}", entry.miss_reason_detail));
        lines.push(format!(
            "    prefix={} system={} tools={} few_shots={} dynamic_tail={} tool_count={} dynamic_zones={} before_last_user={}",
            preview_hash(&entry.prefix_fingerprint),
            preview_hash(&entry.system_fingerprint),
            preview_hash(&entry.tool_schema_fingerprint),
            preview_hash(&entry.few_shots_fingerprint),
            preview_hash(&entry.dynamic_tail_fingerprint),
            entry.tool_count,
            entry.dynamic_zone_messages,
            entry.dynamic_zones_before_last_user,
        ));
    }

    lines.join("\n")
}

fn prompt_cache_hit_rate_percent(prompt_tokens: u64, cached_tokens: u64) -> f64 {
    if prompt_tokens == 0 {
        0.0
    } else {
        cached_tokens.min(prompt_tokens) as f64 / prompt_tokens as f64 * 100.0
    }
}

fn prompt_cache_savings_usd(model: &str, cached_tokens: u64) -> f64 {
    let prompt_price = match model {
        "kimi-k2.5" => 0.0015,
        "kimi-k2-turbo" => 0.0010,
        _ => 0.0015,
    };
    let cached_discount = 0.25;
    (cached_tokens as f64 / 1000.0) * prompt_price * (1.0 - cached_discount)
}

fn preview_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

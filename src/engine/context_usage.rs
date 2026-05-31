use crate::engine::context_compressor::{
    estimate_messages_tokens, estimate_tokens, estimate_tool_schemas_tokens,
};
use crate::engine::model_context::ModelContextProfile;
use crate::services::api::{Message, Tool, Usage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextUsageSnapshot {
    pub message_tokens: u64,
    pub tool_schema_tokens: u64,
    pub system_prompt_tokens: u64,
    pub memory_snapshot_tokens: u64,
    pub estimated_history_tokens: u64,
    pub provider_prompt_tokens: Option<u64>,
    pub provider_completion_tokens: Option<u64>,
    pub provider_reasoning_tokens: Option<u64>,
    pub provider_cached_tokens: Option<u64>,
    pub provider_cache_miss_tokens: Option<u64>,
    pub reserved_output_tokens: u64,
    pub max_context_tokens: u64,
    pub total_request_tokens: u64,
    pub pressure_tokens: u64,
    pub remaining_context_tokens: u64,
}

impl ContextUsageSnapshot {
    pub fn estimate(
        profile: &ModelContextProfile,
        messages: &[Message],
        tools: &[Tool],
        system_prompt: &str,
        memory_snapshot: &str,
        latest_usage: Option<&Usage>,
    ) -> Self {
        let message_tokens = estimate_messages_tokens(messages);
        let tool_schema_tokens = estimate_tool_schemas_tokens(tools);
        let system_prompt_tokens = estimate_tokens(system_prompt);
        let memory_snapshot_tokens = estimate_tokens(memory_snapshot);
        let provider_prompt_tokens = latest_usage.map(|usage| usage.prompt_tokens as u64);
        let provider_completion_tokens = latest_usage.map(|usage| usage.completion_tokens as u64);
        let provider_reasoning_tokens =
            latest_usage.and_then(|usage| usage.reasoning_tokens.map(u64::from));
        let provider_cached_tokens =
            latest_usage.and_then(|usage| usage.cached_tokens.map(u64::from));
        let provider_cache_miss_tokens = latest_usage.map(|usage| {
            crate::engine::cache_stability::prompt_cache_usage(
                u64::from(usage.prompt_tokens),
                usage.cached_tokens.map(u64::from),
            )
            .cache_miss_tokens
        });

        let estimated_history_tokens = message_tokens;
        let total_request_tokens = message_tokens
            .saturating_add(tool_schema_tokens)
            .saturating_add(system_prompt_tokens)
            .saturating_add(memory_snapshot_tokens);
        let provider_pressure = latest_usage.map(|usage| {
            u64::from(usage.prompt_tokens)
                .saturating_add(u64::from(usage.completion_tokens))
                .saturating_add(usage.cached_tokens.map(u64::from).unwrap_or(0))
        });
        let pressure_tokens = provider_pressure
            .unwrap_or(total_request_tokens)
            .saturating_add(profile.reserved_output_tokens);
        let remaining_context_tokens = profile
            .context_window_tokens
            .saturating_sub(pressure_tokens);

        Self {
            message_tokens,
            tool_schema_tokens,
            system_prompt_tokens,
            memory_snapshot_tokens,
            estimated_history_tokens,
            provider_prompt_tokens,
            provider_completion_tokens,
            provider_reasoning_tokens,
            provider_cached_tokens,
            provider_cache_miss_tokens,
            reserved_output_tokens: profile.reserved_output_tokens,
            max_context_tokens: profile.context_window_tokens,
            total_request_tokens,
            pressure_tokens,
            remaining_context_tokens,
        }
    }

    pub fn estimate_with_limits(
        messages: &[Message],
        tools: &[Tool],
        system_prompt: &str,
        memory_snapshot: &str,
        max_context_tokens: u64,
        reserved_output_tokens: u64,
        latest_usage: Option<&Usage>,
    ) -> Self {
        let mut profile = ModelContextProfile::detect("", "openai-compatible-default");
        profile.context_window_tokens = max_context_tokens;
        profile.reserved_output_tokens = reserved_output_tokens;
        profile.auto_compact_threshold_tokens = profile
            .effective_context_window_tokens()
            .saturating_sub(13_000);
        profile.warning_threshold_tokens = profile
            .effective_context_window_tokens()
            .saturating_sub(20_000);
        profile.hard_block_threshold_tokens = profile
            .effective_context_window_tokens()
            .saturating_sub(3_000);
        Self::estimate(
            &profile,
            messages,
            tools,
            system_prompt,
            memory_snapshot,
            latest_usage,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_request_pressure_with_reserved_output() {
        let profile = ModelContextProfile::detect("https://api.openai.com/v1", "gpt-4o");
        let messages = vec![Message::system("sys"), Message::user("hello")];
        let snapshot = ContextUsageSnapshot::estimate(&profile, &messages, &[], "sys", "", None);

        assert!(snapshot.total_request_tokens > 0);
        assert_eq!(snapshot.max_context_tokens, 128_000);
        assert_eq!(snapshot.reserved_output_tokens, 16_000);
        assert!(snapshot.pressure_tokens >= snapshot.total_request_tokens);
        assert!(snapshot.remaining_context_tokens < snapshot.max_context_tokens);
    }

    #[test]
    fn uses_provider_usage_when_available() {
        let profile = ModelContextProfile::detect("https://api.openai.com/v1", "gpt-4o");
        let usage = Usage {
            prompt_tokens: 10_000,
            completion_tokens: 500,
            total_tokens: 10_500,
            reasoning_tokens: Some(100),
            cached_tokens: Some(8_000),
        };
        let snapshot = ContextUsageSnapshot::estimate(&profile, &[], &[], "", "", Some(&usage));

        assert_eq!(snapshot.provider_prompt_tokens, Some(10_000));
        assert_eq!(snapshot.provider_cached_tokens, Some(8_000));
        assert_eq!(snapshot.provider_cache_miss_tokens, Some(2_000));
        assert_eq!(snapshot.provider_reasoning_tokens, Some(100));
        assert!(snapshot.pressure_tokens >= 18_500);
    }
}

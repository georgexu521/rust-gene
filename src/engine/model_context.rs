use crate::services::api::provider_protocol::{ProviderCapabilities, ProviderProtocolFamily};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelContextProfile {
    pub provider_family: ProviderProtocolFamily,
    pub model_pattern: &'static str,
    pub context_window_tokens: u64,
    pub reserved_output_tokens: u64,
    pub auto_compact_threshold_tokens: u64,
    pub warning_threshold_tokens: u64,
    pub hard_block_threshold_tokens: u64,
    pub cache_accounting: CacheAccounting,
    pub source: ProfileSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheAccounting {
    Unknown,
    PromptCachedTokens,
    NotReported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSource {
    Exact,
    FamilyDefault,
    SafeFallback,
}

impl ModelContextProfile {
    pub fn detect(base_url: &str, model: &str) -> Self {
        let capabilities = ProviderCapabilities::detect(base_url, model);
        Self::for_family_and_model(capabilities.protocol_family, model)
    }

    pub fn for_family_and_model(family: ProviderProtocolFamily, model: &str) -> Self {
        let normalized = model.to_ascii_lowercase();
        let mut profile = match family {
            ProviderProtocolFamily::MiniMax => {
                if normalized.contains("m3") {
                    Self::new(
                        family,
                        "minimax-m3",
                        1_000_000,
                        24_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::Exact,
                    )
                } else if normalized.contains("m2.7") || normalized.contains("m1") {
                    Self::new(
                        family,
                        "minimax-m",
                        1_000_000,
                        20_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::Exact,
                    )
                } else {
                    Self::new(
                        family,
                        "minimax-default",
                        128_000,
                        16_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::FamilyDefault,
                    )
                }
            }
            ProviderProtocolFamily::Kimi => {
                if normalized.contains("k2.5") || normalized.contains("k2") {
                    Self::new(
                        family,
                        "kimi-k",
                        128_000,
                        16_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::Exact,
                    )
                } else {
                    Self::new(
                        family,
                        "kimi-default",
                        128_000,
                        16_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::FamilyDefault,
                    )
                }
            }
            ProviderProtocolFamily::AnthropicLike => {
                if normalized.contains("claude") {
                    Self::new(
                        family,
                        "claude",
                        200_000,
                        20_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::Exact,
                    )
                } else {
                    Self::new(
                        family,
                        "anthropic-default",
                        200_000,
                        20_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::FamilyDefault,
                    )
                }
            }
            ProviderProtocolFamily::ReasoningCapable => Self::new(
                family,
                "reasoning-default",
                128_000,
                32_000,
                CacheAccounting::PromptCachedTokens,
                ProfileSource::FamilyDefault,
            ),
            ProviderProtocolFamily::OpenAiCompatible => {
                if normalized.contains("gpt-4.1") || normalized.contains("gpt-4o") {
                    Self::new(
                        family,
                        "openai-gpt4-family",
                        128_000,
                        16_000,
                        CacheAccounting::PromptCachedTokens,
                        ProfileSource::Exact,
                    )
                } else {
                    Self::new(
                        family,
                        "openai-compatible-default",
                        128_000,
                        16_000,
                        CacheAccounting::Unknown,
                        ProfileSource::SafeFallback,
                    )
                }
            }
        };

        if let Ok(raw) = std::env::var("PRIORITY_AGENT_CONTEXT_WINDOW_TOKENS") {
            if let Ok(value) = raw.parse::<u64>() {
                if value > profile.reserved_output_tokens.saturating_add(1_000) {
                    profile.context_window_tokens = value;
                    profile.recompute_thresholds();
                    profile.source = ProfileSource::SafeFallback;
                }
            }
        }

        profile
    }

    pub fn effective_context_window_tokens(&self) -> u64 {
        self.context_window_tokens
            .saturating_sub(self.reserved_output_tokens)
    }

    fn new(
        provider_family: ProviderProtocolFamily,
        model_pattern: &'static str,
        context_window_tokens: u64,
        reserved_output_tokens: u64,
        cache_accounting: CacheAccounting,
        source: ProfileSource,
    ) -> Self {
        let mut profile = Self {
            provider_family,
            model_pattern,
            context_window_tokens,
            reserved_output_tokens,
            auto_compact_threshold_tokens: 0,
            warning_threshold_tokens: 0,
            hard_block_threshold_tokens: 0,
            cache_accounting,
            source,
        };
        profile.recompute_thresholds();
        profile
    }

    fn recompute_thresholds(&mut self) {
        let effective = self.effective_context_window_tokens();
        self.auto_compact_threshold_tokens = effective.saturating_sub(13_000);
        self.warning_threshold_tokens = effective.saturating_sub(20_000);
        self.hard_block_threshold_tokens = effective.saturating_sub(3_000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_minimax_large_context_profile() {
        let profile = ModelContextProfile::detect("https://api.minimaxi.com/v1", "MiniMax-M3");
        assert_eq!(profile.provider_family, ProviderProtocolFamily::MiniMax);
        assert_eq!(profile.context_window_tokens, 1_000_000);
        assert_eq!(profile.reserved_output_tokens, 20_000);
        assert!(profile.auto_compact_threshold_tokens < profile.effective_context_window_tokens());
    }

    #[test]
    fn detects_claude_profile_as_200k_family() {
        let profile = ModelContextProfile::detect("https://api.anthropic.com", "claude-sonnet-4");
        assert_eq!(
            profile.provider_family,
            ProviderProtocolFamily::AnthropicLike
        );
        assert_eq!(profile.context_window_tokens, 200_000);
        assert_eq!(profile.reserved_output_tokens, 20_000);
    }

    #[test]
    fn keeps_safe_fallback_for_unknown_openai_compatible_model() {
        let profile = ModelContextProfile::detect("https://api.example.com/v1", "custom-model");
        assert_eq!(
            profile.provider_family,
            ProviderProtocolFamily::OpenAiCompatible
        );
        assert_eq!(profile.context_window_tokens, 128_000);
        assert_eq!(profile.source, ProfileSource::SafeFallback);
    }
}

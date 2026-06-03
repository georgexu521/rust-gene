use crate::services::api::provider_protocol::ProviderLatencyProfile;

pub(super) fn llm_request_timeout() -> std::time::Duration {
    let secs = std::env::var("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(180)
        .clamp(30, 600);
    std::time::Duration::from_secs(secs)
}

pub(super) fn stream_chunk_idle_timeout() -> std::time::Duration {
    let secs = std::env::var("PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(120)
        .clamp(30, 600);
    std::time::Duration::from_secs(secs)
}

pub(super) fn profile_driven_timeout(profile: &ProviderLatencyProfile) -> std::time::Duration {
    if let Ok(secs) = std::env::var("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS") {
        if let Ok(secs) = secs.parse::<u64>() {
            return std::time::Duration::from_secs(secs.clamp(30, 600));
        }
    }
    profile.timeout
}

pub(super) fn profile_driven_slow_warning(profile: &ProviderLatencyProfile) -> std::time::Duration {
    profile.slow_warning_threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::provider_protocol::{ProviderCapabilities, ProviderProtocolFamily};

    #[test]
    fn llm_request_timeout_clamps_env_value() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "1");

        assert_eq!(llm_request_timeout().as_secs(), 30);
    }

    #[test]
    fn stream_idle_timeout_uses_default_when_unset() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.remove("PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS");

        assert_eq!(stream_chunk_idle_timeout().as_secs(), 120);
    }

    #[test]
    fn profile_driven_timeout_returns_default_when_env_unset() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.remove("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS");

        let capabilities = ProviderCapabilities::for_family(ProviderProtocolFamily::MiniMax);
        let profile = ProviderLatencyProfile::for_request(
            &capabilities,
            "MiniMax-M3",
            true,
            false,
            false,
            5,
            3,
        );

        let timeout = profile_driven_timeout(&profile);
        assert_eq!(timeout.as_secs(), 300);
    }

    #[test]
    fn profile_driven_timeout_env_overrides_profile() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "90");

        let capabilities = ProviderCapabilities::for_family(ProviderProtocolFamily::MiniMax);
        let profile = ProviderLatencyProfile::for_request(
            &capabilities,
            "MiniMax-M3",
            true,
            false,
            false,
            5,
            3,
        );

        let timeout = profile_driven_timeout(&profile);
        assert_eq!(timeout.as_secs(), 90);
    }

    #[test]
    fn minimax_nonstreaming_tool_call_gets_longer_timeout() {
        let capabilities = ProviderCapabilities::for_family(ProviderProtocolFamily::MiniMax);
        let profile = ProviderLatencyProfile::for_request(
            &capabilities,
            "MiniMax-M3",
            true,
            false,
            false,
            5,
            3,
        );

        assert_eq!(profile.timeout.as_secs(), 300);
        assert_eq!(profile.slow_warning_threshold.as_secs(), 90);
        assert!(profile.is_known_slow_path());
    }

    #[test]
    fn openai_streaming_text_gets_standard_timeout() {
        let capabilities =
            ProviderCapabilities::for_family(ProviderProtocolFamily::OpenAiCompatible);
        let profile =
            ProviderLatencyProfile::for_request(&capabilities, "gpt-4.1", false, true, false, 5, 0);

        assert_eq!(profile.timeout.as_secs(), 180);
        assert_eq!(profile.slow_warning_threshold.as_secs(), 45);
        assert!(!profile.is_known_slow_path());
    }
}

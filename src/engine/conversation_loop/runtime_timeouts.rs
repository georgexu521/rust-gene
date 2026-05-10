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

#[cfg(test)]
mod tests {
    use super::*;

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
}

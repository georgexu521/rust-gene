//! Provider reconnect/retry policy.
//!
//! Retries live at the provider request boundary. Tool execution is outside this
//! layer, so a reconnect never re-runs local side effects; it only retries the
//! outbound LLM request that failed before a usable response was received.

use std::fmt::Display;
use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::warn;

const DEFAULT_RECONNECT_ATTEMPTS: usize = 5;
const MAX_RECONNECT_ATTEMPTS: usize = 10;
const DEFAULT_BACKOFF_MS: u64 = 500;
const DEFAULT_MAX_BACKOFF_MS: u64 = 8_000;
const DEFAULT_JITTER_MS: u64 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRetryPolicy {
    /// Number of reconnect opportunities after the initial request.
    reconnect_attempts: usize,
    initial_backoff: Duration,
    max_backoff: Duration,
    jitter: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRetryNotice {
    pub provider: String,
    pub operation: String,
    pub attempt: usize,
    pub max_attempts: usize,
    pub delay: Duration,
    pub error: String,
}

impl ProviderRetryPolicy {
    pub fn from_env() -> Self {
        Self {
            reconnect_attempts: read_usize_env(
                "PRIORITY_AGENT_PROVIDER_RECONNECT_ATTEMPTS",
                DEFAULT_RECONNECT_ATTEMPTS,
            )
            .or_else(|| {
                read_usize_env(
                    "PRIORITY_AGENT_PROVIDER_RETRY_ATTEMPTS",
                    DEFAULT_RECONNECT_ATTEMPTS,
                )
            })
            .unwrap_or(DEFAULT_RECONNECT_ATTEMPTS)
            .min(MAX_RECONNECT_ATTEMPTS),
            initial_backoff: Duration::from_millis(
                read_u64_env(
                    "PRIORITY_AGENT_PROVIDER_RECONNECT_BASE_MS",
                    DEFAULT_BACKOFF_MS,
                )
                .unwrap_or(DEFAULT_BACKOFF_MS),
            ),
            max_backoff: Duration::from_millis(
                read_u64_env(
                    "PRIORITY_AGENT_PROVIDER_RECONNECT_MAX_MS",
                    DEFAULT_MAX_BACKOFF_MS,
                )
                .unwrap_or(DEFAULT_MAX_BACKOFF_MS),
            ),
            jitter: Duration::from_millis(
                read_u64_env(
                    "PRIORITY_AGENT_PROVIDER_RECONNECT_JITTER_MS",
                    DEFAULT_JITTER_MS,
                )
                .unwrap_or(DEFAULT_JITTER_MS),
            ),
        }
    }

    #[cfg(test)]
    pub fn for_tests(reconnect_attempts: usize, backoff: Duration) -> Self {
        Self {
            reconnect_attempts,
            initial_backoff: backoff,
            max_backoff: backoff,
            jitter: Duration::ZERO,
        }
    }

    pub fn reconnect_attempts(self) -> usize {
        self.reconnect_attempts
    }

    pub async fn retry<F, Fut, T, E>(
        self,
        provider: &str,
        operation: &str,
        request: F,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: Display,
    {
        self.retry_with_optional_observer(provider, operation, request, None)
            .await
    }

    pub async fn retry_with_optional_observer<F, Fut, T, E>(
        self,
        provider: &str,
        operation: &str,
        mut request: F,
        observer: Option<&(dyn Fn(ProviderRetryNotice) + Send + Sync)>,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: Display,
    {
        for reconnect in 0..=self.reconnect_attempts {
            match request().await {
                Ok(value) => return Ok(value),
                Err(error) => {
                    let error_string = error.to_string();
                    if reconnect >= self.reconnect_attempts
                        || !is_retryable_provider_error(&error_string)
                    {
                        return Err(error);
                    }
                    let next_attempt = reconnect + 1;
                    let delay = self.delay_for_reconnect(next_attempt);
                    if let Some(observer) = observer {
                        observer(ProviderRetryNotice {
                            provider: provider.to_string(),
                            operation: operation.to_string(),
                            attempt: next_attempt,
                            max_attempts: self.reconnect_attempts,
                            delay,
                            error: error_string.clone(),
                        });
                    }
                    warn!(
                        "Provider request failed transiently; reconnecting {}/{} for {} {} after {:?}: {}",
                        next_attempt,
                        self.reconnect_attempts,
                        provider,
                        operation,
                        delay,
                        error_string
                    );
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        unreachable!("provider retry loop returns on success or terminal error")
    }

    fn delay_for_reconnect(self, reconnect: usize) -> Duration {
        if self.initial_backoff.is_zero() {
            return Duration::ZERO;
        }
        let shift = reconnect.saturating_sub(1).min(10) as u32;
        let multiplier = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
        let base_ms = self
            .initial_backoff
            .as_millis()
            .saturating_mul(multiplier as u128)
            .min(self.max_backoff.as_millis()) as u64;
        Duration::from_millis(base_ms.saturating_add(self.jitter_ms(reconnect)))
    }

    fn jitter_ms(self, reconnect: usize) -> u64 {
        let max = self.jitter.as_millis() as u64;
        if max == 0 {
            return 0;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_millis() as u64;
        (now.wrapping_add((reconnect as u64).wrapping_mul(37))) % max
    }
}

pub fn is_retryable_provider_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "400",
            "401",
            "403",
            "unauthorized",
            "forbidden",
            "invalid params",
            "bad_request",
            "schema",
            "does not follow tool call",
            "context_length_exceeded",
        ],
    ) {
        return false;
    }
    contains_any(
        &lower,
        &[
            "error sending request",
            "operation timed out",
            "timeout",
            "connection reset",
            "connection refused",
            "connection closed",
            "connection aborted",
            "unexpected eof",
            "incomplete message",
            "body write aborted",
            "tls",
            "ssl_read",
            "502",
            "503",
            "504",
            "bad gateway",
            "service unavailable",
            "gateway timeout",
        ],
    )
}

fn read_usize_env(name: &str, default: usize) -> Option<usize> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().parse::<usize>().unwrap_or(default))
}

fn read_u64_env(name: &str, default: u64) -> Option<u64> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().parse::<u64>().unwrap_or(default))
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[derive(Debug, Clone)]
    struct TestError(&'static str);

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    #[tokio::test]
    async fn retries_transient_transport_errors_until_success() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let policy = ProviderRetryPolicy::for_tests(5, Duration::ZERO);

        let result = policy
            .retry("test", "chat", {
                let attempts = attempts.clone();
                move || {
                    let attempts = attempts.clone();
                    async move {
                        let current = attempts.fetch_add(1, Ordering::SeqCst);
                        if current < 2 {
                            Err(TestError("error sending request for url"))
                        } else {
                            Ok("ok")
                        }
                    }
                }
            })
            .await
            .unwrap();

        assert_eq!(result, "ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn observer_records_actual_reconnect_attempts() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let policy = ProviderRetryPolicy::for_tests(3, Duration::ZERO);
        let observer = {
            let observed = observed.clone();
            move |notice: ProviderRetryNotice| {
                observed.lock().unwrap().push(notice);
            }
        };

        let result = policy
            .retry_with_optional_observer(
                "test",
                "chat",
                {
                    let attempts = attempts.clone();
                    move || {
                        let attempts = attempts.clone();
                        async move {
                            let current = attempts.fetch_add(1, Ordering::SeqCst);
                            if current == 0 {
                                Err(TestError("error sending request for url"))
                            } else {
                                Ok("ok")
                            }
                        }
                    }
                },
                Some(&observer),
            )
            .await
            .unwrap();

        assert_eq!(result, "ok");
        let observed = observed.lock().unwrap();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].provider, "test");
        assert_eq!(observed[0].operation, "chat");
        assert_eq!(observed[0].attempt, 1);
        assert_eq!(observed[0].max_attempts, 3);
        assert!(observed[0].error.contains("error sending request"));
    }

    #[tokio::test]
    async fn does_not_retry_schema_or_auth_errors() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let policy = ProviderRetryPolicy::for_tests(5, Duration::ZERO);

        let result: Result<&str, TestError> = policy
            .retry("test", "chat", {
                let attempts = attempts.clone();
                move || {
                    let attempts = attempts.clone();
                    async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        Err(TestError("bad_request_error: invalid params"))
                    }
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reconnect_attempts_are_bounded() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let policy = ProviderRetryPolicy::for_tests(2, Duration::ZERO);

        let result: Result<&str, TestError> = policy
            .retry("test", "chat", {
                let attempts = attempts.clone();
                move || {
                    let attempts = attempts.clone();
                    async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        Err(TestError("OpenSSL SSL_read unexpected eof while reading"))
                    }
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn classifies_retryable_provider_errors() {
        assert!(is_retryable_provider_error("error sending request for url"));
        assert!(is_retryable_provider_error(
            "OpenSSL SSL_read: unexpected eof while reading"
        ));
        assert!(is_retryable_provider_error(
            "status 503 service unavailable"
        ));
        assert!(!is_retryable_provider_error(
            "bad_request_error: invalid params, tool call result does not follow tool call"
        ));
        assert!(!is_retryable_provider_error("401 unauthorized"));
    }
}

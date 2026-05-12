//! 错误分类器
//!
//! 参考 hermes-agent 的 FailoverReason 设计：
//! - 对 API 错误进行分类（不是所有错误都一样处理）
//! - 每个错误附带恢复策略（重试？压缩？切换 key？放弃？）
//! - 一次分类，后续只根据策略决策

use std::fmt;

/// 错误恢复策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// 立即重试
    Retry,
    /// 等待后重试（指数退避）
    RetryWithBackoff { backoff_ms: u64 },
    /// 压缩上下文后重试
    CompressAndRetry,
    /// 切换 API key/credential
    RotateCredential,
    /// 切换到 fallback 模型
    FallbackModel,
    /// 降低 max_tokens 重试
    ReduceTokensAndRetry,
    /// 放弃，不可恢复
    Abort,
}

impl fmt::Display for RecoveryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryAction::Retry => write!(f, "retry"),
            RecoveryAction::RetryWithBackoff { backoff_ms } => {
                write!(f, "retry after {}ms", backoff_ms)
            }
            RecoveryAction::CompressAndRetry => write!(f, "compress and retry"),
            RecoveryAction::RotateCredential => write!(f, "rotate credential"),
            RecoveryAction::FallbackModel => write!(f, "fallback model"),
            RecoveryAction::ReduceTokensAndRetry => write!(f, "reduce tokens and retry"),
            RecoveryAction::Abort => write!(f, "abort"),
        }
    }
}

/// 错误类别（参考 hermes FailoverReason）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    /// 认证失败 (401, 403)
    Auth,
    /// 额度/计费问题 (402)
    Billing,
    /// 速率限制 (429)
    RateLimited,
    /// 服务器过载 (500, 502, 503)
    Overloaded,
    /// 上下文溢出 (400 + 相关错误信息)
    ContextOverflow,
    /// 请求体过大 (413)
    PayloadTooLarge,
    /// 内容安全策略拦截
    ContentFiltered,
    /// 网络超时
    Timeout,
    /// 网络连接错误
    ConnectionError,
    /// JSON 解析错误（API 返回了畸形响应）
    MalformedResponse,
    /// 请求 schema / 参数不符合 provider 要求
    RequestSchema,
    /// provider 对消息协议顺序或 tool-call 关联的拒绝
    ProviderProtocol,
    /// 未知错误
    Unknown,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Auth => write!(f, "auth"),
            ErrorCategory::Billing => write!(f, "billing"),
            ErrorCategory::RateLimited => write!(f, "rate_limited"),
            ErrorCategory::Overloaded => write!(f, "overloaded"),
            ErrorCategory::ContextOverflow => write!(f, "context_overflow"),
            ErrorCategory::PayloadTooLarge => write!(f, "payload_too_large"),
            ErrorCategory::ContentFiltered => write!(f, "content_filtered"),
            ErrorCategory::Timeout => write!(f, "timeout"),
            ErrorCategory::ConnectionError => write!(f, "connection_error"),
            ErrorCategory::MalformedResponse => write!(f, "malformed_response"),
            ErrorCategory::RequestSchema => write!(f, "schema"),
            ErrorCategory::ProviderProtocol => write!(f, "provider_protocol"),
            ErrorCategory::Unknown => write!(f, "unknown"),
        }
    }
}

/// 分类后的错误
#[derive(Debug, Clone)]
pub struct ClassifiedError {
    /// 错误类别
    pub category: ErrorCategory,
    /// 恢复策略
    pub action: RecoveryAction,
    /// 原始错误信息
    pub message: String,
    /// 是否可重试
    pub retryable: bool,
    /// 当前重试次数
    pub attempt: u32,
}

impl ClassifiedError {
    /// 创建新的分类错误
    pub fn new(category: ErrorCategory, action: RecoveryAction, message: String) -> Self {
        let retryable = !matches!(action, RecoveryAction::Abort);
        Self {
            category,
            action,
            message,
            retryable,
            attempt: 0,
        }
    }

    /// 设置重试次数
    pub fn with_attempt(mut self, attempt: u32) -> Self {
        self.attempt = attempt;
        self
    }

    /// 是否应该继续重试
    pub fn should_retry(&self) -> bool {
        self.retryable && self.attempt < 3
    }

    /// 获取退避时间
    pub fn backoff_duration(&self) -> std::time::Duration {
        let base_ms = match &self.action {
            RecoveryAction::RetryWithBackoff { backoff_ms } => *backoff_ms,
            _ => match &self.category {
                ErrorCategory::RateLimited => 5000,
                ErrorCategory::Overloaded => 2000,
                _ => 1000,
            },
        };
        // 指数退避
        let ms = base_ms * 2u64.pow(self.attempt.min(5));
        std::time::Duration::from_millis(ms)
    }
}

impl fmt::Display for ClassifiedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} (action: {}, attempt: {})",
            self.category, self.message, self.action, self.attempt
        )
    }
}

/// 错误分类器
pub struct ErrorClassifier;

impl ErrorClassifier {
    /// 从 HTTP 状态码和响应体分类
    pub fn from_http(status: u16, body: &str) -> ClassifiedError {
        let body_lower = body.to_lowercase();

        match status {
            401 | 403 => ClassifiedError::new(
                ErrorCategory::Auth,
                RecoveryAction::RotateCredential,
                format!(
                    "Authentication failed ({}): {}",
                    status,
                    truncate(body, 200)
                ),
            ),
            402 => ClassifiedError::new(
                ErrorCategory::Billing,
                RecoveryAction::Abort,
                format!("Billing/quota issue: {}", truncate(body, 200)),
            ),
            429 => {
                // 尝试从 headers 提取 retry-after
                let backoff = Self::extract_retry_after(body).unwrap_or(5000);
                ClassifiedError::new(
                    ErrorCategory::RateLimited,
                    RecoveryAction::RetryWithBackoff {
                        backoff_ms: backoff,
                    },
                    format!("Rate limited: {}", truncate(body, 200)),
                )
            }
            400 => {
                if Self::is_context_overflow(&body_lower) {
                    ClassifiedError::new(
                        ErrorCategory::ContextOverflow,
                        RecoveryAction::CompressAndRetry,
                        "Context window overflow".to_string(),
                    )
                } else if Self::is_provider_protocol_error(&body_lower) {
                    ClassifiedError::new(
                        ErrorCategory::ProviderProtocol,
                        RecoveryAction::Abort,
                        format!(
                            "Provider protocol rejected request: {}",
                            truncate(body, 200)
                        ),
                    )
                } else if Self::is_request_schema_error(&body_lower) {
                    ClassifiedError::new(
                        ErrorCategory::RequestSchema,
                        RecoveryAction::Abort,
                        format!("Request schema rejected: {}", truncate(body, 200)),
                    )
                } else {
                    ClassifiedError::new(
                        ErrorCategory::Unknown,
                        RecoveryAction::Abort,
                        format!("Bad request (400): {}", truncate(body, 200)),
                    )
                }
            }
            413 => ClassifiedError::new(
                ErrorCategory::PayloadTooLarge,
                RecoveryAction::CompressAndRetry,
                "Request payload too large".to_string(),
            ),
            500 | 502 | 503 => ClassifiedError::new(
                ErrorCategory::Overloaded,
                RecoveryAction::RetryWithBackoff { backoff_ms: 2000 },
                format!("Server error ({}): {}", status, truncate(body, 200)),
            ),
            _ => ClassifiedError::new(
                ErrorCategory::Unknown,
                if status >= 500 {
                    RecoveryAction::RetryWithBackoff { backoff_ms: 1000 }
                } else {
                    RecoveryAction::Abort
                },
                format!("HTTP {}: {}", status, truncate(body, 200)),
            ),
        }
    }

    /// 从 anyhow::Error 分类（网络错误、超时等）
    pub fn from_anyhow(err: &anyhow::Error) -> ClassifiedError {
        let msg = err.to_string();
        let msg_lower = msg.to_lowercase();

        if Self::is_provider_protocol_error(&msg_lower) {
            ClassifiedError::new(
                ErrorCategory::ProviderProtocol,
                RecoveryAction::Abort,
                format!(
                    "Provider protocol rejected request: {}",
                    truncate(&msg, 200)
                ),
            )
        } else if Self::is_request_schema_error(&msg_lower) {
            ClassifiedError::new(
                ErrorCategory::RequestSchema,
                RecoveryAction::Abort,
                format!("Request schema rejected: {}", truncate(&msg, 200)),
            )
        } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
            ClassifiedError::new(
                ErrorCategory::Timeout,
                RecoveryAction::RetryWithBackoff { backoff_ms: 3000 },
                format!("Request timed out: {}", truncate(&msg, 200)),
            )
        } else if msg_lower.contains("connection") || msg_lower.contains("connect") {
            ClassifiedError::new(
                ErrorCategory::ConnectionError,
                RecoveryAction::RetryWithBackoff { backoff_ms: 2000 },
                format!("Connection error: {}", truncate(&msg, 200)),
            )
        } else if msg_lower.contains("context")
            || msg_lower.contains("token")
            || msg_lower.contains("maximum")
        {
            ClassifiedError::new(
                ErrorCategory::ContextOverflow,
                RecoveryAction::CompressAndRetry,
                format!("Possible context overflow: {}", truncate(&msg, 200)),
            )
        } else {
            ClassifiedError::new(
                ErrorCategory::Unknown,
                RecoveryAction::RetryWithBackoff { backoff_ms: 1000 },
                truncate(&msg, 200),
            )
        }
    }

    /// 从 serde 解析错误分类
    pub fn from_parse_error(err: &serde_json::Error) -> ClassifiedError {
        ClassifiedError::new(
            ErrorCategory::MalformedResponse,
            RecoveryAction::Retry,
            format!("Failed to parse API response: {}", err),
        )
    }

    /// 检查是否是上下文溢出
    fn is_context_overflow(body: &str) -> bool {
        let indicators = [
            "context length",
            "context window",
            "maximum context",
            "token limit",
            "too many tokens",
            "input length",
            "max_tokens",
            "context_length_exceeded",
        ];
        indicators.iter().any(|i| body.contains(i))
    }

    fn is_provider_protocol_error(body: &str) -> bool {
        let indicators = [
            "does not follow tool call",
            "tool call result",
            "tool_call_id",
            "tool_calls",
            "messages with role 'tool'",
            "missing tool response",
        ];
        indicators.iter().any(|i| body.contains(i))
    }

    fn is_request_schema_error(body: &str) -> bool {
        let indicators = [
            "invalid params",
            "bad_request",
            "invalid request",
            "schema",
            "json schema",
            "invalid parameter",
        ];
        indicators.iter().any(|i| body.contains(i))
    }

    /// 从响应中提取 retry-after 秒数
    fn extract_retry_after(body: &str) -> Option<u64> {
        // 简单实现：尝试从 body 中找数字
        // 实际应该从 HTTP headers 解析
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(retry) = json.get("retry_after").and_then(|v| v.as_f64()) {
                return Some((retry * 1000.0) as u64);
            }
            if let Some(retry) = json
                .get("error")
                .and_then(|e| e.get("retry_after"))
                .and_then(|v| v.as_f64())
            {
                return Some((retry * 1000.0) as u64);
            }
        }
        None
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_rate_limit() {
        let err = ErrorClassifier::from_http(429, "rate limit exceeded");
        assert_eq!(err.category, ErrorCategory::RateLimited);
        assert!(err.retryable);
        assert!(err.should_retry());
    }

    #[test]
    fn test_classify_auth_error() {
        let err = ErrorClassifier::from_http(401, "invalid api key");
        assert_eq!(err.category, ErrorCategory::Auth);
        assert_eq!(err.action, RecoveryAction::RotateCredential);
        // RotateCredential is retryable (try another key)
        assert!(err.retryable);
    }

    #[test]
    fn test_classify_context_overflow() {
        let err =
            ErrorClassifier::from_http(400, "This model's maximum context length is 128000 tokens");
        assert_eq!(err.category, ErrorCategory::ContextOverflow);
        assert_eq!(err.action, RecoveryAction::CompressAndRetry);
    }

    #[test]
    fn test_classify_provider_protocol_bad_request() {
        let err = ErrorClassifier::from_http(
            400,
            "bad_request_error: invalid params, tool call result does not follow tool call",
        );
        assert_eq!(err.category, ErrorCategory::ProviderProtocol);
        assert_eq!(err.action, RecoveryAction::Abort);
        assert!(!err.retryable);
    }

    #[test]
    fn test_classify_request_schema_bad_request() {
        let err = ErrorClassifier::from_http(400, "bad_request_error: invalid params");
        assert_eq!(err.category, ErrorCategory::RequestSchema);
        assert_eq!(err.action, RecoveryAction::Abort);
        assert!(!err.retryable);
    }

    #[test]
    fn test_classify_server_error() {
        let err = ErrorClassifier::from_http(503, "service unavailable");
        assert_eq!(err.category, ErrorCategory::Overloaded);
        assert!(err.retryable);
    }

    #[test]
    fn test_classify_timeout() {
        let err = ErrorClassifier::from_anyhow(&anyhow::anyhow!("connection timed out after 30s"));
        assert_eq!(err.category, ErrorCategory::Timeout);
        assert!(err.retryable);
    }

    #[test]
    fn test_backoff_exponential() {
        let err = ClassifiedError::new(
            ErrorCategory::Overloaded,
            RecoveryAction::RetryWithBackoff { backoff_ms: 1000 },
            "test".to_string(),
        );
        assert_eq!(
            err.backoff_duration(),
            std::time::Duration::from_millis(1000)
        );

        let err = err.with_attempt(2);
        assert_eq!(
            err.backoff_duration(),
            std::time::Duration::from_millis(4000)
        );

        let err = err.with_attempt(3);
        assert_eq!(
            err.backoff_duration(),
            std::time::Duration::from_millis(8000)
        );
    }

    #[test]
    fn test_max_retries() {
        let err = ClassifiedError::new(
            ErrorCategory::Overloaded,
            RecoveryAction::Retry,
            "test".to_string(),
        )
        .with_attempt(3);
        assert!(!err.should_retry()); // 3 attempts exhausted
    }
}

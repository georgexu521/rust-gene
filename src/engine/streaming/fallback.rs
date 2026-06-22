//! Streaming runtime helper module.
//!
//! Keeps stream fallback and turn-message shaping separate from the core query engine.

/// Fallback 状态追踪
#[derive(Debug, Clone)]
pub(super) struct FallbackState {
    /// 连续 529 (Model Overloaded) 错误计数
    pub(super) consecutive_529_count: u32,
    /// 上次错误类型
    pub(super) last_error_type: ErrorType,
    /// 是否已触发 fallback
    pub(super) fallback_triggered: bool,
    /// fallback 尝试次数
    pub(super) fallback_attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ErrorType {
    RateLimit,       // 429
    ModelOverloaded, // 529
    ContextTooLong,  // 413
    Timeout,
    AuthError,   // 401/403
    ServerError, // 500
    Unknown,
}

impl ErrorType {
    pub(super) fn from_error_str(err_str: &str) -> Self {
        if err_str.contains("rate limit") || err_str.contains("429") {
            ErrorType::RateLimit
        } else if err_str.contains("overloaded")
            || err_str.contains("529")
            || err_str.contains("model overloaded")
        {
            ErrorType::ModelOverloaded
        } else if err_str.contains("context")
            || err_str.contains("413")
            || err_str.contains("too long")
        {
            ErrorType::ContextTooLong
        } else if err_str.contains("timeout") || err_str.contains("timed out") {
            ErrorType::Timeout
        } else if err_str.contains("401")
            || err_str.contains("403")
            || err_str.contains("unauthorized")
            || err_str.contains("forbidden")
        {
            ErrorType::AuthError
        } else if err_str.contains("500") || err_str.contains("internal server error") {
            ErrorType::ServerError
        } else if err_str.contains("model") {
            ErrorType::ModelOverloaded
        } else {
            ErrorType::Unknown
        }
    }
}

impl FallbackState {
    pub(super) fn new() -> Self {
        Self {
            consecutive_529_count: 0,
            last_error_type: ErrorType::Unknown,
            fallback_triggered: false,
            fallback_attempts: 0,
        }
    }

    /// 记录错误并更新状态
    pub(super) fn record_error(&mut self, error_type: ErrorType) {
        self.last_error_type = error_type;
        if error_type == ErrorType::ModelOverloaded {
            self.consecutive_529_count += 1;
        } else {
            self.consecutive_529_count = 0;
        }
    }

    /// 检查是否应该触发 fallback（连续 3 次 529 后触发）
    pub(super) fn should_trigger_fallback(&self) -> bool {
        self.consecutive_529_count >= 3
    }

    /// 获取最大 fallback 尝试次数
    fn max_fallback_attempts() -> u32 {
        std::env::var("PRIORITY_AGENT_FALLBACK_MAX_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3)
    }

    /// 检查是否达到最大尝试次数
    pub(super) fn max_attempts_reached(&self) -> bool {
        self.fallback_attempts >= Self::max_fallback_attempts()
    }
}

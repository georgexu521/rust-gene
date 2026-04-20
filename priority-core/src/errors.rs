//! 统一领域错误类型
//!
//! 使用 thiserror 定义领域级错误，替代到处使用的 anyhow::Result。
//! 外部代码仍可用 anyhow::? 自动转换。

use thiserror::Error;

/// Agent 核心错误
#[derive(Debug, Error)]
pub enum AgentError {
    // ── 工具系统 ──
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    // ── API / LLM ──
    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Context overflow")]
    ContextOverflow,

    // ── 引擎 ──
    #[error("Max iterations reached ({0})")]
    MaxIterations(usize),

    #[error("Streaming error: {0}")]
    Streaming(String),

    // ── 会话 / 持久化 ──
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Storage error: {0}")]
    Storage(String),

    // ── 配置 ──
    #[error("Configuration error: {0}")]
    Config(String),

    // ── 通用 ──
    #[error("{0}")]
    Other(String),
}

/// 便捷转换：anyhow::Error → AgentError::Other
impl From<anyhow::Error> for AgentError {
    fn from(err: anyhow::Error) -> Self {
        AgentError::Other(err.to_string())
    }
}

/// 便捷转换：std::io::Error → AgentError::Other
impl From<std::io::Error> for AgentError {
    fn from(err: std::io::Error) -> Self {
        AgentError::Other(err.to_string())
    }
}

/// 统一 Result 类型
pub type Result<T> = std::result::Result<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgentError::ToolNotFound("bash".into());
        assert_eq!(err.to_string(), "Tool not found: bash");

        let err = AgentError::RateLimited {
            retry_after_secs: 30,
        };
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("something broke");
        let agent_err: AgentError = anyhow_err.into();
        assert_eq!(agent_err.to_string(), "something broke");
    }
}

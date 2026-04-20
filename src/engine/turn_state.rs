//! Agent 轮次状态
//!
//! 参考 Claude Code 的 State 对象设计：
//! - 每次循环迭代记录完整状态
//! - 显式的 TransitionReason 记录"为什么继续"
//! - 可审计、可调试、可回放

use std::fmt;

/// 循环转移原因
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionReason {
    /// 正常完成，模型返回了最终文本
    Complete,
    /// 模型调用了工具，执行后继续
    ToolCallExecuted { tool_name: String, tool_id: String },
    /// token 预算不够，压缩后重试
    ContextCompressed { removed_messages: usize },
    /// API 失败，重试
    ApiRetry {
        attempt: u32,
        category: String,
        action: String,
    },
    /// 达到最大迭代次数
    MaxIterationsReached,
    /// 用户中断
    UserInterrupt,
    /// fallback 到另一个模型
    ModelFallback { from: String, to: String },
    /// token 输出限制恢复
    OutputTokenRecovery { attempt: u32 },
}

impl fmt::Display for TransitionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransitionReason::Complete => write!(f, "complete"),
            TransitionReason::ToolCallExecuted { tool_name, .. } => write!(f, "tool:{}", tool_name),
            TransitionReason::ContextCompressed { removed_messages } => {
                write!(f, "compressed(-{} msgs)", removed_messages)
            }
            TransitionReason::ApiRetry {
                attempt, category, ..
            } => {
                write!(f, "retry#{}({})", attempt, category)
            }
            TransitionReason::MaxIterationsReached => write!(f, "max_iterations"),
            TransitionReason::UserInterrupt => write!(f, "interrupted"),
            TransitionReason::ModelFallback { from, to } => write!(f, "fallback({}→{})", from, to),
            TransitionReason::OutputTokenRecovery { attempt } => {
                write!(f, "output_recovery#{}", attempt)
            }
        }
    }
}

/// 轮次状态
#[derive(Debug, Clone)]
pub struct TurnState {
    /// 当前迭代次数
    pub iteration: u32,
    /// 最大迭代次数
    pub max_iterations: u32,
    /// 当前转移原因
    pub transition: TransitionReason,
    /// 历史转移原因（记录完整路径）
    pub transition_history: Vec<TransitionReason>,
    /// 总消耗的 input tokens
    pub total_input_tokens: u64,
    /// 总消耗的 output tokens
    pub total_output_tokens: u64,
    /// 已执行的工具调用数
    pub tool_calls_made: u32,
    /// 压缩次数
    pub compression_count: u32,
    /// 重试次数
    pub retry_count: u32,
    /// 开始时间
    pub started_at: std::time::Instant,
}

impl TurnState {
    pub fn new(max_iterations: u32) -> Self {
        Self {
            iteration: 0,
            max_iterations,
            transition: TransitionReason::Complete,
            transition_history: Vec::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            tool_calls_made: 0,
            compression_count: 0,
            retry_count: 0,
            started_at: std::time::Instant::now(),
        }
    }

    /// 进入下一次迭代
    pub fn advance(&mut self, reason: TransitionReason) {
        self.iteration += 1;
        self.transition_history.push(self.transition.clone());
        self.transition = reason.clone();

        match &reason {
            TransitionReason::ToolCallExecuted { .. } => {
                self.tool_calls_made += 1;
            }
            TransitionReason::ContextCompressed { .. } => {
                self.compression_count += 1;
            }
            TransitionReason::ApiRetry { .. } => {
                self.retry_count += 1;
            }
            _ => {}
        }
    }

    /// 是否还能继续
    pub fn can_continue(&self) -> bool {
        // iteration 为 0 表示还未开始，总是可以继续
        if self.iteration == 0 {
            return true;
        }
        self.iteration < self.max_iterations
            && !matches!(self.transition, TransitionReason::UserInterrupt)
            && !matches!(self.transition, TransitionReason::MaxIterationsReached)
    }

    /// 记录 token 使用
    pub fn record_tokens(&mut self, input: u64, output: u64) {
        self.total_input_tokens += input;
        self.total_output_tokens += output;
    }

    /// 获取已用时间
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// 生成诊断报告
    pub fn diagnostic_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Turn Diagnostic ===\n");
        report.push_str(&format!(
            "Iterations: {}/{}\n",
            self.iteration, self.max_iterations
        ));
        report.push_str(&format!("Duration: {:.1}s\n", self.elapsed().as_secs_f64()));
        report.push_str(&format!(
            "Tokens: {} in / {} out\n",
            self.total_input_tokens, self.total_output_tokens
        ));
        report.push_str(&format!("Tool calls: {}\n", self.tool_calls_made));
        report.push_str(&format!("Compressions: {}\n", self.compression_count));
        report.push_str(&format!("Retries: {}\n", self.retry_count));
        report.push_str(&format!("Final state: {}\n", self.transition));

        if !self.transition_history.is_empty() {
            report.push_str("\nTransition path:\n");
            for (i, reason) in self.transition_history.iter().enumerate() {
                report.push_str(&format!("  {}. {}\n", i + 1, reason));
            }
            report.push_str(&format!(
                "  {}. {}\n",
                self.transition_history.len() + 1,
                self.transition
            ));
        }

        report
    }
}

/// 轮次结果
#[derive(Debug, Clone)]
pub struct TurnResult {
    /// 最终回复内容
    pub content: String,
    /// 轮次状态
    pub state: TurnState,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果有）
    pub error: Option<String>,
}

impl TurnResult {
    pub fn success(content: String, state: TurnState) -> Self {
        Self {
            content,
            state,
            success: true,
            error: None,
        }
    }

    pub fn failure(error: String, state: TurnState) -> Self {
        Self {
            content: String::new(),
            state,
            success: false,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_state_advance() {
        let mut state = TurnState::new(10);
        assert_eq!(state.iteration, 0);
        assert!(state.can_continue());

        state.advance(TransitionReason::ToolCallExecuted {
            tool_name: "bash".to_string(),
            tool_id: "call_1".to_string(),
        });
        assert_eq!(state.iteration, 1);
        assert_eq!(state.tool_calls_made, 1);

        state.advance(TransitionReason::ContextCompressed {
            removed_messages: 5,
        });
        assert_eq!(state.iteration, 2);
        assert_eq!(state.compression_count, 1);
    }

    #[test]
    fn test_turn_state_max_iterations() {
        let mut state = TurnState::new(2);
        state.advance(TransitionReason::ToolCallExecuted {
            tool_name: "test".to_string(),
            tool_id: "1".to_string(),
        });
        assert!(state.can_continue());

        state.advance(TransitionReason::MaxIterationsReached);
        assert!(!state.can_continue());
    }

    #[test]
    fn test_diagnostic_report() {
        let mut state = TurnState::new(10);
        state.advance(TransitionReason::ToolCallExecuted {
            tool_name: "bash".to_string(),
            tool_id: "1".to_string(),
        });
        state.record_tokens(1000, 500);
        state.advance(TransitionReason::Complete);

        let report = state.diagnostic_report();
        assert!(report.contains("Iterations: 2/10"));
        assert!(report.contains("Tool calls: 1"));
        assert!(report.contains("Transition path"));
    }
}

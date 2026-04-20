//! 进度条组件
//!
//! 显示工具执行进度和状态
#![allow(dead_code)]
#![allow(mismatched_lifetime_syntaxes)]

use ratatui::{
    style::{Color, Style},
    widgets::Paragraph,
};

/// 进度状态
#[derive(Debug, Clone, Default)]
pub enum ProgressState {
    /// 空闲
    #[default]
    Idle,
    /// 进行中
    InProgress { message: String, percent: u8 },
    /// 完成
    Complete { message: String },
    /// 错误
    Error { message: String },
}

/// 进度条组件
#[derive(Debug, Default)]
pub struct ProgressBar {
    state: ProgressState,
}

impl ProgressBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_state(&mut self, state: ProgressState) {
        self.state = state;
    }

    pub fn start(&mut self, message: impl Into<String>) {
        self.state = ProgressState::InProgress {
            message: message.into(),
            percent: 0,
        };
    }

    pub fn update(&mut self, percent: u8) {
        if let ProgressState::InProgress {
            percent: ref mut p, ..
        } = self.state
        {
            *p = percent.min(100);
        }
    }

    pub fn complete(&mut self, message: impl Into<String>) {
        self.state = ProgressState::Complete {
            message: message.into(),
        };
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.state = ProgressState::Error {
            message: message.into(),
        };
    }

    /// 渲染为 ratatui 组件
    pub fn render(&self) -> Paragraph {
        let (content, style) = match &self.state {
            ProgressState::Idle => ("Ready".to_string(), Style::default().fg(Color::Gray)),
            ProgressState::InProgress { message, percent } => {
                let bar_len = 20;
                let filled = (*percent as usize * bar_len / 100).min(bar_len);
                let bar = format!(
                    "[{}{}] {}%",
                    "█".repeat(filled),
                    "░".repeat(bar_len - filled),
                    percent
                );
                let text = format!("{} {}", message, bar);
                (text, Style::default().fg(Color::Yellow))
            }
            ProgressState::Complete { message } => {
                (format!("✓ {}", message), Style::default().fg(Color::Green))
            }
            ProgressState::Error { message } => {
                (format!("✗ {}", message), Style::default().fg(Color::Red))
            }
        };

        Paragraph::new(content).style(style)
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.state, ProgressState::Idle)
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.state, ProgressState::Complete { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_states() {
        let mut bar = ProgressBar::new();
        assert!(bar.is_idle());

        bar.start("Loading...");
        assert!(!bar.is_idle());

        bar.update(50);
        bar.complete("Done");
        assert!(bar.is_complete());

        bar.error("Failed");
        assert!(!bar.is_complete());
    }
}

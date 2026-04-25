//! Denial Tracking - 权限拒绝追踪与学习
//!
//! 对标 Claude Code 的 denialTracking.ts + autoModeDenials.ts。
//! 追踪用户/分类器对工具调用的拒绝，当拒绝频率过高时回退到提示模式，
//! 防止分类器被滥用或 Agent 陷入无限重试循环。

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// 拒绝阈值
const MAX_CONSECUTIVE_DENIALS: u32 = 3;
const MAX_TOTAL_DENIALS: u32 = 20;
const MAX_RECENT_DENIALS: usize = 20;

/// 单次拒绝记录
#[derive(Debug, Clone)]
pub struct DenialRecord {
    pub tool_name: String,
    pub display: String,
    pub reason: String,
    pub timestamp: std::time::SystemTime,
}

/// 拒绝追踪器状态
#[derive(Debug, Clone, Default)]
pub struct DenialState {
    pub consecutive_denials: u32,
    pub total_denials: u32,
    pub recent_denials: Vec<DenialRecord>,
}

impl DenialState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录一次拒绝
    pub fn record_denial(&mut self, tool_name: &str, display: &str, reason: &str) {
        self.consecutive_denials += 1;
        self.total_denials += 1;

        self.recent_denials.push(DenialRecord {
            tool_name: tool_name.to_string(),
            display: display.to_string(),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now(),
        });

        // 只保留最近 N 条
        if self.recent_denials.len() > MAX_RECENT_DENIALS {
            self.recent_denials.remove(0);
        }

        if self.consecutive_denials >= MAX_CONSECUTIVE_DENIALS {
            warn!(
                "Denial threshold reached: {} consecutive denials (tool: {})",
                self.consecutive_denials, tool_name
            );
        }
    }

    /// 记录一次成功（重置连续拒绝计数）
    pub fn record_success(&mut self) {
        if self.consecutive_denials > 0 {
            info!(
                "Resetting consecutive denials (was: {})",
                self.consecutive_denials
            );
            self.consecutive_denials = 0;
        }
    }

    /// 是否应回退到提示模式（不再自动允许）
    pub fn should_fallback_to_prompting(&self) -> bool {
        self.consecutive_denials >= MAX_CONSECUTIVE_DENIALS
            || self.total_denials >= MAX_TOTAL_DENIALS
    }

    /// 获取最近拒绝的摘要（用于 UI 展示）
    pub fn recent_summary(&self, limit: usize) -> Vec<String> {
        self.recent_denials
            .iter()
            .rev()
            .take(limit)
            .map(|d| {
                format!(
                    "[{}] {}: {}",
                    d.tool_name,
                    truncate(&d.display, 40),
                    truncate(&d.reason, 60)
                )
            })
            .collect()
    }
}

/// 线程安全的拒绝追踪器
#[derive(Debug, Clone)]
pub struct DenialTracker {
    state: Arc<Mutex<DenialState>>,
}

impl DenialTracker {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DenialState::new())),
        }
    }

    pub async fn record_denial(&self, tool_name: &str, display: &str, reason: &str) {
        let mut state = self.state.lock().await;
        state.record_denial(tool_name, display, reason);
    }

    pub async fn record_success(&self) {
        let mut state = self.state.lock().await;
        state.record_success();
    }

    pub async fn should_fallback(&self) -> bool {
        let state = self.state.lock().await;
        state.should_fallback_to_prompting()
    }

    pub async fn get_state(&self) -> DenialState {
        self.state.lock().await.clone()
    }

    pub async fn recent_summary(&self, limit: usize) -> Vec<String> {
        let state = self.state.lock().await;
        state.recent_summary(limit)
    }

    /// 重置所有计数（例如用户手动重置安全状态时）
    pub async fn reset(&self) {
        let mut state = self.state.lock().await;
        *state = DenialState::new();
        info!("Denial tracker reset");
    }
}

impl Default for DenialTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        let safe: String = s.chars().take(max_len).collect();
        format!("{}...", safe)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_denial_and_success() {
        let mut state = DenialState::new();
        assert_eq!(state.consecutive_denials, 0);
        assert_eq!(state.total_denials, 0);

        state.record_denial("bash", "rm -rf /", "dangerous command");
        assert_eq!(state.consecutive_denials, 1);
        assert_eq!(state.total_denials, 1);

        state.record_denial("bash", "dd if=/dev/zero", "dangerous command");
        assert_eq!(state.consecutive_denials, 2);
        assert_eq!(state.total_denials, 2);

        state.record_success();
        assert_eq!(state.consecutive_denials, 0);
        assert_eq!(state.total_denials, 2); // total 不重置
    }

    #[test]
    fn test_consecutive_fallback() {
        let mut state = DenialState::new();
        for i in 0..MAX_CONSECUTIVE_DENIALS {
            state.record_denial("bash", &format!("cmd{}", i), "test");
        }
        assert!(state.should_fallback_to_prompting());
    }

    #[test]
    fn test_total_fallback() {
        let mut state = DenialState::new();
        // 每 2 次拒绝后 1 次成功，连续永远不会达到阈值
        for _ in 0..MAX_TOTAL_DENIALS {
            state.record_denial("bash", "cmd", "test");
            state.record_success();
        }
        // 再多一次拒绝
        state.record_denial("bash", "cmd", "test");
        assert!(state.should_fallback_to_prompting());
    }

    #[test]
    fn test_recent_limit() {
        let mut state = DenialState::new();
        for i in 0..30 {
            state.record_denial("bash", &format!("cmd{}", i), "test");
        }
        assert_eq!(state.recent_denials.len(), MAX_RECENT_DENIALS);
        // 最新的应该是 cmd29
        assert!(state
            .recent_denials
            .last()
            .unwrap()
            .display
            .contains("cmd29"));
    }

    #[test]
    fn test_recent_summary() {
        let mut state = DenialState::new();
        state.record_denial("bash", "rm -rf /tmp/old", "dangerous");
        state.record_denial("file_write", "/etc/passwd", "sensitive path");

        let summary = state.recent_summary(5);
        assert_eq!(summary.len(), 2);
        assert!(summary[0].contains("file_write"));
        assert!(summary[1].contains("bash"));
    }

    #[tokio::test]
    async fn test_tracker_async() {
        let tracker = DenialTracker::new();
        tracker.record_denial("bash", "rm -rf /", "dangerous").await;
        tracker
            .record_denial("bash", "dd if=/dev/zero", "dangerous")
            .await;

        assert_eq!(tracker.get_state().await.consecutive_denials, 2);

        tracker.record_success().await;
        assert_eq!(tracker.get_state().await.consecutive_denials, 0);

        assert!(!tracker.should_fallback().await);
    }

    #[tokio::test]
    async fn test_tracker_reset() {
        let tracker = DenialTracker::new();
        for _ in 0..5 {
            tracker.record_denial("bash", "cmd", "test").await;
        }
        assert!(tracker.should_fallback().await);

        tracker.reset().await;
        assert!(!tracker.should_fallback().await);
        assert_eq!(tracker.get_state().await.total_denials, 0);
    }
}

//! Security Audit Log - 安全审计日志
//!
//! 对标 Claude Code 的权限决策追踪机制。
//! 追加式日志，记录每个权限决策、工具调用和审批事件，
//! 用于安全审计、故障排查和分类器效果评估。

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// 安全事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityEventType {
    /// 权限决策（允许/拒绝/询问）
    PermissionDecision,
    /// 工具调用（已执行）
    ToolExecution,
    /// 用户审批（用户手动允许/拒绝）
    UserApproval,
    /// 分类器决策（LLM 分类器结果）
    ClassifierDecision,
    /// 拒绝追踪（denial tracking 触发）
    DenialTracking,
    /// 异常/告警
    Alert,
}

impl std::fmt::Display for SecurityEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityEventType::PermissionDecision => write!(f, "PERM"),
            SecurityEventType::ToolExecution => write!(f, "EXEC"),
            SecurityEventType::UserApproval => write!(f, "USER"),
            SecurityEventType::ClassifierDecision => write!(f, "CLSF"),
            SecurityEventType::DenialTracking => write!(f, "DENY"),
            SecurityEventType::Alert => write!(f, "ALRT"),
        }
    }
}

/// 安全事件记录
#[derive(Debug, Clone)]
pub struct SecurityEvent {
    pub event_type: SecurityEventType,
    pub tool_name: Option<String>,
    pub params_summary: String,
    pub decision: String,
    pub reason: String,
    pub risk_level: Option<String>,
    pub timestamp: std::time::SystemTime,
    pub session_id: Option<String>,
}

impl SecurityEvent {
    pub fn new(
        event_type: SecurityEventType,
        tool_name: Option<&str>,
        params_summary: &str,
        decision: &str,
        reason: &str,
    ) -> Self {
        Self {
            event_type,
            tool_name: tool_name.map(String::from),
            params_summary: params_summary.to_string(),
            decision: decision.to_string(),
            reason: reason.to_string(),
            risk_level: None,
            timestamp: std::time::SystemTime::now(),
            session_id: None,
        }
    }

    pub fn with_risk_level(mut self, level: &str) -> Self {
        self.risk_level = Some(level.to_string());
        self
    }

    pub fn with_session(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    /// 格式化为单行日志字符串
    pub fn to_log_line(&self) -> String {
        let ts = format_timestamp(self.timestamp);
        let tool = self.tool_name.as_deref().unwrap_or("-");
        let risk = self.risk_level.as_deref().unwrap_or("-");
        format!(
            "[{}] {} | tool={} | risk={} | decision={} | reason={} | params={}",
            ts,
            self.event_type,
            tool,
            risk,
            self.decision,
            truncate(&self.reason, 80),
            truncate(&self.params_summary, 100)
        )
    }
}

/// 安全审计日志
#[derive(Debug, Clone)]
pub struct SecurityAuditLog {
    events: Arc<Mutex<Vec<SecurityEvent>>>,
    max_events: usize,
}

impl SecurityAuditLog {
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    pub fn with_capacity(max_events: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            max_events,
        }
    }

    /// 记录事件
    pub async fn log(&self, event: SecurityEvent) {
        let line = event.to_log_line();
        info!("SecurityAudit: {}", line);

        let mut events = self.events.lock().await;
        events.push(event);
        if events.len() > self.max_events {
            events.remove(0);
        }
    }

    /// 快捷方法：记录权限决策
    pub async fn log_permission(
        &self,
        tool_name: &str,
        params_summary: &str,
        decision: &str,
        reason: &str,
        risk_level: &str,
    ) {
        self.log(
            SecurityEvent::new(
                SecurityEventType::PermissionDecision,
                Some(tool_name),
                params_summary,
                decision,
                reason,
            )
            .with_risk_level(risk_level),
        )
        .await;
    }

    /// 快捷方法：记录工具执行
    pub async fn log_execution(
        &self,
        tool_name: &str,
        params_summary: &str,
        success: bool,
        reason: &str,
    ) {
        self.log(
            SecurityEvent::new(
                SecurityEventType::ToolExecution,
                Some(tool_name),
                params_summary,
                if success { "SUCCESS" } else { "FAILED" },
                reason,
            ),
        )
        .await;
    }

    /// 快捷方法：记录用户审批
    pub async fn log_user_approval(
        &self,
        tool_name: &str,
        params_summary: &str,
        approved: bool,
        reason: &str,
    ) {
        self.log(
            SecurityEvent::new(
                SecurityEventType::UserApproval,
                Some(tool_name),
                params_summary,
                if approved { "APPROVED" } else { "DENIED" },
                reason,
            ),
        )
        .await;
    }

    /// 快捷方法：记录分类器决策
    pub async fn log_classifier(
        &self,
        tool_name: &str,
        params_summary: &str,
        decision: &str,
        reason: &str,
        classifier_name: &str,
    ) {
        self.log(
            SecurityEvent::new(
                SecurityEventType::ClassifierDecision,
                Some(tool_name),
                params_summary,
                decision,
                &format!("classifier={} | {}", classifier_name, reason),
            ),
        )
        .await;
    }

    /// 获取最近事件
    pub async fn recent_events(&self, limit: usize) -> Vec<SecurityEvent> {
        let events = self.events.lock().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    /// 获取事件总数
    pub async fn event_count(&self) -> usize {
        self.events.lock().await.len()
    }

    /// 按类型筛选事件
    pub async fn events_by_type(&self, event_type: SecurityEventType) -> Vec<SecurityEvent> {
        let events = self.events.lock().await;
        events
            .iter()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect()
    }

    /// 清空日志
    pub async fn clear(&self) {
        self.events.lock().await.clear();
    }

    /// 生成摘要报告
    pub async fn summary_report(&self) -> String {
        let events = self.events.lock().await;
        let total = events.len();
        let perm_count = events.iter().filter(|e| e.event_type == SecurityEventType::PermissionDecision).count();
        let exec_count = events.iter().filter(|e| e.event_type == SecurityEventType::ToolExecution).count();
        let user_count = events.iter().filter(|e| e.event_type == SecurityEventType::UserApproval).count();
        let clsf_count = events.iter().filter(|e| e.event_type == SecurityEventType::ClassifierDecision).count();
        let alert_count = events.iter().filter(|e| e.event_type == SecurityEventType::Alert).count();

        format!(
            "Security Audit Summary:\n\
             - Total events: {}\n\
             - Permission decisions: {}\n\
             - Tool executions: {}\n\
             - User approvals: {}\n\
             - Classifier decisions: {}\n\
             - Alerts: {}",
            total, perm_count, exec_count, user_count, clsf_count, alert_count
        )
    }
}

impl Default for SecurityAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

fn format_timestamp(ts: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = ts.into();
    datetime.format("%Y-%m-%dT%H:%M:%S%.3f%z").to_string()
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

    #[tokio::test]
    async fn test_log_and_retrieve() {
        let log = SecurityAuditLog::new();
        log.log_permission("bash", "rm -rf /tmp/test", "ASK", "dangerous pattern", "High")
            .await;
        log.log_execution("bash", "rm -rf /tmp/test", true, "executed after approval")
            .await;

        assert_eq!(log.event_count().await, 2);

        let recent = log.recent_events(10).await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].event_type, SecurityEventType::ToolExecution);
        assert_eq!(recent[1].event_type, SecurityEventType::PermissionDecision);
    }

    #[tokio::test]
    async fn test_capacity_limit() {
        let log = SecurityAuditLog::with_capacity(5);
        for i in 0..10 {
            log.log_permission("bash", &format!("cmd{}", i), "ASK", "test", "Low")
                .await;
        }
        assert_eq!(log.event_count().await, 5);
        let recent = log.recent_events(5).await;
        assert!(recent[0].params_summary.contains("cmd9"));
    }

    #[tokio::test]
    async fn test_filter_by_type() {
        let log = SecurityAuditLog::new();
        log.log_permission("bash", "cmd", "ASK", "test", "Low").await;
        log.log_execution("bash", "cmd", true, "ok").await;
        log.log_user_approval("bash", "cmd", true, "user said yes").await;

        let perms = log.events_by_type(SecurityEventType::PermissionDecision).await;
        assert_eq!(perms.len(), 1);

        let execs = log.events_by_type(SecurityEventType::ToolExecution).await;
        assert_eq!(execs.len(), 1);
    }

    #[tokio::test]
    async fn test_summary_report() {
        let log = SecurityAuditLog::new();
        log.log_permission("bash", "cmd", "ASK", "test", "Low").await;
        log.log_execution("bash", "cmd", true, "ok").await;
        log.log_classifier("bash", "cmd", "ALLOW", "low risk", "llm").await;

        let report = log.summary_report().await;
        assert!(report.contains("Total events: 3"));
        assert!(report.contains("Permission decisions: 1"));
        assert!(report.contains("Classifier decisions: 1"));
    }

    #[test]
    fn test_event_to_log_line() {
        let event = SecurityEvent::new(
            SecurityEventType::PermissionDecision,
            Some("bash"),
            "rm -rf /tmp/test",
            "ASK",
            "matches dangerous pattern: rm -rf",
        )
        .with_risk_level("High");

        let line = event.to_log_line();
        assert!(line.contains("PERM"));
        assert!(line.contains("bash"));
        assert!(line.contains("ASK"));
        assert!(line.contains("High"));
    }
}

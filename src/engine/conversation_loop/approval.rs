//! 工具授权通道

use crate::engine::human_review::PermissionReviewDecision;
use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const DEFAULT_APPROVAL_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub tool_call: ToolCall,
    pub prompt: String,
    pub review: Option<crate::engine::human_review::HumanReviewRequest>,
    pub audit: Option<crate::engine::human_review::HumanReviewAuditRecord>,
    /// Optional diff preview for file_write/file_edit permissions (first ~10 lines).
    pub diff_preview: Option<String>,
}

impl ToolApprovalRequest {
    pub fn human_review_request(&self) -> crate::engine::human_review::HumanReviewRequest {
        if let Some(review) = &self.review {
            return review.clone();
        }
        crate::engine::human_review::HumanReviewRequest::tool_permission(
            &self.tool_call,
            &self.prompt,
        )
    }

    pub fn permission_review(&self) -> crate::engine::human_review::PermissionReview {
        crate::engine::human_review::PermissionReview::from_tool_call(&self.tool_call, &self.prompt)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolApprovalResponse {
    pub approved: bool,
    #[serde(default)]
    pub decision: Option<PermissionReviewDecision>,
    #[serde(default)]
    pub rule_decision: Option<String>,
    #[serde(default)]
    pub persistence_scope: Option<String>,
    #[serde(default)]
    pub rule_pattern: Option<String>,
    #[serde(default)]
    pub persisted_path: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

impl ToolApprovalResponse {
    pub fn approved_once() -> Self {
        Self {
            approved: true,
            decision: Some(PermissionReviewDecision::ApproveOnce),
            rule_decision: None,
            persistence_scope: None,
            rule_pattern: None,
            persisted_path: None,
            note: None,
        }
    }

    pub fn approved_session() -> Self {
        Self {
            approved: true,
            decision: Some(PermissionReviewDecision::ApproveSession),
            rule_decision: None,
            persistence_scope: Some("session".to_string()),
            rule_pattern: None,
            persisted_path: None,
            note: None,
        }
    }

    pub fn rejected_once() -> Self {
        Self {
            approved: false,
            decision: Some(PermissionReviewDecision::RejectOnce),
            rule_decision: None,
            persistence_scope: None,
            rule_pattern: None,
            persisted_path: None,
            note: None,
        }
    }

    pub fn with_rule(
        decision: PermissionReviewDecision,
        rule_pattern: impl Into<String>,
        persisted_path: Option<String>,
        note: Option<String>,
    ) -> Self {
        Self {
            approved: decision.approved(),
            rule_decision: decision.rule_decision().map(str::to_string),
            persistence_scope: decision.persistence_scope().map(str::to_string),
            decision: Some(decision),
            rule_pattern: Some(rule_pattern.into()),
            persisted_path,
            note,
        }
    }

    pub fn decision_label(&self) -> Option<&'static str> {
        self.decision.map(PermissionReviewDecision::as_str)
    }
}

/// 待审批的工具请求 + 响应通道
type PendingApproval = Option<(
    ToolApprovalRequest,
    tokio::sync::oneshot::Sender<ToolApprovalResponse>,
)>;

/// 工具授权通道（类似 PlanApprovalChannel）
pub struct ToolApprovalChannel {
    pending: Arc<Mutex<PendingApproval>>,
    timeout: Duration,
}

impl ToolApprovalChannel {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
            timeout: approval_timeout(),
        }
    }

    /// 提交授权请求并等待响应。
    pub async fn submit(
        &self,
        request: ToolApprovalRequest,
    ) -> anyhow::Result<ToolApprovalResponse> {
        let request_id = request.tool_call.id.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            *pending = Some((request, tx));
        }
        match tokio::time::timeout(self.timeout, rx).await {
            Ok(result) => result.map_err(|_| anyhow::anyhow!("Approval channel closed")),
            Err(_) => {
                let mut pending = self.pending.lock().await;
                if pending
                    .as_ref()
                    .map(|(pending_request, _)| pending_request.tool_call.id == request_id)
                    .unwrap_or(false)
                {
                    pending.take();
                }
                Err(anyhow::anyhow!(
                    "Tool approval timed out after {} seconds",
                    self.timeout.as_secs()
                ))
            }
        }
    }

    /// TUI 取出待审批的请求
    pub async fn take_pending(
        &self,
    ) -> Option<(
        ToolApprovalRequest,
        tokio::sync::oneshot::Sender<ToolApprovalResponse>,
    )> {
        let mut pending = self.pending.lock().await;
        pending.take()
    }

    /// 是否有待审批的请求
    pub async fn has_pending(&self) -> bool {
        self.pending.lock().await.is_some()
    }
}

impl Default for ToolApprovalChannel {
    fn default() -> Self {
        Self::new()
    }
}

fn approval_timeout() -> Duration {
    let secs = std::env::var("PRIORITY_AGENT_APPROVAL_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.clamp(30, 1800))
        .unwrap_or(DEFAULT_APPROVAL_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

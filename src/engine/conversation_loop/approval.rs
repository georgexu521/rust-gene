//! 工具授权通道

use crate::services::api::ToolCall;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub tool_call: ToolCall,
    pub prompt: String,
    pub review: Option<crate::engine::human_review::HumanReviewRequest>,
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

/// 待审批的工具请求 + 响应通道
type PendingApproval = Option<(ToolApprovalRequest, tokio::sync::oneshot::Sender<bool>)>;

/// 工具授权通道（类似 PlanApprovalChannel）
pub struct ToolApprovalChannel {
    pending: Arc<Mutex<PendingApproval>>,
}

impl ToolApprovalChannel {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// 提交授权请求并等待响应（60 秒超时）
    pub async fn submit(&self, request: ToolApprovalRequest) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            *pending = Some((request, tx));
        }
        match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
            Ok(result) => result.map_err(|_| anyhow::anyhow!("Approval channel closed")),
            Err(_) => Err(anyhow::anyhow!("Tool approval timed out after 60 seconds")),
        }
    }

    /// TUI 取出待审批的请求
    pub async fn take_pending(
        &self,
    ) -> Option<(ToolApprovalRequest, tokio::sync::oneshot::Sender<bool>)> {
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

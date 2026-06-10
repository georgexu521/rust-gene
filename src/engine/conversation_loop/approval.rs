//! 工具授权通道

use crate::engine::human_review::PermissionReviewDecision;
use crate::services::api::ToolCall;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

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

pub fn diff_preview_for_tool_call(tool_call: &ToolCall) -> Option<String> {
    let args = &tool_call.arguments;
    let mut lines = match tool_call.name.as_str() {
        "file_write" => {
            let path = args.get("path").and_then(|value| value.as_str())?;
            let content = args
                .get("content")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let mut lines = vec![
                "--- /dev/null".to_string(),
                format!("+++ b/{path}"),
                format!("@@ -0,0 +1,{} @@", content.lines().count()),
            ];
            lines.extend(content.lines().map(|line| format!("+{line}")));
            lines
        }
        "file_edit" => {
            let path = args.get("path").and_then(|value| value.as_str())?;
            let old_string = args
                .get("old_string")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let new_string = args
                .get("new_string")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let mut lines = vec![format!("File: {path}")];
            if let Some(after) = args.get("insert_after").and_then(|value| value.as_str()) {
                lines.push(format!(" insert after: {}", truncate_chars(after, 80)));
                lines.extend(new_string.lines().map(|line| format!("+{line}")));
            } else if let Some(before) = args.get("insert_before").and_then(|value| value.as_str())
            {
                lines.push(format!(" insert before: {}", truncate_chars(before, 80)));
                lines.extend(new_string.lines().map(|line| format!("+{line}")));
            } else {
                lines.extend(old_string.lines().map(|line| format!("-{line}")));
                lines.extend(new_string.lines().map(|line| format!("+{line}")));
            }
            lines
        }
        "file_patch" => {
            let operations = args
                .get("operations")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            let mut lines = vec![format!("Patch operations: {}", operations.len())];
            for (index, operation) in operations.iter().enumerate() {
                let path = operation
                    .get("path")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                lines.push(format!("{}. {path}", index + 1));
                if let Some(replacements) = operation.get("replacements").and_then(|v| v.as_array())
                {
                    for replacement in replacements.iter().take(2) {
                        if let Some(old) = replacement.get("old_string").and_then(|v| v.as_str()) {
                            lines.push(format!("-{}", truncate_chars(old, 80)));
                        }
                        if let Some(new) = replacement.get("new_string").and_then(|v| v.as_str()) {
                            lines.push(format!("+{}", truncate_chars(new, 80)));
                        }
                    }
                }
            }
            lines
        }
        "bash" => {
            let command = args.get("command").and_then(|value| value.as_str())?;
            let working_dir = args
                .get("working_dir")
                .and_then(|value| value.as_str())
                .unwrap_or("current directory");
            vec![
                format!("Command: {}", truncate_chars(command, 120)),
                format!("Working directory: {working_dir}"),
            ]
        }
        _ => return None,
    };

    const MAX_LINES: usize = 10;
    let truncated = lines.len() > MAX_LINES;
    lines.truncate(MAX_LINES);
    if truncated {
        lines.push("...".to_string());
    }
    Some(lines.join("\n"))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push_str("...");
    }
    out
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
    crate::services::config::runtime_config().approval_timeout()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(name: &str, arguments: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    #[test]
    fn diff_preview_for_file_write_includes_unified_preview() {
        let preview = diff_preview_for_tool_call(&call(
            "file_write",
            json!({
                "path": "src/main.rs",
                "content": "fn main() {}\n"
            }),
        ))
        .unwrap();

        assert!(preview.contains("--- /dev/null"));
        assert!(preview.contains("+++ b/src/main.rs"));
        assert!(preview.contains("+fn main() {}"));
    }

    #[test]
    fn diff_preview_for_file_edit_is_unicode_safe() {
        let preview = diff_preview_for_tool_call(&call(
            "file_edit",
            json!({
                "path": "src/main.rs",
                "old_string": "旧旧旧旧旧旧旧旧旧旧旧旧旧旧旧旧旧旧旧旧",
                "new_string": "新新新新新新新新新新新新新新新新新新新新"
            }),
        ))
        .unwrap();

        assert!(preview.contains("File: src/main.rs"));
        assert!(preview.contains("-旧"));
        assert!(preview.contains("+新"));
    }
}

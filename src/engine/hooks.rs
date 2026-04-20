//! Tool hook pipeline (pre/post tool execution)
//!
//! Minimal implementation inspired by Claude Code's hook model.
//! Hooks are optional and configured by environment variables:
//! - PRIORITY_AGENT_PRE_TOOL_HOOK
//! - PRIORITY_AGENT_POST_TOOL_HOOK
//! - PRIORITY_AGENT_HOOK_TIMEOUT_MS (optional, default 5000)
//! - PRIORITY_AGENT_HOOK_FAIL_CLOSED (optional, default false)

use crate::services::api::ToolCall;
use crate::tools::{ToolContext, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

const DEFAULT_HOOK_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone)]
struct CommandHook {
    name: String,
    command: String,
    timeout_ms: u64,
    block_on_error: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ToolHookManager {
    pre_tool_hooks: Vec<CommandHook>,
    post_tool_hooks: Vec<CommandHook>,
}

#[derive(Debug, Clone)]
pub struct HookDecision {
    pub allow: bool,
    pub reason: Option<String>,
}

impl HookDecision {
    fn allow() -> Self {
        Self {
            allow: true,
            reason: None,
        }
    }

    fn deny(reason: impl Into<String>) -> Self {
        Self {
            allow: false,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Serialize)]
struct HookPayload<'a> {
    event: &'a str,
    session_id: &'a str,
    working_dir: String,
    tool_call_id: &'a str,
    tool_name: &'a str,
    arguments: &'a Value,
    success: Option<bool>,
    result_content: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct HookResponse {
    allow: Option<bool>,
    reason: Option<String>,
}

impl ToolHookManager {
    pub fn from_env() -> Option<Self> {
        let pre = std::env::var("PRIORITY_AGENT_PRE_TOOL_HOOK").ok();
        let post = std::env::var("PRIORITY_AGENT_POST_TOOL_HOOK").ok();

        if pre.as_ref().is_none_or(|v| v.trim().is_empty())
            && post.as_ref().is_none_or(|v| v.trim().is_empty())
        {
            return None;
        }

        let timeout_ms = std::env::var("PRIORITY_AGENT_HOOK_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_HOOK_TIMEOUT_MS);

        let fail_closed = std::env::var("PRIORITY_AGENT_HOOK_FAIL_CLOSED")
            .ok()
            .map(|s| {
                let normalized = s.trim().to_ascii_lowercase();
                normalized == "1" || normalized == "true" || normalized == "yes"
            })
            .unwrap_or(false);

        let mut mgr = Self::default();

        if let Some(cmd) = pre {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                mgr.pre_tool_hooks.push(CommandHook {
                    name: "env_pre_tool_hook".to_string(),
                    command: cmd.to_string(),
                    timeout_ms,
                    block_on_error: fail_closed,
                });
            }
        }

        if let Some(cmd) = post {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                mgr.post_tool_hooks.push(CommandHook {
                    name: "env_post_tool_hook".to_string(),
                    command: cmd.to_string(),
                    timeout_ms,
                    block_on_error: fail_closed,
                });
            }
        }

        Some(mgr)
    }

    pub async fn run_pre_tool(&self, tool_call: &ToolCall, context: &ToolContext) -> HookDecision {
        if self.pre_tool_hooks.is_empty() {
            return HookDecision::allow();
        }

        let payload = HookPayload {
            event: "PreToolUse",
            session_id: &context.session_id,
            working_dir: context.working_dir.to_string_lossy().to_string(),
            tool_call_id: &tool_call.id,
            tool_name: &tool_call.name,
            arguments: &tool_call.arguments,
            success: None,
            result_content: None,
        };

        for hook in &self.pre_tool_hooks {
            debug!(
                "Running pre-tool hook '{}' for tool '{}'",
                hook.name, tool_call.name
            );
            match self.execute_hook(hook, &payload).await {
                Ok(Some(decision)) if !decision.allow => return decision,
                Ok(_) => {}
                Err(err) => {
                    warn!("Pre-tool hook '{}' failed: {}", hook.name, err);
                    if hook.block_on_error {
                        return HookDecision::deny(format!(
                            "blocked by failing pre-tool hook '{}': {}",
                            hook.name, err
                        ));
                    }
                }
            }
        }

        HookDecision::allow()
    }

    pub async fn run_post_tool(
        &self,
        tool_call: &ToolCall,
        result: &ToolResult,
        context: &ToolContext,
    ) {
        if self.post_tool_hooks.is_empty() {
            return;
        }

        let payload = HookPayload {
            event: "PostToolUse",
            session_id: &context.session_id,
            working_dir: context.working_dir.to_string_lossy().to_string(),
            tool_call_id: &tool_call.id,
            tool_name: &tool_call.name,
            arguments: &tool_call.arguments,
            success: Some(result.success),
            result_content: Some(&result.content),
        };

        for hook in &self.post_tool_hooks {
            debug!(
                "Running post-tool hook '{}' for tool '{}'",
                hook.name, tool_call.name
            );
            if let Err(err) = self.execute_hook(hook, &payload).await {
                warn!("Post-tool hook '{}' failed: {}", hook.name, err);
            }
        }
    }

    async fn execute_hook(
        &self,
        hook: &CommandHook,
        payload: &HookPayload<'_>,
    ) -> Result<Option<HookDecision>, String> {
        let payload_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&hook.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn failed: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(payload_json.as_bytes())
                .await
                .map_err(|e| format!("stdin write failed: {}", e))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| format!("stdin newline write failed: {}", e))?;
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "missing stdout pipe".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "missing stderr pipe".to_string())?;

        let stdout_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut reader = stdout;
            let _ = reader.read_to_end(&mut buf).await;
            buf
        });
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut reader = stderr;
            let _ = reader.read_to_end(&mut buf).await;
            buf
        });

        let status = match timeout(Duration::from_millis(hook.timeout_ms), child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => return Err(format!("wait failed: {}", e)),
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(format!("timed out after {}ms", hook.timeout_ms));
            }
        };

        let stdout_bytes = stdout_task.await.unwrap_or_default();
        let stderr_bytes = stderr_task.await.unwrap_or_default();

        if !status.success() {
            let stderr_text = String::from_utf8_lossy(&stderr_bytes);
            return Err(format!(
                "exit status {} stderr: {}",
                status,
                stderr_text.trim()
            ));
        }

        let stdout_text = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
        if stdout_text.is_empty() {
            return Ok(None);
        }

        match serde_json::from_str::<HookResponse>(&stdout_text) {
            Ok(resp) => {
                if matches!(resp.allow, Some(false)) {
                    return Ok(Some(HookDecision::deny(
                        resp.reason.unwrap_or_else(|| "blocked by hook".to_string()),
                    )));
                }
                Ok(None)
            }
            Err(_) => {
                debug!(
                    "Hook '{}' returned non-JSON output, treating as informational output",
                    hook.name
                );
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager_with_pre(
        command: &str,
        timeout_ms: u64,
        block_on_error: bool,
    ) -> ToolHookManager {
        ToolHookManager {
            pre_tool_hooks: vec![CommandHook {
                name: "test_pre".to_string(),
                command: command.to_string(),
                timeout_ms,
                block_on_error,
            }],
            post_tool_hooks: Vec::new(),
        }
    }

    #[test]
    fn test_from_env_none_when_empty() {
        unsafe {
            std::env::remove_var("PRIORITY_AGENT_PRE_TOOL_HOOK");
            std::env::remove_var("PRIORITY_AGENT_POST_TOOL_HOOK");
        }
        assert!(ToolHookManager::from_env().is_none());
    }

    #[tokio::test]
    async fn test_pre_tool_hook_can_deny_execution() {
        let manager = test_manager_with_pre(
            "echo '{\"allow\": false, \"reason\": \"denied\"}'",
            2_000,
            false,
        );
        let tool_call = ToolCall {
            id: "1".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({"path":"a.txt","content":"x"}),
        };
        let context = ToolContext::new(".", "session-test");

        let decision = manager.run_pre_tool(&tool_call, &context).await;
        assert!(!decision.allow);
        assert_eq!(decision.reason.as_deref(), Some("denied"));
    }

    #[tokio::test]
    async fn test_pre_tool_hook_timeout_fail_open() {
        let manager = test_manager_with_pre("sleep 1", 10, false);
        let tool_call = ToolCall {
            id: "2".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({"path":"a.txt","content":"x"}),
        };
        let context = ToolContext::new(".", "session-test");

        let decision = manager.run_pre_tool(&tool_call, &context).await;
        assert!(decision.allow);
        assert!(decision.reason.is_none());
    }

    #[tokio::test]
    async fn test_pre_tool_hook_timeout_fail_closed() {
        let manager = test_manager_with_pre("sleep 1", 10, true);
        let tool_call = ToolCall {
            id: "3".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({"path":"a.txt","content":"x"}),
        };
        let context = ToolContext::new(".", "session-test");

        let decision = manager.run_pre_tool(&tool_call, &context).await;
        assert!(!decision.allow);
        assert!(decision
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("failing pre-tool hook"));
    }
}

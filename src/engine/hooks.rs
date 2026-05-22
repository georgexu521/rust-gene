//! Tool hook pipeline (pre/post tool execution)
//!
//! Minimal implementation inspired by Claude Code's hook model.
//! Hooks are optional and configured by environment variables:
//! - `PRIORITY_AGENT_PRE_TOOL_HOOK` (全局 pre-tool hook)
//! - `PRIORITY_AGENT_POST_TOOL_HOOK` (全局 post-tool hook)
//! - `PRIORITY_AGENT_PERMISSION_REQUEST_HOOK` (permission request hook)
//! - `PRIORITY_AGENT_PERMISSION_RESOLVED_HOOK` (permission resolution hook)
//! - `PRIORITY_AGENT_HOOK_TIMEOUT_MS` (optional, default 5000)
//! - `PRIORITY_AGENT_HOOK_FAIL_CLOSED` (optional, default false)
//!
//! 细粒度工具钩子（按工具名称）：
//! - `PRIORITY_AGENT_TOOL_HOOK_BEFORE_<NAME>` (特定工具的 pre hook)
//! - `PRIORITY_AGENT_TOOL_HOOK_AFTER_<NAME>` (特定工具的 post hook)

use crate::services::api::ToolCall;
use crate::tools::{ToolContext, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

const DEFAULT_HOOK_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_MAX_HOOK_RECORDS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEventKind {
    PromptSubmit,
    PreToolUse,
    PostToolUse,
    PermissionRequest,
    PermissionResolved,
    ValidationStart,
    ValidationEnd,
    SubagentStart,
    SubagentEnd,
    FileChange,
    Compact,
    SessionEnd,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookProviderKind {
    #[default]
    Env,
    Config,
    Plugin,
}

impl HookProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HookProviderKind::Env => "env",
            HookProviderKind::Config => "config",
            HookProviderKind::Plugin => "plugin",
        }
    }
}

impl std::fmt::Display for HookEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{:?}", self));
        write!(f, "{}", value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRunRecord {
    pub sequence: u64,
    pub event: HookEventKind,
    #[serde(default)]
    pub provider: HookProviderKind,
    pub hook_name: String,
    pub tool_call_id: String,
    pub tool_name: Option<String>,
    pub success: bool,
    pub blocked: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub output_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookRegistration {
    pub event: HookEventKind,
    pub provider: HookProviderKind,
    pub hook_name: String,
    pub scope: String,
    pub timeout_ms: u64,
    pub block_on_error: bool,
    pub command_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookLifecycleSnapshot {
    pub configured: bool,
    pub default_timeout_ms: u64,
    pub fail_closed: bool,
    pub registrations: Vec<HookRegistration>,
    pub recent_records: Vec<HookRunRecord>,
    pub recent_success_count: usize,
    pub recent_failure_count: usize,
    pub recent_blocked_count: usize,
    pub recent_avg_duration_ms: Option<u64>,
}

impl HookLifecycleSnapshot {
    fn from_parts(
        default_timeout_ms: u64,
        fail_closed: bool,
        mut registrations: Vec<HookRegistration>,
        recent_records: Vec<HookRunRecord>,
    ) -> Self {
        registrations.sort_by(|a, b| {
            (
                a.event.to_string(),
                a.scope.as_str(),
                a.hook_name.as_str(),
                a.provider.as_str(),
            )
                .cmp(&(
                    b.event.to_string(),
                    b.scope.as_str(),
                    b.hook_name.as_str(),
                    b.provider.as_str(),
                ))
        });
        let recent_success_count = recent_records
            .iter()
            .filter(|record| record.success)
            .count();
        let recent_failure_count = recent_records
            .iter()
            .filter(|record| !record.success)
            .count();
        let recent_blocked_count = recent_records
            .iter()
            .filter(|record| record.blocked)
            .count();
        let recent_avg_duration_ms = if recent_records.is_empty() {
            None
        } else {
            Some(
                recent_records
                    .iter()
                    .map(|record| record.duration_ms)
                    .sum::<u64>()
                    / recent_records.len() as u64,
            )
        };

        Self {
            configured: !registrations.is_empty(),
            default_timeout_ms,
            fail_closed,
            registrations,
            recent_records,
            recent_success_count,
            recent_failure_count,
            recent_blocked_count,
            recent_avg_duration_ms,
        }
    }
}

#[derive(Debug, Clone)]
struct CommandHook {
    name: String,
    provider: HookProviderKind,
    command: String,
    timeout_ms: u64,
    block_on_error: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ToolHookManager {
    /// 全局 pre-tool hooks
    pre_tool_hooks: Vec<CommandHook>,
    /// 全局 post-tool hooks
    post_tool_hooks: Vec<CommandHook>,
    /// 特定工具的 pre hooks (tool_name -> hooks)
    tool_specific_pre_hooks: HashMap<String, Vec<CommandHook>>,
    /// 特定工具的 post hooks (tool_name -> hooks)
    tool_specific_post_hooks: HashMap<String, Vec<CommandHook>>,
    /// Permission request lifecycle hooks.
    permission_request_hooks: Vec<CommandHook>,
    /// Permission resolution lifecycle hooks.
    permission_resolved_hooks: Vec<CommandHook>,
    /// 最近 hook 执行记录
    recent_records: Arc<Mutex<VecDeque<HookRunRecord>>>,
    /// 单调递增的 hook 执行序号，用于 trace 增量消费
    next_record_sequence: Arc<AtomicU64>,
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
    event: HookEventKind,
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
    pub fn lifecycle_snapshot_from_env() -> HookLifecycleSnapshot {
        let timeout_ms = hook_timeout_ms_from_env();
        let fail_closed = hook_fail_closed_from_env();
        match Self::from_env() {
            Some(manager) => manager.lifecycle_snapshot_with_defaults(timeout_ms, fail_closed),
            None => {
                HookLifecycleSnapshot::from_parts(timeout_ms, fail_closed, Vec::new(), Vec::new())
            }
        }
    }

    pub fn lifecycle_snapshot(&self) -> HookLifecycleSnapshot {
        self.lifecycle_snapshot_with_defaults(
            hook_timeout_ms_from_env(),
            hook_fail_closed_from_env(),
        )
    }

    fn lifecycle_snapshot_with_defaults(
        &self,
        default_timeout_ms: u64,
        fail_closed: bool,
    ) -> HookLifecycleSnapshot {
        HookLifecycleSnapshot::from_parts(
            default_timeout_ms,
            fail_closed,
            self.registrations(),
            self.recent_records(),
        )
    }

    fn registrations(&self) -> Vec<HookRegistration> {
        let mut registrations = Vec::new();
        for hook in &self.pre_tool_hooks {
            registrations.push(hook_registration(
                HookEventKind::PreToolUse,
                "global".to_string(),
                hook,
            ));
        }
        for hook in &self.post_tool_hooks {
            registrations.push(hook_registration(
                HookEventKind::PostToolUse,
                "global".to_string(),
                hook,
            ));
        }
        for (tool, hooks) in &self.tool_specific_pre_hooks {
            for hook in hooks {
                registrations.push(hook_registration(
                    HookEventKind::PreToolUse,
                    format!("tool:{}", tool),
                    hook,
                ));
            }
        }
        for (tool, hooks) in &self.tool_specific_post_hooks {
            for hook in hooks {
                registrations.push(hook_registration(
                    HookEventKind::PostToolUse,
                    format!("tool:{}", tool),
                    hook,
                ));
            }
        }
        for hook in &self.permission_request_hooks {
            registrations.push(hook_registration(
                HookEventKind::PermissionRequest,
                "global".to_string(),
                hook,
            ));
        }
        for hook in &self.permission_resolved_hooks {
            registrations.push(hook_registration(
                HookEventKind::PermissionResolved,
                "global".to_string(),
                hook,
            ));
        }
        registrations
    }

    pub fn recent_records(&self) -> Vec<HookRunRecord> {
        self.recent_records
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    pub fn current_record_sequence(&self) -> u64 {
        self.next_record_sequence.load(Ordering::SeqCst)
    }

    pub fn recent_records_after(&self, sequence: u64) -> Vec<HookRunRecord> {
        self.recent_records
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
            .filter(|record| record.sequence > sequence)
            .cloned()
            .collect()
    }

    pub fn recent_records_after_for(
        &self,
        sequence: u64,
        tool_call_id: &str,
    ) -> Vec<HookRunRecord> {
        self.recent_records
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
            .filter(|record| {
                record.sequence > sequence && record.tool_call_id.as_str() == tool_call_id
            })
            .cloned()
            .collect()
    }

    fn push_record(&self, mut record: HookRunRecord) {
        record.sequence = self.next_record_sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let mut records = self
            .recent_records
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        records.push_back(record);
        while records.len() > DEFAULT_MAX_HOOK_RECORDS {
            records.pop_front();
        }
    }

    pub fn from_env() -> Option<Self> {
        let pre = std::env::var("PRIORITY_AGENT_PRE_TOOL_HOOK").ok();
        let post = std::env::var("PRIORITY_AGENT_POST_TOOL_HOOK").ok();
        let permission_request = std::env::var("PRIORITY_AGENT_PERMISSION_REQUEST_HOOK").ok();
        let permission_resolved = std::env::var("PRIORITY_AGENT_PERMISSION_RESOLVED_HOOK").ok();

        // 检查是否有任何钩子配置
        let has_any_hook = pre.as_ref().is_some_and(|v| !v.trim().is_empty())
            || post.as_ref().is_some_and(|v| !v.trim().is_empty())
            || permission_request
                .as_ref()
                .is_some_and(|v| !v.trim().is_empty())
            || permission_resolved
                .as_ref()
                .is_some_and(|v| !v.trim().is_empty());

        // 检查细粒度钩子
        let mut tool_specific_hooks = false;
        for (key, _) in std::env::vars() {
            if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_BEFORE_")
                || key.starts_with("PRIORITY_AGENT_TOOL_HOOK_AFTER_")
            {
                tool_specific_hooks = true;
                break;
            }
        }

        if !has_any_hook && !tool_specific_hooks {
            return None;
        }

        let timeout_ms = hook_timeout_ms_from_env();
        let fail_closed = hook_fail_closed_from_env();

        let mut mgr = Self::default();

        if let Some(cmd) = pre {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                mgr.pre_tool_hooks.push(CommandHook {
                    name: "env_pre_tool_hook".to_string(),
                    provider: HookProviderKind::Env,
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
                    provider: HookProviderKind::Env,
                    command: cmd.to_string(),
                    timeout_ms,
                    block_on_error: fail_closed,
                });
            }
        }

        if let Some(cmd) = permission_request {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                mgr.permission_request_hooks.push(CommandHook {
                    name: "env_permission_request_hook".to_string(),
                    provider: HookProviderKind::Env,
                    command: cmd.to_string(),
                    timeout_ms,
                    block_on_error: fail_closed,
                });
            }
        }

        if let Some(cmd) = permission_resolved {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                mgr.permission_resolved_hooks.push(CommandHook {
                    name: "env_permission_resolved_hook".to_string(),
                    provider: HookProviderKind::Env,
                    command: cmd.to_string(),
                    timeout_ms,
                    block_on_error: fail_closed,
                });
            }
        }

        // 解析细粒度工具钩子: PRIORITY_AGENT_TOOL_HOOK_BEFORE_<NAME>
        for (key, value) in std::env::vars() {
            if let Some(raw_tool_name) = key.strip_prefix("PRIORITY_AGENT_TOOL_HOOK_BEFORE_") {
                let tool_name = raw_tool_name.to_lowercase();
                if !value.trim().is_empty() {
                    debug!("Registering tool-specific pre hook for '{}'", tool_name);
                    let entry = mgr.tool_specific_pre_hooks.entry(tool_name.clone());
                    entry.or_default().push(CommandHook {
                        name: format!("env_pre_tool_hook_{}", tool_name),
                        provider: HookProviderKind::Env,
                        command: value.trim().to_string(),
                        timeout_ms,
                        block_on_error: fail_closed,
                    });
                }
            }
        }

        // 解析细粒度工具钩子: PRIORITY_AGENT_TOOL_HOOK_AFTER_<NAME>
        for (key, value) in std::env::vars() {
            if let Some(raw_tool_name) = key.strip_prefix("PRIORITY_AGENT_TOOL_HOOK_AFTER_") {
                let tool_name = raw_tool_name.to_lowercase();
                if !value.trim().is_empty() {
                    debug!("Registering tool-specific post hook for '{}'", tool_name);
                    let entry = mgr.tool_specific_post_hooks.entry(tool_name.clone());
                    entry.or_default().push(CommandHook {
                        name: format!("env_post_tool_hook_{}", tool_name),
                        provider: HookProviderKind::Env,
                        command: value.trim().to_string(),
                        timeout_ms,
                        block_on_error: fail_closed,
                    });
                }
            }
        }

        Some(mgr)
    }

    pub async fn run_pre_tool(&self, tool_call: &ToolCall, context: &ToolContext) -> HookDecision {
        let mut all_empty = self.pre_tool_hooks.is_empty();
        if let Some(specific_hooks) = self
            .tool_specific_pre_hooks
            .get(&tool_call.name.to_lowercase())
        {
            all_empty = all_empty && specific_hooks.is_empty();
        }

        if all_empty {
            return HookDecision::allow();
        }

        let payload = HookPayload {
            event: HookEventKind::PreToolUse,
            session_id: &context.session_id,
            working_dir: context.working_dir.to_string_lossy().to_string(),
            tool_call_id: &tool_call.id,
            tool_name: &tool_call.name,
            arguments: &tool_call.arguments,
            success: None,
            result_content: None,
        };

        // 先运行全局钩子
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

        // 再运行特定工具钩子
        if let Some(specific_hooks) = self
            .tool_specific_pre_hooks
            .get(&tool_call.name.to_lowercase())
        {
            for hook in specific_hooks {
                debug!(
                    "Running tool-specific pre-hook '{}' for tool '{}'",
                    hook.name, tool_call.name
                );
                match self.execute_hook(hook, &payload).await {
                    Ok(Some(decision)) if !decision.allow => return decision,
                    Ok(_) => {}
                    Err(err) => {
                        warn!("Tool-specific pre-hook '{}' failed: {}", hook.name, err);
                        if hook.block_on_error {
                            return HookDecision::deny(format!(
                                "blocked by failing tool-specific pre-hook '{}': {}",
                                hook.name, err
                            ));
                        }
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
        let mut all_empty = self.post_tool_hooks.is_empty();
        if let Some(specific_hooks) = self
            .tool_specific_post_hooks
            .get(&tool_call.name.to_lowercase())
        {
            all_empty = all_empty && specific_hooks.is_empty();
        }

        if all_empty {
            return;
        }

        let payload = HookPayload {
            event: HookEventKind::PostToolUse,
            session_id: &context.session_id,
            working_dir: context.working_dir.to_string_lossy().to_string(),
            tool_call_id: &tool_call.id,
            tool_name: &tool_call.name,
            arguments: &tool_call.arguments,
            success: Some(result.success),
            result_content: Some(&result.content),
        };

        // 先运行全局钩子
        for hook in &self.post_tool_hooks {
            debug!(
                "Running post-tool hook '{}' for tool '{}'",
                hook.name, tool_call.name
            );
            if let Err(err) = self.execute_hook(hook, &payload).await {
                warn!("Post-tool hook '{}' failed: {}", hook.name, err);
            }
        }

        // 再运行特定工具钩子
        if let Some(specific_hooks) = self
            .tool_specific_post_hooks
            .get(&tool_call.name.to_lowercase())
        {
            for hook in specific_hooks {
                debug!(
                    "Running tool-specific post-hook '{}' for tool '{}'",
                    hook.name, tool_call.name
                );
                if let Err(err) = self.execute_hook(hook, &payload).await {
                    warn!("Tool-specific post-hook '{}' failed: {}", hook.name, err);
                }
            }
        }
    }

    pub async fn run_permission_request(
        &self,
        tool_call: &ToolCall,
        context: &ToolContext,
    ) -> HookDecision {
        self.run_lifecycle_hooks(
            HookEventKind::PermissionRequest,
            &self.permission_request_hooks,
            tool_call,
            context,
            None,
            None,
        )
        .await
    }

    pub async fn run_permission_resolved(
        &self,
        tool_call: &ToolCall,
        context: &ToolContext,
        approved: bool,
    ) -> HookDecision {
        self.run_lifecycle_hooks(
            HookEventKind::PermissionResolved,
            &self.permission_resolved_hooks,
            tool_call,
            context,
            Some(approved),
            None,
        )
        .await
    }

    async fn run_lifecycle_hooks(
        &self,
        event: HookEventKind,
        hooks: &[CommandHook],
        tool_call: &ToolCall,
        context: &ToolContext,
        success: Option<bool>,
        result_content: Option<&str>,
    ) -> HookDecision {
        if hooks.is_empty() {
            return HookDecision::allow();
        }

        let payload = HookPayload {
            event,
            session_id: &context.session_id,
            working_dir: context.working_dir.to_string_lossy().to_string(),
            tool_call_id: &tool_call.id,
            tool_name: &tool_call.name,
            arguments: &tool_call.arguments,
            success,
            result_content,
        };

        for hook in hooks {
            match self.execute_hook(hook, &payload).await {
                Ok(Some(decision)) if !decision.allow => return decision,
                Ok(_) => {}
                Err(err) => {
                    warn!("Lifecycle hook '{}' failed: {}", hook.name, err);
                    if hook.block_on_error {
                        return HookDecision::deny(format!(
                            "blocked by failing lifecycle hook '{}': {}",
                            hook.name, err
                        ));
                    }
                }
            }
        }

        HookDecision::allow()
    }

    async fn execute_hook(
        &self,
        hook: &CommandHook,
        payload: &HookPayload<'_>,
    ) -> Result<Option<HookDecision>, String> {
        let started_at = Instant::now();
        let finish_record = |manager: &Self,
                             success: bool,
                             blocked: bool,
                             error: Option<String>,
                             output_preview: Option<String>| {
            manager.push_record(HookRunRecord {
                sequence: 0,
                event: payload.event,
                provider: hook.provider,
                hook_name: hook.name.clone(),
                tool_call_id: payload.tool_call_id.to_string(),
                tool_name: Some(payload.tool_name.to_string()),
                success,
                blocked,
                duration_ms: started_at.elapsed().as_millis() as u64,
                error,
                output_preview,
            });
        };
        let payload_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&hook.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                let err = format!("spawn failed: {}", e);
                finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
                err
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(payload_json.as_bytes())
                .await
                .map_err(|e| {
                    let err = format!("stdin write failed: {}", e);
                    finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
                    err
                })?;
            stdin.write_all(b"\n").await.map_err(|e| {
                let err = format!("stdin newline write failed: {}", e);
                finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
                err
            })?;
        }

        let stdout = child.stdout.take().ok_or_else(|| {
            let err = "missing stdout pipe".to_string();
            finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
            err
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            let err = "missing stderr pipe".to_string();
            finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
            err
        })?;

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
            Ok(Err(e)) => {
                let err = format!("wait failed: {}", e);
                finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
                return Err(err);
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                let err = format!("timed out after {}ms", hook.timeout_ms);
                finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
                return Err(err);
            }
        };

        let stdout_bytes = stdout_task.await.unwrap_or_default();
        let stderr_bytes = stderr_task.await.unwrap_or_default();

        if !status.success() {
            let stderr_text = String::from_utf8_lossy(&stderr_bytes);
            let err = format!("exit status {} stderr: {}", status, stderr_text.trim());
            finish_record(self, false, hook.block_on_error, Some(err.clone()), None);
            return Err(err);
        }

        let stdout_text = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
        if stdout_text.is_empty() {
            finish_record(self, true, false, None, None);
            return Ok(None);
        }

        match serde_json::from_str::<HookResponse>(&stdout_text) {
            Ok(resp) => {
                if matches!(resp.allow, Some(false)) {
                    let reason = resp.reason.unwrap_or_else(|| "blocked by hook".to_string());
                    finish_record(
                        self,
                        true,
                        true,
                        None,
                        Some(reason.chars().take(240).collect()),
                    );
                    return Ok(Some(HookDecision::deny(reason)));
                }
                finish_record(
                    self,
                    true,
                    false,
                    None,
                    Some(stdout_text.chars().take(240).collect()),
                );
                Ok(None)
            }
            Err(_) => {
                debug!(
                    "Hook '{}' returned non-JSON output, treating as informational output",
                    hook.name
                );
                finish_record(
                    self,
                    true,
                    false,
                    None,
                    Some(stdout_text.chars().take(240).collect()),
                );
                Ok(None)
            }
        }
    }
}

fn hook_registration(event: HookEventKind, scope: String, hook: &CommandHook) -> HookRegistration {
    HookRegistration {
        event,
        provider: hook.provider,
        hook_name: hook.name.clone(),
        scope,
        timeout_ms: hook.timeout_ms,
        block_on_error: hook.block_on_error,
        command_preview: preview_hook_command(&hook.command),
    }
}

fn hook_timeout_ms_from_env() -> u64 {
    std::env::var("PRIORITY_AGENT_HOOK_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_HOOK_TIMEOUT_MS)
}

fn hook_fail_closed_from_env() -> bool {
    std::env::var("PRIORITY_AGENT_HOOK_FAIL_CLOSED")
        .ok()
        .map(|s| {
            let normalized = s.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes"
        })
        .unwrap_or(false)
}

fn preview_hook_command(command: &str) -> String {
    let compact = command.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 120 {
        compact
    } else {
        let mut out = compact.chars().take(119).collect::<String>();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::env_guard::EnvVarGuard;

    fn test_manager_with_pre(
        command: &str,
        timeout_ms: u64,
        block_on_error: bool,
    ) -> ToolHookManager {
        ToolHookManager {
            pre_tool_hooks: vec![CommandHook {
                name: "test_pre".to_string(),
                provider: HookProviderKind::Env,
                command: command.to_string(),
                timeout_ms,
                block_on_error,
            }],
            post_tool_hooks: Vec::new(),
            tool_specific_pre_hooks: HashMap::new(),
            tool_specific_post_hooks: HashMap::new(),
            permission_request_hooks: Vec::new(),
            permission_resolved_hooks: Vec::new(),
            recent_records: Arc::new(Mutex::new(VecDeque::new())),
            next_record_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    #[test]
    fn test_from_env_none_when_empty() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_PRE_TOOL_HOOK");
        env.remove("PRIORITY_AGENT_POST_TOOL_HOOK");
        env.remove("PRIORITY_AGENT_PERMISSION_REQUEST_HOOK");
        env.remove("PRIORITY_AGENT_PERMISSION_RESOLVED_HOOK");
        // 清理可能的细粒度钩子环境变量
        for (key, _) in std::env::vars() {
            if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_BEFORE_")
                || key.starts_with("PRIORITY_AGENT_TOOL_HOOK_AFTER_")
            {
                env.remove(&key);
            }
        }
        assert!(ToolHookManager::from_env().is_none());
    }

    #[test]
    fn lifecycle_snapshot_from_env_reports_registration_policy() {
        let mut env = EnvVarGuard::acquire_blocking();
        env.remove("PRIORITY_AGENT_PRE_TOOL_HOOK");
        env.remove("PRIORITY_AGENT_POST_TOOL_HOOK");
        env.remove("PRIORITY_AGENT_PERMISSION_REQUEST_HOOK");
        env.remove("PRIORITY_AGENT_PERMISSION_RESOLVED_HOOK");
        for (key, _) in std::env::vars() {
            if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_BEFORE_")
                || key.starts_with("PRIORITY_AGENT_TOOL_HOOK_AFTER_")
            {
                env.remove(&key);
            }
        }
        env.set("PRIORITY_AGENT_PRE_TOOL_HOOK", "echo '{\"allow\": true}'");
        env.set(
            "PRIORITY_AGENT_PERMISSION_REQUEST_HOOK",
            "echo '{\"allow\": true}'",
        );
        env.set("PRIORITY_AGENT_HOOK_TIMEOUT_MS", "2500");
        env.set("PRIORITY_AGENT_HOOK_FAIL_CLOSED", "true");
        env.set("PRIORITY_AGENT_TOOL_HOOK_AFTER_BASH", "printf done");

        let snapshot = ToolHookManager::lifecycle_snapshot_from_env();

        assert!(snapshot.configured);
        assert_eq!(snapshot.default_timeout_ms, 2500);
        assert!(snapshot.fail_closed);
        assert_eq!(snapshot.registrations.len(), 3);
        assert!(snapshot.registrations.iter().any(|registration| {
            registration.event == HookEventKind::PreToolUse
                && registration.scope == "global"
                && registration.block_on_error
        }));
        assert!(snapshot.registrations.iter().any(|registration| {
            registration.event == HookEventKind::PostToolUse
                && registration.scope == "tool:bash"
                && registration.hook_name == "env_post_tool_hook_bash"
        }));
        assert!(snapshot.registrations.iter().any(|registration| {
            registration.event == HookEventKind::PermissionRequest
                && registration.scope == "global"
                && registration.hook_name == "env_permission_request_hook"
        }));
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
        let records = manager.recent_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event, HookEventKind::PreToolUse);
        assert_eq!(records[0].provider, HookProviderKind::Env);
        assert_eq!(records[0].sequence, 1);
        assert_eq!(records[0].tool_call_id, "1");
        assert_eq!(records[0].tool_name.as_deref(), Some("file_write"));
        assert!(records[0].blocked);
        assert!(manager.recent_records_after(0).len() == 1);
        assert!(manager.recent_records_after(1).is_empty());
        assert!(manager.recent_records_after_for(0, "1").len() == 1);
        assert!(manager.recent_records_after_for(0, "other").is_empty());
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
        let records = manager.recent_records();
        assert_eq!(records.len(), 1);
        assert!(!records[0].success);
        assert!(!records[0].blocked);
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
        let records = manager.recent_records();
        assert_eq!(records.len(), 1);
        assert!(!records[0].success);
        assert!(records[0].blocked);
    }

    #[tokio::test]
    async fn permission_request_hook_can_deny() {
        let manager = ToolHookManager {
            pre_tool_hooks: Vec::new(),
            post_tool_hooks: Vec::new(),
            tool_specific_pre_hooks: HashMap::new(),
            tool_specific_post_hooks: HashMap::new(),
            permission_request_hooks: vec![CommandHook {
                name: "test_permission_request".to_string(),
                provider: HookProviderKind::Env,
                command: "echo '{\"allow\": false, \"reason\": \"policy denied\"}'".to_string(),
                timeout_ms: 2_000,
                block_on_error: false,
            }],
            permission_resolved_hooks: Vec::new(),
            recent_records: Arc::new(Mutex::new(VecDeque::new())),
            next_record_sequence: Arc::new(AtomicU64::new(0)),
        };
        let tool_call = ToolCall {
            id: "perm-1".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "npm run dev"}),
        };
        let context = ToolContext::new(".", "session-test");

        let decision = manager.run_permission_request(&tool_call, &context).await;

        assert!(!decision.allow);
        assert_eq!(decision.reason.as_deref(), Some("policy denied"));
        let records = manager.recent_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event, HookEventKind::PermissionRequest);
        assert!(records[0].blocked);
    }

    #[test]
    fn hook_event_kind_serializes_as_lifecycle_name() {
        let value = serde_json::to_value(HookEventKind::ValidationStart).unwrap();
        assert_eq!(value, serde_json::json!("ValidationStart"));
        assert_eq!(HookEventKind::SubagentEnd.to_string(), "SubagentEnd");
    }
}

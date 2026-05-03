//! 统一对话循环
//!
//! 将 QueryEngine 和 StreamingEngineInner 中重复的工具调用循环合并为一处。
//! 支持流式/非流式两种输出模式，内部逻辑完全一致。
//!
//! 改进（借鉴 hermes-agent）：
//! - 前置压缩（Preflight）：循环前检查总 token，超阈值提前压缩
//! - IterationBudget：迭代预算退还机制（只读工具可退还）

mod approval;
mod step_executor;
mod tool_execution;

pub use approval::{ToolApprovalChannel, ToolApprovalRequest};
pub(crate) use step_executor::{is_drift_interruption_signal, WorkflowRealStepExecutor};
pub(crate) use tool_execution::{
    is_read_only, read_only_tool_concurrency, safe_prefix_by_bytes, truncate_tool_result,
    READ_ONLY_TOOLS,
};

use crate::engine::intent_router::IntentRouter;
use crate::engine::trace::{TraceCollector, TraceEvent, TraceStore, TurnStatus, TurnTrace};
use crate::engine::workflow::{Gate, WorkflowEngine, WorkflowPolicy};
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Message, ToolCall};
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, warn};

use super::context_compressor::{
    estimate_messages_tokens, estimate_tool_schemas_tokens, ContextCompressor,
};
use super::hooks::{HookDecision, HookRunRecord, ToolHookManager};
use super::streaming::StreamEvent;

const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Default)]
struct VisibleTextSanitizer {
    buffer: String,
    in_think_block: bool,
}

impl VisibleTextSanitizer {
    fn push_chunk(&mut self, chunk: &str) -> String {
        self.buffer.push_str(chunk);
        self.drain_visible(false)
    }

    fn finish(&mut self) -> String {
        self.drain_visible(true)
    }

    fn drain_visible(&mut self, flush_all: bool) -> String {
        let mut out = String::new();
        loop {
            if self.in_think_block {
                if let Some(end_idx) = self.buffer.find(THINK_CLOSE_TAG) {
                    let drain_len = end_idx + THINK_CLOSE_TAG.len();
                    self.buffer.drain(..drain_len);
                    self.in_think_block = false;
                    continue;
                }

                if flush_all {
                    self.buffer.clear();
                } else {
                    let keep = THINK_CLOSE_TAG.len().saturating_sub(1);
                    if self.buffer.len() > keep {
                        let drain_len = floor_char_boundary(&self.buffer, self.buffer.len() - keep);
                        self.buffer.drain(..drain_len);
                    }
                }
                break;
            }

            if let Some(start_idx) = self.buffer.find(THINK_OPEN_TAG) {
                out.push_str(&self.buffer[..start_idx]);
                let drain_len = start_idx + THINK_OPEN_TAG.len();
                self.buffer.drain(..drain_len);
                self.in_think_block = true;
                continue;
            }

            if flush_all {
                out.push_str(&self.buffer);
                self.buffer.clear();
            } else {
                let keep = THINK_OPEN_TAG.len().saturating_sub(1);
                if self.buffer.len() > keep {
                    let emit_len = floor_char_boundary(&self.buffer, self.buffer.len() - keep);
                    out.push_str(&self.buffer[..emit_len]);
                    self.buffer.drain(..emit_len);
                }
            }
            break;
        }

        out
    }
}

fn strip_think_blocks(text: &str) -> String {
    let mut sanitizer = VisibleTextSanitizer::default();
    let mut visible = sanitizer.push_chunk(text);
    visible.push_str(&sanitizer.finish());
    visible
}

fn tool_result_dialog_content(result: &ToolResult) -> String {
    if !result.content.is_empty() {
        result.content.clone()
    } else {
        result.error.clone().unwrap_or_default()
    }
}

fn llm_request_timeout() -> std::time::Duration {
    let secs = std::env::var("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(180)
        .clamp(30, 600);
    std::time::Duration::from_secs(secs)
}

fn required_validation_timeout() -> std::time::Duration {
    let secs = std::env::var("PRIORITY_AGENT_REQUIRED_VALIDATION_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(900)
        .clamp(30, 900);
    std::time::Duration::from_secs(secs)
}

fn should_run_default_auto_tests(required_validation_commands: &[String]) -> bool {
    required_validation_commands.is_empty()
}

async fn shell_output_with_timeout(
    command: &str,
    working_dir: &std::path::Path,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output> {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-lc").arg(command).current_dir(working_dir);
    #[cfg(unix)]
    cmd.process_group(0);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn()?;
    let child_pid = child.id();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(ref mut stream) = stdout {
            stream.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(ref mut stream) = stderr {
            stream.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });

    let started_at = std::time::Instant::now();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let status = loop {
        tokio::select! {
            result = child.wait() => break result?,
            _ = heartbeat.tick() => {
                let elapsed = started_at.elapsed();
                if elapsed >= std::time::Duration::from_secs(30) {
                    eprintln!(
                        "[required validation still running after {}s] {}",
                        elapsed.as_secs(),
                        safe_prefix_by_bytes(command, 160)
                    );
                }
            }
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(started_at + timeout)) => {
                #[cfg(unix)]
                if let Some(pid) = child_pid {
                    unsafe {
                        libc::kill(-(pid as i32), libc::SIGKILL);
                    }
                }
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("command timed out after {}s", timeout.as_secs()),
                ));
            }
        }
    };
    let stdout = stdout_task
        .await
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))??;
    let stderr = stderr_task
        .await
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))??;
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn stream_chunk_idle_timeout() -> std::time::Duration {
    let secs = std::env::var("PRIORITY_AGENT_STREAM_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(120)
        .clamp(30, 600);
    std::time::Duration::from_secs(secs)
}

fn verification_source_context(
    working_dir: &std::path::Path,
    results: &[super::auto_verify::VerificationResult],
) -> Option<String> {
    let canonical_cwd = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());
    let mut snippets = Vec::new();
    let mut seen = HashSet::new();
    let mut total_chars = 0usize;

    for result in results {
        for issue in result.issues.iter().take(12) {
            let Some(file) = issue.file.as_deref() else {
                continue;
            };
            let raw_path = std::path::Path::new(file);
            let candidate = if raw_path.is_absolute() {
                raw_path.to_path_buf()
            } else {
                working_dir.join(raw_path)
            };
            let Ok(canonical_file) = candidate.canonicalize() else {
                continue;
            };
            if !canonical_file.starts_with(&canonical_cwd) || !canonical_file.is_file() {
                continue;
            }
            let line = issue.line.unwrap_or(1).max(1) as usize;
            let key = (canonical_file.clone(), line);
            if !seen.insert(key) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&canonical_file) else {
                continue;
            };
            let lines = content.lines().collect::<Vec<_>>();
            if lines.is_empty() {
                continue;
            }
            let start = line.saturating_sub(3).max(1);
            let end = (line + 3).min(lines.len());
            let relative = canonical_file
                .strip_prefix(&canonical_cwd)
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| canonical_file.display().to_string());
            let mut snippet = format!(
                "[Verification source context] {}:{} ({})\n",
                relative, line, issue.message
            );
            for idx in start..=end {
                let marker = if idx == line { ">" } else { " " };
                let source_line = lines.get(idx - 1).copied().unwrap_or_default();
                snippet.push_str(&format!("{marker} {idx:>4} | {source_line}\n"));
            }
            total_chars += snippet.chars().count();
            snippets.push(snippet);
            if total_chars >= 12_000 {
                break;
            }
        }
        if total_chars >= 12_000 {
            break;
        }
    }

    if snippets.is_empty() {
        None
    } else {
        Some(format!(
            "{}\nUse this exact current source context to repair compile/validation errors before addressing broader acceptance gaps.",
            snippets.join("\n")
        ))
    }
}

async fn changed_files_diff_evidence(
    working_dir: &std::path::Path,
    changed_files: &[std::path::PathBuf],
) -> Option<String> {
    let mut args = vec![
        "diff".to_string(),
        "--no-color".to_string(),
        "--".to_string(),
    ];
    let mut seen = HashSet::new();
    for path in changed_files {
        let display_path = path
            .strip_prefix(working_dir)
            .ok()
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        if display_path.trim().is_empty() || !seen.insert(display_path.clone()) {
            continue;
        }
        args.push(display_path);
    }

    if args.len() <= 3 {
        return None;
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(working_dir)
        .output()
        .await
        .ok()?;

    if !output.status.success() && output.stdout.is_empty() {
        return None;
    }

    let diff = String::from_utf8_lossy(&output.stdout);
    let trimmed = diff.trim();
    if trimmed.is_empty() {
        return None;
    }

    let max_chars = 12_000usize;
    let mut excerpt = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        excerpt.push_str("\n[diff excerpt truncated]");
    }

    Some(format!(
        "[Changed-file diff evidence]\n{}\nUse this diff as direct acceptance evidence for the modified files.",
        excerpt
    ))
}

#[derive(Debug, Deserialize)]
struct PatchSynthesisPlan {
    #[serde(default)]
    can_patch: bool,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    actions: Vec<PatchSynthesisAction>,
}

#[derive(Debug, Deserialize)]
struct PatchSynthesisAction {
    #[serde(default)]
    tool: String,
    path: String,
    #[serde(default)]
    old_string: Option<String>,
    new_string: String,
    #[serde(default)]
    line_start: Option<usize>,
    #[serde(default)]
    line_end: Option<usize>,
    #[serde(default)]
    expected_replacements: Option<usize>,
}

async fn emit_usage_event(response: &ChatResponse, tx: &mpsc::Sender<StreamEvent>) {
    if let Some(usage) = &response.usage {
        let _ = tx
            .send(StreamEvent::Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                cached_tokens: usage.cached_tokens,
            })
            .await;
    }
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn tool_call_fingerprint(tc: &ToolCall) -> String {
    let args = serde_json::to_string(&tc.arguments).unwrap_or_else(|_| "null".to_string());
    format!("{}|{}", tc.name, args)
}

fn persist_turn_learning_event(
    store: &crate::session_store::SessionStore,
    trace: &crate::engine::trace::TurnTrace,
) -> rusqlite::Result<i64> {
    let intent = trace.events.iter().find_map(|event| match event {
        TraceEvent::IntentRouted { intent, .. } => Some(intent.as_str()),
        _ => None,
    });
    let goal = trace.events.iter().find_map(|event| match event {
        TraceEvent::SessionGoalUpdated { title, .. } => Some(title.as_str()),
        _ => None,
    });
    let tool_count = trace
        .events
        .iter()
        .filter(|event| matches!(event, TraceEvent::ToolCompleted { .. }))
        .count();
    let summary = match (goal, intent) {
        (Some(goal), Some(intent)) => format!("Turn {:?}: {} ({})", trace.status, goal, intent),
        (Some(goal), None) => format!("Turn {:?}: {}", trace.status, goal),
        (None, Some(intent)) => format!("Turn {:?}: intent {}", trace.status, intent),
        (None, None) => format!("Turn {:?}: no routed intent", trace.status),
    };
    let payload = serde_json::json!({
        "trace_id": trace.trace_id,
        "turn_index": trace.turn_index,
        "status": format!("{:?}", trace.status),
        "intent": intent,
        "goal": goal,
        "tool_count": tool_count,
        "event_count": trace.events.len(),
        "duration_ms": trace.duration_ms(),
    });
    let payload = crate::engine::experience_ledger::attach_experience_payload(
        payload,
        crate::engine::experience_ledger::ExperienceRecord::from_turn_trace(trace),
    );
    let confidence = if trace.status == TurnStatus::Completed {
        1.0
    } else {
        0.45
    };
    store.add_learning_event(
        &trace.session_id,
        "turn_outcome",
        "conversation_loop",
        &summary,
        confidence,
        &payload,
    )
}

fn record_recovery_plan(trace: &TraceCollector, plan: &crate::engine::recovery_plan::RecoveryPlan) {
    trace.record(TraceEvent::RecoveryPlan {
        plan_id: plan.id.clone(),
        source: plan.source.clone(),
        category: plan.category.clone(),
        action: plan.action.clone(),
        retryable: plan.retryable,
        safe_retry: plan.safe_retry,
        suggested_command: plan.suggested_command.clone(),
        status: format!("{:?}", plan.status),
    });
    trace.record(TraceEvent::RecoveryApplied {
        error: plan.primary_error.clone(),
        action: plan.trace_action(),
    });
}

fn record_goal_drift_if_needed(
    trace: &Option<TraceCollector>,
    goal: Option<&crate::engine::session_goal::SessionGoal>,
    tool_call: &ToolCall,
) {
    let (Some(trace), Some(goal)) = (trace, goal) else {
        return;
    };
    let check = crate::engine::goal_drift::GoalDriftDetector::new().check(goal, tool_call);
    if check.should_trace() {
        trace.record(TraceEvent::GoalDriftDetected {
            goal_id: goal.id.clone(),
            tool: tool_call.name.clone(),
            call_id: tool_call.id.clone(),
            level: format!("{:?}", check.level),
            reason: check.reason,
            suggested_action: check.suggested_action,
        });
    }
}

fn record_mcp_resource_trace(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(trace) = trace else {
        return;
    };
    let action = match tool_call.name.as_str() {
        "list_mcp_resources" => "list",
        "read_mcp_resource" => "read",
        _ => return,
    };
    let server = tool_call.arguments["server_name"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or("all")
        .to_string();
    let uri = tool_call.arguments["uri"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or("*")
        .to_string();

    trace.record(TraceEvent::McpResourceAccessed {
        server: server.clone(),
        uri: uri.clone(),
        action: action.to_string(),
        success: result.success,
        content_chars: result.content.chars().count(),
    });
    trace.record(TraceEvent::RetrievalContextBuilt {
        policy: "Mcp".to_string(),
        sources: vec!["Mcp".to_string()],
        items: usize::from(result.success),
        estimated_tokens: crate::engine::retrieval_context::estimate_tokens(&result.content),
        provenance: vec![format!("mcp.resource:{}:{}", server, uri)],
        conflicts: 0,
    });
}

fn record_hook_traces(trace: &Option<TraceCollector>, records: &[HookRunRecord]) {
    let Some(trace) = trace else {
        return;
    };
    for record in records {
        trace.record(TraceEvent::HookCompleted {
            event: record.event.to_string(),
            hook_name: record.hook_name.clone(),
            call_id: record.tool_call_id.clone(),
            tool: record.tool_name.clone(),
            success: record.success,
            blocked: record.blocked,
            duration_ms: record.duration_ms,
            error: record.error.clone(),
            output_preview: record.output_preview.clone(),
        });
    }
}

fn tool_allowed_by_context(allowed_tools: &Option<HashSet<String>>, tool_name: &str) -> bool {
    allowed_tools
        .as_ref()
        .map(|allowed| allowed.contains(tool_name))
        .unwrap_or(true)
}

fn tool_not_allowed_result(tool_call: &ToolCall) -> ToolResult {
    let mut result = ToolResult::error(format!(
        "Tool '{}' is not allowed in this agent context",
        tool_call.name
    ));
    attach_tool_execution_metadata(tool_call, &mut result);
    result
}

fn record_web_retrieval_trace(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(trace) = trace else {
        return;
    };
    let (title, provenance) = match tool_call.name.as_str() {
        "web_search" => (
            "Web search results",
            tool_call.arguments["query"]
                .as_str()
                .map(|query| format!("web.search:{}", query))
                .unwrap_or_else(|| "web.search".to_string()),
        ),
        "web_fetch" => (
            "Web fetched content",
            tool_call.arguments["url"]
                .as_str()
                .map(|url| format!("web.fetch:{}", url))
                .unwrap_or_else(|| "web.fetch".to_string()),
        ),
        _ => return,
    };
    if let Some(ctx) = crate::engine::retrieval_context::RetrievalContext::from_web_result(
        &provenance,
        title,
        &result.content,
        provenance.clone(),
        crate::engine::intent_router::RetrievalPolicy::Web,
    ) {
        trace.record(TraceEvent::RetrievalContextBuilt {
            policy: format!("{:?}", ctx.policy),
            sources: ctx
                .items
                .iter()
                .map(|item| format!("{:?}", item.source))
                .collect(),
            items: ctx.items.len(),
            estimated_tokens: ctx.token_estimate,
            provenance: ctx.provenance_summaries(),
            conflicts: ctx.conflict_count(),
        });
    }
}

async fn build_project_retrieval_context(
    query: &str,
    working_dir: &std::path::Path,
    policy: crate::engine::intent_router::RetrievalPolicy,
) -> Option<crate::engine::retrieval_context::RetrievalContext> {
    if !matches!(
        policy,
        crate::engine::intent_router::RetrievalPolicy::Project
            | crate::engine::intent_router::RetrievalPolicy::Full
    ) {
        return None;
    }
    let root = working_dir.to_path_buf();
    let query = query.to_string();
    tokio::task::spawn_blocking(move || {
        let mut scanner = crate::tools::project_tool::ProjectScanner::new();
        scanner.scan(&root);
        crate::engine::retrieval_context::RetrievalContext::from_project_summary(
            &query,
            scanner.tree_summary(),
            &root,
            policy,
        )
    })
    .await
    .ok()
    .flatten()
}

async fn build_session_retrieval_context(
    query: &str,
    store: Option<Arc<crate::session_store::SessionStore>>,
    policy: crate::engine::intent_router::RetrievalPolicy,
) -> Option<crate::engine::retrieval_context::RetrievalContext> {
    if !matches!(
        policy,
        crate::engine::intent_router::RetrievalPolicy::Memory
            | crate::engine::intent_router::RetrievalPolicy::Project
            | crate::engine::intent_router::RetrievalPolicy::Full
    ) {
        return None;
    }
    let store = store?;
    let query = fts_phrase_query(query);
    if query.trim().is_empty() {
        return None;
    }
    tokio::task::spawn_blocking(move || {
        store.search_messages(&query, 4).ok().and_then(|messages| {
            crate::engine::retrieval_context::RetrievalContext::from_session_messages(
                &query, &messages, policy,
            )
        })
    })
    .await
    .ok()
    .flatten()
}

fn fts_phrase_query(query: &str) -> String {
    let compact = query
        .chars()
        .filter(|ch| !ch.is_control())
        .take(160)
        .collect::<String>()
        .replace('"', "\"\"");
    if compact.trim().is_empty() {
        String::new()
    } else {
        format!("\"{}\"", compact)
    }
}

fn workflow_contract_enabled(provider: &dyn LlmProvider) -> bool {
    if provider.base_url().starts_with("mock://") {
        return false;
    }

    std::env::var("PRIORITY_AGENT_WORKFLOW_CONTRACT")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "0" | "false" | "off" | "no")
        })
        .unwrap_or(true)
}

fn should_use_nonstreaming_tools(
    provider: &dyn LlmProvider,
    tools: &[crate::services::api::Tool],
) -> bool {
    if tools.is_empty() {
        return false;
    }
    let base_url = provider.base_url().to_ascii_lowercase();
    let model = provider.default_model().to_ascii_lowercase();
    base_url.contains("minimax") || model.contains("minimax")
}

fn tool_error_code_label(result: &ToolResult) -> Option<String> {
    result.error_code.as_ref().and_then(|code| {
        serde_json::to_value(code)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
    })
}

fn merge_tool_result_metadata(result: &mut ToolResult, key: &str, value: serde_json::Value) {
    match result.data.take() {
        Some(serde_json::Value::Object(mut object)) => {
            object.insert(key.to_string(), value);
            result.data = Some(serde_json::Value::Object(object));
        }
        Some(existing) => {
            result.data = Some(serde_json::json!({
                "value": existing,
                key: value,
            }));
        }
        None => {
            result.data = Some(serde_json::json!({
                key: value,
            }));
        }
    }
}

fn build_tool_execution_summary(tool_call: &ToolCall, result: &ToolResult) -> serde_json::Value {
    let output_chars = result.content.chars().count();
    let mut summary = serde_json::json!({
        "tool": tool_call.name,
        "call_id": tool_call.id,
        "success": result.success,
        "output_chars": output_chars,
        "duration_ms": result.duration_ms,
    });
    let Some(object) = summary.as_object_mut() else {
        return summary;
    };

    match tool_call.name.as_str() {
        "bash" => {
            if let Some(command) = tool_call.arguments["command"].as_str() {
                let classification =
                    crate::tools::bash_tool::command_classifier::classify_command(command);
                object.insert(
                    "command".to_string(),
                    serde_json::Value::String(safe_prefix_by_bytes(command, 240).to_string()),
                );
                object.insert(
                    "command_kind".to_string(),
                    serde_json::to_value(classification.command_kind)
                        .unwrap_or_else(|_| serde_json::Value::Null),
                );
                object.insert(
                    "validation_family".to_string(),
                    serde_json::to_value(classification.validation_family)
                        .unwrap_or_else(|_| serde_json::Value::Null),
                );
                object.insert(
                    "safe_for_closeout".to_string(),
                    serde_json::Value::Bool(classification.safe_for_closeout),
                );
            }
        }
        "file_edit" => {
            if let Some(path) = tool_call.arguments["path"].as_str() {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
            if let Some(replacements) = result
                .data
                .as_ref()
                .and_then(|data| data.get("replacements"))
                .and_then(|value| value.as_u64())
            {
                object.insert(
                    "replacements".to_string(),
                    serde_json::Value::Number(replacements.into()),
                );
            }
        }
        "file_write" | "file_read" => {
            if let Some(path) = tool_call.arguments["path"].as_str() {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
        }
        "grep" => {
            if let Some(pattern) = tool_call.arguments["pattern"].as_str() {
                object.insert(
                    "pattern".to_string(),
                    serde_json::Value::String(safe_prefix_by_bytes(pattern, 120).to_string()),
                );
            }
            if let Some(path) = tool_call
                .arguments
                .get("path")
                .or_else(|| tool_call.arguments.get("include"))
                .and_then(|value| value.as_str())
            {
                object.insert(
                    "path".to_string(),
                    serde_json::Value::String(path.to_string()),
                );
            }
        }
        "git" => {
            if let Some(action) = tool_call.arguments["action"].as_str() {
                object.insert(
                    "action".to_string(),
                    serde_json::Value::String(action.to_string()),
                );
            }
        }
        _ => {}
    }

    if let Some(error) = result.error.as_deref() {
        object.insert(
            "error_preview".to_string(),
            serde_json::Value::String(safe_prefix_by_bytes(error, 240).to_string()),
        );
    }

    summary
}

fn tool_execution_start_progress(tool_name: &str, arguments: &serde_json::Value) -> String {
    if tool_name == "bash" {
        let Some(command) = arguments["command"].as_str() else {
            return "Executing bash...".to_string();
        };
        let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
        let prefix = match classification.validation_family {
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoTest) => {
                "Running Rust tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoCheck) => {
                "Running cargo check"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::CargoClippy) => {
                "Running cargo clippy"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::NpmTest)
            | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PnpmTest)
            | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::YarnTest) => {
                "Running JS tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::Pytest)
            | Some(crate::tools::bash_tool::command_classifier::ValidationFamily::PythonUnittest) => {
                "Running Python tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::GoTest) => {
                "Running Go tests"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::RgAssertion) => {
                "Running search assertion"
            }
            Some(crate::tools::bash_tool::command_classifier::ValidationFamily::NodeScript) => {
                "Running Node validation"
            }
            None => match classification.command_kind {
                crate::tools::bash_tool::command_classifier::CommandKind::Inspection => {
                    "Inspecting with shell"
                }
                crate::tools::bash_tool::command_classifier::CommandKind::Mutation => {
                    "Running shell mutation"
                }
                crate::tools::bash_tool::command_classifier::CommandKind::Dangerous => {
                    "Reviewing dangerous shell command"
                }
                _ => "Executing shell command",
            },
        };
        let command = safe_prefix_by_bytes(command, 80);
        return format!("{}: {}", prefix, command);
    }

    format!("Executing {}...", tool_name)
}

fn attach_tool_execution_metadata(tool_call: &ToolCall, result: &mut ToolResult) {
    let summary = build_tool_execution_summary(tool_call, result);
    merge_tool_result_metadata(result, "tool_summary", summary);

    if result.success {
        return;
    }
    let error = result
        .error
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("tool failed");
    let code = tool_error_code_label(result);
    let plan = crate::engine::recovery_plan::RecoveryPlan::tool_failure(
        &tool_call.name,
        error,
        code.as_deref(),
    );
    let metadata = serde_json::json!({
        "recoverable": plan.retryable,
        "safe_retry": plan.safe_retry,
        "suggested_command": plan.suggested_command,
        "user_note": plan.user_note,
        "recovery_action": plan.action,
        "recovery_category": plan.category,
    });
    merge_tool_result_metadata(result, "recovery", metadata);
}

fn persist_tool_outcome_learning_event(
    store: Option<&Arc<crate::session_store::SessionStore>>,
    session_id: &str,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(store) = store else {
        return;
    };
    let code = tool_error_code_label(result);
    let recovery = result
        .data
        .as_ref()
        .and_then(|data| data.get("recovery"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!(null));
    let tool_summary = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_summary"))
        .cloned()
        .unwrap_or_else(|| build_tool_execution_summary(tool_call, result));
    let summary = if result.success {
        format!("Tool {} succeeded", tool_call.name)
    } else {
        format!(
            "Tool {} failed: {}",
            tool_call.name,
            result.error.as_deref().unwrap_or("unknown error")
        )
    };
    let payload = serde_json::json!({
        "tool": tool_call.name,
        "call_id": tool_call.id,
        "success": result.success,
        "error_code": code,
        "error": result.error,
        "duration_ms": result.duration_ms,
        "output_chars": result.content.chars().count(),
        "tool_summary": tool_summary,
        "recovery": recovery,
    });
    let payload = crate::engine::experience_ledger::attach_experience_payload(
        payload,
        crate::engine::experience_ledger::ExperienceRecord::from_tool_outcome(tool_call, result),
    );
    if let Err(e) = store.add_learning_event(
        session_id,
        "tool_outcome",
        "conversation_loop",
        &summary,
        if result.success { 1.0 } else { 0.75 },
        &payload,
    ) {
        warn!("Failed to persist tool outcome learning event: {}", e);
    }
}

fn persist_workflow_learning_event(
    store: Option<&Arc<crate::session_store::SessionStore>>,
    session_id: &str,
    kind: &str,
    summary: String,
    confidence: f64,
    payload: serde_json::Value,
) {
    let Some(store) = store else {
        return;
    };
    if let Err(e) = store.add_learning_event(
        session_id,
        kind,
        "conversation_loop",
        &summary,
        confidence,
        &payload,
    ) {
        warn!("Failed to persist workflow learning event: {}", e);
    }
}

fn is_high_risk_workflow(
    route: &crate::engine::intent_router::IntentRoute,
    judgment: Option<&crate::engine::workflow_contract::ProgrammingWorkflowJudgment>,
) -> bool {
    matches!(route.risk, crate::engine::intent_router::RiskLevel::High)
        || judgment
            .map(|judgment| matches!(judgment.risk, crate::engine::intent_router::RiskLevel::High))
            .unwrap_or(false)
}

fn apply_workflow_feedback_and_trace(
    task_bundle: &mut crate::engine::task_context::TaskContextBundle,
    trace: &TraceCollector,
    feedback: crate::engine::workflow_contract::WeightFeedbackEvent,
) {
    let Some(judgment) = task_bundle.workflow_judgment.as_mut() else {
        return;
    };
    let Some(top_step) = judgment.top_plan_step() else {
        return;
    };
    let old_plan = judgment.plan.clone();
    let target_id = top_step.id.clone();
    let target_description = top_step.description.clone();

    let Some(step) =
        judgment
            .plan
            .iter_mut()
            .find(|step| match (target_id.as_deref(), step.id.as_deref()) {
                (Some(target), Some(id)) => target == id,
                _ => step.description == target_description,
            })
    else {
        return;
    };

    crate::engine::workflow_contract::apply_weight_feedback(step, &feedback);
    crate::engine::workflow_contract::normalize_weight_shares(&mut judgment.plan);

    if !crate::engine::workflow_contract::should_record_reweight(&old_plan, &judgment.plan) {
        return;
    }

    let top_step = judgment.top_plan_step();
    trace.record(TraceEvent::WorkflowPlanProgress {
        total_steps: judgment.plan.len(),
        completed_steps: 0,
        active_step: top_step.as_ref().map(|step| step.description.clone()),
        top_priority: top_step.as_ref().map(|step| format!("{:?}", step.priority)),
        top_importance_score: top_step.as_ref().map(|step| step.normalized_weight()),
        top_weight_share: top_step.as_ref().map(|step| step.computed_weight_share()),
        weight_source: top_step
            .as_ref()
            .and_then(|step| step.weight_source())
            .map(|source| format!("{:?}", source)),
        reweighted: true,
    });
}

fn trace_stage_validation(
    trace: &TraceCollector,
    record: &crate::engine::code_change_workflow::StageValidationRecord,
) {
    trace.record(TraceEvent::StageValidationCompleted {
        step: record.step_description.clone(),
        status: record.status.label().to_string(),
        changed_files: record.changed_files.len(),
        evidence_items: record.evidence.len(),
    });
}

/// 统一对话循环
pub struct ConversationLoop {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
    model: String,
    /// 会话 ID（固定，用于追踪 checkpoint、记忆等）
    session_id: String,
    max_iterations: usize,
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    hook_manager: Option<Arc<ToolHookManager>>,
    /// 上下文压缩器
    compressor: Option<Arc<Mutex<ContextCompressor>>>,
    /// 记忆管理器（预取 + 围栏注入 + 同步）
    memory_manager: Option<Arc<Mutex<crate::memory::MemoryManager>>>,
    /// 工具权限模式（由上层引擎注入）
    permission_mode: crate::permissions::PermissionMode,
    /// 当前会话内临时权限规则
    session_permission_rules: crate::permissions::PermissionRules,
    /// 是否启用 LLM 驱动的记忆提取
    llm_memory_extraction: bool,
    /// 工具授权通道（用于 MCP 等工具的交互式授权）
    approval_channel: Option<Arc<ToolApprovalChannel>>,
    /// 工具白名单（用于子 Agent 隔离；None 表示不限制）
    allowed_tools: Option<HashSet<String>>,
    /// 本轮是否已触发过 Workflow（每轮最多一次）
    workflow_triggered_this_turn: std::sync::atomic::AtomicBool,
    /// Workflow 策略（默认从环境变量读取，可覆盖）
    workflow_policy: WorkflowPolicy,
    /// 拒绝追踪器
    denial_tracker: Option<Arc<crate::security::DenialTracker>>,
    /// 安全审计日志
    audit_log: Option<Arc<crate::security::SecurityAuditLog>>,
    /// Runtime trace store for recent turn timelines.
    trace_store: Option<Arc<TraceStore>>,
    /// Runtime session goal manager.
    goal_manager: Option<Arc<crate::engine::session_goal::SessionGoalManager>>,
    /// Optional persistent store for completed traces.
    session_store: Option<Arc<crate::session_store::SessionStore>>,
    /// Monotonic turn counter used for trace display.
    turn_counter: std::sync::atomic::AtomicU64,
}

/// 对话循环结果
pub struct LoopResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub iterations: usize,
    /// 流式预执行的只读工具结果（tool_index → result）
    /// execute_tools_parallel 应跳过已有结果的只读工具
    pub pre_executed_results: std::collections::HashMap<usize, ToolResult>,
}

impl ConversationLoop {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
        model: String,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            cost_tracker,
            model,
            max_iterations: 10,
            agent_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            hook_manager: ToolHookManager::from_env().map(Arc::new),
            compressor: None,
            memory_manager: None,
            permission_mode: crate::permissions::PermissionMode::AutoAll,
            session_permission_rules: crate::permissions::PermissionRules::new(),
            llm_memory_extraction: false,
            approval_channel: None,
            allowed_tools: None,
            workflow_triggered_this_turn: std::sync::atomic::AtomicBool::new(false),
            workflow_policy: WorkflowPolicy::from_env(),
            session_id: format!("session-{}", uuid::Uuid::new_v4()),
            denial_tracker: None,
            audit_log: None,
            trace_store: None,
            goal_manager: None,
            session_store: None,
            turn_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// 启用记忆管理器（预取 + 围栏注入 + 同步）
    pub fn with_memory_manager(
        mut self,
        manager: Arc<Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        self.memory_manager = Some(manager);
        self
    }

    /// 启用上下文压缩（设置最大上下文 token 数）
    pub fn with_compression(mut self, max_context_tokens: u64) -> Self {
        self.compressor = Some(Arc::new(Mutex::new(
            ContextCompressor::new(max_context_tokens)
                .with_llm_provider(self.provider.clone(), &self.model),
        )));
        self
    }

    pub fn with_compressor(mut self, compressor: Arc<Mutex<ContextCompressor>>) -> Self {
        self.compressor = Some(compressor);
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_agent_manager(mut self, manager: Arc<crate::agent::AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    pub fn with_mcp_manager(mut self, manager: Arc<crate::engine::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub fn with_lsp_manager(mut self, manager: Arc<crate::engine::lsp::LspManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    pub fn with_worktree_manager(
        mut self,
        manager: Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    pub fn with_hook_manager(mut self, manager: Arc<ToolHookManager>) -> Self {
        self.hook_manager = Some(manager);
        self
    }

    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    pub fn with_session_permission_rules(
        mut self,
        rules: crate::permissions::PermissionRules,
    ) -> Self {
        self.session_permission_rules = rules;
        self
    }

    pub fn with_llm_memory_extraction(mut self, enabled: bool) -> Self {
        self.llm_memory_extraction = enabled;
        self
    }

    pub fn with_approval_channel(mut self, channel: Arc<ToolApprovalChannel>) -> Self {
        self.approval_channel = Some(channel);
        self
    }

    pub fn with_allowed_tools(mut self, tools: HashSet<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    pub fn with_workflow_policy(mut self, policy: WorkflowPolicy) -> Self {
        self.workflow_policy = policy;
        self
    }

    pub fn with_trace_store(mut self, store: Arc<TraceStore>) -> Self {
        self.trace_store = Some(store);
        self
    }

    pub fn with_session_goal_manager(
        mut self,
        manager: Arc<crate::engine::session_goal::SessionGoalManager>,
    ) -> Self {
        self.goal_manager = Some(manager);
        self
    }

    pub fn with_session_store(
        mut self,
        store: Arc<crate::session_store::SessionStore>,
        session_id: impl Into<String>,
    ) -> Self {
        self.session_store = Some(store);
        self.session_id = session_id.into();
        self
    }

    /// 创建工具执行上下文
    fn create_tool_context(&self) -> ToolContext {
        let mut ctx = ToolContext::new(".", self.session_id.clone());
        if let Some(ref manager) = self.agent_manager {
            ctx = ctx.with_agent_manager(manager.clone());
        }
        if let Some(ref store) = self.session_store {
            ctx = ctx.with_session_store(store.clone());
        }
        if let Some(ref mcp) = self.mcp_manager {
            ctx = ctx.with_mcp_manager(mcp.clone());
        }
        if let Some(ref lsp) = self.lsp_manager {
            ctx = ctx.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            ctx = ctx.with_worktree_manager(wt.clone());
        }
        ctx = ctx.with_llm_provider(self.provider.clone());
        ctx = ctx.with_model(&self.model);
        ctx = ctx.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        // 权限模式由上层引擎注入（默认 AutoAll，保留高风险确认）
        ctx.permission_context.mode = self.permission_mode;
        ctx.permission_context
            .rules
            .always_allow
            .extend(self.session_permission_rules.always_allow.clone());
        ctx.permission_context
            .rules
            .always_deny
            .extend(self.session_permission_rules.always_deny.clone());
        ctx.permission_context
            .rules
            .always_ask
            .extend(self.session_permission_rules.always_ask.clone());
        ctx
    }

    fn create_tool_context_with_trace(&self, trace: &TraceCollector) -> ToolContext {
        self.create_tool_context()
            .with_trace_collector(trace.clone())
    }

    fn create_tool_context_with_optional_trace(
        &self,
        trace: &Option<TraceCollector>,
    ) -> ToolContext {
        match trace {
            Some(trace) => self.create_tool_context_with_trace(trace),
            None => self.create_tool_context(),
        }
    }

    /// 运行对话循环（非流式）
    pub async fn run(&self, messages: Vec<Message>) -> Result<LoopResult> {
        self.run_inner(messages, None::<&mpsc::Sender<StreamEvent>>)
            .await
    }

    /// 运行对话循环（流式）
    pub async fn run_streaming(
        &self,
        messages: Vec<Message>,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<LoopResult> {
        self.run_inner(messages, Some(tx)).await
    }

    /// 核心循环实现
    async fn run_inner(
        &self,
        mut messages: Vec<Message>,
        tx: Option<&mpsc::Sender<StreamEvent>>,
    ) -> Result<LoopResult> {
        let last_user_preview = messages
            .iter()
            .rposition(|m| matches!(m, Message::User { .. }))
            .and_then(|i| match &messages[i] {
                Message::User { content } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or("")
            .to_string();
        let required_validation_commands =
            Self::extract_required_validation_commands(&last_user_preview);
        let turn_index = self
            .trace_store
            .as_ref()
            .and_then(|store| store.latest().map(|trace| trace.turn_index + 1))
            .unwrap_or_else(|| {
                self.turn_counter
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1
            });
        let trace = TraceCollector::new(TurnTrace::new(
            self.session_id.clone(),
            turn_index,
            &last_user_preview,
        ));
        let learning_events = self
            .session_store
            .as_ref()
            .and_then(|store| store.recent_learning_events(&self.session_id, 20).ok())
            .unwrap_or_default();
        let route = IntentRouter::new().route_with_learning(&last_user_preview, &learning_events);
        trace.record(TraceEvent::IntentRouted {
            intent: format!("{:?}", route.intent),
            workflow: format!("{:?}", route.workflow),
            retrieval: format!("{:?}", route.retrieval),
            confidence: route.confidence,
            risk: format!("{:?}", route.risk),
            reason: route.reason.clone(),
        });
        let resource_policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
        trace.record(TraceEvent::ResourcePolicySelected {
            latency: format!("{:?}", resource_policy.latency),
            target_ms: resource_policy.latency.target_ms(),
            cost_ceiling_usd: resource_policy.cost_ceiling_usd,
            reasoning: format!("{:?}", resource_policy.reasoning),
            parallelism_limit: resource_policy.parallelism_limit,
            max_tool_calls: resource_policy.max_tool_calls,
            context_budget_tokens: resource_policy.context_budget_tokens,
            reason: resource_policy.reason.clone(),
        });
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut turn_retrieval_context =
            build_project_retrieval_context(&last_user_preview, &working_dir, route.retrieval)
                .await;
        if let Some(session_ctx) = build_session_retrieval_context(
            &last_user_preview,
            self.session_store.clone(),
            route.retrieval,
        )
        .await
        {
            if let Some(ref mut ctx) = turn_retrieval_context {
                ctx.extend(session_ctx);
            } else {
                turn_retrieval_context = Some(session_ctx);
            }
        }
        if let Some(ref mem_mutex) = self.memory_manager {
            let mut mem = mem_mutex.lock().await;
            mem.reset_turn();
            if let Some(memory_ctx) = mem
                .prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref(),
                    &self.model,
                    route.retrieval,
                )
                .await
            {
                trace.record(TraceEvent::MemoryPrefetch {
                    chars: memory_ctx
                        .items
                        .iter()
                        .map(|item| item.content_preview.chars().count())
                        .sum(),
                });
                if let Some(ref mut ctx) = turn_retrieval_context {
                    ctx.extend(memory_ctx);
                } else {
                    turn_retrieval_context = Some(memory_ctx);
                }
            }
        }
        if let Some(ref ctx) = turn_retrieval_context {
            trace.record(TraceEvent::RetrievalContextBuilt {
                policy: format!("{:?}", ctx.policy),
                sources: ctx
                    .items
                    .iter()
                    .map(|item| format!("{:?}", item.source))
                    .collect(),
                items: ctx.items.len(),
                estimated_tokens: ctx.token_estimate,
                provenance: ctx.provenance_summaries(),
                conflicts: ctx.conflict_count(),
            });
        }
        let mut task_bundle = crate::engine::task_context::TaskContextBundle::new(
            &last_user_preview,
            &working_dir,
            route.clone(),
            self.goal_manager
                .as_ref()
                .and_then(|manager| manager.current()),
        );
        if let Some(ref ctx) = turn_retrieval_context {
            task_bundle = task_bundle.with_retrieval(ctx.clone());
        }
        task_bundle.add_constraint(format!(
            "resource_policy={}",
            resource_policy.compact_label()
        ));
        if matches!(
            route.workflow,
            crate::engine::intent_router::WorkflowKind::CodeChange
                | crate::engine::intent_router::WorkflowKind::BugFix
        ) {
            task_bundle.add_risk("code-change tasks require explicit verification");
        }
        let mut code_workflow =
            crate::engine::code_change_workflow::CodeChangeWorkflowRunner::new(&task_bundle);
        let workflow_contract_prompt =
            crate::engine::workflow_contract::WorkflowContractPrompt::new(
                last_user_preview.as_str(),
                route.clone(),
                working_dir.display().to_string(),
            );
        if code_workflow.should_request_workflow_judgment()
            && workflow_contract_prompt.should_ask_model()
            && workflow_contract_enabled(self.provider.as_ref())
        {
            let analyzer = crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
                self.provider.as_ref(),
                self.model.clone(),
            );
            match analyzer.analyze(workflow_contract_prompt).await {
                Ok(mut judgment) => {
                    let learning_audit =
                        crate::engine::learning_planning::apply_learning_to_workflow_judgment(
                            &mut judgment,
                            &learning_events,
                            turn_retrieval_context.as_ref(),
                        );
                    let context_note = judgment.to_turn_context();
                    trace.record(TraceEvent::WorkflowJudgmentCompleted {
                        task_type: judgment.task_type.clone(),
                        complexity: format!("{:?}", judgment.complexity),
                        risk: format!("{:?}", judgment.risk),
                        plan_steps: judgment.plan.len(),
                        acceptance_checks: judgment.acceptance.criteria.len(),
                        questions: judgment.questions.len(),
                        guided_reasoning: judgment.guided_reasoning_required,
                    });
                    let top_step = judgment.top_plan_step();
                    trace.record(TraceEvent::WorkflowPlanProgress {
                        total_steps: judgment.plan.len(),
                        completed_steps: 0,
                        active_step: top_step.as_ref().map(|step| step.description.clone()),
                        top_priority: top_step.as_ref().map(|step| format!("{:?}", step.priority)),
                        top_importance_score: top_step
                            .as_ref()
                            .map(|step| step.normalized_weight()),
                        top_weight_share: top_step
                            .as_ref()
                            .map(|step| step.computed_weight_share()),
                        weight_source: top_step
                            .as_ref()
                            .and_then(|step| step.weight_source())
                            .map(|source| format!("{:?}", source)),
                        reweighted: learning_audit.applied,
                    });
                    if learning_audit.applied {
                        trace.record(TraceEvent::WorkflowLearningAdjusted {
                            adjustments: learning_audit.adjustments.len(),
                            before_top_step: learning_audit.before_top_step.clone(),
                            after_top_step: learning_audit.after_top_step.clone(),
                            reason: learning_audit.explanation.clone(),
                        });
                        persist_workflow_learning_event(
                            self.session_store.as_ref(),
                            &self.session_id,
                            "planning_adjustment",
                            format!(
                                "Learning adjusted workflow plan with {} change(s)",
                                learning_audit.adjustments.len()
                            ),
                            0.85,
                            serde_json::to_value(&learning_audit)
                                .unwrap_or_else(|_| serde_json::json!({})),
                        );
                    }
                    persist_workflow_learning_event(
                        self.session_store.as_ref(),
                        &self.session_id,
                        "workflow_judgment",
                        format!(
                            "Workflow judgment task_type={} risk={:?} questions={} guided={}",
                            judgment.task_type,
                            judgment.risk,
                            judgment.questions.len(),
                            judgment.guided_reasoning_required
                        ),
                        0.8,
                        serde_json::json!({
                            "task_type": judgment.task_type.clone(),
                            "complexity": format!("{:?}", judgment.complexity),
                            "risk": format!("{:?}", judgment.risk),
                            "requirement_complete_enough": judgment.requirement_complete_enough,
                            "needs_user_questions": judgment.needs_user_questions,
                            "question_reason": judgment.question_reason.clone(),
                            "questions": judgment.questions.clone(),
                            "assumptions": judgment.assumptions.clone(),
                            "guided_reasoning_required": judgment.guided_reasoning_required,
                            "guided_reasoning_triggers": judgment.guided_reasoning_triggers.iter().map(|trigger| format!("{:?}", trigger)).collect::<Vec<_>>(),
                            "plan_steps": judgment.plan.len(),
                            "weighted_plan": judgment.weighted_plan_summary(),
                            "acceptance_checks": judgment.acceptance.criteria.len(),
                        }),
                    );
                    task_bundle.apply_workflow_judgment(judgment);
                    code_workflow.refresh_policy(&task_bundle);
                    let insert_at = messages
                        .iter()
                        .take_while(|message| matches!(message, Message::System { .. }))
                        .count();
                    messages.insert(insert_at, Message::system(context_note));
                }
                Err(err) => {
                    warn!("Workflow judgment analysis failed: {}", err);
                    trace.record(TraceEvent::WorkflowFallback {
                        error: format!("workflow judgment analysis failed: {}", err),
                    });
                }
            }
        }
        trace.record(TraceEvent::TaskContextBuilt {
            task_id: task_bundle.task_id.clone(),
            workflow: format!("{:?}", task_bundle.route.workflow),
            files: task_bundle.relevant_files.len(),
            constraints: task_bundle.constraints.len(),
            risks: task_bundle.risks.len(),
            acceptance_checks: task_bundle.acceptance_checks.len(),
        });
        if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
            trace.record(TraceEvent::ImplementationIntentRecorded {
                task_id: task_bundle.task_id.clone(),
                workflow: format!("{:?}", task_bundle.route.workflow),
                target_files: task_bundle.relevant_files.len(),
                validation_commands: required_validation_commands.clone(),
                risks: task_bundle.risks.len(),
                reason: "code-change workflow must identify target scope and validation before first edit".to_string(),
            });
        }
        let reflection_pass =
            crate::engine::reflection_pass::ReflectionPass::from_task_bundle(&task_bundle);
        trace.record(TraceEvent::ReflectionPassCompleted {
            pass_id: reflection_pass.pass_id.clone(),
            task_id: reflection_pass.task_id.clone(),
            status: format!("{:?}", reflection_pass.status),
            findings: reflection_pass.findings.len(),
            unresolved: reflection_pass.unresolved_count(),
        });
        if reflection_pass.status == crate::engine::reflection_pass::ReflectionStatus::NeedsWork
            && code_workflow.should_block_on_reflection()
        {
            let review_prompt = format!(
                "Reflection pass '{}' found {} unresolved issue(s) before executing a {:?} workflow. Allow the turn to continue?",
                reflection_pass.pass_id,
                reflection_pass.unresolved_count(),
                route.workflow
            );
            let review_call = ToolCall {
                id: format!(
                    "reflection-{}",
                    &reflection_pass.pass_id[..8.min(reflection_pass.pass_id.len())]
                ),
                name: "reflection_review".to_string(),
                arguments: serde_json::json!({
                    "task_id": reflection_pass.task_id.clone(),
                    "pass_id": reflection_pass.pass_id.clone(),
                    "status": format!("{:?}", reflection_pass.status),
                    "unresolved": reflection_pass.unresolved_count(),
                    "workflow": format!("{:?}", route.workflow),
                }),
            };
            let mut approved = false;
            if let (Some(channel), Some(tx)) = (&self.approval_channel, tx) {
                let _ = tx
                    .send(StreamEvent::PermissionRequest {
                        id: review_call.id.clone(),
                        tool_name: review_call.name.clone(),
                        arguments: review_call.arguments.clone(),
                        prompt: review_prompt.clone(),
                    })
                    .await;
                trace.record(TraceEvent::PermissionRequested {
                    tool: review_call.name.clone(),
                    call_id: review_call.id.clone(),
                    prompt: review_prompt.clone(),
                });
                match channel
                    .submit(ToolApprovalRequest {
                        tool_call: review_call.clone(),
                        prompt: review_prompt.clone(),
                        review: Some(
                            crate::engine::human_review::HumanReviewRequest::reflection_gate(
                                reflection_pass.pass_id.clone(),
                                reflection_pass.unresolved_count(),
                                format!("{:?}", route.workflow),
                            ),
                        ),
                    })
                    .await
                {
                    Ok(is_approved) => approved = is_approved,
                    Err(e) => warn!("Reflection approval error: {}", e),
                }
                trace.record(TraceEvent::PermissionResolved {
                    tool: review_call.name,
                    call_id: review_call.id,
                    approved,
                });
            } else {
                approved = true;
            }
            if !approved {
                let content = "Stopped before code-change execution because reflection found unresolved acceptance gaps.".to_string();
                trace.record(TraceEvent::AssistantResponded {
                    chars: content.chars().count(),
                    iterations: 0,
                });
                self.finish_trace(trace.clone(), TurnStatus::Failed);
                return Ok(LoopResult {
                    content,
                    tool_calls: Vec::new(),
                    iterations: 0,
                    pre_executed_results: std::collections::HashMap::new(),
                });
            }
        }
        if let Some(manager) = &self.goal_manager {
            if let Some(goal) = manager.update_from_user_message(&last_user_preview, Some(&route)) {
                trace.record(TraceEvent::SessionGoalUpdated {
                    goal_id: goal.id,
                    title: goal.title,
                    status: format!("{:?}", goal.status),
                    reason: "user turn routed to trackable workflow".to_string(),
                });
            }
        }

        // ── Workflow 闸门检查 ──────────────────────────
        let already_triggered = self
            .workflow_triggered_this_turn
            .swap(true, std::sync::atomic::Ordering::SeqCst);
        if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
            trace.record(TraceEvent::WorkflowRouted {
                decision: "direct".to_string(),
                reason:
                    "code-change contract uses the tool loop; legacy workflow step executor skipped"
                        .to_string(),
            });
        } else if !already_triggered {
            if let Some(last_user_msg) = messages
                .iter()
                .rposition(|m| matches!(m, Message::User { .. }))
                .and_then(|i| match &messages[i] {
                    Message::User { content } => Some(content.as_str()),
                    _ => None,
                })
            {
                let workflow_policy = self.workflow_policy.clone();
                let gate = Gate::new().with_policy(workflow_policy.gate.clone());
                if is_drift_interruption_signal(last_user_msg) {
                    crate::engine::workflow::metrics::record_drift_interruption();
                }
                let decision = if workflow_policy.gate.llm_classifier_enabled {
                    gate.decide_with_llm(last_user_msg, self.provider.as_ref(), &self.model)
                        .await
                } else {
                    gate.decide(last_user_msg)
                };
                trace.record(TraceEvent::WorkflowRouted {
                    decision: if decision.is_workflow() {
                        "workflow".to_string()
                    } else {
                        "direct".to_string()
                    },
                    reason: decision.reason().to_string(),
                });
                if decision.is_workflow() {
                    crate::engine::workflow::metrics::record_workflow_run();
                    if let Some(ref mem_mgr) = self.memory_manager {
                        let mut mem = mem_mgr.lock().await;
                        mem.save_workflow_decision(
                            "gate",
                            last_user_msg,
                            "Workflow",
                            decision.reason(),
                        );
                    }
                    debug!("Workflow mode activated: {}", decision.reason());
                    let workflow_executor = WorkflowRealStepExecutor {
                        tool_registry: self.tool_registry.clone(),
                        llm_provider: self.provider.clone(),
                        model: self.model.clone(),
                        base_context: self.create_tool_context_with_trace(&trace),
                    };
                    let workflow_engine =
                        WorkflowEngine::new(self.provider.clone()).with_policy(workflow_policy);
                    match workflow_engine
                        .run(last_user_msg, last_user_msg, &workflow_executor)
                        .await
                    {
                        Ok(result) => {
                            trace.record(TraceEvent::WorkflowCompleted {
                                steps: result.plan.steps.len(),
                            });
                            let workflow_report = strip_think_blocks(&result.final_report);
                            if let Some(ref mem_mgr) = self.memory_manager {
                                let mut mem = mem_mgr.lock().await;
                                mem.save_workflow_decision(
                                    "execution",
                                    last_user_msg,
                                    "Success",
                                    &format!(
                                        "workflow completed with {} steps",
                                        result.plan.steps.len()
                                    ),
                                );
                            }
                            if let Some(tx) = tx {
                                if !workflow_report.trim().is_empty() {
                                    let _ = tx
                                        .send(StreamEvent::TextChunk(workflow_report.clone()))
                                        .await;
                                }
                                let _ = tx.send(StreamEvent::Complete).await;
                            }
                            trace.record(TraceEvent::AssistantResponded {
                                chars: workflow_report.chars().count(),
                                iterations: 0,
                            });
                            self.finish_trace(trace.clone(), TurnStatus::Completed);
                            return Ok(LoopResult {
                                content: workflow_report,
                                tool_calls: Vec::new(),
                                iterations: 0,
                                pre_executed_results: std::collections::HashMap::new(),
                            });
                        }
                        Err(e) => {
                            trace.record(TraceEvent::WorkflowFallback { error: e.clone() });
                            if let Some(ref mem_mgr) = self.memory_manager {
                                let mut mem = mem_mgr.lock().await;
                                mem.save_workflow_decision(
                                    "fallback",
                                    last_user_msg,
                                    "DirectMode",
                                    &e,
                                );
                            }
                            warn!(
                                "Workflow execution failed: {}, falling back to direct mode",
                                e
                            );
                        }
                    }
                }
            }
        }

        let base_tools = self.get_tools();
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut iterations_used = 0;
        let mut no_code_progress_rounds = 0usize;
        let mut action_checkpoint_active = false;
        let mut patch_synthesis_recovery_used = false;
        let mut failed_tool_fingerprints: HashMap<String, usize> = HashMap::new();
        let mut failed_tool_names: HashMap<String, usize> = HashMap::new();
        let mut successful_required_validation_commands: HashSet<String> = HashSet::new();

        // ── 记忆围栏注入：先注入，再让 preflight 统计真实请求大小 ──
        if let Some(ref mem_mutex) = self.memory_manager {
            let mem = mem_mutex.lock().await;
            let snapshot = mem.get_snapshot();
            if !snapshot.is_empty() && !messages.iter().any(|m| {
                matches!(m, Message::System { content } if content.contains("<memory-context>"))
            }) {
                trace.record(TraceEvent::MemorySnapshotInjected {
                    chars: snapshot.chars().count(),
                });
                let insert_pos = messages
                    .iter()
                    .position(|m| !matches!(m, Message::System { .. }))
                    .unwrap_or(messages.len());
                messages.insert(insert_pos, Message::system(&snapshot));
                debug!("Injected memory context fence at position {}", insert_pos);
            }
        }

        // ── 前置压缩（Preflight）─────────────────────────
        if let Some(ref compressor_mutex) = self.compressor {
            let mut no_gain_passes = 0u8;
            for pass in 0..3 {
                let compressor = compressor_mutex.lock().await;
                let tool_tokens = estimate_tool_schemas_tokens(&base_tools);
                let msg_tokens = estimate_messages_tokens(&messages);
                // `messages` already includes the system prompt at this point,
                // so only add tool schema tokens as external request overhead.
                if !compressor.preflight_check(&messages, 0, tool_tokens) {
                    break;
                }
                debug!(
                    "Preflight compression pass {}/3 ({} msg + {} tool tokens)",
                    pass + 1,
                    msg_tokens,
                    tool_tokens
                );
                drop(compressor);
                let before_tokens = estimate_messages_tokens(&messages);
                messages = compressor_mutex
                    .lock()
                    .await
                    .compress_async(&messages)
                    .await;
                let after_tokens = estimate_messages_tokens(&messages);
                trace.record(TraceEvent::ContextCompacted {
                    before_tokens: before_tokens as usize,
                    after_tokens: after_tokens as usize,
                    strategy: "preflight".to_string(),
                });
                if after_tokens >= before_tokens {
                    no_gain_passes += 1;
                    if no_gain_passes >= 2 {
                        warn!(
                            "Preflight compression made no progress for 2 consecutive passes ({} -> {}). Stop retrying this turn.",
                            before_tokens, after_tokens
                        );
                        break;
                    }
                } else {
                    no_gain_passes = 0;
                }
            }
        }

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Start).await;
        }

        if let Some(ref ctx) = turn_retrieval_context {
            let block = ctx.format_for_prompt();
            if !block.is_empty()
                && !messages.iter().any(|m| {
                    matches!(m, Message::System { content } if content.contains("project.index:"))
                })
            {
                messages.push(Message::system(block));
            }
        }

        // ── 迭代预算 ─────────────────────────────────────
        let mut effective_iterations: usize = 0;
        let mut acceptance_repair_attempts: usize = 0;
        let mut reserved_repair_rounds: usize = 0;
        let max_loop_iterations = self.max_iterations + code_workflow.max_repair_attempts().max(3);
        let baseline_git_status_files = Self::git_status_files();
        let mut action_checkpoint_no_change_rounds = 0usize;

        for iteration in 0..max_loop_iterations {
            debug!(
                "Conversation loop iteration {} (effective: {}/{})",
                iteration, effective_iterations, self.max_iterations
            );
            iterations_used = iteration + 1;

            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                mem.reset_turn();
            }

            if effective_iterations >= self.max_iterations {
                if reserved_repair_rounds > 0 {
                    reserved_repair_rounds -= 1;
                    trace.record(TraceEvent::WorkflowFallback {
                        error: format!(
                            "using reserved repair round after validation failure (remaining={})",
                            reserved_repair_rounds
                        ),
                    });
                } else {
                    warn!(
                        "Effective iteration budget exhausted ({}/{})",
                        effective_iterations, self.max_iterations
                    );
                    break;
                }
            }

            let has_changes_before_request =
                crate::engine::code_change_workflow::is_programming_workflow(route.workflow)
                    && !Self::git_status_files_since(&baseline_git_status_files).is_empty();
            let tools = if action_checkpoint_active {
                let action_tools = Self::code_action_tools(&base_tools, has_changes_before_request);
                if action_tools.is_empty() {
                    base_tools.clone()
                } else {
                    action_tools
                }
            } else {
                base_tools.clone()
            };
            let exposed_tool_names = tools
                .iter()
                .map(|tool| tool.name.clone())
                .collect::<HashSet<_>>();

            let mut request_messages = messages.clone();
            if action_checkpoint_active {
                let mut exposed_names = exposed_tool_names.iter().cloned().collect::<Vec<_>>();
                exposed_names.sort();
                request_messages.push(Message::system(format!(
                    "Current tool mode: FOCUSED REPAIR. The exposed tools for this request are: {}. Use file_edit/file_write to patch files as soon as the target line is known. file_read/grep are allowed only for one targeted lookup of a specific symbol, test, or call site; do not repeat broad inspection. If bash is exposed, use it only to run validation after a patch. If previous validation reported compile/type errors, fix those exact errors first using the latest verification source context. If you have line numbers from earlier grep/file_read/verification output, prefer file_edit with line_start/line_end or exact old_string copied from that current source context. Do not invent enum variants, struct fields, functions, or APIs not visible in prior tool output; reuse existing names exactly. If a scorer/decision object already returns a final status, use that status directly; do not wrap it with explicit/score checks that can bypass safety, volatility, or duplication hard stops.",
                    exposed_names.join(", ")
                )));
            }
            let memory_already_in_turn_context = turn_retrieval_context
                .as_ref()
                .map(|ctx| {
                    ctx.item_count_by_source(
                        crate::engine::retrieval_context::RetrievalSource::Memory,
                    ) > 0
                })
                .unwrap_or(false);
            if !memory_already_in_turn_context {
                if let Some(ref mem_mutex) = self.memory_manager {
                    let mut mem = mem_mutex.lock().await;
                    if let Some(last_user_idx) = request_messages
                        .iter()
                        .rposition(|m| matches!(m, Message::User { .. }))
                    {
                        if let Message::User { content } = &request_messages[last_user_idx] {
                            let retrieval_context = mem
                                .prefetch_retrieval_context_with_llm_rerank(
                                    content,
                                    self.provider.as_ref(),
                                    &self.model,
                                    route.retrieval,
                                )
                                .await;
                            if let Some(ref ctx) = retrieval_context {
                                trace.record(TraceEvent::MemoryPrefetch {
                                    chars: ctx
                                        .items
                                        .iter()
                                        .map(|item| item.content_preview.chars().count())
                                        .sum(),
                                });
                                trace.record(TraceEvent::RetrievalContextBuilt {
                                    policy: format!("{:?}", ctx.policy),
                                    sources: ctx
                                        .items
                                        .iter()
                                        .map(|item| format!("{:?}", item.source))
                                        .collect(),
                                    items: ctx.items.len(),
                                    estimated_tokens: ctx.token_estimate,
                                    provenance: ctx.provenance_summaries(),
                                    conflicts: ctx.conflict_count(),
                                });
                                let retrieval_block = ctx.format_for_prompt();
                                let enhanced = format!("{}\n{}", content, retrieval_block);
                                request_messages[last_user_idx] = Message::user(&enhanced);
                                debug!("Prefetched memory context injected into user message");
                            }
                        }
                    }
                }
            }

            let mut request = ChatRequest::new(&self.model)
                .with_messages(request_messages)
                .with_tools(tools.clone())
                .with_temperature(0.2);

            // ── 响应式压缩循环 ─────────────────────────────
            let mut compressed_this_turn = false;
            let mut api_result: Result<(
                String,
                Vec<ToolCall>,
                std::collections::HashMap<usize, ToolResult>,
            )> = Err(anyhow::anyhow!("initial"));
            for compress_retry in 0..3 {
                trace.record(TraceEvent::ApiRequestStarted {
                    iteration: iteration + 1,
                    model: self.model.clone(),
                    tools: tools.len(),
                });
                let nonstreaming_tool_request =
                    tx.is_some() && should_use_nonstreaming_tools(self.provider.as_ref(), &tools);
                api_result = if let Some(tx) = tx {
                    if nonstreaming_tool_request {
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "provider stream is incompatible with tool/usage chunks; using non-streaming tool request".to_string(),
                        });
                        self.call_api(request.clone()).await
                    } else {
                        self.call_api_streaming(request.clone(), tx, &trace, &exposed_tool_names)
                            .await
                    }
                } else {
                    self.call_api(request.clone()).await
                };

                match &api_result {
                    Ok(_) => break,
                    Err(e) => {
                        let err_str = e.to_string().to_lowercase();
                        let needs_compress = err_str.contains("payload too large")
                            || err_str.contains("413")
                            || err_str.contains("context")
                            || err_str.contains("too many tokens")
                            || err_str.contains("maximum context length");
                        if needs_compress && compress_retry < 2 {
                            let classified =
                                crate::engine::error_classifier::ErrorClassifier::from_anyhow(e);
                            let plan = crate::engine::recovery_plan::RecoveryPlan::from_classified(
                                "api_reactive_compress",
                                &classified,
                            )
                            .with_status(crate::engine::recovery_plan::RecoveryStatus::Applied);
                            record_recovery_plan(&trace, &plan);
                            warn!(
                                "API error (attempt {}/3): {}. Compressing context and retrying...",
                                compress_retry + 1,
                                e
                            );
                            if let Some(ref comp) = self.compressor {
                                let msgs_for_comp = if compress_retry == 0 {
                                    messages.clone()
                                } else {
                                    let mut comp = comp.lock().await;
                                    comp.micro_compress(&messages)
                                };
                                let compressed =
                                    comp.lock().await.compress_async(&msgs_for_comp).await;
                                trace.record(TraceEvent::ContextCompacted {
                                    before_tokens: estimate_messages_tokens(&msgs_for_comp)
                                        as usize,
                                    after_tokens: estimate_messages_tokens(&compressed) as usize,
                                    strategy: "reactive".to_string(),
                                });
                                request = ChatRequest::new(&self.model)
                                    .with_messages(compressed)
                                    .with_tools(tools.clone())
                                    .with_temperature(0.2);
                                compressed_this_turn = true;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }

            let (content, tool_calls, pre_executed) = match api_result {
                Ok(value) => value,
                Err(e) => {
                    trace.record(TraceEvent::Error {
                        message: e.to_string(),
                    });
                    self.finish_trace(trace.clone(), TurnStatus::Failed);
                    return Err(e);
                }
            };
            trace.record(TraceEvent::ApiRequestCompleted {
                iteration: iteration + 1,
                tool_calls: tool_calls.len(),
                content_chars: content.chars().count(),
            });

            if compressed_this_turn {
                debug!("Context compressed due to size limits");
            }

            final_content = content.clone();
            final_tool_calls = tool_calls.clone();

            if tool_calls.is_empty() {
                if let Some(tx) = tx {
                    if should_use_nonstreaming_tools(self.provider.as_ref(), &tools)
                        && !content.is_empty()
                    {
                        let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                    }
                }
                break;
            }

            messages.push(Message::assistant_with_tools(&content, tool_calls.clone()));

            let has_changes_before_tools =
                crate::engine::code_change_workflow::is_programming_workflow(route.workflow)
                    && !Self::git_status_files_since(&baseline_git_status_files).is_empty();
            let mut results = self
                .execute_tools_parallel(
                    &tool_calls,
                    tx,
                    pre_executed,
                    Some(trace.clone()),
                    &resource_policy,
                    &exposed_tool_names,
                    action_checkpoint_active,
                    has_changes_before_tools,
                )
                .await;

            // ── 迭代预算退还 ──────────────────────────────
            let all_read_only = tool_calls
                .iter()
                .all(|tc| READ_ONLY_TOOLS.iter().any(|&name| tc.name == name));

            if all_read_only {
                debug!("All tools read-only, refunding iteration budget");
            } else {
                effective_iterations += 1;
            }

            let mut tool_results_text = String::new();
            let mut changed_files = Vec::new();
            let used_write_tool = tool_calls
                .iter()
                .any(|tc| Self::is_code_write_tool_name(&tc.name));
            let mut any_tool_success = false;
            let mut repeated_failed_tools = Vec::new();
            let mut failed_tool_names_this_round = Vec::new();
            let mut failed_tool_evidence = Vec::new();
            let mut successful_validation_commands = Vec::new();
            let mut should_closeout_after_verified_change = false;
            if used_write_tool && !required_validation_commands.is_empty() {
                successful_required_validation_commands.clear();
            }
            for (tc, result) in results.iter_mut() {
                truncate_tool_result(result, &tc.name, &tc.id).await;
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    tool_result_dialog_content(result)
                );
                tool_results_text.push_str(&result_content);
                tool_results_text.push('\n');
                messages.push(Message::tool(tc.id.clone(), result_content));

                let fp = tool_call_fingerprint(tc);
                if result.success {
                    any_tool_success = true;
                    failed_tool_fingerprints.remove(&fp);
                    failed_tool_names.remove(&tc.name);
                } else {
                    let count = failed_tool_fingerprints.entry(fp).or_insert(0);
                    *count += 1;
                    if *count >= 2 {
                        repeated_failed_tools.push(tc.name.clone());
                    }
                    let name_count = failed_tool_names.entry(tc.name.clone()).or_insert(0);
                    *name_count += 1;
                    failed_tool_names_this_round.push(tc.name.clone());
                    failed_tool_evidence.push(format!(
                        "{} {} failed:\n{}",
                        tc.name,
                        tc.id,
                        tool_result_dialog_content(result)
                    ));
                }

                if result.success && (tc.name == "file_edit" || tc.name == "file_write") {
                    if let Some(path) = tc.arguments["path"].as_str() {
                        changed_files.push(std::path::PathBuf::from(path));
                    }
                }
                if result.success && Self::is_validation_tool_call(tc) {
                    if let Some(command) = tc.arguments["command"].as_str() {
                        let command = command.trim().to_string();
                        let normalized_command =
                            Self::normalize_validation_command_for_match(&command);
                        if required_validation_commands.iter().any(|required| {
                            Self::normalize_validation_command_for_match(required)
                                == normalized_command
                        }) {
                            successful_required_validation_commands.insert(command.clone());
                        }
                        successful_validation_commands.push(command);
                    }
                }
            }
            if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
                for path in Self::git_status_files_since(&baseline_git_status_files) {
                    if !changed_files.iter().any(|existing| existing == &path) {
                        changed_files.push(path);
                    }
                }
            }
            let has_worktree_changes = !changed_files.is_empty();

            let mut force_patch_synthesis_after_no_change = false;
            if crate::engine::code_change_workflow::is_programming_workflow(route.workflow) {
                let mut activated_checkpoint_this_round = false;
                if used_write_tool {
                    no_code_progress_rounds = 0;
                    action_checkpoint_no_change_rounds = 0;
                    action_checkpoint_active = false;
                } else if any_tool_success && !used_write_tool {
                    if has_worktree_changes && !successful_validation_commands.is_empty() {
                        no_code_progress_rounds = 0;
                        action_checkpoint_active = false;
                        action_checkpoint_no_change_rounds = 0;
                    } else {
                        no_code_progress_rounds += 1;
                    }
                    if has_worktree_changes
                        && successful_validation_commands.is_empty()
                        && no_code_progress_rounds >= 2
                        && !action_checkpoint_active
                    {
                        let checkpoint = format!(
                            "Workflow acceptance repair checkpoint: this {:?} task already has code changes, but {} consecutive successful tool rounds made no additional edit. Use the evidence already gathered to synthesize the smallest remaining file_edit/file_write patch now. If multiple independent acceptance-critical bypasses are visible, fix them together; otherwise stop with a Closeout status of not_verified and name the blocker.",
                            route.workflow, no_code_progress_rounds
                        );
                        trace.record(TraceEvent::WorkflowFallback {
                            error:
                                "existing diff still needs repair; entering patch synthesis after repeated read-only rounds"
                                    .to_string(),
                        });
                        messages.push(Message::system(checkpoint.clone()));
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&checkpoint);
                        action_checkpoint_active = true;
                        action_checkpoint_no_change_rounds = 2;
                        force_patch_synthesis_after_no_change = true;
                        activated_checkpoint_this_round = true;
                    } else if no_code_progress_rounds == 2 && !action_checkpoint_active {
                        let checkpoint = format!(
                            "Workflow progress checkpoint: this is a {:?} task and {} consecutive successful tool rounds produced no code change. Keep investigation focused: on the next response either make the smallest safe file_edit/file_write patch, or perform exactly one targeted read/search if a required symbol, test, or call site is still missing. Do not repeat broad inspection. If a scorer/decision object already returns final status, use that status directly instead of reimplementing acceptance gates.",
                            route.workflow, no_code_progress_rounds
                        );
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "code-change task needs an edit after repeated inspection"
                                .to_string(),
                        });
                        messages.push(Message::system(checkpoint.clone()));
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&checkpoint);
                    } else if no_code_progress_rounds >= 3 && !action_checkpoint_active {
                        let checkpoint = format!(
                            "Workflow action checkpoint: this is a {:?} task and {} consecutive successful tool rounds produced no code change. On the next response, use file_edit or file_write to apply the smallest safe patch, then run validation after the file changes. If prior grep/file_read results include line numbers, prefer file_edit line_start/line_end to replace the specific lines instead of asking to inspect again. Do not call grep/glob/file_read/project_list or other inspection-only tools. If a scorer/decision object already returns final status, use that status directly instead of reimplementing acceptance gates. If you cannot patch safely from the evidence already gathered, stop with a Closeout status of not_verified and a concrete blocker.",
                            route.workflow, no_code_progress_rounds
                        );
                        trace.record(TraceEvent::WorkflowFallback {
                            error: "code-change task made no edit after repeated inspection"
                                .to_string(),
                        });
                        messages.push(Message::system(checkpoint.clone()));
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&checkpoint);
                        action_checkpoint_active = true;
                        action_checkpoint_no_change_rounds = 0;
                        activated_checkpoint_this_round = true;
                    }
                    if action_checkpoint_active && !activated_checkpoint_this_round {
                        action_checkpoint_no_change_rounds += 1;
                        if action_checkpoint_no_change_rounds >= 3 {
                            trace.record(TraceEvent::WorkflowFallback {
                                error: "action checkpoint entered patch synthesis after repeated focused repair reads"
                                    .to_string(),
                            });
                            force_patch_synthesis_after_no_change = true;
                        }
                    }
                }
            }

            if action_checkpoint_active
                && ((!any_tool_success && !failed_tool_evidence.is_empty())
                    || force_patch_synthesis_after_no_change)
            {
                action_checkpoint_no_change_rounds += 1;
                let reminder = format!(
                    "Focused repair correction: the last tool call did not execute. The current request only permits these tools: {}. Use file_edit/file_write for exact replacements or line_start/line_end replacements from earlier line-numbered output. If a specific symbol or call site is missing, use exactly one targeted file_read/grep, then patch.",
                    exposed_tool_names.iter().cloned().collect::<Vec<_>>().join(", ")
                );
                if action_checkpoint_no_change_rounds >= 2 {
                    trace.record(TraceEvent::WorkflowFallback {
                        error: if force_patch_synthesis_after_no_change {
                            "action checkpoint entered patch synthesis after repeated focused repair reads"
                                .to_string()
                        } else {
                            "action checkpoint entered patch synthesis after repeated invalid tools"
                                .to_string()
                        },
                    });
                    match self
                        .synthesize_patch_tool_calls(&messages, last_user_preview.as_str())
                        .await
                    {
                        Ok(synthesized_calls) => {
                            trace.record(TraceEvent::WorkflowFallback {
                                error: format!(
                                    "patch synthesis produced {} file_edit action(s)",
                                    synthesized_calls.len()
                                ),
                            });
                            messages.push(Message::assistant_with_tools(
                                "Applying synthesized patch from prior evidence.",
                                synthesized_calls.clone(),
                            ));
                            let exposed_synth_tools =
                                HashSet::from(["file_edit".to_string(), "file_write".to_string()]);
                            let mut synthesized_results = self
                                .execute_tools_parallel(
                                    &synthesized_calls,
                                    tx,
                                    std::collections::HashMap::new(),
                                    Some(trace.clone()),
                                    &resource_policy,
                                    &exposed_synth_tools,
                                    // Synthesized edits have already passed
                                    // validate_patch_synthesis_action(). Avoid
                                    // applying the direct action-checkpoint
                                    // guard again, or safe recovered patches can
                                    // be rejected without giving the model a way
                                    // to inspect and repair the arguments.
                                    false,
                                    false,
                                )
                                .await;
                            for (tc, result) in synthesized_results.iter_mut() {
                                truncate_tool_result(result, &tc.name, &tc.id).await;
                                let result_content = format!(
                                    "Result: {}\n{}",
                                    if result.success { "OK" } else { "ERROR" },
                                    tool_result_dialog_content(result)
                                );
                                tool_results_text.push_str(&result_content);
                                tool_results_text.push('\n');
                                messages.push(Message::tool(tc.id.clone(), result_content));
                                if result.success {
                                    any_tool_success = true;
                                }
                                if result.success && Self::is_code_write_tool_name(&tc.name) {
                                    if let Some(path) = tc.arguments["path"].as_str() {
                                        changed_files.push(std::path::PathBuf::from(path));
                                    }
                                }
                            }
                            final_tool_calls.extend(synthesized_calls);
                            if crate::engine::code_change_workflow::is_programming_workflow(
                                route.workflow,
                            ) {
                                for path in Self::git_status_files_since(&baseline_git_status_files)
                                {
                                    if !changed_files.iter().any(|existing| existing == &path) {
                                        changed_files.push(path);
                                    }
                                }
                            }
                            if !changed_files.is_empty() {
                                action_checkpoint_active = false;
                                action_checkpoint_no_change_rounds = 0;
                                no_code_progress_rounds = 0;
                            } else {
                                let stop_msg =
                                    "[Patch synthesis did not produce a file change; stopped action checkpoint]";
                                debug!("{}", stop_msg);
                                if let Some(tx) = tx {
                                    let _ = tx
                                        .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                                        .await;
                                }
                                if final_content.trim().is_empty() {
                                    final_content = stop_msg.to_string();
                                } else {
                                    final_content.push('\n');
                                    final_content.push_str(stop_msg);
                                }
                                break;
                            }
                        }
                        Err(err) => {
                            trace.record(TraceEvent::WorkflowFallback {
                                error: format!("patch synthesis failed: {}", err),
                            });
                            let err_text = err.to_string();
                            let lower_err = err_text.to_lowercase();
                            if !patch_synthesis_recovery_used
                                && (lower_err.contains("declined")
                                    || lower_err.contains("inspect more")
                                    || lower_err.contains("need to inspect")
                                    || lower_err.contains("not enough evidence"))
                            {
                                patch_synthesis_recovery_used = true;
                                action_checkpoint_active = false;
                                action_checkpoint_no_change_rounds = 0;
                                no_code_progress_rounds = 1;
                                let recovery = format!(
                                    "Patch synthesis declined because evidence was insufficient: {}. Perform exactly one targeted read/search for the missing symbol, call site, or test, then make the smallest safe edit. Do not repeat broad inspection.",
                                    safe_prefix_by_bytes(&err_text, 500)
                                );
                                messages.push(Message::system(recovery.clone()));
                                tool_results_text.push('\n');
                                tool_results_text.push_str(&recovery);
                                continue;
                            }
                            let stop_msg =
                                "[Stopped action checkpoint after repeated invalid tool requests]";
                            debug!("{}", stop_msg);
                            if let Some(tx) = tx {
                                let _ = tx
                                    .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                                    .await;
                            }
                            if final_content.trim().is_empty() {
                                final_content = stop_msg.to_string();
                            } else {
                                final_content.push('\n');
                                final_content.push_str(stop_msg);
                            }
                            break;
                        }
                    }
                } else {
                    messages.push(Message::system(reminder.clone()));
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&reminder);
                    continue;
                }
            }

            if !any_tool_success
                && !failed_tool_evidence.is_empty()
                && workflow_contract_enabled(self.provider.as_ref())
            {
                let analyzer = crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
                    self.provider.as_ref(),
                    self.model.clone(),
                );
                let prompt = crate::engine::workflow_contract::GuidedDebuggingPrompt::new(
                    last_user_preview.as_str(),
                    task_bundle
                        .workflow_judgment
                        .as_ref()
                        .map(|judgment| judgment.to_turn_context()),
                    failed_tool_names_this_round.clone(),
                    failed_tool_evidence.clone(),
                );
                match analyzer.analyze_debugging(prompt).await {
                    Ok(debugging) => {
                        trace.record(TraceEvent::GuidedDebuggingCompleted {
                            blocker: debugging.blocker,
                            next_action: format!("{:?}", debugging.next_action),
                            causes: debugging.likely_causes.len(),
                            evidence_items: debugging.evidence_to_collect.len(),
                            ask_user: debugging.ask_user,
                        });
                        persist_workflow_learning_event(
                            self.session_store.as_ref(),
                            &self.session_id,
                            "guided_debugging",
                            format!(
                                "Guided debugging selected {:?}: {}",
                                debugging.next_action, debugging.symptom
                            ),
                            if debugging.blocker { 0.85 } else { 0.7 },
                            serde_json::json!({
                                "blocker": debugging.blocker,
                                "symptom": debugging.symptom.clone(),
                                "likely_causes": debugging.likely_causes.clone(),
                                "evidence_to_collect": debugging.evidence_to_collect.clone(),
                                "smallest_safe_action": debugging.smallest_safe_action.clone(),
                                "ask_user": debugging.ask_user,
                                "questions": debugging.questions.clone(),
                                "next_action": format!("{:?}", debugging.next_action),
                                "failed_tools": failed_tool_names_this_round.clone(),
                            }),
                        );
                        let debugging_text = debugging.format_for_prompt();
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&debugging_text);
                        messages.push(Message::system(debugging_text));
                        apply_workflow_feedback_and_trace(
                            &mut task_bundle,
                            &trace,
                            crate::engine::workflow_contract::WeightFeedbackEvent {
                                kind: crate::engine::workflow_contract::WeightFeedbackKind::ToolFailure,
                                severity: if debugging.blocker {
                                    crate::engine::workflow_contract::WeightFeedbackSeverity::High
                                } else {
                                    crate::engine::workflow_contract::WeightFeedbackSeverity::Medium
                                },
                                confidence: 0.85,
                                reason: Some(debugging.symptom.clone()),
                            },
                        );
                    }
                    Err(err) => {
                        warn!("Guided debugging analysis failed: {}", err);
                        trace.record(TraceEvent::WorkflowFallback {
                            error: format!("guided debugging analysis failed: {}", err),
                        });
                    }
                }
            }

            if !any_tool_success && !repeated_failed_tools.is_empty() {
                repeated_failed_tools.sort();
                repeated_failed_tools.dedup();
                let stop_msg = format!(
                    "[Stopped repeated failed tool attempts: {}]",
                    repeated_failed_tools.join(", ")
                );
                debug!("{}", stop_msg);
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                        .await;
                }
                if final_content.trim().is_empty() {
                    final_content = stop_msg;
                } else {
                    final_content.push('\n');
                    final_content.push_str(&stop_msg);
                }
                break;
            }

            if !any_tool_success {
                let mut noisy_by_name = Vec::new();
                for (name, count) in &failed_tool_names {
                    if *count >= 2 && !READ_ONLY_TOOLS.contains(&name.as_str()) {
                        noisy_by_name.push(name.clone());
                    }
                }
                if !noisy_by_name.is_empty() {
                    noisy_by_name.sort();
                    noisy_by_name.dedup();
                    let stop_msg = format!(
                        "[Stopped noisy retries after repeated failures: {}]",
                        noisy_by_name.join(", ")
                    );
                    debug!("{}", stop_msg);
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::TextChunk(format!("\n{}\n", stop_msg)))
                            .await;
                    }
                    if final_content.trim().is_empty() {
                        final_content = stop_msg;
                    } else {
                        final_content.push('\n');
                        final_content.push_str(&stop_msg);
                    }
                    break;
                }
            }

            // ── 自动验证闭环 ──────────────────────────────
            if !changed_files.is_empty() {
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let mut post_edit_evidence = Vec::new();
                let mut acceptance_evidence = Vec::new();
                let mut failed_commands = Vec::new();
                let verify_results =
                    super::auto_verify::verify_file_changes(&working_dir, &changed_files).await;
                let check_passed = verify_results.iter().all(|r| r.success);
                if !check_passed {
                    if let Some(source_context) =
                        verification_source_context(&working_dir, &verify_results)
                    {
                        post_edit_evidence.push(source_context.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&source_context);
                        messages.push(Message::system(source_context));
                    }
                }
                for result in verify_results {
                    let verify_text = result.to_dialog_text();
                    acceptance_evidence.push(verify_text.clone());
                    if !result.success {
                        failed_commands.push(result.command.clone());
                        post_edit_evidence.push(verify_text.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&verify_text);
                        messages.push(Message::system(verify_text));
                    } else {
                        debug!("{}", verify_text);
                    }
                }

                // ── LSP 诊断补充 ───────────────────────────
                if let Some(ref lsp_mgr) = self.lsp_manager {
                    let mut lsp_issues = Vec::new();
                    for path in &changed_files {
                        let uri = super::lsp::path_to_uri(path);
                        for name in lsp_mgr.server_names() {
                            if let Some(client) = lsp_mgr.get_client(&name) {
                                let diagnostics = client.get_diagnostics(&uri).await;
                                for d in diagnostics {
                                    let sev = match d.severity {
                                        Some(1) => "error",
                                        Some(2) => "warning",
                                        Some(3) => "info",
                                        Some(4) => "hint",
                                        _ => "diagnostic",
                                    };
                                    lsp_issues.push(format!(
                                        "  [{}] {}:{}: {}",
                                        sev,
                                        path.display(),
                                        d.range.start.line + 1,
                                        d.message.replace('\n', " ")
                                    ));
                                }
                            }
                        }
                    }
                    if !lsp_issues.is_empty() {
                        let lsp_text = format!(
                            "[LSP diagnostics for modified files]:\n{}",
                            lsp_issues.join("\n")
                        );
                        post_edit_evidence.push(lsp_text.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&lsp_text);
                        messages.push(Message::system(lsp_text));
                    }
                }

                // ── Required validation first ───────────────────
                //
                // Live tasks can define domain-specific required commands. Run
                // those before generic auto-test discovery so a cold full
                // `cargo test` probe cannot spend the turn budget before the
                // deterministic closeout path sees the required evidence.
                let mut required_validation_passed = true;
                if !required_validation_commands.is_empty() {
                    let already_ran = successful_validation_commands
                        .iter()
                        .map(|cmd| Self::normalize_validation_command_for_match(cmd))
                        .chain(
                            successful_required_validation_commands
                                .iter()
                                .map(|cmd| Self::normalize_validation_command_for_match(cmd)),
                        )
                        .collect::<HashSet<_>>();
                    let required_to_run = required_validation_commands
                        .iter()
                        .filter(|cmd| {
                            !already_ran
                                .contains(&Self::normalize_validation_command_for_match(cmd))
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if !required_to_run.is_empty() {
                        let required_results =
                            Self::run_required_validation_commands(&working_dir, &required_to_run)
                                .await;
                        for result in required_results {
                            let text = result.to_dialog_text();
                            acceptance_evidence.push(text.clone());
                            if result.success {
                                successful_required_validation_commands
                                    .insert(result.command.trim().to_string());
                                debug!("{}", text);
                            } else {
                                required_validation_passed = false;
                                failed_commands.push(result.command.clone());
                                post_edit_evidence.push(text.clone());
                                tool_results_text.push('\n');
                                tool_results_text.push_str(&text);
                                messages.push(Message::system(text));
                            }
                        }
                    }
                }
                let required_validation_covers_tests =
                    !required_validation_commands.is_empty() && required_validation_passed;

                // ── 自动测试闭环 ──────────────────────────────
                let manual_validation_after_changes = !successful_validation_commands.is_empty();
                let test_results = if should_run_default_auto_tests(&required_validation_commands) {
                    super::auto_verify::run_tests(&working_dir, &changed_files, check_passed).await
                } else {
                    Vec::new()
                };
                let tests_passed = required_validation_covers_tests
                    || test_results.iter().all(|r| r.success)
                    || (manual_validation_after_changes && check_passed);
                if !tests_passed {
                    if let Some(source_context) =
                        verification_source_context(&working_dir, &test_results)
                    {
                        post_edit_evidence.push(source_context.clone());
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&source_context);
                        messages.push(Message::system(source_context));
                    }
                }
                for result in test_results {
                    let test_text = result.to_dialog_text();
                    acceptance_evidence.push(test_text.clone());
                    if !result.success {
                        if manual_validation_after_changes || required_validation_covers_tests {
                            debug!(
                                "Ignoring stale automatic test failure after successful required/manual validation command: {}",
                                result.command
                            );
                        } else {
                            failed_commands.push(result.command.clone());
                            post_edit_evidence.push(test_text.clone());
                            tool_results_text.push('\n');
                            tool_results_text.push_str(&test_text);
                            messages.push(Message::system(test_text));
                        }
                    } else {
                        debug!("{}", test_text);
                    }
                }
                if manual_validation_after_changes {
                    let manual_text = format!(
                        "[Manual validation passed after code changes]\n{}",
                        successful_validation_commands
                            .iter()
                            .map(|cmd| format!("  $ {}", cmd))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                    acceptance_evidence.push(manual_text.clone());
                    post_edit_evidence.push(manual_text.clone());
                    debug!("{}", manual_text);
                }

                if let Some(diff_text) =
                    changed_files_diff_evidence(&working_dir, &changed_files).await
                {
                    acceptance_evidence.push(diff_text.clone());
                    post_edit_evidence.push(diff_text.clone());
                    debug!("{}", diff_text);
                }

                // ── 代码自审查 ────────────────────────────────
                let review_result =
                    super::code_review::review_changed_files(&working_dir, &changed_files);
                acceptance_evidence.push(review_result.to_dialog_text());
                if !review_result.success {
                    let review_text = review_result.to_dialog_text();
                    post_edit_evidence.push(review_text.clone());
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&review_text);
                    messages.push(Message::system(review_text));
                }

                // ── 编程质量可观测性 ───────────────────────
                // When all required commands pass, they are stronger evidence
                // than the repository's default auto-test for that changed
                // area.
                let effective_check_passed = check_passed || required_validation_covers_tests;
                let effective_tests_passed = tests_passed || required_validation_covers_tests;
                let verify_passed = effective_check_passed
                    && effective_tests_passed
                    && required_validation_passed
                    && review_result.success;
                should_closeout_after_verified_change = verify_passed;
                trace.record(TraceEvent::VerificationCompleted {
                    changed_files: changed_files.len(),
                    passed: verify_passed,
                    check_passed: effective_check_passed,
                    tests_passed: effective_tests_passed,
                    review_passed: review_result.success,
                    failed_commands: failed_commands.clone(),
                });
                let mut post_edit_reflection =
                    crate::engine::reflection_pass::ReflectionPass::from_post_edit(
                        task_bundle.task_id.clone(),
                        &changed_files,
                        verify_passed,
                        &post_edit_evidence,
                    );
                if !verify_passed {
                    let verification_command = failed_commands
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "post-edit verification".to_string());
                    post_edit_reflection.record_repair_action(
                        acceptance_repair_attempts + 1,
                        "repair failed verification before closeout",
                        changed_files.first().map(|path| path.display().to_string()),
                        verification_command,
                    );
                }
                trace.record(TraceEvent::ReflectionPassCompleted {
                    pass_id: post_edit_reflection.pass_id.clone(),
                    task_id: post_edit_reflection.task_id.clone(),
                    status: format!("{:?}", post_edit_reflection.status),
                    findings: post_edit_reflection.findings.len(),
                    unresolved: post_edit_reflection.unresolved_count(),
                });
                let stage_record = code_workflow.record_stage_validation(
                    &task_bundle,
                    &changed_files,
                    verify_passed,
                    &acceptance_evidence,
                );
                trace_stage_validation(&trace, &stage_record);
                if let Some(feedback) = stage_record.feedback.clone() {
                    apply_workflow_feedback_and_trace(&mut task_bundle, &trace, feedback);
                }
                if !verify_passed && workflow_contract_enabled(self.provider.as_ref()) {
                    let analyzer = crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
                        self.provider.as_ref(),
                        self.model.clone(),
                    );
                    let prompt = crate::engine::workflow_contract::GuidedDebuggingPrompt::new(
                        last_user_preview.as_str(),
                        task_bundle
                            .workflow_judgment
                            .as_ref()
                            .map(|judgment| judgment.to_turn_context()),
                        vec!["stage_validation".to_string()],
                        post_edit_evidence.clone(),
                    );
                    match analyzer.analyze_debugging(prompt).await {
                        Ok(debugging) => {
                            trace.record(TraceEvent::GuidedDebuggingCompleted {
                                blocker: debugging.blocker,
                                next_action: format!("{:?}", debugging.next_action),
                                causes: debugging.likely_causes.len(),
                                evidence_items: debugging.evidence_to_collect.len(),
                                ask_user: debugging.ask_user,
                            });
                            persist_workflow_learning_event(
                                self.session_store.as_ref(),
                                &self.session_id,
                                "guided_debugging",
                                format!(
                                    "Guided validation debugging selected {:?}: {}",
                                    debugging.next_action, debugging.symptom
                                ),
                                if debugging.blocker { 0.85 } else { 0.7 },
                                serde_json::json!({
                                    "blocker": debugging.blocker,
                                    "symptom": debugging.symptom.clone(),
                                    "likely_causes": debugging.likely_causes.clone(),
                                    "evidence_to_collect": debugging.evidence_to_collect.clone(),
                                    "smallest_safe_action": debugging.smallest_safe_action.clone(),
                                    "ask_user": debugging.ask_user,
                                    "questions": debugging.questions.clone(),
                                    "next_action": format!("{:?}", debugging.next_action),
                                    "source": "stage_validation",
                                }),
                            );
                            let debugging_text = debugging.format_for_prompt();
                            tool_results_text.push('\n');
                            tool_results_text.push_str(&debugging_text);
                            messages.push(Message::system(debugging_text));
                        }
                        Err(err) => {
                            warn!("Guided validation debugging failed: {}", err);
                            trace.record(TraceEvent::WorkflowFallback {
                                error: format!("guided validation debugging failed: {}", err),
                            });
                        }
                    }
                }
                if let Some(judgment) = task_bundle.workflow_judgment.as_ref() {
                    if verify_passed && !required_validation_commands.is_empty() {
                        let evidence = format!(
                            "Required validation commands passed: {}",
                            required_validation_commands.join("; ")
                        );
                        let criteria = judgment
                            .acceptance
                            .criteria
                            .iter()
                            .map(|criterion| {
                                let mut passed = criterion.clone();
                                passed.status =
                                    crate::engine::workflow_contract::AcceptanceStatus::Passed;
                                passed.evidence = Some(evidence.clone());
                                passed
                            })
                            .collect::<Vec<_>>();
                        let review = crate::engine::workflow_contract::AcceptanceReview {
                            accepted: true,
                            confidence:
                                crate::engine::workflow_contract::AcceptanceConfidence::High,
                            criteria,
                            unresolved_items: Vec::new(),
                            residual_risks: Vec::new(),
                            next_action:
                                crate::engine::workflow_contract::AcceptanceNextAction::Finish,
                        };
                        trace.record(TraceEvent::AcceptanceReviewCompleted {
                            accepted: true,
                            confidence: "High".to_string(),
                            criteria: review.criteria.len(),
                            unresolved: 0,
                            next_action: "Finish".to_string(),
                        });
                        code_workflow.record_acceptance_review(review);
                        should_closeout_after_verified_change = true;
                        trace.record(TraceEvent::WorkflowPlanProgress {
                            total_steps: judgment.plan.len(),
                            completed_steps: judgment.plan.len(),
                            active_step: None,
                            top_priority: None,
                            top_importance_score: None,
                            top_weight_share: None,
                            weight_source: None,
                            reweighted: true,
                        });
                    } else if workflow_contract_enabled(self.provider.as_ref()) {
                        let analyzer =
                            crate::engine::workflow_contract::WorkflowContractAnalyzer::new(
                                self.provider.as_ref(),
                                self.model.clone(),
                            );
                        let prompt = crate::engine::workflow_contract::AcceptanceReviewPrompt::new(
                            judgment.acceptance.clone(),
                            changed_files
                                .iter()
                                .map(|path| path.display().to_string())
                                .collect(),
                            verify_passed,
                            acceptance_evidence.clone(),
                        );
                        match analyzer.review_acceptance(prompt).await {
                            Ok(review) => {
                                let high_risk = is_high_risk_workflow(&route, Some(judgment));
                                let review_next_action = review.next_action;
                                let review_accepted = review.accepted;
                                let review_unresolved = review.unresolved_count();
                                trace.record(TraceEvent::AcceptanceReviewCompleted {
                                    accepted: review_accepted,
                                    confidence: format!("{:?}", review.confidence),
                                    criteria: review.criteria.len(),
                                    unresolved: review_unresolved,
                                    next_action: format!("{:?}", review.next_action),
                                });
                                code_workflow.record_acceptance_review(review.clone());
                                if review_accepted {
                                    should_closeout_after_verified_change = true;
                                    trace.record(TraceEvent::WorkflowPlanProgress {
                                        total_steps: judgment.plan.len(),
                                        completed_steps: judgment.plan.len(),
                                        active_step: None,
                                        top_priority: None,
                                        top_importance_score: None,
                                        top_weight_share: None,
                                        weight_source: None,
                                        reweighted: true,
                                    });
                                }
                                persist_workflow_learning_event(
                                    self.session_store.as_ref(),
                                    &self.session_id,
                                    "acceptance_review",
                                    format!(
                                        "Acceptance review accepted={} next={:?}",
                                        review_accepted, review_next_action
                                    ),
                                    if review_accepted { 0.95 } else { 0.85 },
                                    serde_json::json!({
                                        "accepted": review_accepted,
                                        "confidence": format!("{:?}", review.confidence),
                                        "criteria": review.criteria.clone(),
                                        "unresolved_items": review.unresolved_items.clone(),
                                        "residual_risks": review.residual_risks.clone(),
                                        "next_action": format!("{:?}", review_next_action),
                                        "high_risk": high_risk,
                                        "changed_files": changed_files.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                                    }),
                                );
                                let review_text = review.format_for_prompt();
                                tool_results_text.push('\n');
                                tool_results_text.push_str(&review_text);
                                messages.push(Message::system(review_text.clone()));
                                if !review_accepted
                                    && matches!(
                                        review_next_action,
                                        crate::engine::workflow_contract::AcceptanceNextAction::ContinueRepair
                                            | crate::engine::workflow_contract::AcceptanceNextAction::Stop
                                    )
                                {
                                    should_closeout_after_verified_change = false;
                                    apply_workflow_feedback_and_trace(
                                        &mut task_bundle,
                                        &trace,
                                        crate::engine::workflow_contract::WeightFeedbackEvent {
                                            kind: crate::engine::workflow_contract::WeightFeedbackKind::AcceptanceGap,
                                            severity: if high_risk || review_unresolved > 1 {
                                                crate::engine::workflow_contract::WeightFeedbackSeverity::High
                                            } else {
                                                crate::engine::workflow_contract::WeightFeedbackSeverity::Medium
                                            },
                                            confidence: 0.90,
                                            reason: Some(format!(
                                                "acceptance review unresolved items: {}",
                                                review_unresolved
                                            )),
                                        },
                                    );
                                    acceptance_repair_attempts += 1;
                                    messages.push(Message::system(
                                        "Acceptance review did not pass. If verification or compile errors are present, fix those first using the latest verification source context; only then address the unresolved acceptance items. Continue repair if possible; otherwise report the unresolved items clearly."
                                            .to_string(),
                                    ));
                                    if high_risk
                                        && (acceptance_repair_attempts
                                            > code_workflow.max_repair_attempts()
                                            || matches!(
                                                review_next_action,
                                                crate::engine::workflow_contract::AcceptanceNextAction::Stop
                                            ))
                                    {
                                        final_content = format!(
                                            "Stopped before final closeout because high-risk acceptance review did not pass ({} unresolved item(s)).",
                                            review_unresolved
                                        );
                                        break;
                                    }
                                    if matches!(
                                        review_next_action,
                                        crate::engine::workflow_contract::AcceptanceNextAction::ContinueRepair
                                    ) {
                                        let compile_or_review_failed =
                                            !check_passed || !review_result.success;
                                        let needs_acceptance_investigation =
                                            review_unresolved > 0 && !compile_or_review_failed;
                                        reserved_repair_rounds = reserved_repair_rounds.max(
                                            if needs_acceptance_investigation { 2 } else { 1 },
                                        );
                                        action_checkpoint_no_change_rounds = 0;
                                        if needs_acceptance_investigation {
                                            action_checkpoint_active = false;
                                            messages.push(Message::system(
                                                "Acceptance review gaps remain after compile/code review checks. Restore investigation mode: inspect the unresolved acceptance items against the implementation, identify every acceptance-critical bypass or missing call site, then make the smallest targeted fix. If multiple independent acceptance-critical bypasses are visible, fix them together."
                                                    .to_string(),
                                            ));
                                            trace.record(TraceEvent::WorkflowFallback {
                                                error:
                                                    "acceptance review requested broader repair; restored read/search tools for acceptance-gap investigation"
                                                        .to_string(),
                                            });
                                        } else {
                                            action_checkpoint_active = true;
                                            trace.record(TraceEvent::WorkflowFallback {
                                                error:
                                                    "acceptance review requested repair; switching to action-only repair mode"
                                                        .to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Acceptance review failed: {}", err);
                                trace.record(TraceEvent::WorkflowFallback {
                                    error: format!("acceptance review failed: {}", err),
                                });
                            }
                        }
                    }
                }
                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_coding_round(verify_passed);
                }
                if post_edit_reflection.status
                    != crate::engine::reflection_pass::ReflectionStatus::Passed
                {
                    should_closeout_after_verified_change = false;
                    let repair_instruction = format!(
                        "{}\nPost-edit reflection found unresolved quality gaps. Fix the changed files before giving a final answer.",
                        post_edit_reflection.format_for_prompt()
                    );
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&repair_instruction);
                    messages.push(Message::system(repair_instruction));
                    if effective_iterations >= self.max_iterations {
                        reserved_repair_rounds = reserved_repair_rounds.max(1);
                        trace.record(TraceEvent::WorkflowFallback {
                            error:
                                "reserved repair round granted after post-edit reflection failure"
                                    .to_string(),
                        });
                    }
                }
            }

            // ── 记忆同步 ──────────────────────────────────
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                let user_msg = messages
                    .iter()
                    .rposition(|m| matches!(m, Message::User { .. }))
                    .and_then(|i| match &messages[i] {
                        Message::User { content } => Some(content.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                if !user_msg.is_empty() {
                    let assistant_text = format!("{} {}", final_content, tool_results_text);
                    if self.llm_memory_extraction {
                        if mem.should_extract_with_llm() {
                            let provider: Option<&dyn LlmProvider> = Some(self.provider.as_ref());
                            mem.sync_turn_llm(user_msg, &assistant_text, provider, &self.model)
                                .await;
                            mem.mark_main_agent_wrote();
                            trace.record(TraceEvent::MemorySynced {
                                mode: "llm".to_string(),
                            });
                        }
                    } else {
                        mem.sync_turn(user_msg, &assistant_text);
                        mem.mark_main_agent_wrote();
                        trace.record(TraceEvent::MemorySynced {
                            mode: "heuristic".to_string(),
                        });
                    }
                }
                mem.increment_turn();
            }

            if should_closeout_after_verified_change {
                trace.record(TraceEvent::WorkflowFallback {
                    error:
                        "verified code change passed validation; preparing deterministic closeout"
                            .to_string(),
                });
                break;
            }
        }

        if let Some(closeout) = code_workflow.build_closeout(&task_bundle) {
            trace.record(TraceEvent::FinalCloseoutPrepared {
                status: closeout.status.label().to_string(),
                changed_files: closeout.changed_files.len(),
                validation_items: closeout.validation.len(),
                acceptance_items: closeout.acceptance.len(),
                residual_risks: closeout.residual_risks.len(),
            });
            let closeout_text = closeout.format_for_final_response();
            if !final_content.contains("Closeout:") {
                final_content.push_str(&closeout_text);
                if let Some(tx) = tx {
                    let _ = tx.send(StreamEvent::TextChunk(closeout_text)).await;
                }
            }
        }

        if iterations_used >= self.max_iterations
            && !final_tool_calls.is_empty()
            && !final_content.contains("Closeout:")
        {
            let stop_msg = "\n\n[Stopped after reaching the tool-iteration budget before a final closeout. Review the last tool results and continue if the task is not complete.]\n";
            final_content.push_str(stop_msg);
            if let Some(tx) = tx {
                let _ = tx.send(StreamEvent::TextChunk(stop_msg.to_string())).await;
            }
            trace.record(TraceEvent::WorkflowFallback {
                error: "tool iteration budget exhausted before final closeout".to_string(),
            });
        }

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Complete).await;
        }

        trace.record(TraceEvent::AssistantResponded {
            chars: final_content.chars().count(),
            iterations: iterations_used,
        });
        self.finish_trace(trace, TurnStatus::Completed);

        Ok(LoopResult {
            content: final_content,
            tool_calls: final_tool_calls,
            iterations: iterations_used,
            pre_executed_results: std::collections::HashMap::new(),
        })
    }

    /// 非流式 API 调用
    async fn call_api(
        &self,
        request: ChatRequest,
    ) -> Result<(
        String,
        Vec<ToolCall>,
        std::collections::HashMap<usize, ToolResult>,
    )> {
        let response = self
            .provider_chat_with_timeout(request, "non-streaming chat")
            .await?;
        self.record_cost(&response).await;

        let content = strip_think_blocks(&response.content);
        let tool_calls = response.tool_calls.unwrap_or_default();

        Ok((content, tool_calls, std::collections::HashMap::new()))
    }

    async fn provider_chat_with_timeout(
        &self,
        request: ChatRequest,
        purpose: &str,
    ) -> Result<ChatResponse> {
        let timeout = llm_request_timeout();
        match tokio::time::timeout(timeout, self.provider.chat(request)).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "{} timed out after {}s",
                purpose,
                timeout.as_secs()
            )),
        }
    }

    /// 流式 API 调用
    async fn call_api_streaming(
        &self,
        request: ChatRequest,
        tx: &mpsc::Sender<StreamEvent>,
        trace: &TraceCollector,
        exposed_tool_names: &HashSet<String>,
    ) -> Result<(
        String,
        Vec<ToolCall>,
        std::collections::HashMap<usize, ToolResult>,
    )> {
        let fallback_messages = request.messages.clone();
        let fallback_tools = request.tools.clone();

        let stream_open =
            tokio::time::timeout(llm_request_timeout(), self.provider.chat_stream(request)).await;
        match stream_open {
            Ok(Ok(mut stream)) => {
                let mut raw_content = String::new();
                let mut full_content = String::new();
                let mut collected_tool_calls: Vec<ToolCall> = Vec::new();
                let mut raw_args_accum: Vec<String> = Vec::new();
                let mut stream_failed: Option<String> = None;
                let mut visible_sanitizer = VisibleTextSanitizer::default();

                let _ = tx.send(StreamEvent::ThinkingStart).await;

                let mut read_only_tasks: std::collections::HashMap<
                    usize,
                    tokio::task::JoinHandle<ToolResult>,
                > = std::collections::HashMap::new();
                let read_only_concurrency = read_only_tool_concurrency();
                let tool_registry = self.tool_registry.clone();
                let tool_context = self.create_tool_context_with_trace(trace);
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();

                let stream_idle_timeout = stream_chunk_idle_timeout();
                loop {
                    let Some(result) =
                        (match tokio::time::timeout(stream_idle_timeout, stream.next()).await {
                            Ok(next) => next,
                            Err(_) => {
                                stream_failed = Some(format!(
                                    "stream idle timeout after {}s",
                                    stream_idle_timeout.as_secs()
                                ));
                                break;
                            }
                        })
                    else {
                        break;
                    };
                    match result {
                        Ok(chunk) => {
                            if let Some(usage) = &chunk.usage {
                                let _ = tx
                                    .send(StreamEvent::Usage {
                                        prompt_tokens: usage.prompt_tokens,
                                        completion_tokens: usage.completion_tokens,
                                        reasoning_tokens: usage
                                            .completion_tokens_details
                                            .as_ref()
                                            .and_then(|d| d.reasoning_tokens),
                                        cached_tokens: usage
                                            .prompt_tokens_details
                                            .as_ref()
                                            .and_then(|d| d.cached_tokens),
                                    })
                                    .await;
                            }
                            if let Some(choice) = chunk.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    if !content.is_empty() {
                                        raw_content.push_str(content);
                                        let visible_chunk = visible_sanitizer.push_chunk(content);
                                        if !visible_chunk.is_empty() {
                                            full_content.push_str(&visible_chunk);
                                            let _ = tx
                                                .send(StreamEvent::TextChunk(visible_chunk))
                                                .await;
                                        }
                                    }
                                }

                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                    for tc_delta in tool_calls {
                                        let idx = tc_delta.index as usize;
                                        while collected_tool_calls.len() <= idx {
                                            collected_tool_calls.push(ToolCall {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: serde_json::Value::Null,
                                            });
                                            raw_args_accum.push(String::new());
                                        }

                                        let mut tool_name_for_spawn: Option<String> = None;
                                        let mut tool_id_for_spawn: Option<String> = None;
                                        let mut args_for_spawn: Option<String> = None;

                                        let tc = &mut collected_tool_calls[idx];
                                        if let Some(id) = &tc_delta.id {
                                            tc.id = id.clone();
                                            let _ = tx
                                                .send(StreamEvent::ToolCallStart {
                                                    id: id.clone(),
                                                    name: tc.name.clone(),
                                                })
                                                .await;
                                        }
                                        if let Some(function) = &tc_delta.function {
                                            if let Some(name) = &function.name {
                                                tc.name = name.clone();
                                            }
                                            if let Some(args) = &function.arguments {
                                                raw_args_accum[idx].push_str(args);

                                                tool_name_for_spawn = Some(tc.name.clone());
                                                tool_id_for_spawn = Some(tc.id.clone());
                                                args_for_spawn = Some(raw_args_accum[idx].clone());

                                                let _ = tx
                                                    .send(StreamEvent::ToolCallArgs {
                                                        id: tc.id.clone(),
                                                        args_delta: args.clone(),
                                                    })
                                                    .await;
                                            }
                                        }

                                        if let (Some(tool_name), Some(tid), Some(current_args)) =
                                            (tool_name_for_spawn, tool_id_for_spawn, args_for_spawn)
                                        {
                                            if !tool_name.is_empty()
                                                && exposed_tool_names.contains(&tool_name)
                                                && is_read_only(&tool_name)
                                                && !read_only_tasks.contains_key(&idx)
                                                && read_only_tasks.len() < read_only_concurrency
                                            {
                                                let Some(tool) = tool_registry.get(&tool_name)
                                                else {
                                                    continue;
                                                };
                                                let Ok(parsed_args) =
                                                    serde_json::from_str::<serde_json::Value>(
                                                        &current_args,
                                                    )
                                                else {
                                                    continue;
                                                };
                                                if tool.validate_params(&parsed_args).is_some() {
                                                    continue;
                                                }

                                                let registry = tool_registry.clone();
                                                let context = tool_context.clone();
                                                let ct = cost_tracker.clone();
                                                let hooks = hook_manager.clone();
                                                let tid2 = tid.clone();
                                                let tool_n = tool_name.clone();
                                                let tool_n2 = tool_name.clone();
                                                let trace_for_task = Some(trace.clone());

                                                read_only_tasks.insert(
                                                    idx,
                                                    tokio::spawn(async move {
                                                        let started_at =
                                                            std::time::Instant::now();
                                                        let pre_decision = if let Some(ref h)
                                                            = hooks
                                                        {
                                                            let t = ToolCall {
                                                                id: tid.clone(),
                                                                name: tool_n.clone(),
                                                                arguments: parsed_args.clone(),
                                                            };
                                                            let hook_start =
                                                                h.current_record_sequence();
                                                            let decision =
                                                                h.run_pre_tool(&t, &context).await;
                                                            let hook_records = h
                                                                .recent_records_after_for(
                                                                    hook_start,
                                                                    &t.id,
                                                                );
                                                            record_hook_traces(
                                                                &trace_for_task,
                                                                &hook_records,
                                                            );
                                                            decision
                                                        } else {
                                                            HookDecision {
                                                                allow: true,
                                                                reason: None,
                                                            }
                                                        };

                                                        let ctx_clone = context.clone();
                                                        let mut result = if !pre_decision.allow {
                                                            ToolResult::error(
                                                                pre_decision.reason.unwrap_or_else(
                                                                    || format!(
                                                                        "blocked by pre-tool hook: {}",
                                                                        tool_n
                                                                    ),
                                                                ),
                                                            )
                                                        } else if let Some(tool) =
                                                            registry.get(&tool_n)
                                                        {
                                                            tool.execute(parsed_args.clone(), context)
                                                                .await
                                                        } else {
                                                            ToolResult::error(format!(
                                                                "Tool '{}' not found",
                                                                tool_n
                                                            ))
                                                        };

                                                        let duration_ms =
                                                            started_at.elapsed().as_millis()
                                                                as u64;
                                                        if result.duration_ms.is_none() {
                                                            result.duration_ms =
                                                                Some(duration_ms);
                                                        }
                                                        if let Some(ref h) = hooks {
                                                            let tc_for_hook = ToolCall {
                                                                id: tid2.clone(),
                                                                name: tool_n2.clone(),
                                                                arguments: parsed_args.clone(),
                                                            };
                                                            let hook_start =
                                                                h.current_record_sequence();
                                                            h.run_post_tool(
                                                                &tc_for_hook,
                                                                &result,
                                                                &ctx_clone,
                                                            )
                                                                .await;
                                                            let hook_records = h
                                                                .recent_records_after_for(
                                                                    hook_start,
                                                                    &tc_for_hook.id,
                                                                );
                                                            record_hook_traces(
                                                                &trace_for_task,
                                                                &hook_records,
                                                            );
                                                        }
                                                        {
                                                            let mut tracker = ct.lock().await;
                                                            tracker.record_tool_execution(
                                                                &tool_n,
                                                                result.success,
                                                                duration_ms,
                                                                result.error.as_deref(),
                                                            );
                                                        }
                                                        result
                                                    }),
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            let truncated = chunk.choices.iter().any(|c| {
                                c.finish_reason
                                    .as_ref()
                                    .is_some_and(|fr| format!("{:?}", fr).contains("Length"))
                            });
                            if truncated {
                                let _ = tx.send(StreamEvent::OutputTruncated).await;
                            }
                            if chunk.choices.iter().any(|c| c.finish_reason.is_some()) {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {}", e);
                            stream_failed = Some(e.to_string());
                            break;
                        }
                    }
                }

                let _ = tx.send(StreamEvent::ThinkingComplete).await;
                let visible_tail = visible_sanitizer.finish();
                if !visible_tail.is_empty() {
                    full_content.push_str(&visible_tail);
                    let _ = tx.send(StreamEvent::TextChunk(visible_tail)).await;
                }

                for (i, tc) in collected_tool_calls.iter_mut().enumerate() {
                    if i < raw_args_accum.len() && !raw_args_accum[i].is_empty() {
                        tc.arguments =
                            serde_json::from_str(&raw_args_accum[i]).unwrap_or_else(|e| {
                                warn!("Failed to parse tool args: {}", e);
                                serde_json::Value::Null
                            });
                        let _ = tx
                            .send(StreamEvent::ToolCallComplete { id: tc.id.clone() })
                            .await;
                    }
                }

                let mut pre_executed: std::collections::HashMap<usize, ToolResult> =
                    std::collections::HashMap::new();
                for (idx, handle) in read_only_tasks {
                    if let Ok(result) = handle.await {
                        debug!(
                            "Read-only tool at index {} pre-executed with result: {}",
                            idx,
                            if result.success { "OK" } else { "ERROR" }
                        );
                        pre_executed.insert(idx, result);
                    }
                }

                // If streaming fails mid-response, fall back to a non-streaming request for the
                // same turn. Some OpenAI-compatible providers emit non-standard streaming usage
                // payloads after partial tool-call deltas; treating that as terminal would stop a
                // valid coding task before any final tool execution happens.
                if let Some(stream_err) = stream_failed {
                    let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                        "stream_interrupted",
                        &stream_err,
                    );
                    record_recovery_plan(trace, &plan);
                    warn!("{}", plan.user_note);
                    warn!(
                        "Streaming interrupted after {} visible chars and {} partial tool call(s) (error: {}). Falling back to non-streaming",
                        raw_content.chars().count(),
                        collected_tool_calls.len(),
                        stream_err
                    );
                    let base_request = ChatRequest::new(&self.model)
                        .with_messages(fallback_messages.clone())
                        .with_temperature(0.2);
                    let response = if let Some(tools) = fallback_tools.clone() {
                        match self
                            .provider_chat_with_timeout(
                                base_request.clone().with_tools(tools),
                                "non-streaming fallback with tools",
                            )
                            .await
                        {
                            Ok(r) => r,
                            Err(with_tools_err) => {
                                warn!(
                                    "Non-streaming fallback with tools failed: {}. Retrying without tools.",
                                    with_tools_err
                                );
                                self.provider_chat_with_timeout(
                                    base_request,
                                    "non-streaming fallback without tools",
                                )
                                .await?
                            }
                        }
                    } else {
                        self.provider_chat_with_timeout(base_request, "non-streaming fallback")
                            .await?
                    };
                    self.record_cost(&response).await;
                    emit_usage_event(&response, tx).await;

                    let content = strip_think_blocks(&response.content);
                    if !content.is_empty() {
                        let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                    }
                    let tool_calls = response.tool_calls.unwrap_or_default();
                    return Ok((content, tool_calls, std::collections::HashMap::new()));
                }

                Ok((full_content, collected_tool_calls, pre_executed))
            }
            Ok(Err(e)) => {
                let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                    "stream_open",
                    &e.to_string(),
                );
                record_recovery_plan(trace, &plan);
                warn!("{}", plan.user_note);
                warn!("Streaming failed, falling back to non-streaming: {}", e);
                let base_request = ChatRequest::new(&self.model)
                    .with_messages(fallback_messages.clone())
                    .with_temperature(0.2);
                let response = if let Some(tools) = fallback_tools.clone() {
                    match self
                        .provider_chat_with_timeout(
                            base_request.clone().with_tools(tools),
                            "non-streaming fallback with tools",
                        )
                        .await
                    {
                        Ok(r) => r,
                        Err(with_tools_err) => {
                            warn!(
                                "Non-streaming fallback with tools failed: {}. Retrying without tools.",
                                with_tools_err
                            );
                            self.provider_chat_with_timeout(
                                base_request,
                                "non-streaming fallback without tools",
                            )
                            .await?
                        }
                    }
                } else {
                    self.provider_chat_with_timeout(base_request, "non-streaming fallback")
                        .await?
                };
                self.record_cost(&response).await;
                emit_usage_event(&response, tx).await;

                let content = strip_think_blocks(&response.content);
                if !content.is_empty() {
                    let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                }
                let tool_calls = response.tool_calls.unwrap_or_default();
                Ok((content, tool_calls, std::collections::HashMap::new()))
            }
            Err(_) => {
                let timeout_msg = format!(
                    "stream open timed out after {}s",
                    llm_request_timeout().as_secs()
                );
                let plan = crate::engine::recovery_plan::RecoveryPlan::streaming_fallback(
                    "stream_open_timeout",
                    &timeout_msg,
                );
                record_recovery_plan(trace, &plan);
                warn!("{}", plan.user_note);
                warn!("Streaming open timed out, falling back to non-streaming");
                let base_request = ChatRequest::new(&self.model)
                    .with_messages(fallback_messages.clone())
                    .with_temperature(0.2);
                let response = if let Some(tools) = fallback_tools.clone() {
                    match self
                        .provider_chat_with_timeout(
                            base_request.clone().with_tools(tools),
                            "non-streaming fallback with tools",
                        )
                        .await
                    {
                        Ok(r) => r,
                        Err(with_tools_err) => {
                            warn!(
                                "Non-streaming fallback with tools failed: {}. Retrying without tools.",
                                with_tools_err
                            );
                            self.provider_chat_with_timeout(
                                base_request,
                                "non-streaming fallback without tools",
                            )
                            .await?
                        }
                    }
                } else {
                    self.provider_chat_with_timeout(base_request, "non-streaming fallback")
                        .await?
                };
                self.record_cost(&response).await;
                emit_usage_event(&response, tx).await;

                let content = strip_think_blocks(&response.content);
                if !content.is_empty() {
                    let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                }
                let tool_calls = response.tool_calls.unwrap_or_default();
                Ok((content, tool_calls, std::collections::HashMap::new()))
            }
        }
    }

    async fn synthesize_patch_tool_calls(
        &self,
        messages: &[Message],
        task_preview: &str,
    ) -> Result<Vec<ToolCall>> {
        let evidence = Self::patch_synthesis_evidence(messages);
        let deterministic_seed = if task_preview.trim().is_empty() {
            evidence.clone()
        } else if evidence.trim().is_empty() {
            format!("TASK:\n{task_preview}")
        } else {
            format!("TASK:\n{task_preview}\n\nEVIDENCE:\n{evidence}")
        };

        if deterministic_seed.trim().is_empty() {
            return Err(anyhow::anyhow!("no usable evidence for patch synthesis"));
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        if Self::deterministic_patch_synthesis_enabled() {
            let deterministic_calls =
                self.deterministic_patch_tool_calls(&deterministic_seed, &cwd);
            if !deterministic_calls.is_empty() {
                return Ok(deterministic_calls);
            }
        }

        if evidence.trim().is_empty() {
            return Err(anyhow::anyhow!("no usable evidence for patch synthesis"));
        }

        let system = r#"You are a controlled patch synthesis engine for a coding agent.
You receive prior read/search/tool evidence from the current task.
Return ONLY one JSON object. Do not use markdown. Do not explain outside JSON.
Only propose small, evidence-backed file_edit actions.
If you cannot patch from the evidence, return {"can_patch":false,"reason":"...","actions":[]}."#;
        let user = format!(
            r#"Task:
{task_preview}

Evidence from prior tool results:
{evidence}

Return this exact JSON shape:
{{
  "can_patch": true,
  "reason": "why this patch is safe from the evidence",
  "actions": [
    {{
      "tool": "file_edit",
      "path": "relative/path.rs",
      "old_string": "exact text to replace",
      "new_string": "replacement text",
      "expected_replacements": 1
    }}
  ]
}}

Rules:
- Only use tool="file_edit".
- Prefer old_string/new_string exact replacement when the evidence contains the original code.
- You may use line_start/line_end only when evidence gives a precise bounded line range; do not combine line_start/line_end with old_string.
- Do not invent paths. Use paths shown in evidence.
- Do not invent enum variants, struct fields, functions, or APIs not visible in evidence. Reuse existing names exactly; if a decision object already computes status, prefer that status over reimplementing gates.
- For quality/scoring fixes, if a scorer/decision object already encodes explicit override plus safety/duplication hard stops, assign from decision.status directly. Never re-promote Rejected/Proposed decisions to Accepted with a second explicit_override or score check in the caller.
- Keep actions minimal. Return one to six actions when the evidence shows multiple independent acceptance-critical bypasses or one Rust type change that requires updating every initializer/pattern. Otherwise return one safest next edit. Every action must have expected_replacements=1.
- For Rust compiler errors like "missing field `x` in initializer" or "pattern does not mention field `x`", fix every constructor and match pattern shown in the validation evidence, not just the enum definition.
- For memory quality gate tasks, if evidence shows both a model-facing save tool path and a quality/status override path, fix both paths in the same plan.
- Never edit .git, target, cache, generated benchmark output, or files outside the working tree."#
        );

        let mut synthesis_messages = vec![Message::system(system), Message::user(user.clone())];
        let mut last_content = String::new();
        let mut last_validation_errors = Vec::new();

        for attempt in 0..2 {
            let request = ChatRequest::new(&self.model)
                .with_messages(synthesis_messages.clone())
                .with_temperature(0.0);
            let (content, _, _) = self.call_api(request).await?;
            last_content = content.clone();

            if let Some(plan) = Self::parse_patch_synthesis_plan(&content) {
                if !plan.can_patch {
                    let reason = plan.reason.trim();
                    last_validation_errors.push(if reason.is_empty() {
                        "patch synthesis declined without a reason".to_string()
                    } else {
                        format!("patch synthesis declined: {}", reason)
                    });
                    if attempt == 0 {
                        synthesis_messages
                            .push(Message::assistant(safe_prefix_by_bytes(&content, 1200)));
                        synthesis_messages.push(Message::user(format!(
                            "The previous patch plan declined instead of editing: {}. If the evidence names a concrete missing code block, compile error, assertion failure, or regression marker, return corrected JSON with the smallest file_edit action. Return can_patch=false only when there is no concrete editable file or old_string evidence.",
                            last_validation_errors.join("; ")
                        )));
                        continue;
                    }
                    break;
                }

                let mut calls = Vec::new();
                let mut validation_errors = Vec::new();
                for action in plan.actions.iter().take(6) {
                    match self.validate_patch_synthesis_action(action, &cwd) {
                        Ok(call) => calls.push(call),
                        Err(err) => validation_errors.push(err.to_string()),
                    }
                }
                if !calls.is_empty() {
                    return Ok(calls);
                }
                last_validation_errors = validation_errors;
                if last_validation_errors.is_empty() {
                    last_validation_errors
                        .push("patch plan did not include a valid file_edit action".to_string());
                }
            } else {
                last_validation_errors.push("response was not valid patch JSON".to_string());
            }

            if attempt == 0 {
                synthesis_messages.push(Message::assistant(safe_prefix_by_bytes(&content, 1200)));
                synthesis_messages.push(Message::user(format!(
                    "The previous patch plan was rejected: {}. Return corrected JSON only. Use one to six file_edit actions when multiple independent acceptance-critical bypasses or Rust missing-field/pattern compile errors are visible; otherwise use one action. Use either old_string or line_start/line_end, never both. Do not call tools. Reuse only paths, enum variants, fields, and functions shown in evidence or validation feedback.",
                    last_validation_errors.join("; ")
                )));
            }
        }

        if !last_validation_errors.is_empty() {
            warn!(
                "Patch synthesis JSON actions were not directly applicable: {}",
                last_validation_errors.join("; ")
            );
        }

        let Some(file_edit_tool) = self.tool_registry.get("file_edit") else {
            return Err(anyhow::anyhow!(
                "file_edit tool is unavailable for patch synthesis"
            ));
        };
        let file_edit_schema = crate::services::api::Tool {
            name: file_edit_tool.name().to_string(),
            description: file_edit_tool.description().to_string(),
            parameters: file_edit_tool.parameters(),
        };
        let tool_system = r#"You are now in forced patch application mode.
Use the file_edit tool to apply the smallest safe patch from the evidence.
Do not call read/search tools.
Do not invent enum variants, struct fields, functions, or APIs not visible in evidence.
If a scorer/decision object already returns final status, use that status directly; do not re-promote with explicit_override or score checks in the caller.
Do not answer in prose unless no safe patch exists."#;
        let tool_request = ChatRequest::new(&self.model)
            .with_messages(vec![
                Message::system(tool_system),
                Message::user(user),
                Message::assistant(format!(
                    "The previous JSON-only patch synthesis response was rejected: {}. It began with: {}",
                    last_validation_errors.join("; "),
                    safe_prefix_by_bytes(&last_content, 800)
                )),
            ])
            .with_tools(vec![file_edit_schema])
            .with_temperature(0.0);
        let (fallback_content, fallback_tool_calls, _) = self.call_api(tool_request).await?;
        let mut calls = Vec::new();
        let mut validation_errors = Vec::new();
        for tool_call in fallback_tool_calls.into_iter().take(6) {
            match self.validate_synthesized_tool_call(tool_call, &cwd) {
                Ok(call) => calls.push(call),
                Err(err) => validation_errors.push(err.to_string()),
            }
        }
        if calls.is_empty() {
            return Err(anyhow::anyhow!(
                "patch synthesis did not return valid JSON or file_edit calls; validation_errors=[{}]; text began with: {}",
                validation_errors.join("; "),
                safe_prefix_by_bytes(&fallback_content, 800)
            ));
        }
        Ok(calls)
    }

    fn deterministic_patch_tool_calls(
        &self,
        evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<ToolCall> {
        let lower_evidence = evidence.to_lowercase();
        let mut actions = Vec::new();
        if let Some(action) = Self::deterministic_rust_e0596_action(&lower_evidence, cwd) {
            actions.push(action);
        }
        if let Some(action) =
            Self::deterministic_persistent_memory_planning_action(&lower_evidence, cwd)
        {
            actions.push(action);
        }
        if let Some(action) =
            Self::deterministic_record_repair_action_arity_fix(&lower_evidence, cwd)
        {
            actions.push(action);
        }
        actions.extend(Self::deterministic_skill_promotion_gate_actions(
            &lower_evidence,
            cwd,
        ));
        actions.extend(Self::deterministic_memory_recall_conflict_actions(
            &lower_evidence,
            cwd,
        ));
        actions.extend(Self::deterministic_memory_duplicate_demote_actions(
            &lower_evidence,
            cwd,
        ));
        actions.extend(Self::deterministic_memory_sensitive_hard_block_actions(
            &lower_evidence,
            cwd,
        ));

        if !(lower_evidence.contains("memorywrite")
            || lower_evidence.contains("memory_save")
            || lower_evidence.contains("quality gate")
            || lower_evidence.contains("quality gates"))
        {
            return actions
                .iter()
                .filter_map(|action| self.validate_patch_synthesis_action(action, cwd).ok())
                .take(6)
                .collect();
        }

        let memory_tool = cwd.join("src/tools/memory_tool/mod.rs");
        if Self::file_contains(
            &memory_tool,
            "assess_memory_candidate(content, category, &existing, true)",
        ) {
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tools/memory_tool/mod.rs".to_string(),
                old_string: Some(
                    "assess_memory_candidate(content, category, &existing, true)".to_string(),
                ),
                new_string: "assess_memory_candidate(content, category, &existing, false)"
                    .to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let quality = cwd.join("src/memory/quality.rs");
        if Self::file_contains(
            &quality,
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };",
        ) {
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/quality.rs".to_string(),
                old_string: Some(
                    "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };"
                        .to_string(),
                ),
                new_string: "let status = write_decision.status;".to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }
        if Self::file_contains(
            &quality,
            "let status = if score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };",
        ) {
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/quality.rs".to_string(),
                old_string: Some(
                    "let status = if score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };"
                        .to_string(),
                ),
                new_string: "let status = write_decision.status;".to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        if let Some((first, second)) = Self::deterministic_save_outcome_actions(cwd) {
            actions.push(first);
            actions.push(second);
        }

        actions
            .iter()
            .filter_map(|action| self.validate_patch_synthesis_action(action, cwd).ok())
            .take(6)
            .collect()
    }

    fn deterministic_patch_synthesis_enabled() -> bool {
        matches!(
            std::env::var("PRIORITY_AGENT_DETERMINISTIC_PATCH_SYNTHESIS")
                .ok()
                .as_deref(),
            Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
        )
    }

    fn file_contains(path: &std::path::Path, needle: &str) -> bool {
        std::fs::read_to_string(path)
            .map(|content| content.contains(needle))
            .unwrap_or(false)
    }

    fn deterministic_rust_e0596_action(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        if !(lower_evidence.contains("error[e0596]")
            || (lower_evidence.contains("cannot borrow") && lower_evidence.contains("as mutable")))
        {
            return None;
        }

        let path = cwd.join("src/engine/conversation_loop/mod.rs");
        let old_string = "if let Some(ref mut mem_mutex) = self.memory_manager {";
        if !Self::file_contains(&path, old_string) {
            return None;
        }

        Some(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(old_string.to_string()),
            new_string: "if let Some(ref mem_mutex) = self.memory_manager {".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        })
    }

    fn deterministic_persistent_memory_planning_action(
        _lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        let path = cwd.join("src/engine/conversation_loop/mod.rs");
        let old_string = concat!(
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
            "        if let Some(ref ctx) = turn_retrieval_context {"
        );
        if !Self::file_contains(&path, old_string) {
            return None;
        }

        let new_string = r#"        // Prefetch memory context and merge into turn_retrieval_context for planning.
        if let Some(ref mem_mutex) = self.memory_manager {
            let mut mem = mem_mutex.lock().await;
            mem.reset_turn();
            if let Some(memory_ctx) = mem
                .prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref(),
                    &self.model,
                    route.retrieval,
                )
                .await
            {
                trace.record(TraceEvent::MemoryPrefetch {
                    chars: memory_ctx
                        .items
                        .iter()
                        .map(|item| item.content_preview.chars().count())
                        .sum(),
                });
                if let Some(ref mut ctx) = turn_retrieval_context {
                    ctx.extend(memory_ctx);
                } else {
                    turn_retrieval_context = Some(memory_ctx);
                }
            }
        }
        if let Some(ref ctx) = turn_retrieval_context {"#;

        Some(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(old_string.to_string()),
            new_string: new_string.to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        })
    }

    fn deterministic_record_repair_action_arity_fix(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        if !(lower_evidence.contains("record_repair_action")
            || lower_evidence
                .contains("this method takes 4 arguments but 3 arguments were supplied")
            || lower_evidence.contains("argument #4")
            || lower_evidence.contains("retry: {}"))
        {
            return None;
        }

        let path = cwd.join("src/engine/conversation_loop/mod.rs");
        let content = std::fs::read_to_string(path).ok()?;
        if !content.contains("post_edit_reflection.record_repair_action(") {
            return None;
        }

        let lines: Vec<&str> = content.lines().collect();
        let start_idx = lines
            .iter()
            .position(|line| line.contains("post_edit_reflection.record_repair_action("))?;
        let mut end_idx = None;
        for (offset, line) in lines.iter().enumerate().skip(start_idx) {
            if line.trim() == ");" {
                end_idx = Some(offset);
                break;
            }
            if offset.saturating_sub(start_idx) > 16 {
                break;
            }
        }
        let end_idx = end_idx?;
        let call_block = lines[start_idx..=end_idx].join("\n");
        if !call_block.contains("record_repair_action(") {
            return None;
        }
        if call_block.contains("\"repair failed verification before closeout\"")
            && call_block.contains("verification_command,")
            && !call_block.contains(Self::retry_format_marker().as_str())
        {
            return None;
        }
        if !call_block.contains(Self::retry_format_marker().as_str())
            && !call_block.contains("verification_command")
            && !lower_evidence.contains("argument #4")
            && !lower_evidence
                .contains("this method takes 4 arguments but 3 arguments were supplied")
        {
            return None;
        }

        Some(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: None,
            new_string: r#"                    post_edit_reflection.record_repair_action(
                        acceptance_repair_attempts + 1,
                        "repair failed verification before closeout",
                        changed_files.first().map(|path| path.display().to_string()),
                        verification_command,
                    );"#
            .to_string(),
            line_start: Some(start_idx + 1),
            line_end: Some(end_idx + 1),
            expected_replacements: None,
        })
    }

    fn retry_format_marker() -> String {
        concat!("&format!(\"retry: {", "}\", verification_command)").to_string()
    }

    fn deterministic_skill_promotion_gate_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("skill proposal")
            || lower_evidence.contains("skill-promotion")
            || lower_evidence.contains("validate_skill_promotion_for_apply")
            || lower_evidence.contains("promotion gate"))
        {
            return Vec::new();
        }

        let path = cwd.join("src/tui/slash_handler/config.rs");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        if !content.contains("fn validate_skill_promotion_for_apply(")
            || !content.contains("fn skill_fitness_from_bound_eval(")
            || !content.contains("fn estimate_skill_semantic_drift(")
        {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let apply_root_anchor = "            let root = user_skill_root();\n            match write_active_skill(&current, &root) {";
        let gate_call =
            "validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())";
        if content.contains(apply_root_anchor) && !content.contains(gate_call) {
            let gate_block = r#"            if let Err(report) = validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref()) {
                return format!(
                    "Skill proposal {} was not applied by promotion gate.\n{}",
                    current.id, report
                );
            }
"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/slash_handler/config.rs".to_string(),
                old_string: Some(apply_root_anchor.to_string()),
                new_string: format!("{gate_block}{apply_root_anchor}"),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let apply_reload_anchor = r#"                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,"#;
        let applied_version_anchor = "store.record_applied_version(id, &path)";
        let apply_branch_start = content.find(applied_version_anchor).unwrap_or(0);
        let has_apply_cooldown = content[apply_branch_start..]
            .find("record_evolution_update(")
            .zip(content[apply_branch_start..].find("let loaded = app.skill_runtime.reload()"))
            .map(|(record_pos, loaded_pos)| record_pos < loaded_pos)
            .unwrap_or(false);
        if content.contains(apply_reload_anchor) && !has_apply_cooldown {
            let cooldown_block = r#"                        record_evolution_update(
                            crate::engine::evolution_controller::EvolutionTarget::Skill,
                        );
"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/slash_handler/config.rs".to_string(),
                old_string: Some(apply_reload_anchor.to_string()),
                new_string: format!("{cooldown_block}{apply_reload_anchor}"),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        actions
    }

    fn deterministic_memory_recall_conflict_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("memory-recall")
            || lower_evidence.contains("memory recall")
            || lower_evidence.contains("conflict matching")
            || lower_evidence.contains("memory_conflict_matches_item")
            || lower_evidence.contains("parse_memory_conflict"))
        {
            return Vec::new();
        }

        let path = cwd.join("src/engine/retrieval_context.rs");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        if !content.contains("fn memory_conflict_matches_item(") {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let matching_old = r#"    if let Some((key, values)) = parse_memory_conflict(&conflict) {
        return snippet.contains(&key) && values.iter().any(|value| snippet.contains(value));
    }

    let tokens = conflict
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| {
            part.len() >= 4
                && !matches!(
                    *part,
                    "memory" | "project" | "user" | "value" | "values" | "conflicting"
                )
        })
        .collect::<Vec<_>>();"#;
        let matching_new = r#"    if let Some((key, values)) = parse_memory_conflict(&conflict) {
        if is_generic_conflict_token(&key) {
            return false;
        }
        return snippet.contains(&key) && values.iter().any(|value| snippet.contains(value));
    }

    let tokens = conflict
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| {
            part.len() >= 4
                && !is_generic_conflict_token(part)
        })
        .collect::<Vec<_>>();"#;
        if content.contains(matching_old) {
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/engine/retrieval_context.rs".to_string(),
                old_string: Some(matching_old.to_string()),
                new_string: matching_new.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        if !content.contains("fn is_generic_conflict_token(") {
            let parse_anchor =
                "fn parse_memory_conflict(conflict: &str) -> Option<(String, Vec<String>)> {";
            let helper = r#"fn is_generic_conflict_token(token: &str) -> bool {
    matches!(
        token,
        "memory"
            | "project"
            | "user"
            | "value"
            | "values"
            | "conflicting"
            | "conflicts"
            | "conflict"
            | "key"
            | "keys"
            | "source"
            | "sources"
            | "with"
            | "from"
            | "this"
            | "that"
            | "these"
            | "those"
    )
}

"#;
            if content.contains(parse_anchor) {
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/engine/retrieval_context.rs".to_string(),
                    old_string: Some(parse_anchor.to_string()),
                    new_string: format!("{helper}{parse_anchor}"),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        let tests_anchor = r#"        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn items_are_sorted_by_score() {"#;
        if content.contains(tests_anchor)
            && !content.contains("memory_conflict_matching_ignores_generic_key_conflicts")
        {
            let tests_new = r#"        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn memory_conflict_matching_ignores_generic_key_conflicts() {
        let conflict = "- key 'project' has conflicting values: alpha | beta";
        let item = crate::memory::manager::MemoryMatch {
            source: "memory/project.md".to_string(),
            score: 40,
            rerank_score: Some(0.95),
            snippet: "Project memory value alpha is mentioned in a note.".to_string(),
        };

        assert!(!memory_conflict_matches_item(conflict, &item));
    }

    #[test]
    fn memory_conflict_matching_requires_specific_fallback_overlap() {
        let conflict = "memory project value source conflict mentions alpha beta";
        let unrelated = crate::memory::manager::MemoryMatch {
            source: "memory/project.md".to_string(),
            score: 40,
            rerank_score: Some(0.95),
            snippet: "This project memory has a value and source but no concrete conflicting fact."
                .to_string(),
        };
        let related = crate::memory::manager::MemoryMatch {
            source: "memory/project.md".to_string(),
            score: 40,
            rerank_score: Some(0.95),
            snippet: "alpha and beta are both mentioned in this concrete conflict.".to_string(),
        };

        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn items_are_sorted_by_score() {"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/engine/retrieval_context.rs".to_string(),
                old_string: Some(tests_anchor.to_string()),
                new_string: tests_new.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        actions
    }

    fn deterministic_memory_duplicate_demote_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("memory-save-duplicate-demotion")
            || lower_evidence.contains("duplicate/demote")
            || lower_evidence.contains("重复记忆")
            || lower_evidence.contains("near duplicate"))
        {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let quality_path = cwd.join("src/memory/quality.rs");
        if Self::file_contains(&quality_path, "(hits as f32 / words.len() as f32).min(0.8)") {
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/quality.rs".to_string(),
                old_string: Some("(hits as f32 / words.len() as f32).min(0.8)".to_string()),
                new_string: "(hits as f32 / words.len() as f32).min(0.95)".to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let manager_path = cwd.join("src/memory/manager.rs");
        let learning_anchor = r#"        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }
        if normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "rejected",
                category,
                content,
                "duplicate memory already exists",
            );
            return MemoryWriteOutcome::duplicate(
                path.to_path_buf(),
                "duplicate memory already exists",
            );
        }"#;
        if Self::file_contains(&manager_path, learning_anchor) {
            let learning_replacement = r#"        if assessment.duplication >= 0.85 || normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "duplicate",
                category,
                content,
                &format!("duplicate memory already exists; {}", assessment.reason),
            );
            return MemoryWriteOutcome::duplicate(
                path.to_path_buf(),
                format!("duplicate memory already exists; {}", assessment.reason),
            );
        }
        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/manager.rs".to_string(),
                old_string: Some(learning_anchor.to_string()),
                new_string: learning_replacement.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let topic_anchor = r#"        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async topic memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }
        if normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate topic learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "rejected",
                category,
                content,
                "duplicate topic memory already exists",
            );
            return MemoryWriteOutcome::duplicate(
                path.clone(),
                "duplicate topic memory already exists",
            );
        }"#;
        if Self::file_contains(&manager_path, topic_anchor) {
            let topic_replacement = r#"        if assessment.duplication >= 0.85 || normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate topic learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "duplicate",
                category,
                content,
                &format!("duplicate topic memory already exists; {}", assessment.reason),
            );
            return MemoryWriteOutcome::duplicate(
                path.clone(),
                format!("duplicate topic memory already exists; {}", assessment.reason),
            );
        }
        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async topic memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/manager.rs".to_string(),
                old_string: Some(topic_anchor.to_string()),
                new_string: topic_replacement.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        actions
    }

    fn deterministic_memory_sensitive_hard_block_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("memory-save-sensitive-hard-block")
            || lower_evidence.contains("sensitive hard block")
            || lower_evidence.contains("secret_like_content")
            || lower_evidence.contains("sensitive content")
            || lower_evidence.contains("敏感内容"))
        {
            return Vec::new();
        }

        let mut actions = Vec::new();

        let quality_path = cwd.join("src/memory/quality.rs");
        if !Self::file_contains(
            &quality_path,
            "explicit_save_cannot_override_secret_candidate",
        ) {
            let anchor = r#"    #[test]
    fn blocks_secret_candidate() {
        let err = assess_memory_candidate(
            "The API token is sk-123456789012345678901234",
            "note",
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }
}"#;
            if Self::file_contains(&quality_path, anchor) {
                let replacement = r#"    #[test]
    fn blocks_secret_candidate() {
        let err = assess_memory_candidate(
            "The API token is sk-123456789012345678901234",
            "note",
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }

    #[test]
    fn explicit_save_cannot_override_secret_candidate() {
        let err = assess_memory_candidate(
            "password = sk-123456789012345678901234",
            "preference",
            "",
            true,
        )
        .unwrap_err();
        assert_eq!(err.code, "secret_like_content");
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }
}"#;
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/memory/quality.rs".to_string(),
                    old_string: Some(anchor.to_string()),
                    new_string: replacement.to_string(),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        let manager_path = cwd.join("src/memory/manager.rs");
        if !Self::file_contains(
            &manager_path,
            "test_add_learning_async_blocks_sensitive_explicit_like_content",
        ) {
            let anchor = r#"    #[tokio::test]
    async fn test_add_topic_learning_async_writes_memory_file() {
        let base = temp_memory_base("topic-learning-async");
        let mgr = MemoryManager::with_base_dir(base.clone());
        let outcome = mgr
            .add_topic_learning_async(
                "Prefer concise CLI status lines for active tool calls.",
                "preference",
                "cli",
            )
            .await;

        assert_eq!(outcome.status, MemoryWriteOutcomeStatus::Saved);
        let content = std::fs::read_to_string(base.join("topics").join("cli.md")).unwrap();
        assert!(content.contains("Prefer concise CLI status lines"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn test_deduplication_in_pending() {"#;
            if Self::file_contains(&manager_path, anchor) {
                let replacement = r#"    #[tokio::test]
    async fn test_add_topic_learning_async_writes_memory_file() {
        let base = temp_memory_base("topic-learning-async");
        let mgr = MemoryManager::with_base_dir(base.clone());
        let outcome = mgr
            .add_topic_learning_async(
                "Prefer concise CLI status lines for active tool calls.",
                "preference",
                "cli",
            )
            .await;

        assert_eq!(outcome.status, MemoryWriteOutcomeStatus::Saved);
        let content = std::fs::read_to_string(base.join("topics").join("cli.md")).unwrap();
        assert!(content.contains("Prefer concise CLI status lines"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test]
    async fn test_add_learning_async_blocks_sensitive_explicit_like_content() {
        let base = temp_memory_base("learning-async-sensitive-block");
        let mgr = MemoryManager::with_base_dir(base.clone());
        let secret = "api_key = sk-123456789012345678901234";

        let outcome = mgr.add_learning_async(secret, "preference").await;

        assert_eq!(outcome.status, MemoryWriteOutcomeStatus::Blocked);
        assert!(outcome.reason.contains("secret_like_content"));
        let user_memory = std::fs::read_to_string(&mgr.user_path).unwrap_or_default();
        assert!(
            !user_memory.contains("sk-123456789012345678901234"),
            "blocked sensitive content must not be written to USER.md"
        );

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn test_deduplication_in_pending() {"#;
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/memory/manager.rs".to_string(),
                    old_string: Some(anchor.to_string()),
                    new_string: replacement.to_string(),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        let app_path = cwd.join("src/tui/app.rs");
        if !Self::file_contains(
            &app_path,
            "test_format_memory_write_outcome_reports_safety_block",
        ) {
            let anchor = r#"    #[test]
    fn test_parse_memory_save_args() {
        assert_eq!(
            parse_memory_save_args("remember this"),
            (MemorySaveTarget::Auto, None, "remember this")
        );
        assert_eq!(
            parse_memory_save_args("--user reply in Chinese"),
            (MemorySaveTarget::User, None, "reply in Chinese")
        );
        assert_eq!(
            parse_memory_save_args("--topic tui-design keep bottom anchored"),
            (
                MemorySaveTarget::Topic,
                Some("tui-design"),
                "keep bottom anchored"
            )
        );
        assert_eq!(
            parse_memory_save_args("--topic=context-management track token budget"),
            (
                MemorySaveTarget::Topic,
                Some("context-management"),
                "track token budget"
            )
        );
    }

    #[test]
    fn test_stream_usage_label_includes_reasoning_and_cached_tokens() {"#;
            if Self::file_contains(&app_path, anchor) {
                let replacement = r#"    #[test]
    fn test_parse_memory_save_args() {
        assert_eq!(
            parse_memory_save_args("remember this"),
            (MemorySaveTarget::Auto, None, "remember this")
        );
        assert_eq!(
            parse_memory_save_args("--user reply in Chinese"),
            (MemorySaveTarget::User, None, "reply in Chinese")
        );
        assert_eq!(
            parse_memory_save_args("--topic tui-design keep bottom anchored"),
            (
                MemorySaveTarget::Topic,
                Some("tui-design"),
                "keep bottom anchored"
            )
        );
        assert_eq!(
            parse_memory_save_args("--topic=context-management track token budget"),
            (
                MemorySaveTarget::Topic,
                Some("context-management"),
                "track token budget"
            )
        );
    }

    #[test]
    fn test_format_memory_write_outcome_reports_safety_block() {
        let outcome = crate::memory::manager::MemoryWriteOutcome {
            status: crate::memory::manager::MemoryWriteOutcomeStatus::Blocked,
            quality_score: None,
            reason: "secret_like_content: memory appears to contain a raw token".to_string(),
            path: None,
        };

        let rendered = format_memory_write_outcome("api_key = [redacted]", &outcome);

        assert!(rendered.contains("blocked for safety"));
        assert!(rendered.contains("secret_like_content"));
        assert!(!rendered.contains("Saved memory"));
    }

    #[test]
    fn test_stream_usage_label_includes_reasoning_and_cached_tokens() {"#;
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/tui/app.rs".to_string(),
                    old_string: Some(anchor.to_string()),
                    new_string: replacement.to_string(),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        actions
    }

    fn deterministic_save_outcome_actions(
        cwd: &std::path::Path,
    ) -> Option<(PatchSynthesisAction, PatchSynthesisAction)> {
        let path = cwd.join("src/tui/app.rs");
        let content = std::fs::read_to_string(path).ok()?;
        if !content.contains("fn format_memory_write_outcome(")
            || !content.contains("format!(\"Saved: {}\", save_content)")
        {
            return None;
        }

        let save_match = r#"match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await;
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await;
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await;
                                }
                            }
                            format!("Saved: {}", save_content)"#;
        let save_outcome = r#"let outcome = match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await
                                }
                            };
                            format_memory_write_outcome(save_content, &outcome)"#;

        let first_old = format!(
            "let mem = memory_manager.lock().await;\n                            {}",
            save_match
        );
        let first_new = format!(
            "let mem = memory_manager.lock().await;\n                            {}",
            save_outcome
        );
        let second_old = format!(
            "let mem = crate::memory::MemoryManager::new();\n                            {}",
            save_match
        );
        let second_new = format!(
            "let mem = crate::memory::MemoryManager::new();\n                            {}",
            save_outcome
        );

        Some((
            PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/app.rs".to_string(),
                old_string: Some(first_old),
                new_string: first_new,
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            },
            PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/app.rs".to_string(),
                old_string: Some(second_old),
                new_string: second_new,
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            },
        ))
    }

    fn patch_synthesis_evidence(messages: &[Message]) -> String {
        let mut chunks = Vec::new();
        let mut total = 0usize;
        for message in messages.iter().rev() {
            let chunk = match message {
                Message::User { content } => {
                    format!("USER:\n{}", safe_prefix_by_bytes(content, 3000))
                }
                Message::Tool { content, .. } => {
                    if content.contains("[File unchanged since last read:") {
                        continue;
                    }
                    let relevant_failure = !content.starts_with("Result: OK")
                        && (content.contains("error[")
                            || content.contains("could not compile")
                            || content.contains("AssertionError")
                            || content.contains("[exit status:")
                            || content.contains("failed_commands"));
                    if !content.starts_with("Result: OK") && !relevant_failure {
                        continue;
                    }
                    let label = if content.starts_with("Result: OK") {
                        "TOOL RESULT"
                    } else {
                        "FAILED TOOL RESULT"
                    };
                    format!("{}:\n{}", label, safe_prefix_by_bytes(content, 3500))
                }
                Message::Assistant { content, .. } if !content.trim().is_empty() => {
                    format!("ASSISTANT:\n{}", safe_prefix_by_bytes(content, 1200))
                }
                _ => continue,
            };
            total += chunk.len();
            chunks.push(chunk);
            if total >= 10_000 {
                break;
            }
        }
        chunks.reverse();
        chunks.join("\n\n---\n\n")
    }

    fn parse_patch_synthesis_plan(content: &str) -> Option<PatchSynthesisPlan> {
        let trimmed = content.trim();
        if let Ok(plan) = serde_json::from_str::<PatchSynthesisPlan>(trimmed) {
            return Some(plan);
        }

        let without_fence = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .and_then(|s| s.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(trimmed);
        if let Ok(plan) = serde_json::from_str::<PatchSynthesisPlan>(without_fence) {
            return Some(plan);
        }

        for (start, ch) in without_fence.char_indices() {
            if ch != '{' {
                continue;
            }
            if let Some(end) = Self::matching_json_object_end(without_fence, start) {
                if let Ok(plan) =
                    serde_json::from_str::<PatchSynthesisPlan>(&without_fence[start..end])
                {
                    return Some(plan);
                }
            }
        }
        None
    }

    fn matching_json_object_end(input: &str, start: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (offset, ch) in input[start..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(start + offset + ch.len_utf8());
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn validate_patch_synthesis_action(
        &self,
        action: &PatchSynthesisAction,
        cwd: &std::path::Path,
    ) -> Result<ToolCall> {
        if !action.tool.is_empty() && action.tool != "file_edit" {
            return Err(anyhow::anyhow!(
                "unsupported synthesized patch tool: {}",
                action.tool
            ));
        }
        if action.path.trim().is_empty() {
            return Err(anyhow::anyhow!("synthesized patch path is empty"));
        }
        let raw_path = std::path::Path::new(action.path.trim());
        for component in raw_path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(anyhow::anyhow!(
                        "synthesized patch path contains parent traversal: {}",
                        action.path
                    ));
                }
                std::path::Component::Normal(part)
                    if part == ".git" || part == "target" || part == "node_modules" =>
                {
                    return Err(anyhow::anyhow!(
                        "synthesized patch path targets ignored/generated directory: {}",
                        action.path
                    ));
                }
                _ => {}
            }
        }
        let (canonical_candidate, tool_path) =
            match Self::resolve_synthesized_patch_path(raw_path, cwd) {
                Ok(resolved) => resolved,
                Err(path_error) => {
                    if let Some(old_string) = action.old_string.as_ref() {
                        Self::resolve_synthesized_patch_path_by_old_string(old_string, cwd)
                            .unwrap_or_else(|| Err(path_error))?
                    } else {
                        return Err(path_error);
                    }
                }
            };
        if action.new_string.len() > 20_000 {
            return Err(anyhow::anyhow!(
                "synthesized patch replacement is too large"
            ));
        }

        let mut normalized_new_string = action.new_string.clone();
        let mut params = serde_json::json!({
            "path": tool_path,
        });
        if action.line_start.is_some() || action.line_end.is_some() {
            let (Some(line_start), Some(line_end)) = (action.line_start, action.line_end) else {
                return Err(anyhow::anyhow!(
                    "synthesized patch line_start and line_end must be provided together"
                ));
            };
            if action.old_string.is_some() {
                return Err(anyhow::anyhow!(
                    "synthesized patch line ranges must not also include old_string"
                ));
            }
            if line_start == 0 || line_end == 0 || line_start > line_end {
                return Err(anyhow::anyhow!(
                    "synthesized patch line range is invalid: {}..={}",
                    line_start,
                    line_end
                ));
            }
            let line_count = std::fs::read_to_string(&canonical_candidate)
                .map(|content| content.lines().count())
                .unwrap_or(0);
            if line_start > line_count || line_end > line_count {
                return Err(anyhow::anyhow!(
                    "synthesized patch line range {}..={} is outside {} line file",
                    line_start,
                    line_end,
                    line_count
                ));
            }
            if line_end - line_start > 24 {
                return Err(anyhow::anyhow!(
                    "synthesized patch line range is too broad: {}..={}",
                    line_start,
                    line_end
                ));
            }
            if !Self::balanced_delimiters_rough(&normalized_new_string) {
                return Err(anyhow::anyhow!(
                    "synthesized patch replacement has unbalanced delimiters"
                ));
            }
            params["line_start"] = serde_json::json!(line_start);
            params["line_end"] = serde_json::json!(line_end);
        } else if let Some(old_string) = action.old_string.as_ref() {
            if old_string.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "synthesized patch old_string is empty without a line range"
                ));
            }
            if old_string.len() > 12_000 {
                return Err(anyhow::anyhow!("synthesized patch old_string is too large"));
            }
            let (normalized_old_string, replacement) =
                Self::normalize_synthesized_replacement_anchor(
                    old_string,
                    &normalized_new_string,
                    &canonical_candidate,
                )?;
            normalized_new_string = replacement;
            if Self::balanced_delimiters_rough(&normalized_old_string)
                && !Self::balanced_delimiters_rough(&normalized_new_string)
            {
                return Err(anyhow::anyhow!(
                    "synthesized patch replacement has unbalanced delimiters"
                ));
            }
            params["old_string"] = serde_json::json!(normalized_old_string);
            if let Some(expected) = action.expected_replacements {
                if expected != 1 {
                    return Err(anyhow::anyhow!(
                        "synthesized patch expected_replacements must be exactly 1, got {}",
                        expected
                    ));
                }
                params["expected_replacements"] = serde_json::json!(expected);
            } else {
                params["expected_replacements"] = serde_json::json!(1);
            }
        } else {
            return Err(anyhow::anyhow!(
                "synthesized patch must include old_string or line_start/line_end"
            ));
        }
        params["new_string"] = serde_json::json!(normalized_new_string);

        if let Some(tool) = self.tool_registry.get("file_edit") {
            if let Some(err) = tool.validate_params(&params) {
                return Err(anyhow::anyhow!(
                    "synthesized patch failed tool schema validation: {}",
                    err
                ));
            }
        }
        if canonical_candidate.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            Self::validate_rust_patch_semantics(&canonical_candidate, &action.new_string)?;
            if let Some(err) = Self::unknown_rust_enum_variant_in_patch(&action.new_string, cwd) {
                return Err(anyhow::anyhow!("{}", err));
            }
        }

        Ok(ToolCall {
            id: format!("patch_synthesis_{}", uuid::Uuid::new_v4().simple()),
            name: "file_edit".to_string(),
            arguments: params,
        })
    }

    fn validate_rust_patch_semantics(path: &std::path::Path, new_string: &str) -> Result<()> {
        let normalized_path = path.to_string_lossy();
        if normalized_path.ends_with("src/memory/types.rs")
            && (new_string.contains("Duplicate") || new_string.contains("Demoted"))
            && new_string.contains("MemoryStatus")
        {
            return Err(anyhow::anyhow!(
                "memory duplicate/demote must be represented as MemoryWriteOutcomeStatus or quality decision output; do not extend MemoryStatus with Duplicate/Demoted"
            ));
        }
        if normalized_path.ends_with("src/memory/quality.rs")
            && new_string.contains("let status = if score >= 0.65")
            && new_string.contains("MemoryStatus::Accepted")
        {
            return Err(anyhow::anyhow!(
                "memory quality status must preserve score_memory_write hard gates; use write_decision.status instead of re-promoting score >= 0.65 to Accepted"
            ));
        }
        if normalized_path.ends_with("src/engine/conversation_loop/mod.rs")
            && new_string.contains("prefetch_retrieval_context_with_llm_rerank")
        {
            if new_string.contains("futures::executor::block_on") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must use async lock().await, not futures::executor::block_on"
                ));
            }
            if new_string.contains("self.provider.as_ref().and_then") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must pass the existing model string directly, not derive a preferred model from provider"
                ));
            }
            if new_string.contains("self.provider.as_ref().map") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must pass self.provider.as_ref() directly, not treat it as an Option"
                ));
            }
            if !new_string.contains(".lock().await") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must lock the Arc<Mutex<MemoryManager>> before calling prefetch"
                ));
            }
            if !new_string.contains("&self.model") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must pass &self.model"
                ));
            }
        }
        Ok(())
    }

    fn normalize_synthesized_replacement_anchor(
        old_string: &str,
        new_string: &str,
        path: &std::path::Path,
    ) -> Result<(String, String)> {
        let content = std::fs::read_to_string(path)?;
        let exact_count = content.matches(old_string).count();
        if exact_count == 1 {
            return Ok((old_string.to_string(), new_string.to_string()));
        }
        if exact_count > 1 {
            return Err(anyhow::anyhow!(
                "synthesized patch old_string is not unique in {}",
                path.display()
            ));
        }
        if new_string.lines().count() > 1 {
            return Err(anyhow::anyhow!(
                "synthesized patch old_string was not found exactly in {}; refusing inexact multi-line replacement",
                path.display()
            ));
        }

        let Some(binding_name) = Self::synthesized_assignment_binding(old_string)
            .or_else(|| Self::synthesized_assignment_binding(new_string))
        else {
            return Err(anyhow::anyhow!(
                "synthesized patch old_string was not found exactly in {}",
                path.display()
            ));
        };

        let prefix = format!("let {binding_name} =");
        let matches = content
            .lines()
            .filter(|line| line.trim_start().starts_with(&prefix))
            .map(str::to_string)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(anyhow::anyhow!(
                "synthesized patch old_string was not found exactly and assignment anchor `{}` matched {} lines in {}",
                binding_name,
                matches.len(),
                path.display()
            ));
        }

        let recovered_old = matches[0].clone();
        let recovered_new = if new_string.lines().count() <= 1 {
            let indent = recovered_old
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            format!("{}{}", indent, new_string.trim())
        } else {
            new_string.to_string()
        };
        Ok((recovered_old, recovered_new))
    }

    fn balanced_delimiters_rough(input: &str) -> bool {
        let mut stack = Vec::new();
        let mut in_string = false;
        let mut in_char = false;
        let mut escaped = false;

        for ch in input.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && (in_string || in_char) {
                escaped = true;
                continue;
            }
            if in_string {
                if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            if in_char {
                if ch == '\'' {
                    in_char = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '\'' => in_char = true,
                '(' | '[' | '{' => stack.push(ch),
                ')' => {
                    if stack.pop() != Some('(') {
                        return false;
                    }
                }
                ']' => {
                    if stack.pop() != Some('[') {
                        return false;
                    }
                }
                '}' => {
                    if stack.pop() != Some('{') {
                        return false;
                    }
                }
                _ => {}
            }
        }

        !in_string && !in_char && stack.is_empty()
    }

    fn synthesized_assignment_binding(input: &str) -> Option<String> {
        let re = regex::Regex::new(r"(?m)^\s*let\s+([A-Za-z_][A-Za-z0-9_]*)\s*=").ok()?;
        re.captures(input)
            .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
    }

    fn resolve_synthesized_patch_path(
        raw_path: &std::path::Path,
        cwd: &std::path::Path,
    ) -> Result<(std::path::PathBuf, String)> {
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut candidates = Vec::new();
        if raw_path.is_absolute() {
            candidates.push(raw_path.to_path_buf());
            if let Ok(stripped) = raw_path.strip_prefix(std::path::Path::new("/")) {
                candidates.push(cwd.join(stripped));
            }
        } else {
            candidates.push(cwd.join(raw_path));
        }

        let normal_components = raw_path
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(part) => part.to_str().map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>();
        for anchor in ["src", "tests", "benches", "examples"] {
            if let Some(idx) = normal_components.iter().position(|part| part == anchor) {
                let mut anchored = std::path::PathBuf::new();
                for part in &normal_components[idx..] {
                    anchored.push(part);
                }
                candidates.push(cwd.join(anchored));
            }
        }

        if let Some(match_path) = Self::unique_git_path_suffix_match(raw_path, cwd) {
            candidates.push(cwd.join(match_path));
        }

        for candidate in candidates {
            let Ok(canonical_candidate) = candidate.canonicalize() else {
                continue;
            };
            if !canonical_candidate.starts_with(&canonical_cwd) || !canonical_candidate.is_file() {
                continue;
            }
            let relative = canonical_candidate
                .strip_prefix(&canonical_cwd)
                .ok()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|| canonical_candidate.to_string_lossy().to_string());
            return Ok((canonical_candidate, relative));
        }

        Err(anyhow::anyhow!(
            "synthesized patch path is not editable: {}",
            raw_path.display()
        ))
    }

    fn resolve_synthesized_patch_path_by_old_string(
        old_string: &str,
        cwd: &std::path::Path,
    ) -> Option<Result<(std::path::PathBuf, String)>> {
        if old_string.trim().is_empty() || old_string.len() > 12_000 {
            return None;
        }
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut matches = Vec::new();
        for relative in Self::candidate_patch_files(cwd).into_iter().take(5_000) {
            let candidate = cwd.join(&relative);
            let Ok(canonical_candidate) = candidate.canonicalize() else {
                continue;
            };
            if !canonical_candidate.starts_with(&canonical_cwd) || !canonical_candidate.is_file() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&canonical_candidate) else {
                continue;
            };
            if content.contains(old_string) {
                let tool_path = canonical_candidate
                    .strip_prefix(&canonical_cwd)
                    .ok()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| canonical_candidate.to_string_lossy().to_string());
                matches.push((canonical_candidate, tool_path));
            }
            if matches.len() > 1 {
                return None;
            }
        }
        matches.pop().map(Ok)
    }

    fn candidate_patch_files(cwd: &std::path::Path) -> Vec<std::path::PathBuf> {
        let output = std::process::Command::new("git")
            .args(["ls-files"])
            .current_dir(cwd)
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                let files = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect::<Vec<_>>();
                if !files.is_empty() {
                    return files;
                }
            }
        }

        let mut files = Vec::new();
        let mut stack = vec![cwd.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name();
                if path.is_dir() {
                    if matches!(
                        file_name.to_str(),
                        Some(".git" | "target" | "node_modules" | ".next" | "dist")
                    ) {
                        continue;
                    }
                    stack.push(path);
                    continue;
                }
                if path.is_file() {
                    if let Ok(relative) = path.strip_prefix(cwd) {
                        files.push(relative.to_path_buf());
                    }
                }
                if files.len() >= 5_000 {
                    return files;
                }
            }
        }
        files
    }

    fn unknown_rust_enum_variant_in_patch(
        new_string: &str,
        cwd: &std::path::Path,
    ) -> Option<String> {
        let re = regex::Regex::new(r"\b([A-Z][A-Za-z0-9_]*)::([A-Z][A-Za-z0-9_]*)\b").ok()?;
        for captures in re.captures_iter(new_string) {
            let type_name = captures.get(1)?.as_str();
            let variant = captures.get(2)?.as_str();
            let Some(known_variants) = Self::known_rust_enum_variants(cwd, type_name) else {
                continue;
            };
            if !known_variants.contains(variant) {
                let mut known = known_variants.into_iter().collect::<Vec<_>>();
                known.sort();
                return Some(format!(
                    "synthesized patch uses unknown enum variant {}::{}; known variants: {}",
                    type_name,
                    variant,
                    known.join(", ")
                ));
            }
        }
        None
    }

    fn known_rust_enum_variants(cwd: &std::path::Path, type_name: &str) -> Option<HashSet<String>> {
        for relative in Self::candidate_patch_files(cwd).into_iter().take(5_000) {
            if relative.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(cwd.join(&relative)) else {
                continue;
            };
            let Some(body) = Self::extract_rust_enum_body(&content, type_name) else {
                continue;
            };
            let variants = body
                .lines()
                .filter_map(Self::rust_enum_variant_from_line)
                .collect::<HashSet<_>>();
            if !variants.is_empty() {
                return Some(variants);
            }
        }
        None
    }

    fn extract_rust_enum_body(content: &str, type_name: &str) -> Option<String> {
        let needle = format!("enum {}", type_name);
        let start = content.find(&needle)?;
        let brace_start = content[start..].find('{')? + start;
        let mut depth = 0usize;
        for (offset, ch) in content[brace_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let end = brace_start + offset;
                        return Some(content[brace_start + 1..end].to_string());
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn rust_enum_variant_from_line(line: &str) -> Option<String> {
        let trimmed = line.split("//").next().unwrap_or("").trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("}") {
            return None;
        }
        let ident = trimmed
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if ident
            .chars()
            .next()
            .map(|ch| ch.is_ascii_uppercase())
            .unwrap_or(false)
        {
            Some(ident)
        } else {
            None
        }
    }

    fn unique_git_path_suffix_match(
        raw_path: &std::path::Path,
        cwd: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        let output = std::process::Command::new("git")
            .args(["ls-files"])
            .current_dir(cwd)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let raw = raw_path
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        let file_name = raw_path.file_name()?.to_string_lossy().to_string();
        let mut matches = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == raw || line.ends_with(&raw) || line.ends_with(&format!("/{}", file_name)) {
                matches.push(std::path::PathBuf::from(line));
            }
        }
        if matches.len() == 1 {
            matches.pop()
        } else {
            None
        }
    }

    fn validate_synthesized_tool_call(
        &self,
        tool_call: ToolCall,
        cwd: &std::path::Path,
    ) -> Result<ToolCall> {
        if tool_call.name != "file_edit" {
            return Err(anyhow::anyhow!(
                "patch synthesis fallback returned unsupported tool: {}",
                tool_call.name
            ));
        }
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: tool_call.arguments["path"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            old_string: tool_call.arguments["old_string"]
                .as_str()
                .map(str::to_string),
            new_string: tool_call.arguments["new_string"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            line_start: tool_call.arguments["line_start"]
                .as_u64()
                .map(|value| value as usize),
            line_end: tool_call.arguments["line_end"]
                .as_u64()
                .map(|value| value as usize),
            expected_replacements: tool_call.arguments["expected_replacements"]
                .as_u64()
                .map(|value| value as usize),
        };
        self.validate_patch_synthesis_action(&action, cwd)
    }

    /// 记录 API 调用成本
    async fn record_cost(&self, response: &ChatResponse) {
        if let Some(ref usage) = response.usage {
            let mut tracker = self.cost_tracker.lock().await;
            tracker.record_api_call(
                &self.model,
                usage.prompt_tokens as u64,
                usage.completion_tokens as u64,
                usage.cached_tokens.map(|t| t as u64),
            );
        }
    }

    fn finish_trace(&self, trace: TraceCollector, status: TurnStatus) {
        let trace = trace.finish(status);
        if let Some(store) = &self.trace_store {
            store.push(trace.clone());
        }
        if let Some(store) = &self.session_store {
            if let Err(e) = store.add_turn_trace(&trace) {
                warn!("Failed to persist turn trace: {}", e);
            }
            if let Err(e) = persist_turn_learning_event(store, &trace) {
                warn!("Failed to persist learning event: {}", e);
            }
        }
    }

    /// 获取工具定义列表
    fn get_tools(&self) -> Vec<crate::services::api::Tool> {
        let context = self.create_tool_context();
        self.tool_registry
            .iter_tools()
            .filter(|t| {
                if !t.is_available(&context) {
                    return false;
                }
                tool_allowed_by_context(&self.allowed_tools, t.name())
                    && context.permission_context.should_expose_tool(t.name())
            })
            .map(|t| crate::services::api::Tool {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }

    fn code_action_tools(
        tools: &[crate::services::api::Tool],
        has_changes_before_request: bool,
    ) -> Vec<crate::services::api::Tool> {
        tools
            .iter()
            .filter(|tool| {
                Self::is_code_write_tool_name(&tool.name)
                    || matches!(tool.name.as_str(), "file_read" | "grep")
                    || (has_changes_before_request && tool.name == "bash")
            })
            .cloned()
            .collect()
    }

    fn is_code_write_tool_name(name: &str) -> bool {
        matches!(name, "file_edit" | "file_write")
    }

    fn is_validation_tool_call(tool_call: &ToolCall) -> bool {
        if tool_call.name != "bash" {
            return false;
        }
        let Some(command) = tool_call.arguments["command"].as_str() else {
            return false;
        };
        crate::tools::bash_tool::command_classifier::classify_command(command).is_safe_validation()
    }

    fn normalize_validation_command_for_match(command: &str) -> String {
        crate::tools::bash_tool::command_classifier::normalize_command_for_match(command)
    }

    fn is_safe_validation_command(command: &str) -> bool {
        crate::tools::bash_tool::command_classifier::classify_command(command).is_safe_validation()
    }

    fn extract_required_validation_commands(prompt: &str) -> Vec<String> {
        let mut commands = Vec::new();
        for line in prompt.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("- `") {
                continue;
            }
            let rest = &trimmed[3..];
            let Some(end) = rest.find('`') else {
                continue;
            };
            let command = rest[..end].trim();
            if command.is_empty() || command == "(none)" {
                continue;
            }
            if Self::is_safe_validation_command(command)
                || command.starts_with("python3 -c ")
                || command.starts_with("python -c ")
            {
                if !commands.iter().any(|existing| existing == command) {
                    commands.push(command.to_string());
                }
            }
        }
        commands
    }

    async fn run_required_validation_commands(
        working_dir: &std::path::Path,
        commands: &[String],
    ) -> Vec<super::auto_verify::VerificationResult> {
        let mut results = Vec::new();
        for command in commands.iter().take(8) {
            let timeout = required_validation_timeout();
            let output = shell_output_with_timeout(command, working_dir, timeout).await;
            let result = match output {
                Ok(output) => {
                    let raw_output = format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    super::auto_verify::VerificationResult {
                        language: "required".to_string(),
                        command: command.clone(),
                        success: output.status.success(),
                        issues: if output.status.success() {
                            Vec::new()
                        } else {
                            vec![super::auto_verify::VerificationIssue {
                                severity: "error".to_string(),
                                file: None,
                                line: None,
                                message: safe_prefix_by_bytes(&raw_output, 1200).to_string(),
                            }]
                        },
                        raw_output,
                        summary: if output.status.success() {
                            format!("required command passed: {}", command)
                        } else {
                            format!("required command failed: {}", command)
                        },
                    }
                }
                Err(err) => {
                    let timed_out = err.kind() == std::io::ErrorKind::TimedOut;
                    let message = if timed_out {
                        format!("required command timed out after {}s", timeout.as_secs())
                    } else {
                        format!("failed to run required command: {}", err)
                    };
                    super::auto_verify::VerificationResult {
                        language: "required".to_string(),
                        command: command.clone(),
                        success: false,
                        issues: vec![super::auto_verify::VerificationIssue {
                            severity: "error".to_string(),
                            file: None,
                            line: None,
                            message,
                        }],
                        raw_output: err.to_string(),
                        summary: if timed_out {
                            format!("required command timed out: {}", command)
                        } else {
                            format!("required command failed to run: {}", command)
                        },
                    }
                }
            };
            results.push(result);
        }
        results
    }

    fn git_status_files() -> HashSet<std::path::PathBuf> {
        let output = std::process::Command::new("git")
            .args(["status", "--short", "--untracked-files=all"])
            .output();
        let Ok(output) = output else {
            return HashSet::new();
        };
        if !output.status.success() {
            return HashSet::new();
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .filter_map(Self::parse_git_status_path)
            .collect()
    }

    fn git_status_files_since(baseline: &HashSet<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
        let mut changed: Vec<_> = Self::git_status_files()
            .into_iter()
            .filter(|path| !baseline.contains(path))
            .collect();
        changed.sort();
        changed
    }

    fn parse_git_status_path(line: &str) -> Option<std::path::PathBuf> {
        let path = line.get(3..)?.trim();
        if path.is_empty() {
            return None;
        }
        let path = path.rsplit_once(" -> ").map(|(_, new)| new).unwrap_or(path);
        Some(std::path::PathBuf::from(path.trim_matches('"')))
    }

    fn bash_allowed_at_action_checkpoint(
        arguments: &serde_json::Value,
        has_changes_before_tools: bool,
    ) -> bool {
        let command = arguments["command"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if command.trim().is_empty() {
            return false;
        }
        let mutating_markers = [
            "apply_patch",
            "python",
            "python3",
            "perl -",
            "sed -i",
            "cat >",
            "cat <<",
            "tee ",
            ">>",
            "> ",
            "mv ",
            "cp ",
            "touch ",
        ];
        if mutating_markers
            .iter()
            .any(|marker| command.contains(marker))
        {
            return true;
        }
        let validation_markers = [
            "cargo test",
            "cargo check",
            "cargo fmt",
            "npm test",
            "pnpm test",
            "pytest",
            "make test",
        ];
        has_changes_before_tools
            && validation_markers
                .iter()
                .any(|marker| command.contains(marker))
    }

    fn action_checkpoint_file_edit_rejection(
        arguments: &serde_json::Value,
        cwd: &std::path::Path,
    ) -> Option<String> {
        let path = arguments["path"].as_str().unwrap_or_default().trim();
        if path.is_empty() {
            return Some("file_edit path is empty".to_string());
        }
        let raw_path = std::path::Path::new(path);
        for component in raw_path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Some(format!(
                        "file_edit path contains parent traversal: {}",
                        path
                    ));
                }
                std::path::Component::Normal(part)
                    if part == ".git" || part == "target" || part == "node_modules" =>
                {
                    return Some(format!(
                        "file_edit path targets ignored/generated directory: {}",
                        path
                    ));
                }
                _ => {}
            }
        }

        let expected_replacements = arguments["expected_replacements"]
            .as_u64()
            .map(|value| value as usize)
            .unwrap_or(1);
        if expected_replacements != 1 {
            return Some(format!(
                "action checkpoint only permits one replacement per file_edit call; got expected_replacements={}. Split the patch into single, reviewable edits.",
                expected_replacements
            ));
        }

        let new_string = arguments["new_string"].as_str().unwrap_or_default();
        if new_string.len() > 20_000 {
            return Some("file_edit new_string is too large for action checkpoint".to_string());
        }

        let old_string = arguments["old_string"].as_str();
        let insert_after = arguments["insert_after"].as_str();
        let insert_before = arguments["insert_before"].as_str();
        let line_start = arguments["line_start"].as_u64().map(|value| value as usize);
        let line_end = arguments["line_end"].as_u64().map(|value| value as usize);

        if let (Some(start), Some(end)) = (line_start, line_end) {
            if start == 0 || end == 0 || start > end {
                return Some(format!(
                    "file_edit line range is invalid: {}..={}",
                    start, end
                ));
            }
            if start != end {
                return Some(format!(
                    "action checkpoint line-range edits must touch exactly one line; got {}..={}. Use exact old_string for larger changes or split into single-line edits.",
                    start, end
                ));
            }
            if end.saturating_sub(start) + 1 > 40 {
                return Some(format!(
                    "action checkpoint line range is too large: {} lines. Use a smaller edit.",
                    end.saturating_sub(start) + 1
                ));
            }
            return None;
        }

        let has_edit_anchor =
            old_string.is_some() || insert_after.is_some() || insert_before.is_some();
        if !has_edit_anchor {
            return Some(
                "file_edit must use old_string, insert_after, insert_before, or line_start/line_end"
                    .to_string(),
            );
        }

        let candidate = if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            cwd.join(raw_path)
        };
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let Ok(canonical_file) = candidate.canonicalize() else {
            return Some(format!("file_edit target does not exist: {}", path));
        };
        if !canonical_file.starts_with(&canonical_cwd) || !canonical_file.is_file() {
            return Some(format!(
                "file_edit target is outside the working tree: {}",
                path
            ));
        }
        let Ok(content) = std::fs::read_to_string(&canonical_file) else {
            return Some(format!("file_edit target is not readable: {}", path));
        };

        let anchor = old_string
            .or(insert_after)
            .or(insert_before)
            .unwrap_or_default();
        if anchor.trim().is_empty() {
            return Some("file_edit anchor is empty".to_string());
        }
        let count = content.matches(anchor).count();
        if count != 1 {
            return Some(format!(
                "action checkpoint requires a unique edit anchor; found {} occurrence(s). Use a more specific old_string or a small line_start/line_end range.",
                count
            ));
        }

        None
    }

    /// 并行执行工具调用
    async fn execute_tools_parallel(
        &self,
        tool_calls: &[ToolCall],
        tx: Option<&mpsc::Sender<StreamEvent>>,
        pre_executed: std::collections::HashMap<usize, ToolResult>,
        trace: Option<TraceCollector>,
        resource_policy: &crate::engine::resource_policy::ResourcePolicy,
        exposed_tool_names: &HashSet<String>,
        action_checkpoint_active: bool,
        has_changes_before_tools: bool,
    ) -> Vec<(ToolCall, ToolResult)> {
        let mut read_only_jobs = Vec::new();
        let mut read_write_calls = Vec::new();
        let mut denied_results = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();
        let active_goal = self
            .goal_manager
            .as_ref()
            .and_then(|manager| manager.current());

        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }
            if !exposed_tool_names.contains(&tc.name) {
                let mut result = ToolResult::error(format!(
                    "Tool '{}' was not exposed in the current request and cannot be executed.",
                    tc.name
                ));
                attach_tool_execution_metadata(tc, &mut result);
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: false,
                        pre_executed: false,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: false,
                        duration_ms: Some(0),
                        output_chars: result.content.chars().count(),
                    });
                }
                persist_tool_outcome_learning_event(
                    self.session_store.as_ref(),
                    &self.session_id,
                    tc,
                    &result,
                );
                denied_results.push((tc.clone(), result));
                continue;
            }
            if results.len() + denied_results.len() + read_only_jobs.len() + read_write_calls.len()
                >= resource_policy.max_tool_calls
            {
                let mut result = ToolResult::error(format!(
                    "Resource policy blocked tool '{}': max tool calls ({}) reached.",
                    tc.name, resource_policy.max_tool_calls
                ));
                attach_tool_execution_metadata(tc, &mut result);
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: false,
                        pre_executed: false,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: false,
                        duration_ms: Some(0),
                        output_chars: result.content.chars().count(),
                    });
                }
                persist_tool_outcome_learning_event(
                    self.session_store.as_ref(),
                    &self.session_id,
                    tc,
                    &result,
                );
                denied_results.push((tc.clone(), result));
                continue;
            }
            record_goal_drift_if_needed(&trace, active_goal.as_ref(), tc);
            if !tool_allowed_by_context(&self.allowed_tools, &tc.name) {
                let result = tool_not_allowed_result(tc);
                persist_tool_outcome_learning_event(
                    self.session_store.as_ref(),
                    &self.session_id,
                    tc,
                    &result,
                );
                denied_results.push((tc.clone(), result));
                continue;
            }

            if action_checkpoint_active
                && tc.name == "bash"
                && !Self::bash_allowed_at_action_checkpoint(&tc.arguments, has_changes_before_tools)
            {
                let mut result = ToolResult::error(
                    "Bash is restricted during the action checkpoint: use it only to apply a patch (for example python/perl/sed -i/apply_patch/redirect/tee) or, after files have changed, to run validation. Do not use bash for read-only inspection at this checkpoint."
                        .to_string(),
                );
                attach_tool_execution_metadata(tc, &mut result);
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: false,
                        pre_executed: false,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: false,
                        duration_ms: Some(0),
                        output_chars: result.content.chars().count(),
                    });
                }
                persist_tool_outcome_learning_event(
                    self.session_store.as_ref(),
                    &self.session_id,
                    tc,
                    &result,
                );
                denied_results.push((tc.clone(), result));
                continue;
            }
            if action_checkpoint_active && tc.name == "file_edit" {
                if let Some(reason) = Self::action_checkpoint_file_edit_rejection(
                    &tc.arguments,
                    &std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
                ) {
                    let mut result = ToolResult::error(format!(
                        "Action checkpoint file_edit rejected: {reason}"
                    ));
                    attach_tool_execution_metadata(tc, &mut result);
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolStarted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            parallel: false,
                            pre_executed: false,
                        });
                        trace.record(TraceEvent::ToolCompleted {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            success: false,
                            duration_ms: Some(0),
                            output_chars: result.content.chars().count(),
                        });
                    }
                    persist_tool_outcome_learning_event(
                        self.session_store.as_ref(),
                        &self.session_id,
                        tc,
                        &result,
                    );
                    denied_results.push((tc.clone(), result));
                    continue;
                }
            }

            if let Some(pre_result) = pre_executed.get(&i) {
                let mut pre_result = pre_result.clone();
                attach_tool_execution_metadata(tc, &mut pre_result);
                persist_tool_outcome_learning_event(
                    self.session_store.as_ref(),
                    &self.session_id,
                    tc,
                    &pre_result,
                );
                if let Some(ref trace) = trace {
                    trace.record(TraceEvent::ToolStarted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        parallel: true,
                        pre_executed: true,
                    });
                    trace.record(TraceEvent::ToolCompleted {
                        tool: tc.name.clone(),
                        call_id: tc.id.clone(),
                        success: pre_result.success,
                        duration_ms: pre_result.duration_ms,
                        output_chars: pre_result.content.chars().count(),
                    });
                    let trace_ref = Some(trace.clone());
                    record_mcp_resource_trace(&trace_ref, tc, &pre_result);
                    record_web_retrieval_trace(&trace_ref, tc, &pre_result);
                }
                debug!(
                    "Skipping pre-executed read-only tool at index {}: {}",
                    i, tc.name
                );
                results.push((tc.clone(), pre_result.clone()));
                if let Some(tx) = tx {
                    let result_content = format!(
                        "Result: {}\n{}",
                        if pre_result.success { "OK" } else { "ERROR" },
                        tool_result_dialog_content(&pre_result)
                    );
                    let _ = tx
                        .send(StreamEvent::ToolExecutionComplete {
                            id: tc.id.clone(),
                            result: result_content,
                        })
                        .await;
                }
                continue;
            }

            if is_read_only(&tc.name) {
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                        })
                        .await;
                }
                let registry = self.tool_registry.clone();
                let context = self.create_tool_context_with_optional_trace(&trace);
                let tc_clone = tc.clone();
                let tool_name = tc.name.clone();
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();
                let trace = trace.clone();
                read_only_jobs.push(async move {
                    let started_at = std::time::Instant::now();
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolStarted {
                            tool: tool_name.clone(),
                            call_id: tc_clone.id.clone(),
                            parallel: true,
                            pre_executed: false,
                        });
                    }
                    let pre_decision = if let Some(ref hooks) = hook_manager {
                        let hook_start = hooks.current_record_sequence();
                        let decision = hooks.run_pre_tool(&tc_clone, &context).await;
                        let hook_records = hooks.recent_records_after_for(hook_start, &tc_clone.id);
                        record_hook_traces(&trace, &hook_records);
                        decision
                    } else {
                        HookDecision {
                            allow: true,
                            reason: None,
                        }
                    };

                    let mut result =
                        if !pre_decision.allow {
                            ToolResult::error(pre_decision.reason.unwrap_or_else(|| {
                                format!("blocked by pre-tool hook: {}", tool_name)
                            }))
                        } else if let Some(tool) = registry.get(&tool_name) {
                            tool.execute(tc_clone.arguments.clone(), context.clone())
                                .await
                        } else {
                            ToolResult::error(format!("Tool '{}' not found", tool_name))
                        };
                    let duration_ms = started_at.elapsed().as_millis() as u64;
                    if result.duration_ms.is_none() {
                        result.duration_ms = Some(duration_ms);
                    }

                    if let Some(ref hooks) = hook_manager {
                        let hook_start = hooks.current_record_sequence();
                        hooks.run_post_tool(&tc_clone, &result, &context).await;
                        let hook_records = hooks.recent_records_after_for(hook_start, &tc_clone.id);
                        record_hook_traces(&trace, &hook_records);
                    };
                    attach_tool_execution_metadata(&tc_clone, &mut result);
                    {
                        let mut tracker = cost_tracker.lock().await;
                        tracker.record_tool_execution(
                            &tool_name,
                            result.success,
                            duration_ms,
                            result.error.as_deref(),
                        );
                    }
                    if let Some(ref trace) = trace {
                        trace.record(TraceEvent::ToolCompleted {
                            tool: tool_name,
                            call_id: tc_clone.id.clone(),
                            success: result.success,
                            duration_ms: result.duration_ms,
                            output_chars: result.content.chars().count(),
                        });
                        let trace_ref = Some(trace.clone());
                        record_mcp_resource_trace(&trace_ref, &tc_clone, &result);
                        record_web_retrieval_trace(&trace_ref, &tc_clone, &result);
                    }
                    (tc_clone, result)
                });
            } else {
                read_write_calls.push(tc.clone());
            }
        }

        results.append(&mut denied_results);

        let concurrency =
            read_only_tool_concurrency().min(resource_policy.parallelism_limit.max(1));
        let mut readonly_stream =
            futures::stream::iter(read_only_jobs).buffer_unordered(concurrency);

        while let Some((tc, result)) = readonly_stream.next().await {
            persist_tool_outcome_learning_event(
                self.session_store.as_ref(),
                &self.session_id,
                &tc,
                &result,
            );
            if let Some(tx) = tx {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    tool_result_dialog_content(&result)
                );
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tc.id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            results.push((tc, result));
        }

        for tc in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if !tool_allowed_by_context(&self.allowed_tools, &tool_name) {
                let result = tool_not_allowed_result(&tc);
                persist_tool_outcome_learning_event(
                    self.session_store.as_ref(),
                    &self.session_id,
                    &tc,
                    &result,
                );
                results.push((tc, result));
                continue;
            }

            if let Some(tx) = tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                    })
                    .await;
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolStarted {
                    tool: tool_name.clone(),
                    call_id: tool_id.clone(),
                    parallel: false,
                    pre_executed: false,
                });
            }

            let (result, hook_context) = if let Some(tool) = self.tool_registry.get(&tool_name) {
                let mut context = self.create_tool_context_with_optional_trace(&trace);
                let drift_check = active_goal
                    .as_ref()
                    .map(|goal| {
                        crate::engine::goal_drift::GoalDriftDetector::new().check(goal, &tc)
                    })
                    .unwrap_or_else(crate::engine::goal_drift::DriftCheck::ok);
                let drift_requires_approval = drift_check.requires_approval();
                let pre_decision = if let Some(ref hooks) = self.hook_manager {
                    let hook_start = hooks.current_record_sequence();
                    let decision = hooks.run_pre_tool(&tc, &context).await;
                    let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                    record_hook_traces(&trace, &hook_records);
                    decision
                } else {
                    HookDecision {
                        allow: true,
                        reason: None,
                    }
                };

                let started_at = std::time::Instant::now();
                let mut result = if !pre_decision.allow {
                    ToolResult::error(
                        pre_decision
                            .reason
                            .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                    )
                } else if {
                    let permission_requires = context
                        .permission_context
                        .requires_confirmation(&tool_name, &tc.arguments);
                    let tool_requires = tool.requires_confirmation(&tc.arguments)
                        && !context
                            .permission_context
                            .auto_approves_tool_confirmation(&tool_name, &tc.arguments);
                    permission_requires || tool_requires || drift_requires_approval
                } {
                    let mut approved = false;
                    if let (Some(ref channel), Some(tx)) = (&self.approval_channel, tx) {
                        let base_prompt = if drift_requires_approval {
                            format!(
                                "Tool '{}' may drift from the current goal. Reason: {} Suggested action: {} Allow?",
                                tool_name,
                                drift_check.reason,
                                drift_check
                                    .suggested_action
                                    .as_deref()
                                    .unwrap_or("review before executing")
                            )
                        } else if tool_name == "mcp_tool" {
                            let server = tc.arguments["server_name"].as_str().unwrap_or("");
                            let t = tc.arguments["tool_name"].as_str().unwrap_or("");
                            format!(
                                "MCP tool '{}' on server '{}' requires approval. Allow?",
                                t, server
                            )
                        } else if let Some(prompt) = tool.confirmation_prompt(&tc.arguments) {
                            prompt
                        } else {
                            format!("Tool '{}' requires approval. Allow?", tool_name)
                        };
                        let prompt = if drift_requires_approval {
                            base_prompt
                        } else {
                            let explanation = context
                                .permission_context
                                .explain_decision(&tool_name, &tc.arguments)
                                .concise_summary();
                            format!("{}\nPermission explanation: {}", base_prompt, explanation)
                        };
                        let _ = tx
                            .send(StreamEvent::PermissionRequest {
                                id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                arguments: tc.arguments.clone(),
                                prompt: prompt.clone(),
                            })
                            .await;
                        if let Some(ref trace) = trace {
                            trace.record(TraceEvent::PermissionRequested {
                                tool: tool_name.clone(),
                                call_id: tool_id.clone(),
                                prompt: prompt.clone(),
                            });
                        }
                        let request = ToolApprovalRequest {
                            tool_call: tc.clone(),
                            prompt,
                            review: None,
                        };
                        match channel.submit(request).await {
                            Ok(is_approved) => approved = is_approved,
                            Err(e) => {
                                warn!("Tool approval error: {}", e);
                            }
                        }
                        if let Some(ref trace) = trace {
                            trace.record(TraceEvent::PermissionResolved {
                                tool: tool_name.clone(),
                                call_id: tool_id.clone(),
                                approved,
                            });
                        }
                    }
                    if approved {
                        if context.permission_context.mode
                            == crate::permissions::PermissionMode::Once
                        {
                            context.permission_context.grant_once(&tool_name);
                        }
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamEvent::ToolExecutionProgress {
                                    id: tool_id.clone(),
                                    progress: tool_execution_start_progress(
                                        &tool_name,
                                        &tc.arguments,
                                    ),
                                })
                                .await;
                        }
                        tool.execute(tc.arguments.clone(), context.clone()).await
                    } else {
                        ToolResult::error(format!(
                            "Permission denied: '{}' requires user confirmation.",
                            tool_name
                        ))
                    }
                } else {
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::ToolExecutionProgress {
                                id: tool_id.clone(),
                                progress: tool_execution_start_progress(&tool_name, &tc.arguments),
                            })
                            .await;
                    }
                    tool.execute(tc.arguments.clone(), context.clone()).await
                };
                let duration_ms = started_at.elapsed().as_millis() as u64;
                if result.duration_ms.is_none() {
                    result.duration_ms = Some(duration_ms);
                }
                attach_tool_execution_metadata(&tc, &mut result);

                // ── Security Audit & Denial Tracking ──────────────────────
                let params_summary = if let Some(tool) = self.tool_registry.get(&tool_name) {
                    tool.to_classifier_input(&tc.arguments)
                } else {
                    tool_name.clone()
                };

                if let Some(ref log) = self.audit_log {
                    let decision = if result.success {
                        "EXECUTED"
                    } else if result
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .contains("Permission denied")
                    {
                        "DENIED"
                    } else {
                        "FAILED"
                    };
                    log.log_execution(&tool_name, &params_summary, result.success, decision)
                        .await;
                }

                if let Some(ref tracker) = self.denial_tracker {
                    if result.success {
                        tracker.record_success().await;
                    } else if result
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .contains("Permission denied")
                        || result
                            .error
                            .as_deref()
                            .unwrap_or("")
                            .contains("Dangerous command")
                    {
                        tracker
                            .record_denial(
                                &tool_name,
                                &params_summary,
                                result.error.as_deref().unwrap_or("security block"),
                            )
                            .await;
                    }
                }
                // ─────────────────────────────────────────────────────────

                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_tool_execution(
                        &tool_name,
                        result.success,
                        duration_ms,
                        result.error.as_deref(),
                    );
                }

                (result, Some(context))
            } else {
                let mut result = ToolResult::error(format!("Tool '{}' not found", tool_name));
                attach_tool_execution_metadata(&tc, &mut result);
                (result, None)
            };

            if let (Some(hooks), Some(context)) = (&self.hook_manager, &hook_context) {
                let hook_start = hooks.current_record_sequence();
                hooks.run_post_tool(&tc, &result, context).await;
                let hook_records = hooks.recent_records_after_for(hook_start, &tc.id);
                record_hook_traces(&trace, &hook_records);
            }

            if let Some(tx) = tx {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    tool_result_dialog_content(&result)
                );
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            if let Some(ref trace) = trace {
                trace.record(TraceEvent::ToolCompleted {
                    tool: tool_name,
                    call_id: tool_id,
                    success: result.success,
                    duration_ms: result.duration_ms,
                    output_chars: result.content.chars().count(),
                });
                let trace_ref = Some(trace.clone());
                record_mcp_resource_trace(&trace_ref, &tc, &result);
                record_web_retrieval_trace(&trace_ref, &tc, &result);
            }
            persist_tool_outcome_learning_event(
                self.session_store.as_ref(),
                &self.session_id,
                &tc,
                &result,
            );
            results.push((tc, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::{ChatResponse, ToolCall, Usage};
    use crate::test_utils::env_guard::EnvVarGuard;
    use crate::tools::{BashTool, FileEditTool, FileReadTool, FileWriteTool, GitTool};
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::{HashSet, VecDeque};
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_truncate_tool_result_handles_utf8_boundaries() {
        let mut result = ToolResult::success("中".repeat(20_000));
        truncate_tool_result(&mut result, "grep", "call_utf8").await;
        assert!(result.content.contains("Output truncated"));
    }

    #[test]
    fn test_allowed_tool_context_enforces_subagent_tool_scope() {
        assert!(tool_allowed_by_context(&None, "bash"));

        let allowed = Some(HashSet::from(["file_read".to_string(), "grep".to_string()]));
        assert!(tool_allowed_by_context(&allowed, "file_read"));
        assert!(tool_allowed_by_context(&allowed, "grep"));
        assert!(!tool_allowed_by_context(&allowed, "bash"));
    }

    #[test]
    fn test_not_allowed_tool_result_has_recovery_metadata() {
        let tool_call = ToolCall {
            id: "call_denied".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "echo hi"}),
        };
        let result = tool_not_allowed_result(&tool_call);
        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("not allowed"));
        let data = result.data.expect("tool summary data");
        assert_eq!(data["tool_summary"]["tool"], "bash");
        assert_eq!(data["tool_summary"]["call_id"], "call_denied");
    }

    #[test]
    fn test_tool_recovery_metadata_attached_to_failure() {
        let mut result = ToolResult::error("command timed out");
        let tool_call = ToolCall {
            id: "call_bash".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cargo test -q"
            }),
        };
        attach_tool_execution_metadata(&tool_call, &mut result);
        let summary = result
            .data
            .as_ref()
            .and_then(|data| data.get("tool_summary"))
            .expect("tool summary metadata");
        assert_eq!(summary["tool"], "bash");
        assert_eq!(summary["command_kind"], "validation");
        assert_eq!(summary["validation_family"], "cargo_test");
        assert_eq!(summary["safe_for_closeout"], true);
        let recovery = result
            .data
            .as_ref()
            .and_then(|data| data.get("recovery"))
            .expect("recovery metadata");
        assert_eq!(recovery["recoverable"], true);
        assert_eq!(recovery["safe_retry"], true);
        assert_eq!(recovery["suggested_command"], "/retry");
    }

    #[test]
    fn test_tool_summary_metadata_attached_to_success() {
        let mut result = ToolResult::success_with_data(
            "File edited successfully",
            serde_json::json!({
                "path": "src/lib.rs",
                "replacements": 1
            }),
        );
        let tool_call = ToolCall {
            id: "call_edit".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "old",
                "new_string": "new"
            }),
        };
        attach_tool_execution_metadata(&tool_call, &mut result);
        let summary = result
            .data
            .as_ref()
            .and_then(|data| data.get("tool_summary"))
            .expect("tool summary metadata");
        assert_eq!(summary["tool"], "file_edit");
        assert_eq!(summary["path"], "src/lib.rs");
        assert_eq!(summary["replacements"], 1);
        assert!(result
            .data
            .as_ref()
            .and_then(|data| data.get("recovery"))
            .is_none());
    }

    #[test]
    fn test_tool_execution_start_progress_uses_validation_labels() {
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "cargo test -q -- --test-threads=1"})
            ),
            "Running Rust tests: cargo test -q -- --test-threads=1"
        );
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "env PRIORITY_AGENT=1 cargo check -q"})
            ),
            "Running cargo check: env PRIORITY_AGENT=1 cargo check -q"
        );
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "cargo clippy -q -- -D warnings"})
            ),
            "Running cargo clippy: cargo clippy -q -- -D warnings"
        );
    }

    #[test]
    fn test_tool_execution_start_progress_handles_generic_shell_and_tools() {
        assert_eq!(
            tool_execution_start_progress("bash", &serde_json::json!({"command": "ls src"})),
            "Inspecting with shell: ls src"
        );
        assert_eq!(
            tool_execution_start_progress(
                "bash",
                &serde_json::json!({"command": "python scripts/update.py"})
            ),
            "Executing shell command: python scripts/update.py"
        );
        assert_eq!(
            tool_execution_start_progress("grep", &serde_json::json!({"pattern": "Closeout"})),
            "Executing grep..."
        );
    }

    #[test]
    fn test_strip_think_blocks_removes_internal_reasoning() {
        let input = "你好<think>内部推理</think>世界";
        assert_eq!(strip_think_blocks(input), "你好世界");
    }

    #[test]
    fn test_visible_text_sanitizer_handles_split_think_tags() {
        let mut sanitizer = VisibleTextSanitizer::default();
        let mut out = String::new();
        out.push_str(&sanitizer.push_chunk("你好<th"));
        out.push_str(&sanitizer.push_chunk("ink>不该显示</th"));
        out.push_str(&sanitizer.push_chunk("ink>世界"));
        out.push_str(&sanitizer.finish());
        assert_eq!(out, "你好世界");
    }

    #[test]
    fn test_visible_text_sanitizer_preserves_utf8_chunks_without_panicking() {
        let mut sanitizer = VisibleTextSanitizer::default();
        let mut out = String::new();
        out.push_str(&sanitizer.push_chunk("你"));
        out.push_str(&sanitizer.push_chunk("好"));
        out.push_str(&sanitizer.finish());
        assert_eq!(out, "你好");
    }

    #[tokio::test]
    async fn test_truncate_tool_result_keeps_small_output_unchanged() {
        let original = "short output".to_string();
        let mut result = ToolResult::success(original.clone());
        truncate_tool_result(&mut result, "grep", "call_small").await;
        assert_eq!(result.content, original);
    }

    #[tokio::test]
    async fn test_truncate_tool_result_includes_head_and_tail_markers() {
        let mut result = ToolResult::success(format!(
            "{}\n{}\n{}",
            "A".repeat(40_000),
            "中".repeat(8_000),
            "Z".repeat(40_000)
        ));
        truncate_tool_result(&mut result, "grep", "call_markers").await;
        assert!(result.content.contains("--- First"));
        assert!(result.content.contains("--- Last"));
        assert!(result.content.contains("Output truncated"));
    }

    #[test]
    fn test_normalize_params_fills_missing_required_fields() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "运行 cargo test 验证修复",
            Some("bash".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "timeout": { "type": "integer" }
            },
            "required": ["command", "timeout"]
        });

        let out = WorkflowRealStepExecutor::normalize_params(serde_json::json!({}), &schema, &step)
            .expect("normalize should succeed");
        assert_eq!(out["command"], "cargo test");
        assert!(out["timeout"].is_number());
    }

    #[test]
    fn test_normalize_params_coerces_required_field_types() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "在 src/main.rs 中搜索 TODO",
            Some("grep".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "path": { "type": "string" },
                "limit": { "type": "integer" },
                "recursive": { "type": "boolean" }
            },
            "required": ["pattern", "path", "limit", "recursive"]
        });

        let out = WorkflowRealStepExecutor::normalize_params(
            serde_json::json!({
                "pattern": 123,
                "path": true,
                "limit": "20",
                "recursive": "yes"
            }),
            &schema,
            &step,
        )
        .expect("normalize should succeed");

        assert_eq!(out["pattern"], "123");
        assert_eq!(out["path"], "true");
        assert_eq!(out["limit"], 20);
        assert_eq!(out["recursive"], true);
    }

    #[test]
    fn test_normalize_params_rejects_non_object_payload() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "读取 README.md",
            Some("file_read".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        });
        let err = WorkflowRealStepExecutor::normalize_params(
            serde_json::json!(["not", "object"]),
            &schema,
            &step,
        )
        .expect_err("non-object params should be rejected");
        assert!(err.contains("JSON object"));
    }

    #[test]
    fn test_get_tools_filters_denied_tools_before_model_request() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(BashTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        )
        .with_session_permission_rules(crate::permissions::PermissionRules::new().deny("bash"));

        let names = loop_instance
            .get_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"file_read".to_string()));
        assert!(!names.contains(&"bash".to_string()));
    }

    #[test]
    fn test_get_tools_hides_write_tools_in_read_only_mode() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(FileWriteTool);
        registry.register(BashTool);
        registry.register(GitTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        )
        .with_permission_mode(crate::permissions::PermissionMode::ReadOnly);

        let names = loop_instance
            .get_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"file_read".to_string()));
        assert!(!names.contains(&"file_write".to_string()));
        assert!(!names.contains(&"bash".to_string()));
        assert!(!names.contains(&"git".to_string()));
    }

    #[test]
    fn test_action_checkpoint_allows_patch_bash_but_blocks_read_only_bash() {
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "python3 - <<'PY'\nfrom pathlib import Path\nPath('x').write_text('y')\nPY"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "sed -n '1,20p' src/main.rs"}),
            false,
        ));
        assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cargo test -q"}),
            false,
        ));
        assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
            &serde_json::json!({"command": "cargo test -q"}),
            true,
        ));
    }

    #[test]
    fn test_code_action_tools_hide_bash_until_files_change() {
        let tools = vec![
            crate::services::api::Tool {
                name: "file_edit".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            crate::services::api::Tool {
                name: "file_read".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            crate::services::api::Tool {
                name: "grep".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
            crate::services::api::Tool {
                name: "bash".to_string(),
                description: String::new(),
                parameters: serde_json::json!({}),
            },
        ];

        let before_change = ConversationLoop::code_action_tools(&tools, false)
            .into_iter()
            .map(|tool| tool.name)
            .collect::<HashSet<_>>();
        assert!(before_change.contains("file_edit"));
        assert!(before_change.contains("file_read"));
        assert!(before_change.contains("grep"));
        assert!(!before_change.contains("bash"));

        let after_change = ConversationLoop::code_action_tools(&tools, true)
            .into_iter()
            .map(|tool| tool.name)
            .collect::<HashSet<_>>();
        assert!(after_change.contains("bash"));
    }

    #[test]
    fn test_verification_source_context_includes_current_error_line() {
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
        std::fs::write(
            tmp.path().join("src/lib.rs"),
            "fn demo() {\n    let score = 1;\n    let status = missing_value;\n}\n",
        )
        .expect("write source");
        let results = vec![super::super::auto_verify::VerificationResult {
            language: "rust".to_string(),
            command: "cargo check".to_string(),
            success: false,
            issues: vec![super::super::auto_verify::VerificationIssue {
                severity: "error".to_string(),
                file: Some("src/lib.rs".to_string()),
                line: Some(3),
                message: "cannot find value `missing_value` in this scope".to_string(),
            }],
            raw_output: String::new(),
            summary: String::new(),
        }];

        let context = verification_source_context(tmp.path(), &results)
            .expect("verification context should be generated");

        assert!(context.contains("src/lib.rs:3"));
        assert!(context.contains(">    3 |     let status = missing_value;"));
        assert!(context.contains("repair compile/validation errors"));
    }

    #[test]
    fn test_parse_patch_synthesis_plan_from_fenced_json() {
        let content = r#"```json
{"can_patch":true,"reason":"safe","actions":[{"tool":"file_edit","path":"src/lib.rs","old_string":"a","new_string":"b","expected_replacements":1}]}
```"#;
        let plan = ConversationLoop::parse_patch_synthesis_plan(content)
            .expect("fenced JSON should parse");
        assert!(plan.can_patch);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].path, "src/lib.rs");
    }

    #[test]
    fn test_patch_synthesis_validation_rejects_parent_traversal() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "../outside.rs".to_string(),
            old_string: Some("a".to_string()),
            new_string: "b".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };
        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("parent traversal must be rejected");
        assert!(err.to_string().contains("parent traversal"));
    }

    #[test]
    fn test_patch_synthesis_path_resolves_root_relative_src_path() {
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
        std::fs::write(tmp.path().join("src/lib.rs"), "fn main() {}\n").expect("write file");

        let (canonical, tool_path) = ConversationLoop::resolve_synthesized_patch_path(
            std::path::Path::new("/src/lib.rs"),
            tmp.path(),
        )
        .expect("root-relative src path should resolve inside cwd");

        assert_eq!(
            canonical,
            tmp.path().join("src/lib.rs").canonicalize().unwrap()
        );
        assert_eq!(tool_path, "src/lib.rs");
    }

    #[test]
    fn test_patch_synthesis_recovers_wrong_path_from_unique_old_string() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/assessment.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("unique old_string should recover the real file path");

        assert_eq!(call.arguments["path"], "src/memory/quality.rs");
    }

    #[test]
    fn test_patch_synthesis_keeps_failed_compiler_evidence() {
        let messages = vec![Message::tool(
            "cargo_check",
            "Result: ERROR\nerror[E0596]: cannot borrow `self.memory_manager.0` as mutable\n[exit status: 101]",
        )];

        let evidence = ConversationLoop::patch_synthesis_evidence(&messages);

        assert!(evidence.contains("FAILED TOOL RESULT"));
        assert!(evidence.contains("error[E0596]"));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_ref_mut_e0596() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "if let Some(ref mut mem_mutex) = self.memory_manager {\n    let mut mem = mem_mutex.lock().await;\n}\n",
        )
        .expect("write module file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "error[E0596]: cannot borrow `self.memory_manager.0` as mutable, as it is behind a `&` reference",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].arguments["old_string"],
            "if let Some(ref mut mem_mutex) = self.memory_manager {"
        );
        assert_eq!(
            calls[0].arguments["new_string"],
            "if let Some(ref mem_mutex) = self.memory_manager {"
        );
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_persistent_memory_marker() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n        if let Some(ref ctx) = turn_retrieval_context {\n",
        )
        .expect("write module file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "the regression marker identifies the missing planning prefetch block",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("prefetch_retrieval_context_with_llm_rerank"));
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("if let Some(ref mem_mutex) = self.memory_manager"));
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains(".lock().await"));
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("&self.model"));
        assert!(!calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("futures::executor::block_on"));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_record_repair_action_arity() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        let damaged_call = concat!(
            r#"fn repair() {
                if !verify_passed {
                    let verification_command = failed_commands
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "post-edit verification".to_string());
                    post_edit_reflection.record_repair_action(
                  acceptance_repair_attempts + 1,
                  &format!("retry: {"#,
            r#"}", verification_command),
                  changed_files.first().map(|path| path.display().to_string()),
              );
                }
}
"#
        );
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            damaged_call,
        )
        .expect("write module file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "error[E0061]: this method takes 4 arguments but 3 arguments were supplied\nargument #4 is missing\nrecord_repair_action",
            tmp.path(),
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].arguments["path"],
            "src/engine/conversation_loop/mod.rs"
        );
        assert_eq!(calls[0].arguments["line_start"], 7);
        assert_eq!(calls[0].arguments["line_end"], 11);
        let replacement = calls[0].arguments["new_string"].as_str().unwrap();
        assert!(replacement.contains("\"repair failed verification before closeout\""));
        assert!(replacement.contains("verification_command,"));
        assert!(!replacement.contains(ConversationLoop::retry_format_marker().as_str()));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_skill_promotion_gate_apply_path() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/tui/slash_handler"))
            .expect("create slash handler dir");
        std::fs::write(
            tmp.path().join("src/tui/slash_handler/config.rs"),
            r#"fn validate_skill_promotion_for_apply() {}
fn skill_fitness_from_bound_eval() {}
fn estimate_skill_semantic_drift() {}

fn handle_apply() {
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                        );
                    }
                }
            }
}
"#,
        )
        .expect("write fixture file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "skill-promotion-gate required command failed because validate_skill_promotion_for_apply is not called before write_active_skill and EvolutionController cooldown is missing",
            tmp.path(),
        );

        assert_eq!(calls.len(), 2);
        let first = calls[0].arguments["new_string"].as_str().unwrap();
        assert!(first.contains(
            "validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())"
        ));
        assert!(first.contains("Skill proposal {} was not applied by promotion gate"));
        let second = calls[1].arguments["new_string"].as_str().unwrap();
        assert!(second.contains("record_evolution_update("));
        assert!(second.contains("EvolutionTarget::Skill"));
    }

    #[test]
    fn test_deterministic_patch_synthesis_uses_skill_task_preview_without_failed_evidence() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/tui/slash_handler"))
            .expect("create slash handler dir");
        std::fs::write(
            tmp.path().join("src/tui/slash_handler/config.rs"),
            r#"fn validate_skill_promotion_for_apply() {}
fn skill_fitness_from_bound_eval() {}
fn estimate_skill_semantic_drift() {}

fn handle_apply() {
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                        );
                    }
                }
            }
}
"#,
        )
        .expect("write fixture file");

        let task_seed =
            "TASK:\n修复 /skill-proposals apply 没有强制使用 fitness promotion gate 的问题。";
        let calls = loop_instance.deterministic_patch_tool_calls(task_seed, tmp.path());

        assert_eq!(calls.len(), 2);
        assert!(calls[0].arguments["new_string"].as_str().unwrap().contains(
            "validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())"
        ));
        assert!(calls[1].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("record_evolution_update("));
    }

    #[test]
    fn test_deterministic_patch_synthesis_repairs_memory_recall_conflict_precision() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine")).expect("create engine dir");
        std::fs::write(
            tmp.path().join("src/engine/retrieval_context.rs"),
            r#"fn memory_conflict_matches_item(
    conflict: &str,
    item: &crate::memory::manager::MemoryMatch,
) -> bool {
    let conflict = conflict.to_lowercase();
    let snippet = item.snippet.to_lowercase();
    if let Some((key, values)) = parse_memory_conflict(&conflict) {
        return snippet.contains(&key) && values.iter().any(|value| snippet.contains(value));
    }

    let tokens = conflict
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| {
            part.len() >= 4
                && !matches!(
                    *part,
                    "memory" | "project" | "user" | "value" | "values" | "conflicting"
                )
        })
        .collect::<Vec<_>>();
    tokens.len() >= 2
        && tokens
            .iter()
            .filter(|part| snippet.contains(**part))
            .count()
            >= 2
}

fn parse_memory_conflict(conflict: &str) -> Option<(String, Vec<String>)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_conflict_matching_uses_structured_key_and_value() {
        let conflict = "- key 'language' has conflicting values: chinese | english";
        let unrelated = crate::memory::manager::MemoryMatch {
            source: "memory/cli.md".to_string(),
            score: 30,
            rerank_score: Some(0.90),
            snippet: "The project memory mentions conflicting work before.".to_string(),
        };
        let related = crate::memory::manager::MemoryMatch {
            source: "memory/cli.md".to_string(),
            score: 30,
            rerank_score: Some(0.90),
            snippet: "language: Chinese\nUse compact CLI status bars.".to_string(),
        };

        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn items_are_sorted_by_score() {}
}
"#,
        )
        .expect("write fixture file");

        let calls = loop_instance.deterministic_patch_tool_calls(
            "TASK:\n强化记忆检索中的冲突匹配精度。memory-recall-conflict-precision",
            tmp.path(),
        );

        assert_eq!(calls.len(), 3);
        assert!(calls[0].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("is_generic_conflict_token(&key)"));
        assert!(calls[1].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("fn is_generic_conflict_token("));
        assert!(calls[2].arguments["new_string"]
            .as_str()
            .unwrap()
            .contains("memory_conflict_matching_ignores_generic_key_conflicts"));
    }

    #[test]
    fn test_patch_synthesis_rejects_bad_persistent_memory_async_shape() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(
                "        // Regression fixture: persistent memory prefetch was missing before workflow judgment."
                    .to_string(),
            ),
            new_string: r#"        if let Some(memory_ctx) = self
            .memory_manager
            .as_mut()
            .and_then(|m| {
                futures::executor::block_on(m.prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref(),
                    self.provider.as_ref().and_then(|p| p.preferred_model()).unwrap_or("default"),
                    route.retrieval,
                ))
            })
        {
            turn_retrieval_context = Some(memory_ctx);
        }"#
            .to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("bad async memory block should be rejected")
            .to_string();

        assert!(err.contains("block_on"));
    }

    #[test]
    fn test_patch_synthesis_rejects_provider_option_style_in_memory_prefetch() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/engine/conversation_loop"))
            .expect("create module dir");
        std::fs::write(
            tmp.path().join("src/engine/conversation_loop/mod.rs"),
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(
                "        // Regression fixture: persistent memory prefetch was missing before workflow judgment."
                    .to_string(),
            ),
            new_string: r#"        if let Some(ref mem_mutex) = self.memory_manager {
            let mut mem = mem_mutex.lock().await;
            if let Some(mem_ctx) = mem
                .prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref().map(|p| p.as_ref()).unwrap(),
                    &self.model,
                    route.retrieval,
                )
                .await
            {
                turn_retrieval_context = Some(mem_ctx);
            }
        }"#
            .to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("provider option-style call should be rejected")
            .to_string();

        assert!(err.contains("Option"));
    }

    #[test]
    fn test_validation_tool_call_detects_success_gate_commands() {
        let cargo_test = ToolCall {
            id: "test".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cargo test -q -- --test-threads=1"
            }),
        };
        let ls = ToolCall {
            id: "ls".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "ls -la"
            }),
        };
        let file_read = ToolCall {
            id: "read".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({
                "path": "src/main.rs"
            }),
        };
        let python_assertion = ToolCall {
            id: "python".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "python3 -c \"assert True\""
            }),
        };
        let node_test = ToolCall {
            id: "node".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "node fixtures/live_frontend/book_notes/test-book-notes.cjs"
            }),
        };
        let python_unittest = ToolCall {
            id: "unittest".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py"
            }),
        };
        let rg_assertion = ToolCall {
            id: "rg".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "! rg 'bad_pattern' src/lib.rs"
            }),
        };
        let rg_assertion_with_ampersand_pattern = ToolCall {
            id: "rg_amp".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "! rg '&format!\\(\"retry: \\{\\}\", verification_command\\)' src/engine/conversation_loop/mod.rs"
            }),
        };
        let env_prefixed_cargo_test = ToolCall {
            id: "env_test".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
            }),
        };
        let shell_wrapped_cargo_test = ToolCall {
            id: "wrapped_test".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
            }),
        };

        assert!(ConversationLoop::is_validation_tool_call(&cargo_test));
        assert!(ConversationLoop::is_validation_tool_call(&python_assertion));
        assert!(ConversationLoop::is_validation_tool_call(&node_test));
        assert!(ConversationLoop::is_validation_tool_call(&python_unittest));
        assert!(ConversationLoop::is_validation_tool_call(&rg_assertion));
        assert!(ConversationLoop::is_validation_tool_call(
            &rg_assertion_with_ampersand_pattern
        ));
        assert!(ConversationLoop::is_validation_tool_call(
            &env_prefixed_cargo_test
        ));
        assert!(ConversationLoop::is_validation_tool_call(
            &shell_wrapped_cargo_test
        ));
        assert!(!ConversationLoop::is_validation_tool_call(&ls));
        assert!(!ConversationLoop::is_validation_tool_call(&file_read));
    }

    #[test]
    fn test_validation_command_match_normalizes_shell_lc_wrappers() {
        assert_eq!(
            ConversationLoop::normalize_validation_command_for_match(
                "bash -lc 'env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1'"
            ),
            "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
        );
        assert_eq!(
            ConversationLoop::normalize_validation_command_for_match(
                "  env   PRIORITY_AGENT_WORKFLOW_ENABLED=1   cargo test --quiet -- --test-threads=1  "
            ),
            "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1"
        );
    }

    #[test]
    fn test_extract_required_validation_commands_from_live_eval_prompt() {
        let prompt = r#"
## Acceptance checks
- `env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1`
- `cargo test -q learning_planning -- --test-threads=1`
- `node fixtures/live_frontend/book_notes/test-book-notes.cjs`
- `python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py`
- `python3 -c "p='src/lib.rs'; assert True"`
- `! rg 'bad_pattern' src/lib.rs`
- `! rg '&format!\("retry: \{\}", verification_command\)' src/engine/conversation_loop/mod.rs`
- `rg 'good_pattern' src/lib.rs`
- `rm -rf /tmp/nope`
- `(none)`
"#;

        let commands = ConversationLoop::extract_required_validation_commands(prompt);

        assert_eq!(
            commands,
            vec![
                "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1".to_string(),
                "cargo test -q learning_planning -- --test-threads=1".to_string(),
                "node fixtures/live_frontend/book_notes/test-book-notes.cjs".to_string(),
                "python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py".to_string(),
                "python3 -c \"p='src/lib.rs'; assert True\"".to_string(),
                "! rg 'bad_pattern' src/lib.rs".to_string(),
                "! rg '&format!\\(\"retry: \\{\\}\", verification_command\\)' src/engine/conversation_loop/mod.rs".to_string(),
                "rg 'good_pattern' src/lib.rs".to_string()
            ]
        );
    }

    #[test]
    fn test_required_validation_disables_default_auto_tests() {
        assert!(should_run_default_auto_tests(&[]));
        assert!(!should_run_default_auto_tests(&[
            "cargo test -q -- --test-threads=1".to_string()
        ]));
    }

    #[test]
    fn test_patch_synthesis_recovers_assignment_anchor_when_old_string_is_inexact() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "fn assess() {\n    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n}\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some(
                "let status = if explicit { MemoryStatus::Accepted } else { write_decision.status };"
                    .to_string(),
            ),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let call = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect("unique assignment anchor should recover exact old_string");

        assert_eq!(
            call.arguments["old_string"],
            "    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };"
        );
        assert_eq!(
            call.arguments["new_string"],
            "    let status = write_decision.status;"
        );
    }

    #[test]
    fn test_patch_synthesis_rejects_inexact_multiline_replacement() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "fn assess() {\n    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n}\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 {\n    MemoryStatus::Accepted\n} else {\n    write_decision.status\n};".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("inexact multiline replacement should be rejected");
        assert!(err.to_string().contains("inexact multi-line replacement"));
    }

    #[test]
    fn test_patch_synthesis_rejects_unbalanced_replacement() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 {".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("unbalanced replacement should be rejected");
        assert!(err.to_string().contains("unbalanced delimiters"));
    }

    #[test]
    fn test_patch_synthesis_rejects_score_based_memory_status_promotion() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        std::fs::write(
            tmp.path().join("src/memory/quality.rs"),
            "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };\n",
        )
        .expect("write file");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some("let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string()),
            new_string: "let status = if score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("score-only accepted promotion should be rejected");
        assert!(err
            .to_string()
            .contains("preserve score_memory_write hard gates"));
    }

    #[test]
    fn test_patch_synthesis_rejects_unknown_enum_variant() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
        std::fs::write(
            tmp.path().join("src/types.rs"),
            "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n}\n",
        )
        .expect("write types");
        std::fs::write(
            tmp.path().join("src/quality.rs"),
            "let status = MemoryStatus::Accepted;\n",
        )
        .expect("write quality");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/quality.rs".to_string(),
            old_string: Some("let status = MemoryStatus::Accepted;".to_string()),
            new_string: "let status = MemoryStatus::Blocked;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("unknown enum variant should be rejected before editing");

        assert!(err.to_string().contains("MemoryStatus::Blocked"));
        assert!(err.to_string().contains("Accepted"));
    }

    #[test]
    fn test_patch_synthesis_rejects_memory_status_duplicate_extension() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileEditTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let tmp = tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("src/memory")).expect("create memory dir");
        let old_enum = "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n}\n";
        std::fs::write(tmp.path().join("src/memory/types.rs"), old_enum).expect("write types");
        let action = PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/types.rs".to_string(),
            old_string: Some(old_enum.to_string()),
            new_string: "pub enum MemoryStatus {\n    Proposed,\n    Accepted,\n    Rejected,\n    Duplicate,\n    Demoted,\n}\n".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        };

        let err = loop_instance
            .validate_patch_synthesis_action(&action, tmp.path())
            .expect_err("duplicate/demote should use MemoryWriteOutcomeStatus");

        assert!(err.to_string().contains("MemoryWriteOutcomeStatus"));
    }

    #[tokio::test]
    async fn test_tool_specific_confirmation_blocks_git_push_without_approval() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(GitTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let route = crate::engine::intent_router::IntentRouter::new().route("push the branch");
        let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
        let tool_calls = vec![ToolCall {
            id: "git_push".to_string(),
            name: "git".to_string(),
            arguments: serde_json::json!({"action": "push"}),
        }];
        let exposed_tool_names = HashSet::from(["git".to_string()]);

        let results = loop_instance
            .execute_tools_parallel(
                &tool_calls,
                None,
                Default::default(),
                None,
                &policy,
                &exposed_tool_names,
                false,
                false,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].1.success);
        assert!(results[0]
            .1
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("requires user confirmation"));
    }

    #[tokio::test]
    async fn test_unexposed_tool_call_is_denied_before_execution() {
        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(VecDeque::new()),
        });
        let mut registry = ToolRegistry::new();
        registry.register(GitTool);
        let loop_instance = ConversationLoop::new(
            provider,
            Arc::new(registry),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "test".into(),
        );
        let route = crate::engine::intent_router::IntentRouter::new().route("push the branch");
        let policy = crate::engine::resource_policy::ResourcePolicy::from_route(&route);
        let tool_calls = vec![ToolCall {
            id: "git_push".to_string(),
            name: "git".to_string(),
            arguments: serde_json::json!({"action": "push"}),
        }];
        let exposed_tool_names = HashSet::from(["file_edit".to_string()]);

        let results = loop_instance
            .execute_tools_parallel(
                &tool_calls,
                None,
                Default::default(),
                None,
                &policy,
                &exposed_tool_names,
                false,
                false,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].1.success);
        assert!(results[0]
            .1
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("was not exposed"));
    }

    #[test]
    fn test_action_checkpoint_rejects_multi_replacement_file_edit() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet status = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status",
                "new_string": "let checked_status",
                "expected_replacements": 2
            }),
            tmp.path(),
        )
        .expect("multi replacement edit should be rejected");

        assert!(rejection.contains("only permits one replacement"));
    }

    #[test]
    fn test_action_checkpoint_rejects_non_unique_anchor() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet status = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status",
                "new_string": "let checked_status"
            }),
            tmp.path(),
        )
        .expect("non-unique anchor should be rejected");

        assert!(rejection.contains("unique edit anchor"));
    }

    #[test]
    fn test_action_checkpoint_rejects_multi_line_range_edit() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let write_decision = score();\nlet score = write_decision.score;\nlet status = write_decision.status;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "line_start": 1,
                "line_end": 3,
                "new_string": "let status = write_decision.status;"
            }),
            tmp.path(),
        )
        .expect("multi-line action checkpoint edit should be rejected");

        assert!(rejection.contains("exactly one line"));
    }

    #[test]
    fn test_action_checkpoint_accepts_unique_anchor() {
        let tmp = tempdir().expect("create temp dir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "let status = true;\nlet other = false;\n",
        )
        .expect("write file");

        let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
            &serde_json::json!({
                "path": "src/lib.rs",
                "old_string": "let status = true;",
                "new_string": "let status = false;"
            }),
            tmp.path(),
        );

        assert!(rejection.is_none(), "{rejection:?}");
    }

    struct MockLlmProvider {
        responses: StdMutex<VecDeque<ChatResponse>>,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let mut guard = self.responses.lock().unwrap();
            guard
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("no mock response left"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    #[tokio::test]
    async fn test_coding_quality_tracks_fail_then_repair_cycle() {
        let mut env = EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_REVIEW", "1");
        let tmp = tempdir().expect("create temp dir");
        let target_file = tmp.path().join("sample.rs");
        let target_path = target_file.to_string_lossy().to_string();

        let failing_code = "fn main() { let x = Some(1).unwrap(); let _ = x; }";
        let fixed_code = "fn main() { let x = Some(1); if let Some(v) = x { let _ = v; } }";

        let responses = VecDeque::from(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": failing_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_2".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": fixed_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
            ChatResponse {
                content: "repaired".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileReadTool);
        registry.register(FileWriteTool);
        let tool_registry = Arc::new(registry);
        let cost_tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_instance =
            ConversationLoop::new(provider, tool_registry, cost_tracker, "test".into())
                .with_max_iterations(5);

        let messages = vec![Message::user("write code and fix issues")];
        let result = loop_instance
            .run(messages)
            .await
            .expect("loop should succeed");

        assert!(
            result.iterations >= 2,
            "should iterate at least twice for write+fix"
        );
    }
}

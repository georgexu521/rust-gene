//! Runtime turn tracing.
//!
//! The trace spine records high-level events for a user turn without storing
//! full sensitive tool outputs. It is designed to back `/trace` and future
//! eval assertions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

const DEFAULT_MAX_TRACES: usize = 100;
const PREVIEW_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTrace {
    pub trace_id: String,
    pub session_id: String,
    pub turn_index: u64,
    pub user_message_preview: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: TurnStatus,
    pub events: Vec<TraceEvent>,
}

impl TurnTrace {
    pub fn new(session_id: impl Into<String>, turn_index: u64, user_message: &str) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            turn_index,
            user_message_preview: preview(user_message),
            started_at: Utc::now(),
            finished_at: None,
            status: TurnStatus::Running,
            events: vec![TraceEvent::UserPromptSubmitted {
                chars: user_message.chars().count(),
            }],
        }
    }

    pub fn finish(&mut self, status: TurnStatus) {
        self.status = status;
        self.finished_at = Some(Utc::now());
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at
            .map(|end| (end - self.started_at).num_milliseconds())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    UserPromptSubmitted {
        chars: usize,
    },
    IntentRouted {
        intent: String,
        workflow: String,
        retrieval: String,
        confidence: f32,
        risk: String,
        reason: String,
    },
    SessionGoalUpdated {
        goal_id: String,
        title: String,
        status: String,
        reason: String,
    },
    GoalDriftDetected {
        goal_id: String,
        tool: String,
        call_id: String,
        level: String,
        reason: String,
        suggested_action: Option<String>,
    },
    WorkflowRouted {
        decision: String,
        reason: String,
    },
    WorkflowCompleted {
        steps: usize,
    },
    WorkflowFallback {
        error: String,
    },
    MemorySnapshotInjected {
        chars: usize,
    },
    MemoryPrefetch {
        chars: usize,
    },
    RetrievalContextBuilt {
        policy: String,
        sources: Vec<String>,
        items: usize,
        estimated_tokens: usize,
    },
    MemorySynced {
        mode: String,
    },
    ContextCompacted {
        before_tokens: usize,
        after_tokens: usize,
        strategy: String,
    },
    ApiRequestStarted {
        iteration: usize,
        model: String,
        tools: usize,
    },
    ApiRequestCompleted {
        iteration: usize,
        tool_calls: usize,
        content_chars: usize,
    },
    ToolStarted {
        tool: String,
        call_id: String,
        parallel: bool,
        pre_executed: bool,
    },
    PermissionRequested {
        tool: String,
        call_id: String,
        prompt: String,
    },
    PermissionResolved {
        tool: String,
        call_id: String,
        approved: bool,
    },
    ToolCompleted {
        tool: String,
        call_id: String,
        success: bool,
        duration_ms: Option<u64>,
        output_chars: usize,
    },
    VerificationCompleted {
        changed_files: usize,
        passed: bool,
    },
    RecoveryApplied {
        error: String,
        action: String,
    },
    RecoveryPlan {
        plan_id: String,
        source: String,
        category: String,
        action: String,
        retryable: bool,
        safe_retry: bool,
        suggested_command: Option<String>,
        status: String,
    },
    McpResourceAccessed {
        server: String,
        uri: String,
        action: String,
        success: bool,
        content_chars: usize,
    },
    AssistantResponded {
        chars: usize,
        iterations: usize,
    },
    Error {
        message: String,
    },
}

impl TraceEvent {
    pub fn label(&self) -> &'static str {
        match self {
            TraceEvent::UserPromptSubmitted { .. } => "prompt",
            TraceEvent::IntentRouted { .. } => "intent",
            TraceEvent::SessionGoalUpdated { .. } => "goal",
            TraceEvent::GoalDriftDetected { .. } => "goal.drift",
            TraceEvent::WorkflowRouted { .. } => "workflow.route",
            TraceEvent::WorkflowCompleted { .. } => "workflow.done",
            TraceEvent::WorkflowFallback { .. } => "workflow.fallback",
            TraceEvent::MemorySnapshotInjected { .. } => "memory.snapshot",
            TraceEvent::MemoryPrefetch { .. } => "memory.prefetch",
            TraceEvent::RetrievalContextBuilt { .. } => "retrieval.context",
            TraceEvent::MemorySynced { .. } => "memory.sync",
            TraceEvent::ContextCompacted { .. } => "context.compact",
            TraceEvent::ApiRequestStarted { .. } => "api.start",
            TraceEvent::ApiRequestCompleted { .. } => "api.done",
            TraceEvent::ToolStarted { .. } => "tool.start",
            TraceEvent::PermissionRequested { .. } => "permission.request",
            TraceEvent::PermissionResolved { .. } => "permission.resolve",
            TraceEvent::ToolCompleted { .. } => "tool.done",
            TraceEvent::VerificationCompleted { .. } => "verify.done",
            TraceEvent::RecoveryApplied { .. } => "recovery",
            TraceEvent::RecoveryPlan { .. } => "recovery.plan",
            TraceEvent::McpResourceAccessed { .. } => "mcp.resource",
            TraceEvent::AssistantResponded { .. } => "assistant",
            TraceEvent::Error { .. } => "error",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            TraceEvent::UserPromptSubmitted { chars } => format!("user prompt: {} chars", chars),
            TraceEvent::IntentRouted {
                intent,
                workflow,
                retrieval,
                confidence,
                risk,
                reason,
            } => format!(
                "intent={} workflow={} retrieval={} risk={} confidence={:.2}: {}",
                intent,
                workflow,
                retrieval,
                risk,
                confidence,
                preview(reason)
            ),
            TraceEvent::SessionGoalUpdated {
                goal_id,
                title,
                status,
                reason,
            } => format!(
                "goal {} {}: {} ({})",
                short_id(goal_id),
                status,
                preview(title),
                preview(reason)
            ),
            TraceEvent::GoalDriftDetected {
                goal_id,
                tool,
                call_id,
                level,
                reason,
                suggested_action,
            } => format!(
                "{} {} drift={} goal={} reason={} suggested={}",
                tool,
                short_id(call_id),
                level,
                short_id(goal_id),
                preview(reason),
                suggested_action.as_deref().unwrap_or("none")
            ),
            TraceEvent::WorkflowRouted { decision, reason } => {
                format!("workflow decision: {} ({})", decision, preview(reason))
            }
            TraceEvent::WorkflowCompleted { steps } => {
                format!("workflow completed: {} steps", steps)
            }
            TraceEvent::WorkflowFallback { error } => {
                format!("workflow fallback: {}", preview(error))
            }
            TraceEvent::MemorySnapshotInjected { chars } => {
                format!("memory snapshot injected: {} chars", chars)
            }
            TraceEvent::MemoryPrefetch { chars } => format!("memory prefetch: {} chars", chars),
            TraceEvent::RetrievalContextBuilt {
                policy,
                sources,
                items,
                estimated_tokens,
            } => format!(
                "retrieval context: policy={} sources={} items={} tokens~{}",
                policy,
                sources.join(","),
                items,
                estimated_tokens
            ),
            TraceEvent::MemorySynced { mode } => format!("memory synced: {}", mode),
            TraceEvent::ContextCompacted {
                before_tokens,
                after_tokens,
                strategy,
            } => format!(
                "context compacted: {} -> {} tokens ({})",
                before_tokens, after_tokens, strategy
            ),
            TraceEvent::ApiRequestStarted {
                iteration,
                model,
                tools,
            } => format!(
                "api request #{}: model={}, tools={}",
                iteration, model, tools
            ),
            TraceEvent::ApiRequestCompleted {
                iteration,
                tool_calls,
                content_chars,
            } => format!(
                "api response #{}: {} tool calls, {} chars",
                iteration, tool_calls, content_chars
            ),
            TraceEvent::ToolStarted {
                tool,
                call_id,
                parallel,
                pre_executed,
            } => format!(
                "{} {} started{}{}",
                tool,
                short_id(call_id),
                if *parallel { " in parallel" } else { "" },
                if *pre_executed { " (pre-executed)" } else { "" }
            ),
            TraceEvent::PermissionRequested {
                tool,
                call_id,
                prompt,
            } => format!(
                "{} {} requested permission: {}",
                tool,
                short_id(call_id),
                preview(prompt)
            ),
            TraceEvent::PermissionResolved {
                tool,
                call_id,
                approved,
            } => format!(
                "{} {} permission {}",
                tool,
                short_id(call_id),
                if *approved { "approved" } else { "denied" }
            ),
            TraceEvent::ToolCompleted {
                tool,
                call_id,
                success,
                duration_ms,
                output_chars,
            } => format!(
                "{} {} {} in {}ms ({} chars)",
                tool,
                short_id(call_id),
                if *success { "ok" } else { "failed" },
                duration_ms.unwrap_or_default(),
                output_chars
            ),
            TraceEvent::VerificationCompleted {
                changed_files,
                passed,
            } => format!(
                "verification {} for {} changed files",
                if *passed { "passed" } else { "failed" },
                changed_files
            ),
            TraceEvent::RecoveryApplied { error, action } => {
                format!("recovery: {} -> {}", preview(error), action)
            }
            TraceEvent::RecoveryPlan {
                plan_id,
                source,
                category,
                action,
                retryable,
                safe_retry,
                suggested_command,
                status,
            } => format!(
                "{} {} {} action={} retryable={} safe_retry={} suggested={} status={}",
                source,
                short_id(plan_id),
                category,
                preview(action),
                retryable,
                safe_retry,
                suggested_command.as_deref().unwrap_or("none"),
                status
            ),
            TraceEvent::McpResourceAccessed {
                server,
                uri,
                action,
                success,
                content_chars,
            } => format!(
                "{} resource {} on {} success={} ({} chars)",
                action,
                preview(uri),
                server,
                success,
                content_chars
            ),
            TraceEvent::AssistantResponded { chars, iterations } => {
                format!(
                    "assistant responded: {} chars, {} iterations",
                    chars, iterations
                )
            }
            TraceEvent::Error { message } => format!("error: {}", preview(message)),
        }
    }
}

#[derive(Clone)]
pub struct TraceCollector {
    inner: Arc<Mutex<TurnTrace>>,
}

impl TraceCollector {
    pub fn new(trace: TurnTrace) -> Self {
        Self {
            inner: Arc::new(Mutex::new(trace)),
        }
    }

    pub fn record(&self, event: TraceEvent) {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.events.push(event);
    }

    pub fn finish(&self, status: TurnStatus) -> TurnTrace {
        let mut trace = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        trace.finish(status);
        trace.clone()
    }
}

#[derive(Debug)]
pub struct TraceStore {
    max_traces: usize,
    traces: RwLock<VecDeque<TurnTrace>>,
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_TRACES)
    }
}

impl TraceStore {
    pub fn new(max_traces: usize) -> Self {
        Self {
            max_traces: max_traces.max(1),
            traces: RwLock::new(VecDeque::new()),
        }
    }

    pub fn push(&self, trace: TurnTrace) {
        let mut traces = self.traces.write().unwrap_or_else(|e| e.into_inner());
        traces.push_back(trace);
        while traces.len() > self.max_traces {
            traces.pop_front();
        }
    }

    pub fn latest(&self) -> Option<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .back()
            .cloned()
    }

    pub fn recent(&self, limit: usize) -> Vec<TurnTrace> {
        self.traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.traces.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub fn format_trace_summary(trace: &TurnTrace, max_events: usize) -> String {
    let duration = trace
        .duration_ms()
        .map(|ms| format!("{}ms", ms))
        .unwrap_or_else(|| "running".to_string());
    let mut lines = vec![format!(
        "Trace {}\nSession: {}\nTurn: {}\nStatus: {:?}\nDuration: {}\nPrompt: {}",
        short_id(&trace.trace_id),
        trace.session_id,
        trace.turn_index,
        trace.status,
        duration,
        trace.user_message_preview
    )];

    lines.push("\nEvents:".to_string());
    for (idx, event) in trace.events.iter().take(max_events).enumerate() {
        lines.push(format!(
            "{:>2}. {:<20} {}",
            idx + 1,
            event.label(),
            event.summary()
        ));
    }
    if trace.events.len() > max_events {
        lines.push(format!(
            "... {} more events",
            trace.events.len().saturating_sub(max_events)
        ));
    }

    lines.join("\n")
}

fn preview(text: &str) -> String {
    let mut out: String = text.chars().take(PREVIEW_CHARS).collect();
    if text.chars().count() > PREVIEW_CHARS {
        out.push_str("...");
    }
    out.replace('\n', " ")
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_store_retains_latest_entries() {
        let store = TraceStore::new(2);
        store.push(TurnTrace::new("s1", 1, "one"));
        store.push(TurnTrace::new("s1", 2, "two"));
        store.push(TurnTrace::new("s1", 3, "three"));

        assert_eq!(store.len(), 2);
        assert_eq!(store.latest().unwrap().turn_index, 3);
        assert_eq!(store.recent(2)[1].turn_index, 2);
    }

    #[test]
    fn trace_summary_includes_events() {
        let collector = TraceCollector::new(TurnTrace::new("s1", 1, "hello"));
        collector.record(TraceEvent::ToolStarted {
            tool: "bash".to_string(),
            call_id: "abcdef123".to_string(),
            parallel: false,
            pre_executed: false,
        });
        let trace = collector.finish(TurnStatus::Completed);
        let summary = format_trace_summary(&trace, 10);
        assert!(summary.contains("tool.start"));
        assert!(summary.contains("bash"));
    }

    #[test]
    fn trace_summary_includes_mcp_resource_access() {
        let collector = TraceCollector::new(TurnTrace::new("s1", 1, "read mcp resource"));
        collector.record(TraceEvent::McpResourceAccessed {
            server: "filesystem".to_string(),
            uri: "file:///tmp/a.txt".to_string(),
            action: "read".to_string(),
            success: true,
            content_chars: 12,
        });

        let trace = collector.finish(TurnStatus::Completed);
        let summary = format_trace_summary(&trace, 10);
        assert!(summary.contains("mcp.resource"));
        assert!(summary.contains("filesystem"));
        assert!(summary.contains("file:///tmp/a.txt"));
    }
}

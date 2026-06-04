//! Runtime turn tracing.
//!
//! The trace spine records high-level events for a user turn without storing
//! full sensitive tool outputs. It is designed to back `/trace` and future
//! eval assertions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

mod collector;
mod diagnostic;
mod event;
mod event_label;
mod event_summary;
mod event_summary_workflow;
mod formatting;

pub use collector::{TraceCollector, TraceStore};
pub use diagnostic::*;
pub use event::TraceEvent;
pub use formatting::{format_trace_recent_line, format_trace_summary};

pub(super) const DEFAULT_MAX_TRACES: usize = 100;
const PREVIEW_CHARS: usize = 120;
pub const RUNTIME_DIET_PROMPT_TOKEN_BUDGET: u64 = 4_000;
pub const RUNTIME_DIET_TOOL_COUNT_BUDGET: usize = 24;

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

fn runtime_diet_level(prompt_tokens: u64, exposed_tools: usize) -> &'static str {
    if prompt_tokens > RUNTIME_DIET_PROMPT_TOKEN_BUDGET
        || exposed_tools > RUNTIME_DIET_TOOL_COUNT_BUDGET
    {
        "heavy"
    } else {
        "light"
    }
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

fn compact_id_list(ids: &[String]) -> String {
    if ids.is_empty() {
        return "none".to_string();
    }
    let mut compact = ids
        .iter()
        .take(4)
        .map(|id| short_id(id))
        .collect::<Vec<_>>();
    if ids.len() > compact.len() {
        compact.push(format!("+{}", ids.len() - compact.len()));
    }
    compact.join(",")
}

fn compact_label_list(labels: &[String]) -> String {
    if labels.is_empty() {
        return "none".to_string();
    }
    let mut compact = labels.iter().take(4).cloned().collect::<Vec<_>>();
    if labels.len() > compact.len() {
        compact.push(format!("+{}", labels.len() - compact.len()));
    }
    compact.join(",")
}

#[cfg(test)]
mod tests;

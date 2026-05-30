//! Sub-agent progress streaming.
//!
//! Defines progress event types and a broadcast channel for streaming
//! sub-agent execution progress to the parent agent or TUI.

use crate::agent::manager::AgentResult;
/// Progress event emitted by a sub-agent during execution.
#[derive(Debug, Clone)]
pub enum AgentProgressEvent {
    /// Sub-agent started execution.
    Started { agent_id: String, task: String },
    /// Sub-agent entered a new phase (e.g., "exploring", "summarizing").
    Phase { agent_id: String, phase: String },
    /// A tool call was dispatched.
    ToolCall {
        agent_id: String,
        tool: String,
        args_summary: String,
    },
    /// A tool call completed.
    ToolResult {
        agent_id: String,
        tool: String,
        success: bool,
        result_summary: String,
    },
    /// Streaming text chunk from the sub-agent (throttled).
    TextChunk { agent_id: String, text: String },
    /// Sub-agent completed successfully.
    Completed {
        agent_id: String,
        result: AgentResult,
    },
    /// Sub-agent failed.
    Failed { agent_id: String, error: String },
}

impl AgentProgressEvent {
    /// Human-readable one-line summary for TUI display.
    pub fn summary(&self) -> String {
        match self {
            Self::Started { task, .. } => format!("Started: {}", task),
            Self::Phase { phase, .. } => format!("Phase: {}", phase),
            Self::ToolCall {
                tool, args_summary, ..
            } => {
                format!("Running: {} {}", tool, args_summary)
            }
            Self::ToolResult { tool, success, .. } => {
                let status = if *success { "✓" } else { "✗" };
                format!("{} {}", status, tool)
            }
            Self::TextChunk { text, .. } => {
                let preview: String = text.chars().take(200).collect();
                format!("Output: {}", preview)
            }
            Self::Completed { .. } => "Completed".to_string(),
            Self::Failed { error, .. } => format!("Failed: {}", error),
        }
    }
}

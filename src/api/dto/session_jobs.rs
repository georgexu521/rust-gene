//! Session jobs DTO — durable shell process lifecycle.
//!
//! Slice D of the opencode programming parity plan.

use serde::{Deserialize, Serialize};

/// A shell job tracked by the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJobItem {
    pub job_id: String,
    pub session_id: String,
    pub command: String,
    pub cwd: Option<String>,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub tool_output_uri: Option<String>,
    pub cancelled: bool,
}

/// Session jobs index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJobsPage {
    pub session_id: String,
    pub jobs: Vec<SessionJobItem>,
    pub total: usize,
}

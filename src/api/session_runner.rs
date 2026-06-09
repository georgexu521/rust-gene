//! Per-session API runner registry.
//!
//! Replaces the global serialized API drain with a session-keyed runner map,
//! without racing the shared `RuntimeController` session binding.
//!
//! Slice A of the opencode programming parity plan.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// Durable run status projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiRunStatus {
    Idle,
    Queued { pending_count: usize },
    Running,
    WaitingPermission,
    Cancelling,
    Completed,
    Failed { error: String },
    Cancelled,
}

impl std::fmt::Display for ApiRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Queued { pending_count } => write!(f, "queued:{}", pending_count),
            Self::Running => write!(f, "running"),
            Self::WaitingPermission => write!(f, "waiting_permission"),
            Self::Cancelling => write!(f, "cancelling"),
            Self::Completed => write!(f, "completed"),
            Self::Failed { error } => write!(f, "failed:{error}"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Per-session run state tracked by the registry.
#[derive(Debug)]
struct SessionRunState {
    status: ApiRunStatus,
    /// Woken when the session becomes idle.
    idle_notify: Arc<Notify>,
}

impl SessionRunState {
    fn new() -> Self {
        Self {
            status: ApiRunStatus::Idle,
            idle_notify: Arc::new(Notify::new()),
        }
    }
}

/// Registry of per-session runner handles.
///
/// Thread-safe: all methods lock the internal map briefly.
#[derive(Debug, Default)]
pub struct ApiSessionRunnerRegistry {
    states: Mutex<HashMap<String, SessionRunState>>,
}

impl ApiSessionRunnerRegistry {
    pub fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
        }
    }

    /// Get the current run status for a session.
    pub fn status(&self, session_id: &str) -> ApiRunStatus {
        let guard = self
            .states
            .lock()
            .expect("session runner states lock poisoned");
        guard
            .get(session_id)
            .map(|s| s.status.clone())
            .unwrap_or(ApiRunStatus::Idle)
    }

    /// Mark a session as running.  Returns false if already busy.
    pub fn start_run(&self, session_id: &str) -> bool {
        let mut guard = self
            .states
            .lock()
            .expect("session runner states lock poisoned");
        let entry = guard
            .entry(session_id.to_string())
            .or_insert_with(SessionRunState::new);
        match entry.status {
            ApiRunStatus::Idle
            | ApiRunStatus::Queued { .. }
            | ApiRunStatus::Failed { .. }
            | ApiRunStatus::Completed
            | ApiRunStatus::Cancelled => {
                entry.status = ApiRunStatus::Running;
                true
            }
            _ => false,
        }
    }

    /// Mark a session as completed.
    pub fn finish_run(&self, session_id: &str, status: ApiRunStatus) {
        let mut guard = self
            .states
            .lock()
            .expect("session runner states lock poisoned");
        if let Some(entry) = guard.get_mut(session_id) {
            entry.status = status;
            entry.idle_notify.notify_waiters();
        }
    }

    /// Update non-terminal status without waking idle waiters.
    pub fn set_status(&self, session_id: &str, status: ApiRunStatus) {
        let mut guard = self
            .states
            .lock()
            .expect("session runner states lock poisoned");
        let entry = guard
            .entry(session_id.to_string())
            .or_insert_with(SessionRunState::new);
        entry.status = status;
    }

    /// Mark as queued.
    pub fn enqueue(&self, session_id: &str) {
        let mut guard = self
            .states
            .lock()
            .expect("session runner states lock poisoned");
        let entry = guard
            .entry(session_id.to_string())
            .or_insert_with(SessionRunState::new);
        match &entry.status {
            ApiRunStatus::Queued { pending_count } => {
                entry.status = ApiRunStatus::Queued {
                    pending_count: pending_count + 1,
                };
            }
            ApiRunStatus::Idle => {
                entry.status = ApiRunStatus::Queued { pending_count: 1 };
            }
            _ => {}
        }
    }

    /// Wait until the session is idle (non-blocking async).
    pub async fn wait_idle(&self, session_id: &str) {
        loop {
            let notify = {
                let guard = self
                    .states
                    .lock()
                    .expect("session runner states lock poisoned");
                if let Some(entry) = guard.get(session_id) {
                    match entry.status {
                        ApiRunStatus::Idle
                        | ApiRunStatus::Completed
                        | ApiRunStatus::Failed { .. }
                        | ApiRunStatus::Cancelled => return,
                        _ => entry.idle_notify.clone(),
                    }
                } else {
                    return;
                }
            };
            notify.notified().await;
        }
    }

    /// Request cancellation of the current run.
    pub fn request_cancel(&self, session_id: &str) -> bool {
        let mut guard = self
            .states
            .lock()
            .expect("session runner states lock poisoned");
        if let Some(entry) = guard.get_mut(session_id) {
            match entry.status {
                ApiRunStatus::Running | ApiRunStatus::WaitingPermission => {
                    entry.status = ApiRunStatus::Cancelling;
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Whether a cancellation has been requested for this session.
    pub fn is_cancelling(&self, session_id: &str) -> bool {
        matches!(self.status(session_id), ApiRunStatus::Cancelling)
    }

    /// Mark cancellation as completed.
    pub fn cancel_completed(&self, session_id: &str) {
        self.finish_run(session_id, ApiRunStatus::Cancelled);
    }

    /// Restart recovery: scan session_inputs for stale rows and mark them.
    pub fn recover_stale_inputs(
        &self,
        conn: &rusqlite::Connection,
    ) -> Result<usize, rusqlite::Error> {
        conn.execute(
            "UPDATE session_inputs
             SET state = 'pending',
                 promoted_at = NULL,
                 promoted_seq = NULL,
                 error = 'recovered_after_restart'
             WHERE state = 'promoted' AND promoted_at IS NOT NULL",
            [],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_allows_one_active_run() {
        let reg = ApiSessionRunnerRegistry::new();
        assert!(reg.start_run("s1"));
        assert!(!reg.start_run("s1"), "second start should fail");
        reg.finish_run("s1", ApiRunStatus::Completed);
        assert!(reg.start_run("s1"), "should start after finish");
    }

    #[test]
    fn registry_tracks_different_sessions_independently() {
        let reg = ApiSessionRunnerRegistry::new();
        assert!(reg.start_run("s1"));
        assert!(
            reg.start_run("s2"),
            "different sessions should be independent"
        );
        reg.finish_run("s1", ApiRunStatus::Completed);
        assert!(reg.start_run("s1"));
        reg.finish_run(
            "s2",
            ApiRunStatus::Failed {
                error: "test".to_string(),
            },
        );
    }

    #[test]
    fn registry_status_reflects_current_state() {
        let reg = ApiSessionRunnerRegistry::new();
        assert_eq!(reg.status("s1"), ApiRunStatus::Idle);
        reg.start_run("s1");
        assert_eq!(reg.status("s1"), ApiRunStatus::Running);
        reg.finish_run("s1", ApiRunStatus::Completed);
        assert_eq!(reg.status("s1"), ApiRunStatus::Completed);
    }

    #[test]
    fn registry_cancel_transitions_running_to_cancelling() {
        let reg = ApiSessionRunnerRegistry::new();
        reg.start_run("s1");
        assert!(reg.request_cancel("s1"));
        assert_eq!(reg.status("s1"), ApiRunStatus::Cancelling);
        reg.cancel_completed("s1");
        assert_eq!(reg.status("s1"), ApiRunStatus::Cancelled);
    }
}

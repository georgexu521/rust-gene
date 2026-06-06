//! Session run coordinator.
//!
//! Phase 4 (opencode core alignment): guarantees at most one active run per
//! session. Follow-up user inputs are queued as `steer` (interrupt current
//! run) or `queue` (execute after current run completes).
//!
//! This is a lightweight coordinator — the actual run lifecycle is driven
//! by the existing `StreamingQueryEngine` and `RuntimeFacadeState`.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Delivery mode for a session input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDelivery {
    /// Interrupt the current run and inject this prompt.
    Steer,
    /// Execute after the current run completes.
    Queue,
}

impl InputDelivery {
    pub fn label(self) -> &'static str {
        match self {
            Self::Steer => "steer",
            Self::Queue => "queue",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "steer" => Self::Steer,
            _ => Self::Queue,
        }
    }
}

/// Minimal session run coordinator.
///
/// Tracks whether a run is active for a given session and allows
/// follow-up inputs to be queued.
#[derive(Debug, Clone, Default)]
pub struct SessionRunCoordinator {
    /// Whether a run is currently active.
    active: Arc<AtomicBool>,
}

impl SessionRunCoordinator {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Mark a run as started. Returns false if a run was already active.
    pub fn start_run(&self) -> bool {
        self.active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Mark a run as completed.
    pub fn finish_run(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    /// Whether a run is currently active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Queue a new user input. Returns the delivery mode that should be used.
    pub fn admit_input(&self, delivery: InputDelivery) -> InputDelivery {
        if self.is_active() {
            delivery
        } else {
            InputDelivery::Queue
        }
    }
}

/// Persist a session input to the database.
pub fn persist_session_input(
    conn: &rusqlite::Connection,
    session_id: &str,
    content: &str,
    delivery: InputDelivery,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO session_inputs (session_id, delivery, content) VALUES (?1, ?2, ?3)",
        rusqlite::params![session_id, delivery.label(), content],
    )?;
    Ok(())
}

/// Promote a queued input for a session.
pub fn promote_session_input(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, content FROM session_inputs WHERE session_id = ?1 AND promoted_at IS NULL ORDER BY id ASC LIMIT 1",
    )?;
    let result = stmt.query_row([session_id], |row| {
        let id: i64 = row.get(0)?;
        let content: String = row.get(1)?;
        Ok((id, content))
    });
    match result {
        Ok((id, content)) => {
            conn.execute(
                "UPDATE session_inputs SET promoted_at = datetime('now') WHERE id = ?1",
                [id],
            )?;
            Ok(Some(content))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Clean up promoted session inputs.
pub fn cleanup_session_inputs(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<usize, rusqlite::Error> {
    Ok(conn.execute(
        "DELETE FROM session_inputs WHERE session_id = ?1 AND promoted_at IS NOT NULL",
        [session_id],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn coordinator_allows_one_active_run() {
        let coord = SessionRunCoordinator::new();
        assert!(!coord.is_active());
        assert!(coord.start_run());
        assert!(coord.is_active());
        assert!(!coord.start_run(), "second start should fail");
        coord.finish_run();
        assert!(!coord.is_active());
        assert!(coord.start_run(), "should start after finish");
    }

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, title TEXT, model TEXT);
             CREATE TABLE IF NOT EXISTS session_inputs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                delivery TEXT NOT NULL DEFAULT 'queue',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                promoted_at TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            CREATE INDEX IF NOT EXISTS idx_session_inputs_session ON session_inputs(session_id);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn persists_and_promotes_session_inputs() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s1', 'test', 'kimi')",
            [],
        )
        .unwrap();

        persist_session_input(&conn, "s1", "fix the bug", InputDelivery::Queue).unwrap();
        persist_session_input(&conn, "s1", "add tests", InputDelivery::Steer).unwrap();

        let promoted = promote_session_input(&conn, "s1").unwrap();
        assert_eq!(promoted.as_deref(), Some("fix the bug"));

        let promoted2 = promote_session_input(&conn, "s1").unwrap();
        assert_eq!(promoted2.as_deref(), Some("add tests"));

        let promoted3 = promote_session_input(&conn, "s1").unwrap();
        assert!(promoted3.is_none());
    }
}

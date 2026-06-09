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
    /// Execute immediately if no run is active.
    Run,
    /// Durably admit the input without starting execution.
    AdmitOnly,
    /// Interrupt the current run and inject this prompt.
    Steer,
    /// Execute after the current run completes.
    Queue,
}

impl InputDelivery {
    pub fn label(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::AdmitOnly => "admit_only",
            Self::Steer => "steer",
            Self::Queue => "queue",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "run" => Self::Run,
            "admit_only" => Self::AdmitOnly,
            "steer" => Self::Steer,
            _ => Self::Queue,
        }
    }
}

/// Minimal session run coordinator.
///
/// Tracks whether a run is active for a given session and allows
/// follow-up inputs to be queued.  Wake semantics prevent concurrent
/// drain spawns while ensuring a finished run always triggers queue
/// processing.
#[derive(Debug, Clone, Default)]
pub struct SessionRunCoordinator {
    /// Whether a run is currently active.
    active: Arc<AtomicBool>,
    /// Whether a wake/drain has been requested but not yet processed.
    wake_requested: Arc<AtomicBool>,
}

impl SessionRunCoordinator {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            wake_requested: Arc::new(AtomicBool::new(false)),
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

    /// Request a queue drain. Returns `true` if the caller should start
    /// the drain loop (i.e. no drain is currently in progress). Multiple
    /// wakes collapse into a single drain.
    pub fn wake(&self) -> bool {
        self.wake_requested
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Mark the current wake as being processed. Call this before entering
    /// the drain loop.
    pub fn accept_wake(&self) {
        self.wake_requested.store(false, Ordering::SeqCst);
    }

    /// Whether a wake is pending.
    pub fn is_wake_pending(&self) -> bool {
        self.wake_requested.load(Ordering::SeqCst)
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

/// Idempotent admission of a session input with caller-provided prompt id.
///
/// - Same prompt_id + same content hash → returns existing row (no insert).
/// - Same prompt_id + different content hash → returns an error (conflict).
/// - New prompt_id → inserts a new row with state `pending`.
///
/// Reserved internal ids (starting with `__`) are rejected.
pub fn admit_session_input(
    conn: &rusqlite::Connection,
    session_id: &str,
    prompt_id: &str,
    prompt: &str,
    delivery: InputDelivery,
) -> Result<PromptAdmissionStatus, rusqlite::Error> {
    admit_session_input_with_metadata(conn, session_id, prompt_id, prompt, delivery, None)
}

/// Idempotent admission with optional JSON metadata stored in attachments_json.
pub fn admit_session_input_with_metadata(
    conn: &rusqlite::Connection,
    session_id: &str,
    prompt_id: &str,
    prompt: &str,
    delivery: InputDelivery,
    metadata: Option<&serde_json::Value>,
) -> Result<PromptAdmissionStatus, rusqlite::Error> {
    if prompt_id.starts_with("__") {
        return Ok(PromptAdmissionStatus::Rejected {
            reason: "prompt_id starting with '__' is reserved for internal use".to_string(),
        });
    }
    let prompt_hash = hash_prompt(prompt);

    // Check for existing prompt with same id.
    let existing = conn.query_row(
        "SELECT prompt_hash, state FROM session_inputs WHERE session_id = ?1 AND prompt_id = ?2",
        rusqlite::params![session_id, prompt_id],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
    );
    match existing {
        Ok((Some(existing_hash), state)) => {
            if existing_hash == prompt_hash {
                return Ok(PromptAdmissionStatus::AlreadyAdmitted { state });
            }
            Ok(PromptAdmissionStatus::Conflict {
                existing_hash,
                new_hash: prompt_hash,
            })
        }
        Ok((None, state)) => {
            // Existing row without hash may have been created pre-v12, treat as
            // already admitted with unknown hash.
            Ok(PromptAdmissionStatus::AlreadyAdmitted { state })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // Fresh admission.
            let metadata = metadata.map(serde_json::Value::to_string);
            conn.execute(
                "INSERT INTO session_inputs
                 (session_id, delivery, content, prompt_id, prompt_hash, attachments_json, state, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', datetime('now'))",
                rusqlite::params![
                    session_id,
                    delivery.label(),
                    prompt,
                    prompt_id,
                    prompt_hash,
                    metadata,
                ],
            )?;
            Ok(PromptAdmissionStatus::Admitted)
        }
        Err(e) => Err(e),
    }
}

fn hash_prompt(prompt: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut s = std::collections::hash_map::DefaultHasher::new();
    prompt.hash(&mut s);
    format!("{:016x}", s.finish())
}

/// Result of idempotent prompt admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAdmissionStatus {
    /// New prompt admitted successfully.
    Admitted,
    /// Prompt already exists with same content (idempotent retry).
    AlreadyAdmitted { state: String },
    /// Prompt id reused with different content (hard conflict).
    Conflict {
        existing_hash: String,
        new_hash: String,
    },
    /// Admission rejected by policy.
    Rejected { reason: String },
}

impl PromptAdmissionStatus {
    pub fn is_new(&self) -> bool {
        matches!(self, Self::Admitted)
    }
}

/// Promoted queued input with enough metadata to resume through a runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotedSessionInput {
    pub session_id: String,
    pub prompt_id: Option<String>,
    pub content: String,
    pub agent_mode: Option<String>,
}

/// Promote a queued input for a session.
///
/// Sets `promoted_at`, `promoted_seq` (to the next session event sequence),
/// and updates `state` to `promoted`.
pub fn promote_session_input(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<String>, rusqlite::Error> {
    Ok(promote_session_input_record(conn, session_id)?.map(|record| record.content))
}

/// Promote a queued input for a session and return the full record.
pub fn promote_session_input_record(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<PromotedSessionInput>, rusqlite::Error> {
    let next_seq: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM session_events WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .unwrap_or(1);

    let mut stmt = conn.prepare(
        "SELECT id, prompt_id, content, attachments_json
         FROM session_inputs
         WHERE session_id = ?1 AND state = 'pending'
         ORDER BY id ASC
         LIMIT 1",
    )?;
    let result = stmt.query_row([session_id], |row| {
        let id: i64 = row.get(0)?;
        let prompt_id: Option<String> = row.get(1)?;
        let content: String = row.get(2)?;
        let attachments_json: Option<String> = row.get(3)?;
        Ok((id, prompt_id, content, attachments_json))
    });
    match result {
        Ok((id, prompt_id, content, attachments_json)) => {
            conn.execute(
                "UPDATE session_inputs SET promoted_at = datetime('now'), promoted_seq = ?1, state = 'promoted' WHERE id = ?2",
                rusqlite::params![next_seq, id],
            )?;
            let agent_mode = attachments_json
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .and_then(|value| value["agent_mode"].as_str().map(str::to_string));
            Ok(Some(PromotedSessionInput {
                session_id: session_id.to_string(),
                prompt_id,
                content,
                agent_mode,
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// List sessions with pending queued/admitted inputs.
pub fn pending_session_ids(
    conn: &rusqlite::Connection,
    limit: usize,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT session_id
         FROM session_inputs
         WHERE state = 'pending'
         ORDER BY id ASC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |row| row.get(0))?;
    rows.collect()
}

/// User-visible queued/pending session input row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInputRecord {
    pub id: i64,
    pub prompt_id: Option<String>,
    pub delivery: InputDelivery,
    pub content_preview: String,
    pub state: String,
    pub created_at: String,
}

/// List queued inputs that are still pending for a session.
pub fn list_pending_session_inputs(
    conn: &rusqlite::Connection,
    session_id: &str,
    limit: usize,
) -> Result<Vec<SessionInputRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, prompt_id, delivery, content, state, created_at
         FROM session_inputs
         WHERE session_id = ?1 AND state = 'pending'
         ORDER BY id ASC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![session_id, limit as i64], |row| {
        let delivery: String = row.get(2)?;
        let content: String = row.get(3)?;
        Ok(SessionInputRecord {
            id: row.get(0)?,
            prompt_id: row.get(1)?,
            delivery: InputDelivery::from_label(&delivery),
            content_preview: preview_input(&content),
            state: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    rows.collect()
}

/// Cancel one pending input by numeric id or prompt_id.
pub fn cancel_session_input(
    conn: &rusqlite::Connection,
    session_id: &str,
    id_or_prompt_id: &str,
) -> Result<bool, rusqlite::Error> {
    let changed = if let Ok(id) = id_or_prompt_id.parse::<i64>() {
        conn.execute(
            "UPDATE session_inputs
             SET state = 'cancelled', error = 'cancelled_by_user'
             WHERE session_id = ?1 AND id = ?2 AND state = 'pending'",
            rusqlite::params![session_id, id],
        )?
    } else {
        conn.execute(
            "UPDATE session_inputs
             SET state = 'cancelled', error = 'cancelled_by_user'
             WHERE session_id = ?1 AND prompt_id = ?2 AND state = 'pending'",
            rusqlite::params![session_id, id_or_prompt_id],
        )?
    };
    Ok(changed > 0)
}

/// Mark one admitted input by prompt id.
pub fn mark_session_input_state_by_prompt_id(
    conn: &rusqlite::Connection,
    session_id: &str,
    prompt_id: &str,
    state: &str,
    error: Option<&str>,
) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute(
        "UPDATE session_inputs
         SET state = ?1, error = ?2
         WHERE session_id = ?3 AND prompt_id = ?4",
        rusqlite::params![state, error, session_id, prompt_id],
    )?;
    Ok(changed > 0)
}

/// Clean up promoted session inputs.
pub fn cleanup_session_inputs(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "DELETE FROM session_inputs WHERE session_id = ?1 AND promoted_at IS NOT NULL",
        [session_id],
    )
}

fn preview_input(content: &str) -> String {
    const MAX_CHARS: usize = 120;
    let mut chars = content.chars();
    let preview = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
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
             CREATE TABLE IF NOT EXISTS session_events (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 session_id TEXT NOT NULL,
                 seq INTEGER NOT NULL,
                 event_type TEXT NOT NULL,
                 timestamp_ms INTEGER NOT NULL DEFAULT 0,
                 payload TEXT NOT NULL DEFAULT '{}'
             );
             CREATE TABLE IF NOT EXISTS session_inputs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                delivery TEXT NOT NULL DEFAULT 'queue',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                promoted_at TEXT,
                prompt_id TEXT,
                prompt_hash TEXT,
                attachments_json TEXT,
                promoted_seq INTEGER,
                state TEXT NOT NULL DEFAULT 'pending',
                error TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            CREATE INDEX IF NOT EXISTS idx_session_inputs_session ON session_inputs(session_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_session_inputs_prompt_id
                ON session_inputs(session_id, prompt_id)
                WHERE prompt_id IS NOT NULL;",
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

    #[test]
    fn idempotent_admission_same_id_same_content_returns_existing() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s1', 'test', 'kimi')",
            [],
        )
        .unwrap();

        let status =
            admit_session_input(&conn, "s1", "prompt-1", "fix the bug", InputDelivery::Queue)
                .unwrap();
        assert!(status.is_new());

        // Same id, same content → idempotent retry.
        let status2 =
            admit_session_input(&conn, "s1", "prompt-1", "fix the bug", InputDelivery::Queue)
                .unwrap();
        assert!(matches!(
            status2,
            PromptAdmissionStatus::AlreadyAdmitted { .. }
        ));

        // Same id, different content → conflict.
        let status3 = admit_session_input(
            &conn,
            "s1",
            "prompt-1",
            "fix another bug",
            InputDelivery::Queue,
        )
        .unwrap();
        assert!(matches!(status3, PromptAdmissionStatus::Conflict { .. }));
    }

    #[test]
    fn idempotent_admission_rejects_reserved_ids() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s1', 'test', 'kimi')",
            [],
        )
        .unwrap();

        let status =
            admit_session_input(&conn, "s1", "__internal", "prompt", InputDelivery::Queue).unwrap();
        assert!(matches!(status, PromptAdmissionStatus::Rejected { .. }));
    }

    #[test]
    fn lists_and_cancels_pending_session_inputs() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s1', 'test', 'kimi')",
            [],
        )
        .unwrap();

        admit_session_input(&conn, "s1", "prompt-1", "fix bug", InputDelivery::Queue).unwrap();
        admit_session_input(
            &conn,
            "s1",
            "prompt-2",
            &"long ".repeat(60),
            InputDelivery::Steer,
        )
        .unwrap();

        let pending = list_pending_session_inputs(&conn, "s1", 10).unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].prompt_id.as_deref(), Some("prompt-1"));
        assert_eq!(pending[1].delivery, InputDelivery::Steer);
        assert!(
            pending[1].content_preview.ends_with("..."),
            "long queued input should be previewed"
        );

        assert!(cancel_session_input(&conn, "s1", "prompt-1").unwrap());
        let pending = list_pending_session_inputs(&conn, "s1", 10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].prompt_id.as_deref(), Some("prompt-2"));
        assert!(!cancel_session_input(&conn, "s1", "missing").unwrap());
    }

    #[test]
    fn marks_session_input_state_by_prompt_id() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s1', 'test', 'kimi')",
            [],
        )
        .unwrap();
        admit_session_input(&conn, "s1", "prompt-1", "fix bug", InputDelivery::Run).unwrap();

        assert!(
            mark_session_input_state_by_prompt_id(&conn, "s1", "prompt-1", "running", None)
                .unwrap()
        );
        let state: String = conn
            .query_row(
                "SELECT state FROM session_inputs WHERE session_id = 's1' AND prompt_id = 'prompt-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "running");
    }

    #[test]
    fn promotes_session_input_record_with_metadata() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s1', 'test', 'kimi')",
            [],
        )
        .unwrap();
        admit_session_input_with_metadata(
            &conn,
            "s1",
            "prompt-1",
            "fix bug",
            InputDelivery::Queue,
            Some(&serde_json::json!({ "agent_mode": "review" })),
        )
        .unwrap();

        let promoted = promote_session_input_record(&conn, "s1")
            .unwrap()
            .expect("promoted input");
        assert_eq!(promoted.session_id, "s1");
        assert_eq!(promoted.prompt_id.as_deref(), Some("prompt-1"));
        assert_eq!(promoted.content, "fix bug");
        assert_eq!(promoted.agent_mode.as_deref(), Some("review"));
    }

    #[test]
    fn lists_sessions_with_pending_inputs() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s1', 'test', 'kimi')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, title, model) VALUES ('s2', 'test', 'kimi')",
            [],
        )
        .unwrap();
        admit_session_input(&conn, "s1", "prompt-1", "fix bug", InputDelivery::Queue).unwrap();
        admit_session_input(&conn, "s2", "prompt-2", "add tests", InputDelivery::Queue).unwrap();

        let sessions = pending_session_ids(&conn, 10).unwrap();
        assert_eq!(sessions, vec!["s1".to_string(), "s2".to_string()]);
    }
}

//! v10 migration — session inputs for durable input queue (Phase 4: opencode core alignment).

use rusqlite::{Connection, Result as SqlResult};

pub struct V10AddSessionInputs;

impl crate::migrations::Migration for V10AddSessionInputs {
    fn version(&self) -> i32 {
        10
    }

    fn name(&self) -> &str {
        "v10_add_session_inputs"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_SESSION_INPUTS)
    }
}

const CREATE_SESSION_INPUTS: &str = r#"
CREATE TABLE IF NOT EXISTS session_inputs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    delivery TEXT NOT NULL DEFAULT 'queue',
    content TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    promoted_at TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_session_inputs_session ON session_inputs(session_id);
"#;

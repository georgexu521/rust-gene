//! v9 migration — session events for durable replay (Phase 2: opencode core alignment).

use rusqlite::{Connection, Result as SqlResult};

pub struct V9AddSessionEvents;

impl crate::migrations::Migration for V9AddSessionEvents {
    fn version(&self) -> i32 {
        9
    }

    fn name(&self) -> &str {
        "v9_add_session_events"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_SESSION_EVENTS)
    }
}

const CREATE_SESSION_EVENTS: &str = r#"
CREATE TABLE IF NOT EXISTS session_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_session_events_session ON session_events(session_id, seq);
"#;

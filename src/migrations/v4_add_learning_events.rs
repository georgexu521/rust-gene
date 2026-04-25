//! Migration v4 - add durable learning events.

use rusqlite::{Connection, Result as SqlResult};

pub struct V4AddLearningEvents;

impl crate::migrations::Migration for V4AddLearningEvents {
    fn version(&self) -> i32 {
        4
    }

    fn name(&self) -> &str {
        "v4_add_learning_events"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_LEARNING_EVENTS_SCHEMA)
    }
}

const CREATE_LEARNING_EVENTS_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS learning_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    source TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    confidence REAL NOT NULL DEFAULT 1.0,
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_learning_events_session ON learning_events(session_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_learning_events_kind ON learning_events(kind, created_at DESC);
"#;

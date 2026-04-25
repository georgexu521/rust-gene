//! Migration v3 - add turn trace tables.

use rusqlite::{Connection, Result as SqlResult};

pub struct V3AddTraces;

impl crate::migrations::Migration for V3AddTraces {
    fn version(&self) -> i32 {
        3
    }

    fn name(&self) -> &str {
        "v3_add_traces"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_TRACES_SCHEMA)
    }
}

const CREATE_TRACES_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS turn_traces (
    trace_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_index INTEGER NOT NULL,
    user_message_preview TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'running',
    started_at TEXT NOT NULL,
    finished_at TEXT,
    duration_ms INTEGER,
    event_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_turn_traces_session ON turn_traces(session_id, turn_index DESC);
CREATE INDEX IF NOT EXISTS idx_turn_traces_started ON turn_traces(started_at DESC);

CREATE TABLE IF NOT EXISTS trace_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trace_id TEXT NOT NULL,
    event_index INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (trace_id) REFERENCES turn_traces(trace_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_trace_events_trace ON trace_events(trace_id, event_index);
"#;

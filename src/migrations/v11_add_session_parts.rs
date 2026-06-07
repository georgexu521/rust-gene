//! v11 migration — persisted session part projection.

use rusqlite::{Connection, Result as SqlResult};

pub struct V11AddSessionParts;

impl crate::migrations::Migration for V11AddSessionParts {
    fn version(&self) -> i32 {
        11
    }

    fn name(&self) -> &str {
        "v11_add_session_parts"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_SESSION_PARTS)
    }
}

const CREATE_SESSION_PARTS: &str = r#"
CREATE TABLE IF NOT EXISTS session_parts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    part_index INTEGER NOT NULL,
    part_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    tool_call_id TEXT,
    tool_name TEXT,
    status TEXT,
    payload TEXT NOT NULL DEFAULT '{}',
    projected_to_seq INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_session_parts_session_part
    ON session_parts(session_id, part_id);
CREATE INDEX IF NOT EXISTS idx_session_parts_session_order
    ON session_parts(session_id, part_index);
CREATE INDEX IF NOT EXISTS idx_session_parts_tool_call
    ON session_parts(session_id, tool_call_id);
"#;

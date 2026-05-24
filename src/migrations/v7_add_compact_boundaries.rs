//! Migration v7 - add durable compact boundaries.

use rusqlite::{Connection, Result as SqlResult};

pub struct V7AddCompactBoundaries;

impl crate::migrations::Migration for V7AddCompactBoundaries {
    fn version(&self) -> i32 {
        7
    }

    fn name(&self) -> &str {
        "v7_add_compact_boundaries"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_COMPACT_BOUNDARIES_SCHEMA)
    }
}

const CREATE_COMPACT_BOUNDARIES_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS compact_boundaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    boundary_id TEXT NOT NULL,
    sequence INTEGER,
    strategy TEXT NOT NULL,
    trigger TEXT,
    before_tokens INTEGER NOT NULL DEFAULT 0,
    after_tokens INTEGER NOT NULL DEFAULT 0,
    messages_before INTEGER NOT NULL DEFAULT 0,
    messages_after INTEGER NOT NULL DEFAULT 0,
    preserved_tail_count INTEGER,
    retained_items TEXT NOT NULL DEFAULT '[]',
    provenance TEXT NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id),
    UNIQUE(session_id, boundary_id)
);

CREATE INDEX IF NOT EXISTS idx_compact_boundaries_session ON compact_boundaries(session_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_compact_boundaries_boundary ON compact_boundaries(boundary_id);
"#;

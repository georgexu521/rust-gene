//! v13 migration — durable session revert metadata.

use rusqlite::{Connection, Result as SqlResult};

pub struct V13AddSessionReverts;

impl crate::migrations::Migration for V13AddSessionReverts {
    fn version(&self) -> i32 {
        13
    }

    fn name(&self) -> &str {
        "v13_add_session_reverts"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_SESSION_REVERTS)
    }
}

const CREATE_SESSION_REVERTS: &str = r#"
CREATE TABLE IF NOT EXISTS session_reverts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    operation TEXT NOT NULL DEFAULT 'revert',
    status TEXT NOT NULL,
    message_id TEXT,
    target_part_id TEXT,
    part_ids_json TEXT NOT NULL DEFAULT '[]',
    checkpoint_ids_json TEXT NOT NULL DEFAULT '[]',
    snapshot_checkpoint_id TEXT,
    paths_json TEXT NOT NULL DEFAULT '[]',
    restored_files_json TEXT NOT NULL DEFAULT '[]',
    removed_files_json TEXT NOT NULL DEFAULT '[]',
    errors_json TEXT NOT NULL DEFAULT '[]',
    diff_summary TEXT,
    unrevert_possible INTEGER NOT NULL DEFAULT 0,
    unreverted INTEGER NOT NULL DEFAULT 0,
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_session_reverts_session_created
    ON session_reverts(session_id, created_at, id);

CREATE INDEX IF NOT EXISTS idx_session_reverts_session_target
    ON session_reverts(session_id, target_part_id);
"#;

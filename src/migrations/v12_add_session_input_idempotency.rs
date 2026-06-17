//! v12 migration — session_input idempotency (Phase 3: opencode third alignment).

use rusqlite::{Connection, Result as SqlResult};

use crate::migrations::framework::add_column_if_missing;

pub struct V12AddSessionInputIdempotency;

impl crate::migrations::Migration for V12AddSessionInputIdempotency {
    fn version(&self) -> i32 {
        12
    }

    fn name(&self) -> &str {
        "v12_add_session_input_idempotency"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        add_column_if_missing(conn, "session_inputs", "prompt_id", "TEXT")?;
        add_column_if_missing(conn, "session_inputs", "prompt_hash", "TEXT")?;
        add_column_if_missing(conn, "session_inputs", "attachments_json", "TEXT")?;
        add_column_if_missing(conn, "session_inputs", "promoted_seq", "INTEGER")?;
        add_column_if_missing(
            conn,
            "session_inputs",
            "state",
            "TEXT NOT NULL DEFAULT 'pending'",
        )?;
        add_column_if_missing(conn, "session_inputs", "error", "TEXT")?;

        conn.execute_batch(
            r#"
CREATE UNIQUE INDEX IF NOT EXISTS idx_session_inputs_prompt_id
    ON session_inputs(session_id, prompt_id)
    WHERE prompt_id IS NOT NULL;
"#,
        )
    }
}

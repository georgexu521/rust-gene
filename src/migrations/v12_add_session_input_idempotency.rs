//! v12 migration — session_input idempotency (Phase 3: opencode third alignment).

use rusqlite::{Connection, Result as SqlResult};

pub struct V12AddSessionInputIdempotency;

impl crate::migrations::Migration for V12AddSessionInputIdempotency {
    fn version(&self) -> i32 {
        12
    }

    fn name(&self) -> &str {
        "v12_add_session_input_idempotency"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(ALTER_SESSION_INPUTS)
    }
}

const ALTER_SESSION_INPUTS: &str = r#"
ALTER TABLE session_inputs ADD COLUMN prompt_id TEXT;
ALTER TABLE session_inputs ADD COLUMN prompt_hash TEXT;
ALTER TABLE session_inputs ADD COLUMN attachments_json TEXT;
ALTER TABLE session_inputs ADD COLUMN promoted_seq INTEGER;
ALTER TABLE session_inputs ADD COLUMN state TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE session_inputs ADD COLUMN error TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_session_inputs_prompt_id
    ON session_inputs(session_id, prompt_id)
    WHERE prompt_id IS NOT NULL;
"#;

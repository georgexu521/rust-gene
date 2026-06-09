//! v16 migration — add message_id to session_parts for turn-level attribution.

use rusqlite::{Connection, Result as SqlResult};

pub struct V16AddSessionPartMessageId;

impl crate::migrations::Migration for V16AddSessionPartMessageId {
    fn version(&self) -> i32 {
        16
    }

    fn name(&self) -> &str {
        "v16_add_session_part_message_id"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(ADD_MESSAGE_ID)
    }
}

const ADD_MESSAGE_ID: &str = r#"
ALTER TABLE session_parts ADD COLUMN message_id TEXT;
CREATE INDEX IF NOT EXISTS idx_session_parts_message
    ON session_parts(session_id, message_id);
"#;

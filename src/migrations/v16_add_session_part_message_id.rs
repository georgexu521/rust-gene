//! v16 migration — add message_id to session_parts for turn-level attribution.

use rusqlite::{Connection, Result as SqlResult};

use crate::migrations::framework::add_column_if_missing;

pub struct V16AddSessionPartMessageId;

impl crate::migrations::Migration for V16AddSessionPartMessageId {
    fn version(&self) -> i32 {
        16
    }

    fn name(&self) -> &str {
        "v16_add_session_part_message_id"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        add_column_if_missing(conn, "session_parts", "message_id", "TEXT")?;
        conn.execute_batch(
            r#"
CREATE INDEX IF NOT EXISTS idx_session_parts_message
    ON session_parts(session_id, message_id);
"#,
        )
    }
}

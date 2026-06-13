//! v19 migration — persist UI/runtime metadata for stored messages.

use rusqlite::{Connection, Result as SqlResult};

pub struct V19AddMessageMetadata;

impl crate::migrations::Migration for V19AddMessageMetadata {
    fn version(&self) -> i32 {
        19
    }

    fn name(&self) -> &str {
        "v19_add_message_metadata"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(ADD_MESSAGE_METADATA)
    }
}

const ADD_MESSAGE_METADATA: &str = r#"
ALTER TABLE messages ADD COLUMN metadata TEXT;
"#;

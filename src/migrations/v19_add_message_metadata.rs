//! v19 migration — persist UI/runtime metadata for stored messages.

use rusqlite::{Connection, Result as SqlResult};

use crate::migrations::framework::add_column_if_missing;

pub struct V19AddMessageMetadata;

impl crate::migrations::Migration for V19AddMessageMetadata {
    fn version(&self) -> i32 {
        19
    }

    fn name(&self) -> &str {
        "v19_add_message_metadata"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        add_column_if_missing(conn, "messages", "metadata", "TEXT")
    }
}

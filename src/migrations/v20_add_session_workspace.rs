//! v20 migration — tag sessions with their detected workspace root.

use rusqlite::{Connection, Result as SqlResult};

use crate::migrations::framework::add_column_if_missing;

pub struct V20AddSessionWorkspace;

impl crate::migrations::Migration for V20AddSessionWorkspace {
    fn version(&self) -> i32 {
        20
    }

    fn name(&self) -> &str {
        "v20_add_session_workspace"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        add_column_if_missing(conn, "sessions", "workspace_root", "TEXT")
    }
}

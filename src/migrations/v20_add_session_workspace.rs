//! v20 migration — tag sessions with their detected workspace root.

use rusqlite::{Connection, Result as SqlResult};

pub struct V20AddSessionWorkspace;

impl crate::migrations::Migration for V20AddSessionWorkspace {
    fn version(&self) -> i32 {
        20
    }

    fn name(&self) -> &str {
        "v20_add_session_workspace"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(ADD_SESSION_WORKSPACE)
    }
}

const ADD_SESSION_WORKSPACE: &str = r#"
ALTER TABLE sessions ADD COLUMN workspace_root TEXT;
"#;

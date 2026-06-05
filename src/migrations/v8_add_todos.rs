//! v8 migration — session-backed coding todos (Phase 5: opencode alignment).

use rusqlite::{Connection, Result as SqlResult};

pub struct V8AddTodos;

impl crate::migrations::Migration for V8AddTodos {
    fn version(&self) -> i32 {
        8
    }

    fn name(&self) -> &str {
        "v8_add_todos"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_TODOS)
    }
}

const CREATE_TODOS: &str = r#"
CREATE TABLE IF NOT EXISTS todos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_todos_session ON todos(session_id);
"#;

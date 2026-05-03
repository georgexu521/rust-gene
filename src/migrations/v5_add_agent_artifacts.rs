//! Migration v5 - add durable subagent artifacts.

use rusqlite::{Connection, Result as SqlResult};

pub struct V5AddAgentArtifacts;

impl crate::migrations::Migration for V5AddAgentArtifacts {
    fn version(&self) -> i32 {
        5
    }

    fn name(&self) -> &str {
        "v5_add_agent_artifacts"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_AGENT_ARTIFACTS_SCHEMA)
    }
}

const CREATE_AGENT_ARTIFACTS_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS agent_artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    profile TEXT,
    role TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    output TEXT NOT NULL DEFAULT '',
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_agent_artifacts_session ON agent_artifacts(session_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_artifacts_agent ON agent_artifacts(agent_id, created_at DESC);
"#;

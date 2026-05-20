//! Migration v6 - add durable subagent task states.

use rusqlite::{Connection, Result as SqlResult};

pub struct V6AddAgentTaskStates;

impl crate::migrations::Migration for V6AddAgentTaskStates {
    fn version(&self) -> i32 {
        6
    }

    fn name(&self) -> &str {
        "v6_add_agent_task_states"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_AGENT_TASK_STATES_SCHEMA)
    }
}

const CREATE_AGENT_TASK_STATES_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS agent_task_states (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    profile TEXT,
    role TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    transcript_path TEXT,
    tool_ids_in_progress TEXT NOT NULL DEFAULT '[]',
    permission_requests TEXT NOT NULL DEFAULT '[]',
    result_artifact_id INTEGER,
    cleanup_hooks TEXT NOT NULL DEFAULT '[]',
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id),
    FOREIGN KEY (result_artifact_id) REFERENCES agent_artifacts(id),
    UNIQUE(session_id, task_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_task_states_session ON agent_task_states(session_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_task_states_agent ON agent_task_states(agent_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_task_states_status ON agent_task_states(status, updated_at DESC);
"#;

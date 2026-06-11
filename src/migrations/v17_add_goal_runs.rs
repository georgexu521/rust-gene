//! v17 migration — add goal_runs and goal_steps for durable Codex-style goal mode.

use rusqlite::{Connection, Result as SqlResult};

pub struct V17AddGoalRuns;

impl crate::migrations::Migration for V17AddGoalRuns {
    fn version(&self) -> i32 {
        17
    }

    fn name(&self) -> &str {
        "v17_add_goal_runs"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(ADD_GOAL_TABLES)
    }
}

const ADD_GOAL_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS goal_runs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    objective TEXT NOT NULL,
    status TEXT NOT NULL,
    stop_rules_json TEXT,
    budget_json TEXT,
    turn_count INTEGER NOT NULL DEFAULT 0,
    last_closeout_status TEXT,
    last_blocker TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE TABLE IF NOT EXISTS goal_steps (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    turn_index INTEGER NOT NULL,
    prompt TEXT NOT NULL,
    closeout_status TEXT,
    verification_status TEXT,
    changed_files INTEGER NOT NULL DEFAULT 0,
    validation_items INTEGER NOT NULL DEFAULT 0,
    decision TEXT NOT NULL,
    summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (goal_id) REFERENCES goal_runs(id),
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_goal_runs_session
    ON goal_runs(session_id);

CREATE INDEX IF NOT EXISTS idx_goal_runs_status
    ON goal_runs(session_id, status);

CREATE INDEX IF NOT EXISTS idx_goal_steps_goal
    ON goal_steps(goal_id, turn_index);
"#;

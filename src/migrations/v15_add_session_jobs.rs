//! v15 migration — session_jobs table for durable shell process lifecycle.
//!
//! Slice D of the opencode programming parity plan.

use rusqlite::{Connection, Result as SqlResult};

pub struct V15AddSessionJobs;

impl crate::migrations::Migration for V15AddSessionJobs {
    fn version(&self) -> i32 {
        15
    }

    fn name(&self) -> &str {
        "v15_add_session_jobs"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_SESSION_JOBS)
    }
}

const CREATE_SESSION_JOBS: &str = r#"
CREATE TABLE IF NOT EXISTS session_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    command TEXT NOT NULL DEFAULT '',
    cwd TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    exit_code INTEGER,
    timed_out INTEGER NOT NULL DEFAULT 0,
    tool_output_uri TEXT,
    cancelled INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_session_jobs_job
    ON session_jobs(session_id, job_id);
CREATE INDEX IF NOT EXISTS idx_session_jobs_status
    ON session_jobs(session_id, status);
"#;

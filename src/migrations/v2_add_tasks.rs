//! 迁移 v2 - 添加 tasks 表，支持 TaskManager 持久化

use rusqlite::{Connection, Result as SqlResult};

pub struct V2AddTasks;

impl crate::migrations::Migration for V2AddTasks {
    fn version(&self) -> i32 {
        2
    }

    fn name(&self) -> &str {
        "v2_add_tasks"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_TASKS_SCHEMA)
    }
}

const CREATE_TASKS_SCHEMA: &str = r#"
-- 任务表（用于 TaskManager 持久化）
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    task_type TEXT NOT NULL DEFAULT 'local',
    parent_id TEXT,
    children TEXT,
    created_at TEXT,
    completed_at TEXT,
    metadata TEXT,
    output TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_id);
"#;

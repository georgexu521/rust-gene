//! v14 migration — provider health runs for product status (Phase 2: productization).

use rusqlite::{Connection, Result as SqlResult};

pub struct V14AddProviderHealthRuns;

impl crate::migrations::Migration for V14AddProviderHealthRuns {
    fn version(&self) -> i32 {
        14
    }

    fn name(&self) -> &str {
        "v14_add_provider_health_runs"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(CREATE_PROVIDER_HEALTH_RUNS)
    }
}

const CREATE_PROVIDER_HEALTH_RUNS: &str = r#"
CREATE TABLE IF NOT EXISTS provider_health_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    protocol_family TEXT NOT NULL DEFAULT 'openai_compatible',
    status TEXT NOT NULL,
    error_category TEXT,
    latency_ms INTEGER,
    step_count INTEGER NOT NULL DEFAULT 0,
    passed_steps INTEGER NOT NULL DEFAULT 0,
    details_json TEXT NOT NULL DEFAULT '{}',
    run_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_provider_health_runs_provider
    ON provider_health_runs(provider_id, model_id);
CREATE INDEX IF NOT EXISTS idx_provider_health_runs_run_at
    ON provider_health_runs(run_at);
"#;

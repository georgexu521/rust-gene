# Workflow Metrics Storage Spec (S1)

## 1. Goal
Persist workflow execution north-star metrics into SQLite so trend analysis and weekly reporting can be automated.

## 2. Current implementation (2026-04-22)
- Added SQLite persistence entrypoint: `persist_workflow_metrics(task, goal, metrics)`.
- Storage path: `$PRIORITY_AGENT_WORKFLOW_METRICS_DB` or default `~/.priority-agent/workflow_metrics.db`.
- Table: `workflow_metrics_runs`.
- Hook point: Workflow report generation (`WorkflowEngine::build_report`) persists one snapshot per workflow run.

## 3. Schema

```sql
CREATE TABLE workflow_metrics_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_at TEXT NOT NULL DEFAULT (datetime('now')),
    week_key TEXT NOT NULL,
    task TEXT NOT NULL,
    goal TEXT NOT NULL,
    total_steps INTEGER NOT NULL,
    success INTEGER NOT NULL,
    failed INTEGER NOT NULL,
    needs_refactor INTEGER NOT NULL,
    skipped INTEGER NOT NULL,
    total_duration_ms INTEGER NOT NULL,
    total_retries INTEGER NOT NULL,
    success_rate REAL NOT NULL,
    rework_rate REAL NOT NULL,
    mainline_hit INTEGER NOT NULL,
    drift_interruption_rate REAL NOT NULL,
    first_plan_coverage REAL NOT NULL
);
```

## 4. Data semantics
- `week_key`: `%Y-W%W`, used for weekly trend grouping.
- `mainline_hit`: boolean encoded as 0/1.
- `drift_interruption_rate`: current run-time global approximation (to be refined in S1.2).
- One workflow run -> one row.

## 5. Validation
- Unit test: `test_persist_workflow_metrics_writes_sqlite_row`.
- End-to-end: verify report includes `Metrics persisted: yes`.

## 6. Next (S1.2)
1. ✅ Added read/query API for weekly aggregation: `GET /api/workflow/metrics/weekly?limit=8`.
2. ✅ Added `scripts/workflow-weekly-report.sh` with WoW columns.
3. Split metrics by session/task dimension with stable run ids.

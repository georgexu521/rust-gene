//! Workflow 执行指标聚合器
//!
//! M1 范围：轻量级执行统计，不持久化。
//! 从 ExecutionRecord 聚合成功/失败/重构等计数。

use super::executor::{ExecutionOutcome, ExecutionRecord};
use crate::engine::plan_mode::Plan;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static GLOBAL_WORKFLOW_RUNS: AtomicU64 = AtomicU64::new(0);
static GLOBAL_DRIFT_INTERRUPTS: AtomicU64 = AtomicU64::new(0);

/// 单步类型统计
#[derive(Debug, Clone, Default)]
pub struct StepTypeStats {
    pub count: usize,
    pub success: usize,
    pub failed: usize,
    pub needs_refactor: usize,
    pub skipped: usize,
    pub total_duration_ms: u64,
    pub total_retries: usize,
}

impl StepTypeStats {
    pub fn avg_duration_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.count as f64
        }
    }
}

/// Workflow 执行指标
#[derive(Debug, Clone, Default)]
pub struct WorkflowMetrics {
    pub total_steps: usize,
    pub success: usize,
    pub failed: usize,
    pub needs_refactor: usize,
    pub skipped: usize,
    pub total_duration_ms: u64,
    pub total_retries: usize,
    /// 按工具名称分组的统计（None 表示无工具）
    pub by_tool: std::collections::HashMap<String, StepTypeStats>,
    /// 北极星指标（当前为运行时近似值）
    pub north_star: NorthStarMetrics,
    /// 当前策略版本（用于门禁对齐）
    pub policy_version: String,
}

#[derive(Debug, Clone, Default)]
pub struct NorthStarMetrics {
    pub mainline_hit: bool,
    pub drift_interruption_rate: f64,
    pub first_plan_coverage: f64,
    pub rework_rate: f64,
    /// 统一目标函数得分（0-100）
    pub objective_score: f64,
}

impl WorkflowMetrics {
    pub fn new() -> Self {
        Self {
            policy_version: "v1".to_string(),
            ..Self::default()
        }
    }

    /// 从执行记录列表聚合指标
    pub fn from_records(records: &[ExecutionRecord]) -> Self {
        let mut metrics = Self::new();
        for record in records {
            metrics.record(record);
        }
        if metrics.total_steps > 0 {
            metrics.north_star.rework_rate =
                (metrics.needs_refactor as f64 / metrics.total_steps as f64) * 100.0;
        }
        metrics
    }

    pub fn from_workflow(plan: &Plan, records: &[ExecutionRecord], mainline_goal: &str) -> Self {
        let mut metrics = Self::from_records(records);
        metrics.north_star.mainline_hit = records
            .first()
            .map(|r| relevance_score(&r.description, mainline_goal) >= 0.25)
            .unwrap_or(false);
        metrics.north_star.first_plan_coverage = estimate_plan_coverage(plan, mainline_goal);
        metrics.north_star.drift_interruption_rate = global_drift_interruption_rate();
        metrics.north_star.objective_score = metrics.compute_objective_score();
        metrics
    }

    /// 记录单步执行结果
    pub fn record(&mut self, record: &ExecutionRecord) {
        self.total_steps += 1;
        self.total_duration_ms += record.duration_ms;
        self.total_retries += record.retry_count;

        match &record.outcome {
            ExecutionOutcome::Success(_) => self.success += 1,
            ExecutionOutcome::Failed(_) => self.failed += 1,
            ExecutionOutcome::NeedsRefactor(_) => self.needs_refactor += 1,
            ExecutionOutcome::Skipped(_) => self.skipped += 1,
        }

        let tool_key = record.tool.clone().unwrap_or_else(|| "(none)".to_string());
        let stats = self.by_tool.entry(tool_key).or_default();
        stats.count += 1;
        stats.total_duration_ms += record.duration_ms;
        stats.total_retries += record.retry_count;
        match &record.outcome {
            ExecutionOutcome::Success(_) => stats.success += 1,
            ExecutionOutcome::Failed(_) => stats.failed += 1,
            ExecutionOutcome::NeedsRefactor(_) => stats.needs_refactor += 1,
            ExecutionOutcome::Skipped(_) => stats.skipped += 1,
        }
    }

    /// 成功率（百分比）
    pub fn success_rate(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            (self.success as f64 / self.total_steps as f64) * 100.0
        }
    }

    /// 重构率（百分比）
    pub fn refactor_rate(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            (self.needs_refactor as f64 / self.total_steps as f64) * 100.0
        }
    }

    /// 平均步骤耗时（毫秒）
    pub fn avg_duration_ms(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_steps as f64
        }
    }

    /// Markdown 格式摘要
    pub fn summary(&self) -> String {
        let mut output = String::new();
        output.push_str("## 执行指标\n\n");
        output.push_str(&format!(
            "- 总步骤: {} | 成功: {} | 失败: {} | 需重构: {} | 跳过: {}\n",
            self.total_steps, self.success, self.failed, self.needs_refactor, self.skipped
        ));
        output.push_str(&format!(
            "- 成功率: {:.1}% | 重构率: {:.1}%\n",
            self.success_rate(),
            self.refactor_rate()
        ));
        output.push_str(&format!(
            "- 总耗时: {}ms | 平均: {:.1}ms/步 | 总重试: {}\n",
            self.total_duration_ms,
            self.avg_duration_ms(),
            self.total_retries
        ));

        if !self.by_tool.is_empty() {
            output.push_str("\n### 按工具统计\n\n");
            let mut tools: Vec<_> = self.by_tool.iter().collect();
            tools.sort_by_key(|(k, _)| *k);
            for (tool, stats) in tools {
                output.push_str(&format!(
                    "- `{}`: {} 步（成功 {} / 失败 {} / 重构 {} / 跳过 {}），平均 {:.1}ms\n",
                    tool,
                    stats.count,
                    stats.success,
                    stats.failed,
                    stats.needs_refactor,
                    stats.skipped,
                    stats.avg_duration_ms()
                ));
            }
        }

        output.push_str("\n### 北极星指标（近似）\n\n");
        output.push_str(&format!(
            "- Mainline Hit: {}\n",
            if self.north_star.mainline_hit { "yes" } else { "no" }
        ));
        output.push_str(&format!(
            "- Drift Interruption Rate: {:.1}%\n",
            self.north_star.drift_interruption_rate
        ));
        output.push_str(&format!(
            "- First Plan Coverage: {:.1}%\n",
            self.north_star.first_plan_coverage
        ));
        output.push_str(&format!(
            "- Rework Rate: {:.1}%\n",
            self.north_star.rework_rate
        ));
        output.push_str(&format!(
            "- Objective Score: {:.1}\n",
            self.north_star.objective_score
        ));

        output
    }

    /// 统一目标函数（第一性原理版本）
    /// Score = MainlineHit*0.4 + FirstPassQuality*0.35 + CostEfficiency*0.25
    fn compute_objective_score(&self) -> f64 {
        let mainline_hit = if self.north_star.mainline_hit {
            100.0
        } else {
            0.0
        };
        let first_pass_quality = (self.success_rate() - self.refactor_rate()).clamp(0.0, 100.0);
        let cost_efficiency = cost_efficiency_score(self.avg_duration_ms());
        (0.4 * mainline_hit + 0.35 * first_pass_quality + 0.25 * cost_efficiency)
            .clamp(0.0, 100.0)
    }
}

pub fn record_workflow_run() {
    GLOBAL_WORKFLOW_RUNS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_drift_interruption() {
    GLOBAL_DRIFT_INTERRUPTS.fetch_add(1, Ordering::Relaxed);
}

pub fn global_drift_interruption_rate() -> f64 {
    let runs = GLOBAL_WORKFLOW_RUNS.load(Ordering::Relaxed);
    if runs == 0 {
        return 0.0;
    }
    let interrupts = GLOBAL_DRIFT_INTERRUPTS.load(Ordering::Relaxed);
    (interrupts as f64 / runs as f64) * 100.0
}

fn default_metrics_db_path() -> PathBuf {
    if let Ok(v) = std::env::var("PRIORITY_AGENT_WORKFLOW_METRICS_DB") {
        if !v.trim().is_empty() {
            return PathBuf::from(v);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("workflow_metrics.db")
}

fn open_metrics_db(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create metrics dir failed: {}", e))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("open metrics db failed: {}", e))?;
    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS workflow_metrics_runs (
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
    first_plan_coverage REAL NOT NULL,
    objective_score REAL NOT NULL DEFAULT 0.0,
    policy_version TEXT NOT NULL DEFAULT 'v1'
);
CREATE INDEX IF NOT EXISTS idx_workflow_metrics_runs_week ON workflow_metrics_runs(week_key);
CREATE INDEX IF NOT EXISTS idx_workflow_metrics_runs_run_at ON workflow_metrics_runs(run_at DESC);

CREATE TABLE IF NOT EXISTS workflow_metrics_audits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    audit_at TEXT NOT NULL DEFAULT (datetime('now')),
    week_key TEXT NOT NULL,
    task TEXT NOT NULL,
    auto_mainline_hit INTEGER NOT NULL,
    manual_mainline_hit INTEGER NOT NULL,
    auto_coverage REAL NOT NULL,
    manual_coverage REAL NOT NULL,
    auto_objective_score REAL NOT NULL,
    manual_objective_score REAL NOT NULL,
    note TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_workflow_metrics_audits_week ON workflow_metrics_audits(week_key);
"#,
    )
    .map_err(|e| format!("init metrics schema failed: {}", e))?;
    // 兼容旧表结构：尝试补列（重复列错误可忽略）
    let _ = conn.execute(
        "ALTER TABLE workflow_metrics_runs ADD COLUMN objective_score REAL NOT NULL DEFAULT 0.0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE workflow_metrics_runs ADD COLUMN policy_version TEXT NOT NULL DEFAULT 'v1'",
        [],
    );
    Ok(conn)
}

/// 持久化一次 workflow 指标快照到 SQLite。
pub fn persist_workflow_metrics(task: &str, goal: &str, metrics: &WorkflowMetrics) -> Result<(), String> {
    let path = default_metrics_db_path();
    let conn = open_metrics_db(&path)?;
    let week_key = chrono::Local::now().format("%Y-W%W").to_string();
    conn.execute(
        r#"
INSERT INTO workflow_metrics_runs (
    week_key, task, goal, total_steps, success, failed, needs_refactor, skipped,
    total_duration_ms, total_retries, success_rate, rework_rate, mainline_hit,
    drift_interruption_rate, first_plan_coverage, objective_score, policy_version
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
"#,
        params![
            week_key,
            task,
            goal,
            metrics.total_steps as i64,
            metrics.success as i64,
            metrics.failed as i64,
            metrics.needs_refactor as i64,
            metrics.skipped as i64,
            metrics.total_duration_ms as i64,
            metrics.total_retries as i64,
            metrics.success_rate(),
            metrics.refactor_rate(),
            if metrics.north_star.mainline_hit { 1 } else { 0 },
            metrics.north_star.drift_interruption_rate,
            metrics.north_star.first_plan_coverage,
            metrics.north_star.objective_score,
            metrics.policy_version
        ],
    )
    .map_err(|e| format!("insert workflow metrics failed: {}", e))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyMetricSummary {
    pub week_key: String,
    pub runs: usize,
    pub mainline_hit_rate: f64,
    pub avg_first_plan_coverage: f64,
    pub avg_rework_rate: f64,
    pub avg_objective_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyCalibrationSummary {
    pub week_key: String,
    pub samples: usize,
    pub avg_mainline_bias_abs: f64,
    pub avg_coverage_bias_abs: f64,
    pub avg_objective_bias_abs: f64,
}

#[derive(Debug, Clone)]
pub struct ManualCalibrationInput {
    pub task: String,
    pub auto_mainline_hit: bool,
    pub manual_mainline_hit: bool,
    pub auto_coverage: f64,
    pub manual_coverage: f64,
    pub auto_objective_score: f64,
    pub manual_objective_score: f64,
    pub note: Option<String>,
}

/// 读取最近 N 周的指标汇总（按周降序）。
pub fn load_weekly_metric_summary(limit_weeks: usize) -> Result<Vec<WeeklyMetricSummary>, String> {
    let path = default_metrics_db_path();
    let conn = open_metrics_db(&path)?;
    load_weekly_metric_summary_from_conn(&conn, limit_weeks)
}

fn load_weekly_metric_summary_from_conn(
    conn: &Connection,
    limit_weeks: usize,
) -> Result<Vec<WeeklyMetricSummary>, String> {
    let mut stmt = conn
        .prepare(
            r#"
SELECT
  week_key,
  COUNT(*) AS runs,
  AVG(mainline_hit) * 100.0 AS mainline_hit_rate,
  AVG(first_plan_coverage) AS avg_first_plan_coverage,
  AVG(rework_rate) AS avg_rework_rate,
  AVG(objective_score) AS avg_objective_score
FROM workflow_metrics_runs
GROUP BY week_key
ORDER BY week_key DESC
LIMIT ?1
"#,
        )
        .map_err(|e| format!("prepare weekly summary query failed: {}", e))?;
    let rows = stmt
        .query_map(params![limit_weeks as i64], |row| {
            Ok(WeeklyMetricSummary {
                week_key: row.get(0)?,
                runs: row.get::<_, i64>(1)? as usize,
                mainline_hit_rate: row.get(2)?,
                avg_first_plan_coverage: row.get(3)?,
                avg_rework_rate: row.get(4)?,
                avg_objective_score: row.get(5)?,
            })
        })
        .map_err(|e| format!("query weekly summary failed: {}", e))?;

    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| format!("read weekly summary row failed: {}", e))?);
    }
    Ok(out)
}

/// 记录人工抽样校准结果（自动指标 vs 人工标注）
pub fn persist_manual_calibration(
    input: &ManualCalibrationInput,
) -> Result<(), String> {
    let path = default_metrics_db_path();
    let conn = open_metrics_db(&path)?;
    let week_key = chrono::Local::now().format("%Y-W%W").to_string();
    conn.execute(
        r#"
INSERT INTO workflow_metrics_audits (
    week_key, task, auto_mainline_hit, manual_mainline_hit, auto_coverage, manual_coverage,
    auto_objective_score, manual_objective_score, note
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
"#,
        params![
            week_key,
            input.task,
            if input.auto_mainline_hit { 1 } else { 0 },
            if input.manual_mainline_hit { 1 } else { 0 },
            input.auto_coverage,
            input.manual_coverage,
            input.auto_objective_score,
            input.manual_objective_score,
            input.note.as_deref().unwrap_or("")
        ],
    )
    .map_err(|e| format!("insert workflow calibration failed: {}", e))?;
    Ok(())
}

/// 读取最近 N 周人工校准偏差汇总。
pub fn load_weekly_calibration_summary(
    limit_weeks: usize,
) -> Result<Vec<WeeklyCalibrationSummary>, String> {
    let path = default_metrics_db_path();
    let conn = open_metrics_db(&path)?;
    let mut stmt = conn
        .prepare(
            r#"
SELECT
  week_key,
  COUNT(*) AS samples,
  AVG(ABS(manual_mainline_hit - auto_mainline_hit)) * 100.0 AS avg_mainline_bias_abs,
  AVG(ABS(manual_coverage - auto_coverage)) AS avg_coverage_bias_abs,
  AVG(ABS(manual_objective_score - auto_objective_score)) AS avg_objective_bias_abs
FROM workflow_metrics_audits
GROUP BY week_key
ORDER BY week_key DESC
LIMIT ?1
"#,
        )
        .map_err(|e| format!("prepare weekly calibration query failed: {}", e))?;
    let rows = stmt
        .query_map(params![limit_weeks as i64], |row| {
            Ok(WeeklyCalibrationSummary {
                week_key: row.get(0)?,
                samples: row.get::<_, i64>(1)? as usize,
                avg_mainline_bias_abs: row.get(2)?,
                avg_coverage_bias_abs: row.get(3)?,
                avg_objective_bias_abs: row.get(4)?,
            })
        })
        .map_err(|e| format!("query weekly calibration failed: {}", e))?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| format!("read weekly calibration row failed: {}", e))?);
    }
    Ok(out)
}

fn relevance_score(text: &str, mainline: &str) -> f64 {
    let t = text.to_lowercase();
    let m = mainline.to_lowercase();
    if t.is_empty() || m.is_empty() {
        return 0.0;
    }
    let t_set: std::collections::HashSet<char> = t.chars().collect();
    let m_set: std::collections::HashSet<char> = m.chars().collect();
    if t_set.is_empty() || m_set.is_empty() {
        return 0.0;
    }
    let inter = t_set.intersection(&m_set).count() as f64;
    let denom = t_set.len().max(m_set.len()) as f64;
    (inter / denom).clamp(0.0, 1.0)
}

fn estimate_plan_coverage(plan: &Plan, mainline_goal: &str) -> f64 {
    if plan.steps.is_empty() {
        return 0.0;
    }
    let aligned = plan
        .steps
        .iter()
        .filter(|s| relevance_score(&s.description, mainline_goal) >= 0.2)
        .count();
    (aligned as f64 / plan.steps.len() as f64) * 100.0
}

fn cost_efficiency_score(avg_duration_ms: f64) -> f64 {
    if avg_duration_ms <= 0.0 {
        return 100.0;
    }
    // 经验映射：平均 200ms 约 80 分，1000ms 约 50 分，3000ms 约 25 分
    (100.0 / (1.0 + avg_duration_ms / 1000.0)).clamp(0.0, 100.0)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::env_guard::EnvVarGuard;
    use tempfile::tempdir;

    fn make_record(
        tool: Option<&str>,
        outcome: ExecutionOutcome,
        duration_ms: u64,
        retry_count: usize,
    ) -> ExecutionRecord {
        ExecutionRecord {
            step_index: 0,
            description: "test".into(),
            tool: tool.map(String::from),
            outcome,
            duration_ms,
            retry_count,
        }
    }

    #[test]
    fn test_metrics_from_records() {
        let records = vec![
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 100, 0),
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 200, 0),
            make_record(Some("file_edit"), ExecutionOutcome::NeedsRefactor("err".into()), 300, 1),
            make_record(None, ExecutionOutcome::Skipped("skip".into()), 50, 0),
        ];
        let m = WorkflowMetrics::from_records(&records);
        assert_eq!(m.total_steps, 4);
        assert_eq!(m.success, 2);
        assert_eq!(m.needs_refactor, 1);
        assert_eq!(m.skipped, 1);
        assert_eq!(m.total_duration_ms, 650);
        assert_eq!(m.total_retries, 1);
        assert!((m.success_rate() - 50.0).abs() < 0.1);
        assert!((m.refactor_rate() - 25.0).abs() < 0.1);
        assert!((m.avg_duration_ms() - 162.5).abs() < 0.1);
    }

    #[test]
    fn test_by_tool_grouping() {
        let records = vec![
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 100, 0),
            make_record(Some("bash"), ExecutionOutcome::Failed("err".into()), 200, 0),
            make_record(Some("grep"), ExecutionOutcome::Success("ok".into()), 50, 0),
        ];
        let m = WorkflowMetrics::from_records(&records);
        assert_eq!(m.by_tool.len(), 2);
        let bash = m.by_tool.get("bash").unwrap();
        assert_eq!(bash.count, 2);
        assert_eq!(bash.success, 1);
        assert_eq!(bash.failed, 1);
        assert!((bash.avg_duration_ms() - 150.0).abs() < 0.1);
    }

    #[test]
    fn test_empty_records() {
        let m = WorkflowMetrics::from_records(&[]);
        assert_eq!(m.total_steps, 0);
        assert_eq!(m.success_rate(), 0.0);
        assert_eq!(m.refactor_rate(), 0.0);
        assert_eq!(m.avg_duration_ms(), 0.0);
    }

    #[test]
    fn test_summary_contains_key_stats() {
        let records = vec![
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 100, 0),
            make_record(Some("file_edit"), ExecutionOutcome::NeedsRefactor("err".into()), 200, 1),
        ];
        let m = WorkflowMetrics::from_records(&records);
        let s = m.summary();
        assert!(s.contains("总步骤: 2"));
        assert!(s.contains("成功: 1"));
        assert!(s.contains("需重构: 1"));
        assert!(s.contains("成功率: 50.0%"));
        assert!(s.contains("bash"));
        assert!(s.contains("file_edit"));
    }

    #[test]
    fn test_persist_workflow_metrics_writes_sqlite_row() {
        let tmp = tempdir().expect("tmp dir");
        let db_path = tmp.path().join("workflow_metrics.db");

        let mut env = EnvVarGuard::acquire_blocking();
        env.set(
            "PRIORITY_AGENT_WORKFLOW_METRICS_DB",
            db_path.to_string_lossy().as_ref(),
        );

        let records = vec![
            make_record(Some("bash"), ExecutionOutcome::Success("ok".into()), 100, 0),
            make_record(Some("file_edit"), ExecutionOutcome::NeedsRefactor("err".into()), 200, 1),
        ];
        let metrics = WorkflowMetrics::from_records(&records);
        persist_workflow_metrics("task-a", "goal-a", &metrics).expect("persist metrics");

        let conn = Connection::open(&db_path).expect("open db");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM workflow_metrics_runs", [], |r| r.get(0))
            .expect("count rows");
        assert!(count >= 1);
    }

    #[test]
    fn test_load_weekly_metric_summary_from_conn() {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        conn.execute_batch(
            r#"
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
    first_plan_coverage REAL NOT NULL,
    objective_score REAL NOT NULL,
    policy_version TEXT NOT NULL
);
"#,
        )
        .expect("create table");
        conn.execute(
            "INSERT INTO workflow_metrics_runs (week_key, task, goal, total_steps, success, failed, needs_refactor, skipped, total_duration_ms, total_retries, success_rate, rework_rate, mainline_hit, drift_interruption_rate, first_plan_coverage, objective_score, policy_version)
             VALUES ('2026-W16','a','a',3,2,0,1,0,1000,1,66.7,33.3,1,10.0,70.0,72.0,'v1')",
            [],
        )
        .expect("insert row1");
        conn.execute(
            "INSERT INTO workflow_metrics_runs (week_key, task, goal, total_steps, success, failed, needs_refactor, skipped, total_duration_ms, total_retries, success_rate, rework_rate, mainline_hit, drift_interruption_rate, first_plan_coverage, objective_score, policy_version)
             VALUES ('2026-W16','b','b',4,3,0,1,0,1200,1,75.0,25.0,0,20.0,80.0,68.0,'v1')",
            [],
        )
        .expect("insert row2");
        conn.execute(
            "INSERT INTO workflow_metrics_runs (week_key, task, goal, total_steps, success, failed, needs_refactor, skipped, total_duration_ms, total_retries, success_rate, rework_rate, mainline_hit, drift_interruption_rate, first_plan_coverage, objective_score, policy_version)
             VALUES ('2026-W15','c','c',2,2,0,0,0,600,0,100.0,0.0,1,0.0,90.0,92.0,'v1')",
            [],
        )
        .expect("insert row3");

        let summary = load_weekly_metric_summary_from_conn(&conn, 5).expect("load summary");
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].week_key, "2026-W16");
        assert_eq!(summary[0].runs, 2);
        assert!((summary[0].mainline_hit_rate - 50.0).abs() < 0.1);
        assert!((summary[0].avg_first_plan_coverage - 75.0).abs() < 0.1);
        assert!((summary[0].avg_objective_score - 70.0).abs() < 0.1);
    }

    #[test]
    fn test_manual_calibration_roundtrip() {
        let tmp = tempdir().expect("tmp dir");
        let db_path = tmp.path().join("workflow_metrics.db");
        let mut env = EnvVarGuard::acquire_blocking();
        env.set(
            "PRIORITY_AGENT_WORKFLOW_METRICS_DB",
            db_path.to_string_lossy().as_ref(),
        );

        let input = ManualCalibrationInput {
            task: "task-1".to_string(),
            auto_mainline_hit: true,
            manual_mainline_hit: false,
            auto_coverage: 80.0,
            manual_coverage: 60.0,
            auto_objective_score: 75.0,
            manual_objective_score: 55.0,
            note: Some("sample audit".to_string()),
        };
        persist_manual_calibration(&input).expect("persist calibration");

        let rows = load_weekly_calibration_summary(4).expect("load calibration summary");
        assert!(!rows.is_empty());
        assert!(rows[0].samples >= 1);
        assert!(rows[0].avg_mainline_bias_abs >= 0.0);
    }
}

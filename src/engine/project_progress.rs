//! Project-scoped progress ledger, separate from durable user profile memory.

use crate::engine::task_contract::ExecutionReport;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProgressKind {
    ProjectStatus,
    NextStep,
    ValidationBaseline,
    OpenRisk,
}

impl ProjectProgressKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ProjectStatus => "project_status",
            Self::NextStep => "next_step",
            Self::ValidationBaseline => "validation_baseline",
            Self::OpenRisk => "open_risk",
        }
    }

    fn default_stale_after_days(self) -> i64 {
        match self {
            Self::ProjectStatus => 14,
            Self::NextStep => 3,
            Self::ValidationBaseline => 30,
            Self::OpenRisk => 14,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProgressStatus {
    Active,
    Superseded,
    Archived,
}

impl ProjectProgressStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectProgressRecord {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_labels: Vec<String>,
    pub task_id: String,
    pub objective: String,
    pub kind: ProjectProgressKind,
    pub status: ProjectProgressStatus,
    pub task_status: String,
    pub content: String,
    pub evidence: Vec<String>,
    pub stale_after: Option<String>,
    pub supersedes: Vec<String>,
    pub superseded_by: Option<String>,
    pub changed_files: Vec<String>,
    pub validation: Vec<String>,
    pub risks: Vec<String>,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectHeartbeatInput<'a> {
    pub project_name: &'a str,
    pub project_root: &'a std::path::Path,
    pub branch: &'a str,
    pub dirty_count: usize,
    pub dirty_summary: &'a str,
    pub goal: &'a str,
    pub memory: &'a str,
    pub memory_proposal: &'a str,
    pub progress: &'a str,
    pub next_step: &'a str,
}

impl ProjectProgressRecord {
    pub fn from_execution_report(report: &ExecutionReport) -> Vec<Self> {
        let now = chrono::Utc::now();
        let mut records = Vec::new();
        records.push(Self::new(
            report,
            ProjectProgressKind::ProjectStatus,
            format!(
                "{}: {} files={} validation={} risks={}",
                report.status.label(),
                compact_text(&report.objective, 160),
                report.changed_files.len(),
                report.validation_evidence.len(),
                report.risks.len()
            ),
            report.validation_evidence.clone(),
            now,
        ));
        if !report.validation_evidence.is_empty() {
            records.push(Self::new(
                report,
                ProjectProgressKind::ValidationBaseline,
                format!(
                    "Validation for `{}`: {}",
                    compact_text(&report.objective, 120),
                    compact_text(&report.validation_evidence.join("; "), 260)
                ),
                report.validation_evidence.clone(),
                now,
            ));
        }
        for risk in report
            .risks
            .iter()
            .filter(|risk| risk.as_str() != "none recorded")
        {
            records.push(Self::new(
                report,
                ProjectProgressKind::OpenRisk,
                format!(
                    "Open risk for `{}`: {}",
                    compact_text(&report.objective, 120),
                    compact_text(risk, 260)
                ),
                vec![format!("risk: {risk}")],
                now,
            ));
        }
        for next_step in &report.next_steps {
            records.push(Self::new(
                report,
                ProjectProgressKind::NextStep,
                format!(
                    "Next step for `{}`: {}",
                    compact_text(&report.objective, 120),
                    compact_text(next_step, 260)
                ),
                vec![format!("next_step: {next_step}")],
                now,
            ));
        }
        records
    }

    fn new(
        report: &ExecutionReport,
        kind: ProjectProgressKind,
        content: String,
        evidence: Vec<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        let created_at = now.to_rfc3339();
        let stale_after = now + chrono::Duration::days(kind.default_stale_after_days());
        let (project_id, project_labels) = current_project_progress_identity(None);
        Self {
            id: stable_project_progress_id(&report.task_id, kind, &content),
            created_at: created_at.clone(),
            updated_at: created_at,
            project_id,
            project_labels,
            task_id: report.task_id.clone(),
            objective: report.objective.clone(),
            kind,
            status: ProjectProgressStatus::Active,
            task_status: report.status.label().to_string(),
            content,
            evidence,
            stale_after: Some(stale_after.to_rfc3339()),
            supersedes: Vec::new(),
            superseded_by: None,
            changed_files: report.changed_files.clone(),
            validation: report.validation_evidence.clone(),
            risks: report.risks.clone(),
            next_steps: report.next_steps.clone(),
        }
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "{} {}: {} files={} validation={} risks={} next_steps={}",
            self.kind.label(),
            self.task_status,
            compact_text(&self.objective, 120),
            self.changed_files.len(),
            self.validation.len(),
            self.risks.len(),
            self.next_steps.len()
        )
    }

    pub fn heartbeat(input: ProjectHeartbeatInput<'_>) -> Self {
        let now = chrono::Utc::now();
        let created_at = now.to_rfc3339();
        let (project_id, project_labels) =
            current_project_progress_identity(Some(input.project_root));
        let content = format!(
            "Project heartbeat for {}: branch={} git_changes={} progress={} goal={}",
            compact_text(input.project_name, 80),
            compact_text(input.branch, 80),
            input.dirty_count,
            compact_text(input.progress, 180),
            compact_text(input.goal, 180)
        );
        Self {
            id: format!("project-heartbeat-{}", uuid::Uuid::new_v4()),
            created_at: created_at.clone(),
            updated_at: created_at,
            project_id,
            project_labels,
            task_id: "project-heartbeat".to_string(),
            objective: format!(
                "Maintain project progress for {}",
                input.project_root.display()
            ),
            kind: ProjectProgressKind::ProjectStatus,
            status: ProjectProgressStatus::Active,
            task_status: "heartbeat".to_string(),
            content,
            evidence: vec![
                format!("project_root: {}", input.project_root.display()),
                format!("git_dirty: {} ({})", input.dirty_count, input.dirty_summary),
                format!("memory: {}", input.memory),
                format!("memory_proposal: {}", input.memory_proposal),
            ],
            stale_after: Some((now + chrono::Duration::days(7)).to_rfc3339()),
            supersedes: Vec::new(),
            superseded_by: None,
            changed_files: if input.dirty_count > 0 {
                vec![input.dirty_summary.to_string()]
            } else {
                Vec::new()
            },
            validation: Vec::new(),
            risks: Vec::new(),
            next_steps: vec![input.next_step.to_string()],
        }
    }

    pub fn is_stale(&self) -> bool {
        let Some(stale_after) = self.stale_after.as_deref() else {
            return false;
        };
        chrono::DateTime::parse_from_rfc3339(stale_after)
            .map(|timestamp| timestamp.with_timezone(&chrono::Utc) < chrono::Utc::now())
            .unwrap_or(false)
    }
}

fn stable_project_progress_id(task_id: &str, kind: ProjectProgressKind, content: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    task_id.hash(&mut hasher);
    kind.hash(&mut hasher);
    content.hash(&mut hasher);
    format!("project-progress-{:016x}", hasher.finish())
}

fn current_project_progress_identity(project_root: Option<&Path>) -> (Option<String>, Vec<String>) {
    let mut scope = crate::memory::MemoryScope::default();
    if let Some(project_root) = project_root {
        scope.project_root = Some(project_root.to_path_buf());
    }
    let identity = scope.identity();
    if identity.kind == crate::memory::types::MemoryScopeKind::Project {
        (Some(identity.id), identity.labels)
    } else {
        (None, identity.labels)
    }
}

fn backfill_project_progress_identity(
    record: &mut ProjectProgressRecord,
    project_root: Option<&Path>,
) {
    if record.project_id.is_some() || !record.project_labels.is_empty() {
        return;
    }
    let (project_id, project_labels) = current_project_progress_identity(project_root);
    record.project_id = project_id;
    record.project_labels = project_labels;
}

fn project_progress_same_project(
    left: &ProjectProgressRecord,
    right: &ProjectProgressRecord,
) -> bool {
    match (left.project_id.as_deref(), right.project_id.as_deref()) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn project_progress_record_matches_current_project(
    record: &ProjectProgressRecord,
    current_project_id: Option<&str>,
    current_project_labels: &[String],
) -> bool {
    match (record.project_id.as_deref(), current_project_id) {
        (Some(record_project_id), Some(current_project_id)) => {
            record_project_id == current_project_id
        }
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(_), None) => record
            .project_labels
            .iter()
            .any(|label| current_project_labels.contains(label)),
    }
}

#[derive(Debug, Clone)]
pub struct ProjectProgressLedger {
    path: PathBuf,
}

impl ProjectProgressLedger {
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("memory")
            .join("project_progress.jsonl")
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append_execution_report(&self, report: &ExecutionReport) -> anyhow::Result<()> {
        self.append_records(ProjectProgressRecord::from_execution_report(report))
    }

    pub fn append_record(&self, record: &ProjectProgressRecord) -> anyhow::Result<()> {
        self.append_records(vec![record.clone()])
    }

    pub fn append_heartbeat(&self, record: ProjectProgressRecord) -> anyhow::Result<()> {
        self.append_records(vec![record])
    }

    pub fn append_records(
        &self,
        mut new_records: Vec<ProjectProgressRecord>,
    ) -> anyhow::Result<()> {
        if new_records.is_empty() {
            return Ok(());
        }
        let mut records = self.list();
        let now = chrono::Utc::now().to_rfc3339();
        for new_record in &mut new_records {
            backfill_project_progress_identity(new_record, None);
            let mut supersedes = Vec::new();
            for existing in &mut records {
                backfill_project_progress_identity(existing, None);
                if existing.status != ProjectProgressStatus::Active
                    || existing.kind != new_record.kind
                    || existing.id == new_record.id
                    || !project_progress_same_project(existing, new_record)
                {
                    continue;
                }
                existing.status = ProjectProgressStatus::Superseded;
                existing.superseded_by = Some(new_record.id.clone());
                existing.updated_at = now.clone();
                supersedes.push(existing.id.clone());
            }
            new_record.supersedes = supersedes;
        }
        records.extend(new_records);
        write_records_atomically(&self.path, &records)?;
        Ok(())
    }

    pub fn list(&self) -> Vec<ProjectProgressRecord> {
        let content = std::fs::read_to_string(&self.path).unwrap_or_default();
        let mut records = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str::<ProjectProgressRecord>(line).ok())
            .map(|mut record| {
                backfill_project_progress_identity(&mut record, None);
                record
            })
            .collect::<Vec<_>>();
        records.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        records
    }

    pub fn latest(&self) -> Option<ProjectProgressRecord> {
        self.active_records().into_iter().next_back()
    }

    pub fn latest_summary(&self) -> Option<String> {
        let active = self.active_records();
        if active.is_empty() {
            return None;
        }
        let latest_status = active
            .iter()
            .rev()
            .find(|record| record.kind == ProjectProgressKind::ProjectStatus)
            .map(ProjectProgressRecord::compact_summary);
        let latest_next_step = active
            .iter()
            .rev()
            .find(|record| record.kind == ProjectProgressKind::NextStep)
            .map(|record| compact_text(&record.content, 160));
        match (latest_status, latest_next_step) {
            (Some(status), Some(next_step)) => Some(format!("{status}; {next_step}")),
            (Some(status), None) => Some(status),
            (None, Some(next_step)) => Some(next_step),
            (None, None) => active.last().map(ProjectProgressRecord::compact_summary),
        }
    }

    pub fn active_records(&self) -> Vec<ProjectProgressRecord> {
        let (project_id, project_labels) = current_project_progress_identity(None);
        self.list()
            .into_iter()
            .filter(|record| {
                record.status == ProjectProgressStatus::Active
                    && !record.is_stale()
                    && project_progress_record_matches_current_project(
                        record,
                        project_id.as_deref(),
                        &project_labels,
                    )
            })
            .collect()
    }

    pub fn search(&self, query: &str, max_results: usize) -> Vec<ProjectProgressRecord> {
        let terms = query_terms(query);
        if terms.is_empty() {
            return Vec::new();
        }
        let mut scored = self
            .active_records()
            .into_iter()
            .filter_map(|record| {
                let haystack = format!(
                    "{} {} {} {}",
                    record.kind.label(),
                    record.objective,
                    record.content,
                    record.evidence.join(" ")
                )
                .to_ascii_lowercase();
                let score = terms.iter().filter(|term| haystack.contains(*term)).count();
                if score == 0 {
                    None
                } else {
                    Some((score, record))
                }
            })
            .collect::<Vec<_>>();
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.created_at.cmp(&a.1.created_at))
                .then_with(|| a.1.id.cmp(&b.1.id))
        });
        scored
            .into_iter()
            .map(|(_, record)| record)
            .take(max_results)
            .collect()
    }
}

impl Default for ProjectProgressLedger {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

fn compact_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let mut out = trimmed
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::trim)
        .filter(|term| term.chars().count() >= 3)
        .map(str::to_ascii_lowercase)
        .collect()
}

fn write_records_atomically(
    path: &std::path::Path,
    records: &[ProjectProgressRecord],
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut content = String::new();
    for record in records {
        content.push_str(&serde_json::to_string(record)?);
        content.push('\n');
    }
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project_progress.jsonl");
    let tmp_path = parent.join(format!(
        ".{}.{}.tmp",
        file_name,
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&tmp_path, content)?;
    if let Err(error) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::task_contract::ExecutionReportStatus;

    #[test]
    fn project_progress_ledger_records_execution_report_separately() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = ProjectProgressLedger::new(dir.path().join("project_progress.jsonl"));
        let report = ExecutionReport {
            task_id: "task-progress-1".to_string(),
            objective: "finish memory provider PR1".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/memory/provider.rs".to_string()],
            validation_evidence: vec!["cargo test memory_provider: passed".to_string()],
            risks: Vec::new(),
            next_steps: vec!["start proposal review v2".to_string()],
            assumptions: Vec::new(),
        };

        ledger.append_execution_report(&report).unwrap();

        let records = ledger.list();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].task_id, "task-progress-1");
        assert!(ledger
            .latest_summary()
            .unwrap()
            .contains("finish memory provider PR1"));
    }

    #[test]
    fn project_progress_supersedes_old_status_and_searches_active_records() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = ProjectProgressLedger::new(dir.path().join("project_progress.jsonl"));
        let first = ExecutionReport {
            task_id: "task-progress-1".to_string(),
            objective: "fix parser".to_string(),
            status: ExecutionReportStatus::Partial,
            changed_files: vec!["src/parser.rs".to_string()],
            validation_evidence: vec!["cargo test parser failed".to_string()],
            risks: vec!["parser edge case still failing".to_string()],
            next_steps: vec!["repair parser edge case".to_string()],
            assumptions: Vec::new(),
        };
        let second = ExecutionReport {
            task_id: "task-progress-2".to_string(),
            objective: "fix parser".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/parser.rs".to_string()],
            validation_evidence: vec!["cargo test parser passed".to_string()],
            risks: Vec::new(),
            next_steps: vec!["review parser cleanup".to_string()],
            assumptions: Vec::new(),
        };

        ledger.append_execution_report(&first).unwrap();
        ledger.append_execution_report(&second).unwrap();

        let records = ledger.list();
        assert!(records.iter().any(|record| {
            record.kind == ProjectProgressKind::ProjectStatus
                && record.task_id == "task-progress-1"
                && record.status == ProjectProgressStatus::Superseded
                && record.superseded_by.is_some()
        }));
        let active = ledger.active_records();
        assert!(active.iter().any(|record| {
            record.kind == ProjectProgressKind::ProjectStatus
                && record.task_id == "task-progress-2"
                && !record.supersedes.is_empty()
        }));
        let hits = ledger.search("parser passed", 4);
        assert!(hits.iter().any(|record| {
            record.kind == ProjectProgressKind::ValidationBaseline
                && record.content.contains("cargo test parser passed")
        }));
    }

    #[test]
    fn project_progress_active_views_skip_stale_records() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = ProjectProgressLedger::new(dir.path().join("project_progress.jsonl"));
        let old_report = ExecutionReport {
            task_id: "task-stale-progress".to_string(),
            objective: "repair stale parser task".to_string(),
            status: ExecutionReportStatus::Partial,
            changed_files: vec!["src/parser.rs".to_string()],
            validation_evidence: vec!["cargo test parser failed".to_string()],
            risks: Vec::new(),
            next_steps: vec!["obsolete parser repair step".to_string()],
            assumptions: Vec::new(),
        };
        let fresh_report = ExecutionReport {
            task_id: "task-fresh-progress".to_string(),
            objective: "finish current memory lifecycle work".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/memory/provider.rs".to_string()],
            validation_evidence: vec!["cargo test memory_provider passed".to_string()],
            risks: Vec::new(),
            next_steps: vec!["run full memory proposal validation".to_string()],
            assumptions: Vec::new(),
        };

        ledger.append_execution_report(&old_report).unwrap();
        let mut records = ledger.list();
        for record in &mut records {
            record.stale_after =
                Some((chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339());
        }
        write_records_atomically(&ledger.path, &records).unwrap();
        ledger.append_execution_report(&fresh_report).unwrap();

        let active = ledger.active_records();
        assert!(active
            .iter()
            .all(|record| record.task_id != "task-stale-progress"));
        assert!(ledger
            .latest_summary()
            .unwrap()
            .contains("finish current memory lifecycle work"));
        let stale_hits = ledger.search("obsolete parser", 4);
        assert!(stale_hits.is_empty());
        let fresh_hits = ledger.search("memory_provider passed", 4);
        assert!(fresh_hits
            .iter()
            .any(|record| record.task_id == "task-fresh-progress"));
    }

    #[test]
    fn project_progress_active_views_are_project_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = ProjectProgressLedger::new(dir.path().join("project_progress.jsonl"));
        let foreign_report = ExecutionReport {
            task_id: "task-foreign-progress".to_string(),
            objective: "finish unrelated foreign project".to_string(),
            status: ExecutionReportStatus::Partial,
            changed_files: vec!["foreign.rs".to_string()],
            validation_evidence: vec!["foreign test failed".to_string()],
            risks: Vec::new(),
            next_steps: vec!["foreign-only next step".to_string()],
            assumptions: Vec::new(),
        };
        let current_report = ExecutionReport {
            task_id: "task-current-progress".to_string(),
            objective: "finish current project progress isolation".to_string(),
            status: ExecutionReportStatus::Success,
            changed_files: vec!["src/engine/project_progress.rs".to_string()],
            validation_evidence: vec!["cargo test project_progress passed".to_string()],
            risks: Vec::new(),
            next_steps: vec!["run scoped validation".to_string()],
            assumptions: Vec::new(),
        };
        let mut foreign_records = ProjectProgressRecord::from_execution_report(&foreign_report);
        for record in &mut foreign_records {
            record.project_id = Some("project:foreign".to_string());
            record.project_labels = vec!["project_root:/tmp/foreign".to_string()];
        }

        ledger.append_records(foreign_records).unwrap();
        ledger.append_execution_report(&current_report).unwrap();

        let all_records = ledger.list();
        assert!(all_records
            .iter()
            .any(|record| record.task_id == "task-foreign-progress"
                && record.status == ProjectProgressStatus::Active));
        assert!(all_records
            .iter()
            .any(|record| record.task_id == "task-current-progress"
                && record.status == ProjectProgressStatus::Active));
        let active = ledger.active_records();
        assert!(active
            .iter()
            .all(|record| record.task_id != "task-foreign-progress"));
        assert!(active
            .iter()
            .any(|record| record.task_id == "task-current-progress"));
        assert!(ledger.search("foreign-only", 4).is_empty());
        assert!(ledger
            .search("progress isolation", 4)
            .iter()
            .any(|record| record.task_id == "task-current-progress"));
    }

    #[test]
    fn project_progress_heartbeat_records_pull_based_project_state() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = ProjectProgressLedger::new(dir.path().join("project_progress.jsonl"));
        let heartbeat = ProjectProgressRecord::heartbeat(ProjectHeartbeatInput {
            project_name: "demo",
            project_root: dir.path(),
            branch: "main",
            dirty_count: 2,
            dirty_summary: "M src/lib.rs, M docs/status.md",
            goal: "goal: finish memory plan",
            memory: "records=4 review=0 proposed=0 stale=0 rejected=0",
            memory_proposal: "none",
            progress: "next_step: run cargo test",
            next_step:
                "Review the current diff and either finish, validate, or commit the scoped change.",
        });

        ledger.append_heartbeat(heartbeat).unwrap();

        let records = ledger.list();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].task_id, "project-heartbeat");
        assert!(records[0].project_id.is_some());
        assert_eq!(records[0].task_status, "heartbeat");
        assert!(records[0].content.contains("Project heartbeat for demo"));
        assert!(records[0]
            .evidence
            .iter()
            .any(|line| line.contains("git_dirty: 2")));
        assert!(records[0]
            .next_steps
            .iter()
            .any(|step| step.contains("Review the current diff")));
    }
}

//! Internal file/jsonl helpers for `LabStore`.
//!
//! These helpers centralize low-level filesystem and JSON persistence details.
//! Public LabRun code should prefer typed `LabStore` methods from sibling
//! modules rather than calling these helpers directly.

use super::*;

impl LabStore {
    pub(super) fn append_project_event(&self, event: LabEvent) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        append_jsonl(&self.root.join("events.jsonl"), &event)
    }

    pub(super) fn append_run_event(
        &self,
        lab_run_id: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        let event = LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: Some(lab_run_id.to_string()),
            proposal_id: None,
            event_type: event_type.to_string(),
            created_at: Utc::now(),
            payload,
        };
        let run_dir = self.run_dir(lab_run_id);
        fs::create_dir_all(&run_dir)?;
        append_jsonl(&run_dir.join("events.jsonl"), &event)
    }

    pub(super) fn append_run_event_returning(
        &self,
        lab_run_id: &str,
        event_type: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<LabEvent> {
        let event = LabEvent {
            schema_version: LAB_SCHEMA_VERSION,
            event_id: next_id("event"),
            lab_run_id: Some(lab_run_id.to_string()),
            proposal_id: None,
            event_type: event_type.to_string(),
            created_at: Utc::now(),
            payload,
        };
        let run_dir = self.run_dir(lab_run_id);
        fs::create_dir_all(&run_dir)?;
        append_jsonl(&run_dir.join("events.jsonl"), &event)?;
        Ok(event)
    }

    pub(super) fn read_run_events(&self, lab_run_id: &str) -> anyhow::Result<Vec<LabEvent>> {
        let path = self.run_dir(lab_run_id).join("events.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file =
            fs::File::open(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let mut events = Vec::new();
        for line in std::io::BufReader::new(file).lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            events.push(
                serde_json::from_str::<LabEvent>(trimmed)
                    .with_context(|| format!("failed to parse {}", path.display()))?,
            );
        }
        Ok(events)
    }

    pub(super) fn ensure_sqlite_schema(&self, conn: &Connection) -> anyhow::Result<()> {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS lab_runs (
                lab_run_id TEXT PRIMARY KEY,
                schema_version INTEGER NOT NULL,
                project_root TEXT NOT NULL,
                proposal_id TEXT,
                status TEXT NOT NULL,
                current_stage TEXT NOT NULL,
                internal_owner TEXT NOT NULL,
                needs_user INTEGER NOT NULL,
                cycle_count INTEGER NOT NULL,
                failure_count INTEGER NOT NULL,
                artifact_count INTEGER NOT NULL,
                open_task_count INTEGER NOT NULL,
                meeting_count INTEGER NOT NULL,
                blocked_reason TEXT,
                closeout_status TEXT,
                pause_reason TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                state_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS lab_artifacts (
                artifact_id TEXT PRIMARY KEY,
                lab_run_id TEXT NOT NULL,
                artifact_type TEXT NOT NULL,
                stage TEXT NOT NULL,
                status TEXT NOT NULL,
                validation_status TEXT,
                artifact_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lab_artifacts_run
                ON lab_artifacts(lab_run_id, stage, artifact_type);
            CREATE TABLE IF NOT EXISTS lab_events (
                event_id TEXT PRIMARY KEY,
                lab_run_id TEXT,
                proposal_id TEXT,
                event_type TEXT NOT NULL,
                created_at TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lab_events_run
                ON lab_events(lab_run_id, created_at);
            CREATE TABLE IF NOT EXISTS lab_tasks (
                task_id TEXT PRIMARY KEY,
                lab_run_id TEXT NOT NULL,
                status TEXT NOT NULL,
                title TEXT NOT NULL,
                assigned_role TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                result_artifact_id TEXT,
                blocker TEXT,
                task_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lab_tasks_run
                ON lab_tasks(lab_run_id, status);
            ",
        )?;
        Ok(())
    }

    pub(super) fn write_active_run_pointer(&self, lab_run_id: &str) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::write(self.root.join("active_run"), lab_run_id.as_bytes())?;
        Ok(())
    }

    pub(super) fn read_active_run_pointer(&self) -> anyhow::Result<Option<String>> {
        let path = self.root.join("active_run");
        if !path.exists() {
            return Ok(None);
        }
        let value = fs::read_to_string(path)?.trim().to_string();
        Ok((!value.is_empty()).then_some(value))
    }

    pub(super) fn proposals_dir(&self) -> PathBuf {
        self.root.join("proposals")
    }

    pub(super) fn proposal_dir(&self, proposal_id: &str) -> PathBuf {
        self.proposals_dir().join(proposal_id)
    }

    pub(super) fn runs_dir(&self) -> PathBuf {
        self.root.join("runs")
    }

    pub(super) fn runs_index_path(&self) -> PathBuf {
        self.root.join("runs_index.json")
    }

    pub(super) fn refresh_runs_index_entry(&self, run: &LabRun) -> anyhow::Result<()> {
        let mut index = self.load_runs_index()?.unwrap_or_else(|| {
            LabRunIndex::new(self.project_root.display().to_string(), Utc::now())
        });
        index.project_root = self.project_root.display().to_string();
        index.generated_at = Utc::now();
        index
            .entries
            .retain(|entry| entry.lab_run_id != run.lab_run_id);
        index.entries.push(LabRunIndexEntry::from_run(run));
        index.entries.sort_by_key(|entry| entry.updated_at);
        atomic_write_json(&self.runs_index_path(), &index)
    }

    pub(super) fn app_lifecycle_path(&self) -> PathBuf {
        self.root.join("app_lifecycle.json")
    }

    pub(super) fn daemon_state_path(&self) -> PathBuf {
        self.root.join("daemon_state.json")
    }

    pub(super) fn provider_certifications_path(&self) -> PathBuf {
        self.root.join("provider_certifications.jsonl")
    }

    pub(super) fn run_dir(&self, lab_run_id: &str) -> PathBuf {
        self.runs_dir().join(lab_run_id)
    }

    pub(super) fn task_dir(&self, lab_run_id: &str) -> PathBuf {
        self.run_dir(lab_run_id).join("tasks")
    }

    pub(super) fn task_path(&self, lab_run_id: &str, task_id: &str) -> PathBuf {
        self.task_dir(lab_run_id)
            .join(format!("{}.json", safe_path_component(task_id)))
    }

    pub(super) fn dispatch_dir(&self, lab_run_id: &str) -> PathBuf {
        self.run_dir(lab_run_id).join("dispatches")
    }

    pub(super) fn dispatch_path(&self, lab_run_id: &str, dispatch_id: &str) -> PathBuf {
        self.dispatch_dir(lab_run_id)
            .join(format!("{}.json", safe_path_component(dispatch_id)))
    }

    pub(super) fn validation_retry_dir(&self, lab_run_id: &str) -> PathBuf {
        self.run_dir(lab_run_id).join("validation_retries")
    }

    pub(super) fn validation_retry_path(&self, lab_run_id: &str, retry_id: &str) -> PathBuf {
        self.validation_retry_dir(lab_run_id)
            .join(format!("{}.json", safe_path_component(retry_id)))
    }

    pub(super) fn write_graduate_task(&self, task: &GraduateTask) -> anyhow::Result<()> {
        atomic_write_json(&self.task_path(&task.lab_run_id, &task.task_id), task)
    }

    pub(super) fn save_graduate_task_and_sync_run(
        &self,
        mut task: GraduateTask,
        event_type: &str,
    ) -> anyhow::Result<GraduateTask> {
        task.updated_at = Utc::now();
        self.write_graduate_task(&task)?;

        let mut run = self.load_run(&task.lab_run_id)?;
        sync_open_task(&mut run, &task);
        run.updated_at = task.updated_at;
        self.save_run(&run)?;

        self.append_run_event(
            &task.lab_run_id,
            event_type,
            serde_json::json!({
                "task_id": &task.task_id,
                "status": format!("{:?}", task.status),
                "result_artifact_id": &task.result_artifact_id,
                "evidence_ids": &task.evidence_ids,
                "blocker": &task.blocker,
            }),
        )?;
        Ok(task)
    }

    pub(super) fn active_lease_path(&self) -> PathBuf {
        self.root.join("active_lease.json")
    }

    pub(super) fn read_active_lease(&self) -> anyhow::Result<Option<LabLease>> {
        let path = self.active_lease_path();
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    pub(super) fn ensure_no_foreign_fresh_lease(
        &self,
        lab_run_id: Option<&str>,
        now: chrono::DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let Some(lease) = self.read_active_lease()? else {
            return Ok(());
        };
        if lease.is_stale_at(now) {
            return Ok(());
        }
        if Some(lease.lab_run_id.as_str()) == lab_run_id && lease.lease_owner == lease_owner() {
            return Ok(());
        }
        Err(anyhow!(
            "active LabRun lease is held by {} for {}",
            lease.lease_owner,
            lease.lab_run_id
        ))
    }

    pub(super) fn acquire_lease_for_run(
        &self,
        run: &mut LabRun,
        now: chrono::DateTime<Utc>,
    ) -> anyhow::Result<LabLease> {
        self.ensure_no_foreign_fresh_lease(Some(&run.lab_run_id), now)?;
        let lease = LabLease {
            schema_version: LAB_SCHEMA_VERSION,
            lease_id: run.lease_id.clone().unwrap_or_else(|| next_id("lease")),
            lab_run_id: run.lab_run_id.clone(),
            lease_owner: lease_owner(),
            lease_acquired_at: now,
            heartbeat_at: now,
            lease_ttl_seconds: run.lease_ttl_seconds,
        };
        run.lease_id = Some(lease.lease_id.clone());
        run.lease_owner = Some(lease.lease_owner.clone());
        run.heartbeat_at = Some(now);
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.run_dir(&run.lab_run_id))?;
        atomic_write_json(&self.active_lease_path(), &lease)?;
        atomic_write_json(&self.run_dir(&run.lab_run_id).join("lease.json"), &lease)?;
        Ok(lease)
    }

    pub(super) fn release_lease_for_run(&self, lab_run_id: &str) -> anyhow::Result<()> {
        if let Some(lease) = self.read_active_lease()? {
            if lease.lab_run_id == lab_run_id {
                remove_file_if_exists(&self.active_lease_path())?;
            }
        }
        remove_file_if_exists(&self.run_dir(lab_run_id).join("lease.json"))?;
        Ok(())
    }

    pub(super) fn append_sponsor_message_record(
        &self,
        lab_run_id: &str,
        message: &SponsorMessage,
    ) -> anyhow::Result<()> {
        append_jsonl(
            &self.run_dir(lab_run_id).join("sponsor_messages.jsonl"),
            message,
        )
    }

    pub(super) fn write_sponsor_messages(
        &self,
        lab_run_id: &str,
        messages: &[SponsorMessage],
    ) -> anyhow::Result<()> {
        let mut content = String::new();
        for message in messages {
            content.push_str(&serde_json::to_string(message)?);
            content.push('\n');
        }
        atomic_write_text(
            &self.run_dir(lab_run_id).join("sponsor_messages.jsonl"),
            &content,
        )
    }
}

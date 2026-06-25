use crate::lab::orchestrator::LabOrchestrator;

use super::*;

#[test]
fn proposal_approval_creates_run_and_persists_events() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());

    let proposal = store
        .create_proposal("Build the LabRun workflow", Some("session_1".to_string()))
        .unwrap();
    assert!(store
        .root()
        .join("proposals")
        .join(&proposal.proposal_id)
        .join("proposal.json")
        .exists());

    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    assert_eq!(
        run.proposal_id.as_deref(),
        Some(proposal.proposal_id.as_str())
    );
    assert_eq!(run.status, LabRunStatus::Active);
    assert!(run.lease_id.is_some());
    assert!(store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("state.json")
        .exists());
    assert!(store.root().join("active_lease.json").exists());
    assert!(store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("events.jsonl")
        .exists());
    assert_eq!(
        store.latest_run().unwrap().unwrap().lab_run_id,
        run.lab_run_id
    );
}

#[test]
fn labrun_index_updates_from_state_changes_and_can_rebuild() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());

    let proposal = store.create_proposal("Build indexed LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let index = store.load_runs_index().unwrap().unwrap();
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].lab_run_id, run.lab_run_id);
    assert_eq!(index.entries[0].status, LabRunStatus::Active);
    assert_eq!(index.entries[0].current_stage, "professor_discussion");

    let paused = store.pause_latest_run("manual checkpoint").unwrap();
    let updated_index = store.load_runs_index().unwrap().unwrap();
    let entry = updated_index
        .entries
        .iter()
        .find(|entry| entry.lab_run_id == paused.lab_run_id)
        .unwrap();
    assert_eq!(entry.status, LabRunStatus::Paused);
    assert_eq!(entry.pause_reason.as_deref(), Some("manual checkpoint"));

    fs::remove_file(store.runs_index_path()).unwrap();
    assert!(store.load_runs_index().unwrap().is_none());
    let rebuilt = store.rebuild_runs_index().unwrap();
    assert_eq!(rebuilt.entries.len(), 1);
    assert_eq!(rebuilt.entries[0].lab_run_id, paused.lab_run_id);
    assert_eq!(rebuilt.entries[0].status, LabRunStatus::Paused);
    assert!(store.runs_index_path().exists());
}

#[test]
fn sqlite_index_imports_file_backed_labrun_tables() {
    let temp = tempfile::tempdir().unwrap();
    let orchestrator = LabOrchestrator::for_project(temp.path());
    let store = orchestrator.store();

    let proposal = store.create_proposal("Build indexed LabRun", None).unwrap();
    let run = orchestrator
        .approve_proposal(&proposal.proposal_id)
        .unwrap();
    orchestrator
        .create_current_stage_artifact_for_latest("Professor plan")
        .unwrap();
    store
        .create_graduate_task(
            &run.lab_run_id,
            "Implement indexed persistence",
            "Mirror file-backed LabRun state into SQLite.",
            vec!["src/lab/store.rs".to_string()],
            vec!["cargo test -q lab".to_string()],
        )
        .unwrap();

    let summary = store.rebuild_sqlite_index().unwrap();

    assert_eq!(summary.path, store.sqlite_index_path());
    assert_eq!(summary.lab_runs, 1);
    assert_eq!(summary.lab_artifacts, 1);
    assert!(summary.lab_events >= 3);
    assert_eq!(summary.lab_tasks, 1);
    let loaded = store.load_sqlite_index_summary().unwrap().unwrap();
    assert_eq!(loaded, summary);
}

#[test]
fn sponsor_message_is_event_not_direct_task() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let msg = store
        .append_sponsor_message("This is drifting from the product goal")
        .unwrap();

    assert_eq!(msg.lab_run_id, run.lab_run_id);
    assert_eq!(msg.status, SponsorMessageStatus::Queued);
    let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id, msg.message_id);
    assert_eq!(messages[0].message_type, SponsorMessageType::Concern);
    let events = fs::read_to_string(
        store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("sponsor_message"));
    assert!(events.contains("This is drifting"));
}

#[test]
fn intervention_pauses_run_and_queues_urgent_professor_message() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    assert!(store.root().join("active_lease.json").exists());

    let (intervened, message) = store
        .intervene_latest_run("Stop and reassess scope before continuing")
        .unwrap();

    assert_eq!(intervened.lab_run_id, run.lab_run_id);
    assert_eq!(intervened.status, LabRunStatus::NeedsUser);
    assert!(intervened.needs_user);
    assert_eq!(
        intervened.pause_reason.as_deref(),
        Some("sponsor_intervention")
    );
    assert_eq!(message.message_type, SponsorMessageType::PauseRequest);
    assert_eq!(message.urgency, "high");
    assert_eq!(message.status, SponsorMessageStatus::Queued);
    let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id, message.message_id);
    assert!(!store.root().join("active_lease.json").exists());
    let events = fs::read_to_string(
        store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("lab_intervention_recorded"));
    assert!(events.contains("Stop and reassess scope"));
}

#[test]
fn sponsor_message_status_update_rewrites_inbox_and_records_event() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let msg = store
        .append_sponsor_message("Please turn this into a lab meeting")
        .unwrap();

    let updated = store
        .update_latest_sponsor_message_status(
            &msg.message_id,
            SponsorMessageStatus::ConvertedToMeeting,
            "meeting requested",
        )
        .unwrap();

    assert_eq!(updated.status, SponsorMessageStatus::ConvertedToMeeting);
    let messages = store.list_sponsor_messages(&run.lab_run_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].status, SponsorMessageStatus::ConvertedToMeeting);
    let events = fs::read_to_string(
        store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("sponsor_message_status_updated"));
    assert!(events.contains("meeting requested"));
}

#[test]
fn pause_releases_active_lease_and_resume_reacquires_it() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    assert!(store.root().join("active_lease.json").exists());

    let paused = store.pause_latest_run("user").unwrap();
    assert_eq!(paused.lab_run_id, run.lab_run_id);
    assert_eq!(paused.status, LabRunStatus::Paused);
    assert!(!store.root().join("active_lease.json").exists());

    let resumed = store.resume_latest_run().unwrap();
    assert_eq!(resumed.status, LabRunStatus::Active);
    assert!(resumed.lease_id.is_some());
    assert!(store.root().join("active_lease.json").exists());
}

#[test]
fn closeout_marks_run_completed_and_releases_active_lease() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    assert!(store.root().join("active_lease.json").exists());

    let closed = store
        .closeout_latest_run(LabCloseoutStatus::CompletedVerified, "validation passed")
        .unwrap();

    assert_eq!(closed.lab_run_id, run.lab_run_id);
    assert_eq!(closed.status, LabRunStatus::Completed);
    assert_eq!(
        closed.closeout_status,
        Some(LabCloseoutStatus::CompletedVerified)
    );
    assert!(!closed.needs_user);
    assert!(closed.lease_id.is_none());
    assert!(!store.root().join("active_lease.json").exists());
    let events = fs::read_to_string(
        store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("lab_closeout_recorded"));
    assert!(events.contains("validation passed"));
}

#[test]
fn stale_active_lease_recovery_pauses_run_for_resume() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let lease_path = store.root().join("active_lease.json");
    let mut lease: LabLease = read_json(&lease_path).unwrap();
    lease.heartbeat_at = Utc::now() - chrono::Duration::seconds(lease.lease_ttl_seconds as i64 + 5);
    atomic_write_json(&lease_path, &lease).unwrap();

    let recovered = store.recover_stale_active_lease().unwrap();

    assert!(recovered.is_some());
    assert!(!lease_path.exists());
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.status, LabRunStatus::PausedShutdown);
    assert_eq!(saved.pause_reason.as_deref(), Some("stale_heartbeat"));
    assert!(saved.lease_id.is_none());
    assert_eq!(
        store.latest_run().unwrap().unwrap().lab_run_id,
        run.lab_run_id
    );
}

#[test]
fn command_claims_stale_active_lease_without_pausing_run() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let lease_path = store.root().join("active_lease.json");
    let mut lease: LabLease = read_json(&lease_path).unwrap();
    lease.heartbeat_at = Utc::now() - chrono::Duration::seconds(lease.lease_ttl_seconds as i64 + 5);
    atomic_write_json(&lease_path, &lease).unwrap();

    let claimed = store
        .claim_latest_active_run_for_current_process()
        .unwrap()
        .unwrap();

    assert_eq!(claimed.lab_run_id, run.lab_run_id);
    assert_eq!(claimed.status, LabRunStatus::Active);
    assert!(claimed.lease_id.is_some());
    assert!(store.root().join("active_lease.json").exists());
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.status, LabRunStatus::Active);
    assert_eq!(saved.pause_reason, None);
    let events = fs::read_to_string(
        store
            .root()
            .join("runs")
            .join(&run.lab_run_id)
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(events.contains("lab_command_stale_lease_claimed"));
    assert!(events.contains("lab_command_lease_claimed"));

    let released = store
        .release_current_process_lease_without_pausing()
        .unwrap();
    assert!(released.is_some());
    assert!(!store.root().join("active_lease.json").exists());
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.status, LabRunStatus::Active);
    assert!(saved.lease_id.is_none());
    assert!(saved.lease_owner.is_none());
}

#[test]
fn command_claims_active_run_when_lease_file_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    std::fs::remove_file(store.root().join("active_lease.json")).unwrap();

    let claimed = store
        .claim_latest_active_run_for_current_process()
        .unwrap()
        .unwrap();

    assert_eq!(claimed.lab_run_id, run.lab_run_id);
    assert!(store.root().join("active_lease.json").exists());
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.status, LabRunStatus::Active);
    assert!(saved.lease_id.is_some());
    assert!(saved.lease_owner.is_some());

    let released = store
        .release_current_process_lease_without_pausing()
        .unwrap();
    assert!(released.is_some());
    assert!(!store.root().join("active_lease.json").exists());
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.status, LabRunStatus::Active);
    assert!(saved.lease_id.is_none());
    assert!(saved.lease_owner.is_none());
}

#[test]
fn shutdown_pause_releases_lease_and_preserves_resume_target() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let paused = store.pause_latest_run_for_shutdown().unwrap().unwrap();

    assert_eq!(paused.lab_run_id, run.lab_run_id);
    assert_eq!(paused.status, LabRunStatus::PausedShutdown);
    assert_eq!(paused.pause_reason.as_deref(), Some("app_shutdown"));
    assert!(!store.root().join("active_lease.json").exists());
    let resumed = store.resume_latest_run().unwrap();
    assert_eq!(resumed.status, LabRunStatus::Active);
    assert!(resumed.lease_id.is_some());
}

#[test]
fn app_lifecycle_startup_recovers_interrupted_scheduler_state() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let now = Utc::now();
    store
        .write_scheduler_state(&LabSchedulerState {
            schema_version: LAB_SCHEMA_VERSION,
            lab_run_id: run.lab_run_id.clone(),
            status: LabSchedulerStatus::Running,
            updated_at: now,
            started_at: Some(now),
            stopped_at: None,
            max_steps: 20,
            steps_completed: 3,
            interval_ms: 1_000,
            last_action: Some("TickAdvanced".to_string()),
            last_message: Some("background scheduler started".to_string()),
            stop_reason: None,
        })
        .unwrap();

    let lifecycle = store.record_app_lifecycle_startup("lab_cli").unwrap();

    assert_eq!(lifecycle.launch_mode, "lab_cli");
    assert_eq!(
        lifecycle.recovered_scheduler_lab_run_id.as_deref(),
        Some(run.lab_run_id.as_str())
    );
    assert_eq!(
        lifecycle.recovered_scheduler_status,
        Some(LabSchedulerStatus::PausedRestart)
    );
    assert!(store.root().join("app_lifecycle.json").exists());
    let loaded = store.load_app_lifecycle_state().unwrap().unwrap();
    assert_eq!(
        loaded.recovered_scheduler_status,
        lifecycle.recovered_scheduler_status
    );
}

#[test]
fn app_lifecycle_shutdown_pauses_active_run_and_records_state() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let lifecycle = store.record_app_lifecycle_shutdown("lab_cli").unwrap();

    assert_eq!(
        lifecycle.shutdown_paused_lab_run_id.as_deref(),
        Some(run.lab_run_id.as_str())
    );
    assert!(lifecycle.last_shutdown_at.is_some());
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.status, LabRunStatus::PausedShutdown);
    assert_eq!(saved.pause_reason.as_deref(), Some("app_shutdown"));
    let loaded = store.load_app_lifecycle_state().unwrap().unwrap();
    assert_eq!(
        loaded.shutdown_paused_lab_run_id.as_deref(),
        Some(run.lab_run_id.as_str())
    );
}

#[test]
fn daemon_policy_enable_and_disable_are_persisted() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let enabled = store
        .enable_daemon(
            LabDaemonMode::Hybrid,
            9,
            250,
            "continue postdoc repair loop",
        )
        .unwrap();

    assert!(enabled.enabled);
    assert_eq!(enabled.mode, LabDaemonMode::Hybrid);
    assert_eq!(enabled.max_steps, 9);
    assert_eq!(enabled.interval_ms, 250);
    assert_eq!(enabled.instructions, "continue postdoc repair loop");
    assert!(store.root().join("daemon_state.json").exists());
    let loaded = store.load_daemon_state().unwrap().unwrap();
    assert_eq!(loaded.mode, LabDaemonMode::Hybrid);

    let started = store
        .record_daemon_start_result(Some(&run.lab_run_id), None)
        .unwrap()
        .unwrap();
    assert_eq!(
        started.last_started_lab_run_id.as_deref(),
        Some(run.lab_run_id.as_str())
    );
    assert!(started.last_started_at.is_some());
    assert!(started.last_start_error.is_none());

    let disabled = store.disable_daemon("user paused lab daemon").unwrap();

    assert!(!disabled.enabled);
    assert_eq!(disabled.mode, LabDaemonMode::Hybrid);
    assert_eq!(disabled.last_enabled_at, enabled.last_enabled_at);
    assert!(disabled.last_disabled_at.is_some());
    assert!(disabled
        .last_message
        .as_deref()
        .unwrap_or_default()
        .contains("user paused lab daemon"));
}

#[test]
fn daemon_policy_persists_hybrid_cycles_mode() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    store.approve_proposal(&proposal.proposal_id).unwrap();

    let enabled = store
        .enable_daemon_with_cycle_bound(
            LabDaemonMode::HybridCycles,
            4,
            6,
            500,
            "continue bounded lab cycles",
        )
        .unwrap();

    assert!(enabled.enabled);
    assert_eq!(enabled.mode, LabDaemonMode::HybridCycles);
    assert_eq!(enabled.max_steps, 4);
    assert_eq!(enabled.max_steps_per_cycle, 6);
    assert_eq!(enabled.interval_ms, 500);
    assert_eq!(enabled.instructions, "continue bounded lab cycles");
    let loaded = store.load_daemon_state().unwrap().unwrap();
    assert_eq!(loaded.mode, LabDaemonMode::HybridCycles);
    assert_eq!(loaded.max_steps_per_cycle, 6);
}

#[test]
fn interrupted_scheduler_recovery_marks_running_state_resumable() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let now = Utc::now();
    store
        .write_scheduler_state(&LabSchedulerState {
            schema_version: LAB_SCHEMA_VERSION,
            lab_run_id: run.lab_run_id.clone(),
            status: LabSchedulerStatus::Running,
            updated_at: now,
            started_at: Some(now),
            stopped_at: None,
            max_steps: 20,
            steps_completed: 3,
            interval_ms: 1_000,
            last_action: Some("TickAdvanced".to_string()),
            last_message: Some("background scheduler started".to_string()),
            stop_reason: None,
        })
        .unwrap();

    let recovered = store.recover_interrupted_scheduler().unwrap().unwrap();

    assert_eq!(recovered.status, LabSchedulerStatus::PausedRestart);
    assert_eq!(recovered.stop_reason.as_deref(), Some("process_restart"));
    assert_eq!(recovered.steps_completed, 3);
    assert!(recovered.stopped_at.is_some());
    let saved = store
        .load_scheduler_state(&run.lab_run_id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.status, LabSchedulerStatus::PausedRestart);
}

#[test]
fn lab_failure_accounting_escalates_after_retry_budget() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let first = store
        .record_lab_failure(&run.lab_run_id, "graduate_execution", "first failure")
        .unwrap();
    assert_eq!(first.failure_count, 1);
    assert_eq!(first.status, LabRunStatus::Active);
    assert!(!first.needs_user);

    let second = store
        .record_lab_failure(&run.lab_run_id, "graduate_execution", "second failure")
        .unwrap();
    assert_eq!(second.failure_count, 2);
    assert_eq!(second.status, LabRunStatus::NeedsUser);
    assert!(second.needs_user);
    assert_eq!(
        second.closeout_status,
        Some(LabCloseoutStatus::BlockedNeedsUser)
    );
    assert!(second
        .blocked_reason
        .as_deref()
        .unwrap_or_default()
        .contains("failure budget reached"));
}

#[test]
fn graduate_task_revision_requeues_blocked_task_when_scope_is_complete() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Postdoc plan lacks scope.",
            Vec::new(),
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    let blocked = store
        .block_graduate_task(&run.lab_run_id, &task.task_id, "missing allowed_scope")
        .unwrap();
    assert_eq!(blocked.status, LabTaskStatus::Blocked);

    let revised = store
        .revise_graduate_task(
            &run.lab_run_id,
            &task.task_id,
            vec!["src/lab/store.rs".to_string()],
            vec!["cargo check -q --tests".to_string()],
            Some("Use the narrowed LabStore scope."),
        )
        .unwrap();

    assert_eq!(revised.status, LabTaskStatus::Queued);
    assert_eq!(revised.blocker, None);
    assert_eq!(revised.allowed_scope, vec!["src/lab/store.rs".to_string()]);
    assert_eq!(
        revised.required_validation,
        vec!["cargo check -q --tests".to_string()]
    );
    assert!(revised
        .instructions
        .contains("Postdoc revision:\nUse the narrowed LabStore scope."));
}

#[test]
fn graduate_task_revision_stays_blocked_when_scope_is_incomplete() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Postdoc plan lacks validation.",
            vec!["src/lab/store.rs".to_string()],
            Vec::new(),
        )
        .unwrap();

    let revised = store
        .revise_graduate_task(
            &run.lab_run_id,
            &task.task_id,
            vec!["src/lab/store.rs".to_string()],
            Vec::new(),
            None,
        )
        .unwrap();

    assert_eq!(revised.status, LabTaskStatus::Blocked);
    assert!(revised
        .blocker
        .as_deref()
        .unwrap_or("")
        .contains("missing required_validation"));
}

#[test]
fn validation_retry_creates_repair_task_then_escalates_after_budget() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Implement scoped slice",
            "Update only the lab model.",
            vec!["src/lab/model.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    let first = store
        .record_validation_retry_and_repair_task(
            &run.lab_run_id,
            &task.task_id,
            "cargo check failed",
        )
        .unwrap();

    assert_eq!(first.attempt, 1);
    assert!(!first.escalated);
    let repair_id = first.repair_task_id.as_deref().unwrap();
    let repair = store
        .load_graduate_task(&run.lab_run_id, repair_id)
        .unwrap();
    assert!(repair.title.starts_with("Repair validation for"));
    let retries = store.list_validation_retries(&run.lab_run_id).unwrap();
    assert_eq!(retries.len(), 1);

    let second = store
        .record_validation_retry_and_repair_task(
            &run.lab_run_id,
            &task.task_id,
            "cargo test failed",
        )
        .unwrap();
    assert_eq!(second.attempt, 2);
    assert!(second.repair_task_id.is_some());

    let third = store
        .record_validation_retry_and_repair_task(
            &run.lab_run_id,
            &task.task_id,
            "validation still failing",
        )
        .unwrap();
    assert_eq!(third.attempt, 3);
    assert!(third.escalated);
    assert!(third.repair_task_id.is_none());
    let saved_run = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved_run.failure_count, 1);
}

#[test]
fn cost_usage_records_cache_shape_and_summarizes_by_role() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    store
        .record_cost_usage(
            &run.lab_run_id,
            LabRole::Professor,
            "test-model",
            LabCostTokens {
                prompt_tokens: 1_000,
                completion_tokens: 200,
                reasoning_tokens: 50,
                cached_tokens: 700,
                cache_write_tokens: 120,
                cycle_id: Some("0".to_string()),
                meeting_id: None,
            },
            0.0123,
            Some("professor draft"),
        )
        .unwrap();
    store
        .record_cost_usage(
            &run.lab_run_id,
            LabRole::Postdoc,
            "test-model",
            LabCostTokens {
                prompt_tokens: 500,
                completion_tokens: 100,
                reasoning_tokens: 0,
                cached_tokens: 100,
                cache_write_tokens: 20,
                cycle_id: Some("0".to_string()),
                meeting_id: None,
            },
            0.004,
            None,
        )
        .unwrap();

    let summary = store.cost_summary(&run.lab_run_id).unwrap();

    assert_eq!(summary.requests, 2);
    assert_eq!(summary.prompt_tokens, 1_500);
    assert_eq!(summary.completion_tokens, 300);
    assert_eq!(summary.reasoning_tokens, 50);
    assert_eq!(summary.cached_tokens, 800);
    assert_eq!(summary.cache_write_tokens, 140);
    assert_eq!(summary.cache_miss_tokens, 700);
    assert_eq!(summary.total_tokens, 1_850);
    assert_eq!(summary.by_role.len(), 2);
    assert!(store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("cost_usage.jsonl")
        .exists());
}

#[test]
fn evidence_refs_are_refs_only_and_listable() {
    let temp = tempfile::tempdir().unwrap();
    let evidence_file = temp.path().join("proof.txt");
    fs::write(&evidence_file, "validation proof").unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let evidence_path = evidence_file.display().to_string();
    let evidence = store
        .record_evidence_ref(LabEvidenceRefInput {
            lab_run_id: &run.lab_run_id,
            kind: LabEvidenceKind::File,
            role: LabRole::Postdoc,
            reference: &evidence_path,
            summary: "cargo check passed",
            artifact_id: None,
            cycle_id: Some("0"),
        })
        .unwrap();

    assert_eq!(evidence.kind, LabEvidenceKind::File);
    assert_eq!(evidence.summary, "cargo check passed");
    assert!(evidence.metadata_hash.is_some());
    let listed = store.list_evidence_refs(&run.lab_run_id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].evidence_id, evidence.evidence_id);
    assert!(store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("evidence_refs.jsonl")
        .exists());
}

#[test]
fn provider_certifications_are_project_level_and_latest_wins() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());

    let failed = store
        .record_provider_certification(
            "deepseek",
            "deepseek-v4-flash",
            LabProviderCertificationKind::Graduate,
            LabProviderCertificationOutcome::Failed,
            "target/lab-live-validation/failed/report.md",
            "graduate runtime verification failed",
        )
        .unwrap();
    let passed = store
        .record_provider_certification(
            "deepseek",
            "deepseek-v4-flash",
            LabProviderCertificationKind::Graduate,
            LabProviderCertificationOutcome::Passed,
            "target/lab-live-validation/passed/report.md",
            "graduate runtime verification passed",
        )
        .unwrap();

    let listed = store.list_provider_certifications().unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].record_id, failed.record_id);
    let latest = store
        .latest_provider_certification(
            "deepseek",
            "deepseek-v4-flash",
            LabProviderCertificationKind::Graduate,
        )
        .unwrap()
        .unwrap();
    assert_eq!(latest.record_id, passed.record_id);
    assert_eq!(latest.outcome, LabProviderCertificationOutcome::Passed);
    assert!(store.root().join("provider_certifications.jsonl").exists());
}

#[test]
fn graduate_tasks_sync_open_task_resume_cursor() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Implement one narrow slice",
            "Only touch the listed files and report validation evidence.",
            vec![
                "src/lab/model.rs".to_string(),
                "src/lab/store.rs".to_string(),
            ],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();

    assert_eq!(task.status, LabTaskStatus::Queued);
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert_eq!(saved.open_task_ids, vec![task.task_id.clone()]);
    assert_eq!(
        saved.resume_cursor.open_task_ids,
        vec![task.task_id.clone()]
    );
    assert!(store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("tasks")
        .join(format!("{}.json", task.task_id))
        .exists());

    let started = store
        .start_graduate_task(&run.lab_run_id, &task.task_id)
        .unwrap();
    assert_eq!(started.status, LabTaskStatus::InProgress);
    let completed = store
        .complete_graduate_task(
            &run.lab_run_id,
            &task.task_id,
            "artifact_graduate_result_001",
            vec!["labevidence_001".to_string()],
        )
        .unwrap();

    assert_eq!(completed.status, LabTaskStatus::Completed);
    assert_eq!(
        completed.result_artifact_id.as_deref(),
        Some("artifact_graduate_result_001")
    );
    let saved = store.load_run(&run.lab_run_id).unwrap();
    assert!(saved.open_task_ids.is_empty());
    assert!(saved.resume_cursor.open_task_ids.is_empty());
}

#[test]
fn graduate_dispatch_records_are_persisted_and_listable() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Implement one narrow slice",
            "Only touch the listed files and report validation evidence.",
            vec!["src/lab/delegation.rs".to_string()],
            vec!["cargo check -q".to_string()],
        )
        .unwrap();
    let dispatch = crate::lab::delegation::build_graduate_task_dispatch(&task).unwrap();

    let record = store
        .record_graduate_dispatch(&run.lab_run_id, &task.task_id, dispatch)
        .unwrap();

    assert_eq!(record.status, GraduateDispatchStatus::Prepared);
    assert_eq!(record.cleanup_status, GraduateCleanupStatus::CleanupPending);
    assert!(record
        .cleanup_message
        .as_deref()
        .unwrap_or_default()
        .contains("cleanup pending"));
    assert!(record.cleanup_updated_at.is_some());
    assert_eq!(record.task_id, task.task_id);
    assert_eq!(
        record.agent_tool_params["profile"].as_str(),
        Some("lab-graduate")
    );
    let listed = store.list_graduate_dispatches(&run.lab_run_id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].dispatch_id, record.dispatch_id);
    assert_eq!(
        listed[0].cleanup_status,
        GraduateCleanupStatus::CleanupPending
    );
    assert!(store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("dispatches")
        .join(format!("{}.json", record.dispatch_id))
        .exists());
}

#[test]
fn compression_decisions_are_persisted() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();

    let decision = store
        .record_compression_decision(LabCompressionDecision {
            schema_version: LAB_SCHEMA_VERSION,
            decision_id: String::new(),
            lab_run_id: run.lab_run_id.clone(),
            created_at: Utc::now(),
            role: LabRole::Professor,
            action: crate::lab::model::LabCompressionAction::Recommend,
            reason: "near budget".to_string(),
            context_budget_tokens: 100,
            packet_tokens: 70,
            usage_ratio_percent: 70.0,
            stable_prefix_fingerprint: "stable".to_string(),
            dynamic_tail_fingerprint: "dynamic".to_string(),
            cycle_id: Some("0".to_string()),
        })
        .unwrap();

    assert!(!decision.decision_id.is_empty());
    let listed = store.list_compression_decisions(&run.lab_run_id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].decision_id, decision.decision_id);
    assert!(store
        .root()
        .join("runs")
        .join(&run.lab_run_id)
        .join("compression_decisions.jsonl")
        .exists());
}

#[test]
fn artifact_gate_validation_blocks_missing_handoff_fields() {
    let temp = tempfile::tempdir().unwrap();
    let store = LabStore::for_project(temp.path());
    let proposal = store.create_proposal("Build LabRun", None).unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    let mut gate = ArtifactGate::new(
        "postdoc_review",
        "PostdocIntegrationSummary",
        crate::lab::model::LabRole::Postdoc,
    );

    let path = store.write_artifact_gate(&run.lab_run_id, &gate).unwrap();
    assert!(path.exists());
    let err = store
        .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
        .unwrap_err()
        .to_string();
    assert!(err.contains("artifact_id"));
    assert!(err.contains("next_action"));

    gate.artifact_id = Some("artifact_postdoc_summary_001".to_string());
    gate.next_action = Some("professor_review".to_string());
    gate.validation_status = Some("not_verified".to_string());
    store.write_artifact_gate(&run.lab_run_id, &gate).unwrap();
    let err = store
        .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
        .unwrap_err()
        .to_string();
    assert!(err.contains("missing or malformed artifact"));

    let wrong_artifact = StageArtifact::ProfessorPlan(crate::lab::model::LabArtifactEnvelope::new(
        "artifact_postdoc_summary_001".to_string(),
        run.lab_run_id.clone(),
        crate::lab::model::LabArtifactType::ProfessorPlan,
        "Wrong artifact type".to_string(),
        Utc::now(),
        crate::lab::model::ProfessorPlan {
            problem_statement: "Build LabRun".to_string(),
            strategic_direction: "Wrong stage for this gate.".to_string(),
            success_criteria: Vec::new(),
            constraints: Vec::new(),
            risks: Vec::new(),
            handoff_to_postdoc: "Not a postdoc integration summary.".to_string(),
        },
    ));
    store.write_stage_artifact(&wrong_artifact).unwrap();
    let err = store
        .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
        .unwrap_err()
        .to_string();
    assert!(err.contains("has stage 'professor_discussion'"));

    let mut artifact =
        StageArtifact::PostdocIntegrationSummary(crate::lab::model::LabArtifactEnvelope::new(
            "artifact_postdoc_summary_001".to_string(),
            run.lab_run_id.clone(),
            crate::lab::model::LabArtifactType::PostdocIntegrationSummary,
            "Postdoc integration summary".to_string(),
            Utc::now(),
            crate::lab::model::PostdocIntegrationSummary {
                integration_summary: "Integrated graduate result.".to_string(),
                accepted_results: vec!["artifact_graduate_result_001".to_string()],
                validation_status: "validated".to_string(),
                remaining_risks: Vec::new(),
                handoff_to_professor: "Ready for professor review.".to_string(),
            },
        ));
    if let StageArtifact::PostdocIntegrationSummary(envelope) = &mut artifact {
        envelope.evidence_refs = vec!["artifact:artifact_graduate_result_001".to_string()];
    }
    store.write_stage_artifact(&artifact).unwrap();
    store.write_stage_artifact_report(&artifact).unwrap();
    store
        .validate_artifact_gate(&run.lab_run_id, "postdoc_review")
        .unwrap();
}

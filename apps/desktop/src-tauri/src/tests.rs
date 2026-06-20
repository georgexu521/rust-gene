use super::*;
use std::path::Path;
use std::process::Command;

static DESKTOP_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct DesktopEnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}

impl DesktopEnvGuard {
    fn new() -> Self {
        Self { saved: Vec::new() }
    }

    fn set(&mut self, key: &'static str, value: &str) {
        if !self.saved.iter().any(|(saved_key, _)| saved_key == &key) {
            self.saved.push((key, std::env::var(key).ok()));
        }
        unsafe { std::env::set_var(key, value) };
    }
}

impl Drop for DesktopEnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain(..) {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

#[test]
fn desktop_smoke_health_reports_ready_and_cwd() {
    let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
    let health = desktop_health_value(cwd.clone());

    assert_eq!(health.status, "ready");
    assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(health.cwd, cwd.display().to_string());
}

#[test]
fn desktop_smoke_project_path_validation_accepts_directory() {
    let cwd = std::env::current_dir().unwrap();
    let project = validate_project_path(&cwd).unwrap();
    let selected = selected_project_response(project.clone());

    assert!(project.is_dir());
    assert_eq!(selected.path, project.display().to_string());
}

#[test]
fn desktop_smoke_project_path_validation_rejects_missing_path() {
    let missing = std::env::temp_dir().join(format!(
        "priority-agent-desktop-missing-{}",
        std::process::id()
    ));
    let err = validate_project_path(&missing).unwrap_err();

    assert!(err.contains("project path does not exist"));
    assert!(err.contains(&missing.display().to_string()));
}

#[test]
fn desktop_smoke_project_path_validation_rejects_filesystem_root() {
    let err = validate_project_path(Path::new("/")).unwrap_err();

    assert!(err.contains("filesystem root"));
}

#[test]
fn desktop_smoke_recent_sessions_include_message_counts() {
    let store = SessionStore::in_memory().unwrap();
    store
        .create_session("desktop-session", "Desktop Session", "mock-model", None)
        .unwrap();
    store
        .add_message("desktop-session", "user", "hello", None, None)
        .unwrap();
    store
        .add_message("desktop-session", "assistant", "hi", None, None)
        .unwrap();

    let sessions = list_recent_sessions_from_store(&store, 20, &[]).unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "desktop-session");
    assert_eq!(sessions[0].title, "Desktop Session");
    assert_eq!(sessions[0].model, "mock-model");
    assert_eq!(sessions[0].message_count, 2);
    assert!(!sessions[0].updated_at.is_empty());
}

#[test]
fn desktop_smoke_recent_sessions_skip_archived_ids() {
    let store = SessionStore::in_memory().unwrap();
    store
        .create_session("visible-session", "Visible Session", "mock-model", None)
        .unwrap();
    store
        .create_session("archived-session", "Archived Session", "mock-model", None)
        .unwrap();

    let sessions =
        list_recent_sessions_from_store(&store, 20, &["archived-session".to_string()]).unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "visible-session");
}

#[test]
fn desktop_smoke_search_sessions_uses_message_fts() {
    let store = SessionStore::in_memory().unwrap();
    store
        .create_session("desktop-session", "Desktop Session", "mock-model", None)
        .unwrap();
    store
        .add_message(
            "desktop-session",
            "user",
            "Find onboarding diagnostics",
            None,
            None,
        )
        .unwrap();

    let sessions = search_sessions_from_store(&store, "onboarding", 20, &[]).unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "desktop-session");
}

#[test]
fn desktop_smoke_load_messages_preserves_order_and_roles() {
    let store = SessionStore::in_memory().unwrap();
    store
        .create_session("desktop-session", "Desktop Session", "mock-model", None)
        .unwrap();
    store
        .add_message("desktop-session", "user", "first", None, None)
        .unwrap();
    store
        .add_message("desktop-session", "assistant", "second", None, None)
        .unwrap();

    let messages = load_messages_from_store(&store, "desktop-session").unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "first");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "second");
    assert!(!messages[0].created_at.is_empty());
}

#[test]
fn desktop_smoke_loads_persisted_long_session_parts_for_reload() {
    let store = SessionStore::in_memory().unwrap();
    let session_id = "desktop-long-reload";
    store
        .create_session(session_id, "Desktop Long Reload", "mock-model", None)
        .unwrap();
    let writer = priority_agent::session_store::SessionEventWriter::new(
        store.shared_conn(),
        session_id,
    );

    for index in 0..5 {
        writer
            .text_delta(&format!("assistant chunk {index}\n"))
            .unwrap();
        writer
            .tool_called(&format!("call-{index}"), "file_read")
            .unwrap();
        writer
            .tool_result_completed(
                &format!("call-{index}"),
                &format!("read src/file_{index}.rs"),
            )
            .unwrap();
    }
    writer
        .write_event(
            "closeout",
            &serde_json::json!({
                "status": "verified",
                "evidence_summary": "reload smoke"
            })
            .to_string(),
        )
        .unwrap();
    writer
        .write_event(
            "revert",
            &serde_json::json!({
                "status": "completed",
                "target_part_id": "tool_call-2",
                "reverted_after": "tool_call-2",
                "part_ids": ["tool_call-2"],
                "paths": ["src/file_2.rs"],
                "restored_files": ["src/file_2.rs"],
                "removed_files": [],
                "errors": [],
                "unrevert_possible": true
            })
            .to_string(),
        )
        .unwrap();

    let parts = load_session_parts_from_store(&store, session_id).unwrap();

    assert!(parts.len() >= 7);
    assert!(parts.iter().any(|part| part.kind == "assistant_text"));
    assert!(parts
        .iter()
        .any(|part| part.kind == "tool" && part.tool_call_id.as_deref() == Some("call-3")));
    assert!(parts.iter().any(|part| part.kind == "closeout"));
    let revert = parts.iter().find(|part| part.kind == "revert").unwrap();
    assert_eq!(revert.status.as_deref(), Some("completed"));
    assert_eq!(revert.payload["reverted_after"], "tool_call-2");
}

#[test]
fn desktop_run_context_enriches_message_with_git_diff() {
    let project = std::env::temp_dir().join(format!(
        "priority-agent-desktop-diff-context-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();
    run_test_command(&project, &["git", "init"]);
    run_test_command(
        &project,
        &["git", "config", "user.email", "desktop@example.com"],
    );
    run_test_command(&project, &["git", "config", "user.name", "Desktop Test"]);
    std::fs::write(project.join("README.md"), "old\n").unwrap();
    run_test_command(&project, &["git", "add", "README.md"]);
    run_test_command(&project, &["git", "commit", "-m", "initial"]);
    std::fs::write(project.join("README.md"), "new\n").unwrap();

    let message = enrich_message_with_desktop_contexts(
        "Review this".to_string(),
        &[DesktopRunContext {
            context_type: "current_diff".to_string(),
            label: Some("Current diff".to_string()),
            path: None,
            line_start: None,
            line_end: None,
            selection_text: None,
        }],
        &project,
    )
    .unwrap();

    assert!(message.contains("Review this"));
    assert!(message.contains("<desktop_context type=\"current_diff\" label=\"Current diff\">"));
    assert!(message.contains("README.md"));
    assert!(message.contains("-old"));
    assert!(message.contains("+new"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn desktop_run_context_rejects_unknown_context_type() {
    let err = enrich_message_with_desktop_contexts(
        "hello".to_string(),
        &[DesktopRunContext {
            context_type: "unknown".to_string(),
            label: None,
            path: None,
            line_start: None,
            line_end: None,
            selection_text: None,
        }],
        &std::env::current_dir().unwrap(),
    )
    .unwrap_err();

    assert!(err.contains("Unsupported desktop run context"));
}

#[test]
fn desktop_run_context_enriches_message_with_file_preview() {
    let project = std::env::temp_dir().join(format!(
        "priority-agent-desktop-file-context-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).unwrap();
    let file = project.join("src/app.rs");
    std::fs::write(&file, "fn main() {\n    println!(\"desktop\");\n}\n").unwrap();

    let message = enrich_message_with_desktop_contexts(
        "Review this file".to_string(),
        &[DesktopRunContext {
            context_type: "file".to_string(),
            label: Some("app.rs".to_string()),
            path: Some(file.to_string_lossy().to_string()),
            line_start: None,
            line_end: None,
            selection_text: None,
        }],
        &project,
    )
    .unwrap();

    assert!(message.contains("Review this file"));
    assert!(message.contains("<desktop_context type=\"file\" label=\"app.rs\">"));
    assert!(message.contains("Path: src/app.rs"));
    assert!(message.contains("println!(\"desktop\")"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn desktop_run_context_rejects_file_outside_project() {
    let project = std::env::temp_dir().join(format!(
        "priority-agent-desktop-file-context-root-{}",
        std::process::id()
    ));
    let outside = std::env::temp_dir().join(format!(
        "priority-agent-desktop-file-context-outside-{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_file(&outside);
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(&outside, "outside\n").unwrap();

    let err = enrich_message_with_desktop_contexts(
        "Review this file".to_string(),
        &[DesktopRunContext {
            context_type: "file".to_string(),
            label: Some("outside.txt".to_string()),
            path: Some(outside.to_string_lossy().to_string()),
            line_start: None,
            line_end: None,
            selection_text: None,
        }],
        &project,
    )
    .unwrap_err();

    assert!(err.contains("inside the selected project"));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_file(&outside);
}

#[test]
fn desktop_smoke_settings_round_trip() {
    let path = std::env::temp_dir().join(format!(
        "priority-agent-desktop-settings-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let settings = DesktopSettings {
        selected_project: Some("/tmp/project".to_string()),
        active_session_id: Some("session-1".to_string()),
        permission_mode: Some("auto_low_risk".to_string()),
        detail_level: Some("daily".to_string()),
        agent_mode: Some("standard".to_string()),
        provider_name: Some("kimi".to_string()),
        model: Some("kimi-k2.5".to_string()),
        recent_projects: Some(vec!["/tmp/project".to_string()]),
        archived_session_ids: Some(vec!["old-session".to_string()]),
    };

    write_desktop_settings(&path, &settings).unwrap();
    let loaded = load_desktop_settings(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.selected_project.as_deref(), Some("/tmp/project"));
    assert_eq!(loaded.active_session_id.as_deref(), Some("session-1"));
    assert_eq!(loaded.permission_mode.as_deref(), Some("auto_low_risk"));
    assert_eq!(loaded.detail_level.as_deref(), Some("daily"));
    assert_eq!(loaded.provider_name.as_deref(), Some("kimi"));
    assert_eq!(loaded.model.as_deref(), Some("kimi-k2.5"));
    assert_eq!(
        loaded.recent_projects.as_deref(),
        Some(["/tmp/project".to_string()].as_slice())
    );
    assert_eq!(
        loaded.archived_session_ids.as_deref(),
        Some(["old-session".to_string()].as_slice())
    );
}

#[test]
fn desktop_smoke_lab_status_reads_file_backed_labrun_state() {
    let project = std::env::temp_dir().join(format!(
        "priority-agent-desktop-lab-status-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();

    let empty = desktop_lab_status_for_project(&project);
    assert_eq!(empty.state, "none");
    assert!(!empty.available);

    let store = priority_agent::lab::store::LabStore::for_project(&project);
    let proposal = store.create_proposal("Build Lab status panel", None).unwrap();
    let proposed = desktop_lab_status_for_project(&project);
    assert_eq!(proposed.state, "proposal");
    assert_eq!(proposed.proposal_id.as_deref(), Some(proposal.proposal_id.as_str()));
    assert_eq!(proposed.proposal_status.as_deref(), Some("AwaitingApproval"));
    assert!(proposed.needs_user);

    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    store
        .enable_daemon_with_cycle_bound(
            priority_agent::lab::model::LabDaemonMode::HybridCycles,
            4,
            6,
            500,
            "desktop policy",
        )
        .unwrap();
    let active = desktop_lab_status_for_project(&project);
    assert_eq!(active.state, "run");
    assert_eq!(active.lab_run_id.as_deref(), Some(run.lab_run_id.as_str()));
    assert_eq!(active.run_status.as_deref(), Some("Active"));
    assert_eq!(active.stage.as_deref(), Some("professor_discussion"));
    assert_eq!(active.owner.as_deref(), Some("Professor"));
    assert_eq!(active.task_total, 0);
    assert_eq!(active.meeting_count, 0);
    assert!(active.blockers.is_empty());
    let daemon_policy = active.daemon_policy.as_ref().unwrap();
    assert!(daemon_policy.enabled);
    assert_eq!(
        daemon_policy.mode,
        priority_agent::lab::model::LabDaemonMode::HybridCycles
    );
    assert_eq!(daemon_policy.max_steps, 4);
    assert_eq!(daemon_policy.max_steps_per_cycle, 6);
    assert_eq!(daemon_policy.interval_ms, 500);

    let task = store
        .create_graduate_task(
            &run.lab_run_id,
            "Wire status actions",
            "Expose Lab status actions in the desktop panel.",
            vec!["apps/desktop/src/app/components/WorkbenchPanel.tsx".to_string()],
            vec!["pnpm --dir apps/desktop build".to_string()],
        )
        .unwrap();
    store
        .record_validation_retry_and_repair_task(
            &run.lab_run_id,
            &task.task_id,
            "Playwright panel action check failed",
        )
        .unwrap();
    let blocked = desktop_lab_status_for_project(&project);
    assert_eq!(blocked.task_blocked, 1);
    assert_eq!(blocked.validation_retry_count, 1);
    assert_eq!(blocked.validation_retry_escalated_count, 0);
    assert!(blocked
        .latest_validation_retry
        .as_deref()
        .unwrap_or_default()
        .contains("Playwright panel action check failed"));
    assert!(blocked
        .blockers
        .iter()
        .any(|blocker| blocker.contains("Wire status actions")));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn desktop_smoke_subagent_tasks_reads_active_session_artifacts_and_recovery() {
    let store = SessionStore::in_memory().unwrap();
    store
        .create_session("desktop-session", "Desktop Session", "mock-model", None)
        .unwrap();
    let artifact_id = store
        .add_agent_artifact(
            "desktop-session",
            "agent_1",
            Some("implementer"),
            "Specialist",
            "completed",
            "edit focused code",
            "finished child result\nwith more detail",
            &serde_json::json!({
            "task_id": "task_1",
                "completion_sink": "agent_manager",
                "tools_used": ["file_write", "file_read"]
            }),
        )
        .unwrap();
    store
        .upsert_agent_task_state(&priority_agent::session_store::AgentTaskStateUpsert {
            session_id: "desktop-session".to_string(),
            task_id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            profile: Some("implementer".to_string()),
            role: "Specialist".to_string(),
            status: "paused_restart".to_string(),
            description: "edit focused code".to_string(),
            transcript_path: Some("/tmp/a2a.jsonl".to_string()),
            tool_ids_in_progress: Vec::new(),
            permission_requests: Vec::new(),
            result_artifact_id: Some(artifact_id),
            cleanup_hooks: Vec::new(),
            payload: serde_json::json!({
                "child_session_id": "desktop-session:subagent:task_1",
                "recovery_status": "paused_restart",
                "recovery_action": "read the task by task_id, then relaunch"
            }),
        })
        .unwrap();

    let tasks = desktop_subagent_tasks_for_session(&store, Some("desktop-session"));

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_id, "task_1");
    assert_eq!(tasks[0].result_artifact_id, Some(artifact_id));
    assert_eq!(tasks[0].artifact_status.as_deref(), Some("completed"));
    assert_eq!(tasks[0].result_preview.as_deref(), Some("finished child result"));
    assert_eq!(
        tasks[0].tools_used,
        vec!["file_write".to_string(), "file_read".to_string()]
    );
    assert_eq!(tasks[0].completion_sink.as_deref(), Some("agent_manager"));
    assert_eq!(tasks[0].recovery_status.as_deref(), Some("paused_restart"));
    assert!(tasks[0]
        .recovery_action
        .as_deref()
        .unwrap_or_default()
        .contains("task_id"));
}

#[test]
fn desktop_smoke_startup_state_prefers_recoverable_labrun() {
    let project = std::env::temp_dir().join(format!(
        "priority-agent-desktop-lab-recovery-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();

    let store = priority_agent::lab::store::LabStore::for_project(&project);
    let proposal = store
        .create_proposal("Recover interrupted LabRun", None)
        .unwrap();
    let run = store.approve_proposal(&proposal.proposal_id).unwrap();
    store.pause_latest_run_for_shutdown().unwrap().unwrap();

    let startup = desktop_startup_state(&project, Some("restored-session"));
    assert_eq!(startup.status, "lab_recovery");
    assert_eq!(startup.lab_run_id.as_deref(), Some(run.lab_run_id.as_str()));
    assert_eq!(startup.lab_stage.as_deref(), Some("professor_discussion"));
    assert_eq!(startup.lab_owner.as_deref(), Some("Professor"));
    assert_eq!(startup.lab_pause_reason.as_deref(), Some("app_shutdown"));
    assert!(startup.detail.contains("recoverable"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
#[cfg(unix)]
fn desktop_smoke_lab_daemon_supervise_repairs_missing_service() {
    use std::os::unix::fs::PermissionsExt;

    let _env_lock = DESKTOP_ENV_LOCK.lock().unwrap();
    let mut env = DesktopEnvGuard::new();
    let project_root = std::env::temp_dir().join(format!(
        "priority-agent-desktop-lab-daemon-supervise-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project_root);
    let project = project_root.join("daemon-project");
    std::fs::create_dir_all(&project).unwrap();
    let launch_agents = project_root.join("launch-agents");
    let bin_dir = project_root.join("bin");
    std::fs::create_dir_all(&launch_agents).unwrap();
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake_launchctl = bin_dir.join("launchctl");
    let launchctl_log = bin_dir.join("launchctl.log");
    std::fs::write(
        &fake_launchctl,
        r#"#!/bin/sh
printf '%s' "$1" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
shift
for arg in "$@"; do
  printf '|%s' "$arg" >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
done
printf '\n' >> "$PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG"
if [ "$1" = "gui/desktop/com.priority-agent.lab.daemon-project" ]; then
  printf 'missing service\n' >&2
  exit 113
fi
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&fake_launchctl).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_launchctl, permissions).unwrap();
    env.set(
        "PRIORITY_AGENT_LAUNCH_AGENTS_DIR",
        launch_agents.to_str().unwrap(),
    );
    env.set(
        "PRIORITY_AGENT_LAUNCHCTL_BIN",
        fake_launchctl.to_str().unwrap(),
    );
    env.set("PRIORITY_AGENT_LAUNCHCTL_DOMAIN", "gui/desktop");
    env.set(
        "PRIORITY_AGENT_FAKE_LAUNCHCTL_LOG",
        launchctl_log.to_str().unwrap(),
    );

    let store = priority_agent::lab::store::LabStore::for_project(&project);
    store
        .enable_daemon(
            priority_agent::lab::model::LabDaemonMode::Strict,
            3,
            250,
            "",
        )
        .unwrap();

    let result = desktop_lab_daemon_supervise_for_project(&project);

    assert_eq!(result.action, "supervise");
    assert!(result
        .output
        .contains("supervision repaired missing service"));
    assert_eq!(result.lab_status.state, "none");
    let installed = launch_agents.join("com.priority-agent.lab.daemon-project.plist");
    assert!(installed.exists());
    let log = std::fs::read_to_string(launchctl_log).unwrap();
    assert!(log.contains("print|gui/desktop/com.priority-agent.lab.daemon-project"));
    assert!(log.contains(&format!("bootstrap|gui/desktop|{}", installed.display())));

    let _ = std::fs::remove_dir_all(&project_root);
}

#[test]
fn desktop_smoke_initial_project_falls_back_when_saved_path_is_missing() {
    let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
    let expected = default_desktop_project(cwd.clone());
    let settings = DesktopSettings {
        selected_project: Some(
            std::env::temp_dir()
                .join(format!("priority-agent-missing-{}", std::process::id()))
                .display()
                .to_string(),
        ),
        active_session_id: None,
        permission_mode: None,
        detail_level: None,
        agent_mode: None,
        provider_name: None,
        model: None,
        recent_projects: None,
        archived_session_ids: None,
    };

    assert_eq!(initial_desktop_project(cwd, &settings), expected);
}

#[test]
fn desktop_smoke_initial_project_falls_back_when_saved_path_is_filesystem_root() {
    let root = std::env::temp_dir().join(format!(
        "priority-agent-root-saved-root-{}",
        std::process::id()
    ));
    let tauri_dir = root.join("apps/desktop/src-tauri");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(&tauri_dir).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();

    let settings = DesktopSettings {
        selected_project: Some("/".to_string()),
        active_session_id: None,
        permission_mode: None,
        detail_level: None,
        agent_mode: None,
        provider_name: None,
        model: None,
        recent_projects: None,
        archived_session_ids: None,
    };
    let expected = root.canonicalize().unwrap();
    let selected = initial_desktop_project(tauri_dir.canonicalize().unwrap(), &settings);
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(selected, expected);
}

#[test]
fn desktop_smoke_default_project_discovers_repo_root_from_tauri_dir() {
    let root = std::env::temp_dir().join(format!("priority-agent-root-{}", std::process::id()));
    let tauri_dir = root.join("apps/desktop/src-tauri");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(&tauri_dir).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();

    let expected = root.canonicalize().unwrap();
    let discovered = default_desktop_project(tauri_dir.canonicalize().unwrap());
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(discovered, expected);
}

#[test]
fn desktop_smoke_initial_project_migrates_old_tauri_subdir_default() {
    let root = std::env::temp_dir().join(format!("priority-agent-migrate-{}", std::process::id()));
    let tauri_dir = root.join("apps/desktop/src-tauri");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(&tauri_dir).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();

    let settings = DesktopSettings {
        selected_project: Some(tauri_dir.display().to_string()),
        active_session_id: None,
        permission_mode: None,
        detail_level: None,
        agent_mode: None,
        provider_name: None,
        model: None,
        recent_projects: None,
        archived_session_ids: None,
    };
    let expected = root.canonicalize().unwrap();
    let selected = initial_desktop_project(tauri_dir.canonicalize().unwrap(), &settings);
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(selected, expected);
}

#[test]
fn desktop_smoke_diagnostics_include_project_and_settings_access() {
    let project = std::env::current_dir().unwrap().canonicalize().unwrap();
    let settings_path = std::env::temp_dir()
        .join(format!("priority-agent-settings-{}", std::process::id()))
        .join("desktop-settings.json");
    let diagnostic_logs_path = std::env::temp_dir()
        .join(format!("priority-agent-diagnostics-{}", std::process::id()))
        .join("desktop.log");

    let diagnostics = collect_desktop_diagnostics(&project, &settings_path, &diagnostic_logs_path);

    assert!(diagnostics.iter().any(|item| item.id == "provider_keys"));
    assert!(diagnostics
        .iter()
        .any(|item| item.id == "project_access" && matches!(item.status, DiagnosticStatus::Ok)));
    assert!(diagnostics
        .iter()
        .any(|item| item.id == "settings_access" && matches!(item.status, DiagnosticStatus::Ok)));
    assert!(diagnostics
        .iter()
        .any(|item| item.id == "diagnostic_logs" && matches!(item.status, DiagnosticStatus::Ok)));
}

#[test]
fn desktop_smoke_appends_desktop_log_entries() {
    let log_path = std::env::temp_dir()
        .join(format!("priority-agent-desktop-log-{}", std::process::id()))
        .join("desktop.log");
    let _ = std::fs::remove_file(&log_path);

    append_desktop_log(&log_path, "run_error message=first line").unwrap();
    append_desktop_log(&log_path, "run_completed").unwrap();

    let log = std::fs::read_to_string(&log_path).unwrap();
    assert!(log.contains("run_error message=first line"));
    assert!(log.contains("run_completed"));

    let _ = std::fs::remove_dir_all(log_path.parent().unwrap());
}

#[test]
fn desktop_smoke_sanitizes_log_values() {
    assert_eq!(
        sanitize_log_value("failed\nwith\tcontrol chars"),
        "failed with control chars"
    );
}

#[test]
fn desktop_smoke_project_diagnostic_reports_missing_path() {
    let missing = std::env::temp_dir().join(format!(
        "priority-agent-diagnostic-missing-{}",
        std::process::id()
    ));

    let diagnostic = project_access_diagnostic(&missing);

    assert!(matches!(diagnostic.status, DiagnosticStatus::Error));
    assert!(diagnostic.detail.contains("does not exist"));
}

#[test]
fn desktop_smoke_provider_setup_info_uses_shell_profile() {
    let info = provider_setup_info_value();

    assert!(
        info.shell_profile_path.ends_with(".zshrc")
            || info.shell_profile_path.ends_with(".bash_profile")
    );
    assert!(info
        .provider_env_vars
        .contains(&"MINIMAX_API_KEY".to_string()));
    assert!(info
        .provider_env_vars
        .contains(&"KIMI_CODE_API_KEY".to_string()));
    assert!(info
        .provider_env_vars
        .contains(&"DEEPSEEK_API_KEY".to_string()));
    assert!(info.provider_env_vars.contains(&"GLM_API_KEY".to_string()));
    assert!(info
        .provider_env_vars
        .contains(&"MOONSHOT_API_KEY".to_string()));
    assert!(info.example.contains("export "));
}

#[test]
fn desktop_smoke_permission_mode_normalization() {
    assert_eq!(normalized_permission_mode_label(Some("ask")), "default");
    assert_eq!(
        normalized_permission_mode_label(Some("auto_low_risk")),
        "auto_low_risk"
    );
    assert_eq!(normalized_permission_mode_label(Some("auto_all")), "auto");
    assert_eq!(
        normalized_permission_mode_label(Some("readonly")),
        "read_only"
    );
    assert_eq!(normalized_permission_mode_label(Some("once")), "auto");
    assert_eq!(
        parse_desktop_permission_mode("read_only"),
        PermissionMode::ReadOnly
    );
}

#[test]
fn desktop_smoke_detail_level_normalization() {
    assert_eq!(normalized_detail_level_label(None), "coding");
    assert_eq!(normalized_detail_level_label(Some("daily_work")), "daily");
    assert_eq!(normalized_detail_level_label(Some("default")), "daily");
    assert_eq!(normalized_detail_level_label(Some("unknown")), "coding");
}

#[test]
fn desktop_smoke_provider_options_include_missing_defaults() {
    let registry = priority_agent::services::api::provider::ProviderRegistry::new();
    let providers = desktop_provider_options(&registry, Some("kimi"));

    assert!(providers
        .iter()
        .any(|provider| provider.id == "minimax" && !provider.configured));
    assert!(providers
        .iter()
        .any(|provider| provider.id == "deepseek" && provider.note.contains("DEEPSEEK_API_KEY")));
    assert!(providers
        .iter()
        .any(|provider| provider.id == "glm" && provider.note.contains("GLM_API_KEY")));
    assert!(providers
        .iter()
        .any(|provider| provider.id == "kimi-code" && provider.model == "kimi-for-coding"));
    assert!(providers
        .iter()
        .any(|provider| provider.id == "openai" && provider.note.contains("OPENAI_API_KEY")));
    assert!(providers
        .iter()
        .any(|provider| provider.id == "kimi" && provider.model == "kimi-k2.5"));
}

#[test]
fn desktop_smoke_model_options_include_current_model() {
    let registry = priority_agent::services::api::provider::ProviderRegistry::new();
    let models = desktop_model_options(&registry, "openai", "custom-model");

    assert!(models
        .iter()
        .any(|model| model.id == "custom-model" && model.active));
    assert!(models.iter().any(|model| model.id == "gpt-4o"));
    assert!(models.iter().any(|model| model.id == "gpt-4o-mini"));
}

fn run_test_command(project: &Path, command: &[&str]) {
    let status = Command::new(command[0])
        .current_dir(project)
        .args(&command[1..])
        .status()
        .unwrap();
    assert!(status.success(), "command failed: {:?}", command);
}

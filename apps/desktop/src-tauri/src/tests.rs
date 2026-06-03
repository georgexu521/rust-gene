use super::*;

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
        .create_session("desktop-session", "Desktop Session", "mock-model")
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
        .create_session("visible-session", "Visible Session", "mock-model")
        .unwrap();
    store
        .create_session("archived-session", "Archived Session", "mock-model")
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
        .create_session("desktop-session", "Desktop Session", "mock-model")
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
        .create_session("desktop-session", "Desktop Session", "mock-model")
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
    assert!(info.provider_env_vars.contains(&"MINIMAX_API_KEY"));
    assert!(info.provider_env_vars.contains(&"KIMI_CODE_API_KEY"));
    assert!(info.provider_env_vars.contains(&"DEEPSEEK_API_KEY"));
    assert!(info.provider_env_vars.contains(&"GLM_API_KEY"));
    assert!(info.provider_env_vars.contains(&"MOONSHOT_API_KEY"));
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

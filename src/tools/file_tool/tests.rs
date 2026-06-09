use super::*;

async fn read_test_file_for_edit(path: &str, session_id: &str) {
    let read_result = FileReadTool
        .execute(json!({ "path": path }), ToolContext::new(".", session_id))
        .await;
    assert!(read_result.success, "read failed: {:?}", read_result.error);
}

#[tokio::test]
async fn test_file_read() {
    let tool = FileReadTool;
    // 使用 Cargo.toml 作为测试文件
    let params = json!({
        "path": "Cargo.toml"
    });
    let context = ToolContext::new(".", "test-session");

    let result = tool.execute(params, context).await;

    assert!(result.success);
    assert!(result.content.contains("[package]"));
}

#[tokio::test]
async fn file_read_directory_returns_entries_without_shell_metadata() {
    let tool = FileReadTool;
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join(".DS_Store"), "metadata")
        .await
        .unwrap();
    tokio::fs::write(dir.path().join("note.txt"), "hello")
        .await
        .unwrap();
    tokio::fs::create_dir(dir.path().join("nested"))
        .await
        .unwrap();

    let result = tool
        .execute(
            json!({ "path": dir.path().to_string_lossy().to_string() }),
            ToolContext::new(".", "test-session-read-dir"),
        )
        .await;

    assert!(result.success, "read failed: {:?}", result.error);
    assert!(result.content.contains(".DS_Store"));
    assert!(result.content.contains("note.txt"));
    assert!(result.content.contains("nested/"));
    assert!(!result.content.contains("created"));
    assert!(!result.content.contains("size"));
    let data = result.data.expect("directory read should return metadata");
    assert_eq!(data["kind"], "directory");
    assert_eq!(data["entry_count"], 3);
}

#[tokio::test]
async fn file_read_empty_directory_is_explicit() {
    let tool = FileReadTool;
    let dir = tempfile::tempdir().unwrap();

    let result = tool
        .execute(
            json!({ "path": dir.path().to_string_lossy().to_string() }),
            ToolContext::new(".", "test-session-read-empty-dir"),
        )
        .await;

    assert!(result.success, "read failed: {:?}", result.error);
    assert!(result.content.contains("Entries (0):"));
    assert!(result.content.contains("(empty)"));
    let data = result.data.expect("directory read should return metadata");
    assert_eq!(data["kind"], "directory");
    assert_eq!(data["entry_count"], 0);
}

#[tokio::test]
async fn test_file_write_and_read() {
    let write_tool = FileWriteTool;
    let read_tool = FileReadTool;
    let _ = tokio::fs::remove_file("/tmp/test_priority_agent_file.txt").await;

    let test_content = "Hello, World!";
    let params = json!({
        "path": "/tmp/test_priority_agent_file.txt",
        "content": test_content
    });
    let context = ToolContext::new(".", "test-session");

    // 写入
    let write_result = write_tool.execute(params, context.clone()).await;
    assert!(write_result.success);

    // 读取
    let read_params = json!({
        "path": "/tmp/test_priority_agent_file.txt"
    });
    let read_result = read_tool.execute(read_params, context).await;
    assert!(read_result.success);
    assert!(read_result.content.contains("Hello, World!"));

    // 清理
    let _ = tokio::fs::remove_file("/tmp/test_priority_agent_file.txt").await;
}

#[tokio::test]
async fn test_file_write_existing_file_reports_full_replacement_guidance() {
    let write_tool = FileWriteTool;
    let path = "/tmp/test_priority_agent_file_write_existing.txt";
    let session_id = "test-session-file-write-existing";
    let checkpoint_manager = crate::engine::checkpoint::get_checkpoint_manager(session_id).await;
    checkpoint_manager.lock().await.clear_all().await.unwrap();
    tokio::fs::write(path, "old\n").await.unwrap();
    mark_file_read(session_id, path);
    mark_file_read(
        session_id,
        &canonicalize_or_normalize(Path::new(path)).to_string_lossy(),
    );

    let result = write_tool
        .execute(
            json!({
                "path": path,
                "content": "new\n"
            }),
            ToolContext::new(".", session_id),
        )
        .await;

    assert!(result.success, "write failed: {:?}", result.error);
    assert!(result.content.contains("overwritten"));
    let data = result.data.expect("file_write should return metadata");
    assert_eq!(data["existed_before"], true);
    assert!(data["guidance"]
        .as_str()
        .unwrap_or("")
        .contains("file_edit"));
    assert!(data["checkpoint"]["id"].as_str().is_some());
    assert!(data["file_change"]["id"]
        .as_str()
        .unwrap_or("")
        .starts_with("fc_"));
    assert!(data["file_change"]["before_hash"].as_str().is_some());
    assert!(data["file_change"]["after_hash"].as_str().is_some());
    assert!(data["diff"]["unified_diff"]
        .as_str()
        .unwrap_or("")
        .contains("-old"));
    assert!(data["edit_preview"]["before_hash"].as_str().is_some());
    assert!(data["edit_preview"]["after_hash"].as_str().is_some());
    assert_eq!(
        data["edit_preview"]["checkpoint_id"],
        data["checkpoint"]["id"]
    );
    assert_eq!(
        data["edit_preview"]["file_change_id"],
        data["file_change"]["id"]
    );
    assert_eq!(data["edit_preview"]["rollback"]["kind"], "checkpoint");
    assert!(data["edit_preview"]["diff_preview"]
        .as_str()
        .unwrap_or("")
        .contains("-old"));

    let _ = tokio::fs::remove_file(path).await;
    checkpoint_manager.lock().await.clear_all().await.unwrap();
}

#[tokio::test]
async fn file_write_rejects_generated_target_with_recovery_data() {
    let tool = FileWriteTool;
    let dir = tempfile::tempdir().unwrap();
    let result = tool
        .execute(
            json!({
                "path": "target/generated.txt",
                "content": "generated\n"
            }),
            ToolContext::new(dir.path(), "test-session-write-generated-target"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("generated"));
    let data = result
        .data
        .expect("generated target rejection should return recovery data");
    assert_eq!(data["failure"], "generated_or_dependency_target");
    assert_eq!(
        data["recovery"]["recommended_action"],
        "edit_source_file_instead"
    );
    assert!(!dir.path().join("target/generated.txt").exists());
}

#[tokio::test]
async fn file_write_allows_live_eval_worktree_under_target() {
    let tool = FileWriteTool;
    let dir = tempfile::tempdir().unwrap();
    let session_id = format!(
        "test-session-write-live-eval-worktree-{}",
        uuid::Uuid::new_v4().simple()
    );
    let worktree = dir
        .path()
        .join("target/live-evals/run-123/minimum-agent-loop/worktree");
    tokio::fs::create_dir_all(worktree.join("fixtures"))
        .await
        .unwrap();

    let result = tool
        .execute(
            json!({
                "path": "fixtures/generated.py",
                "content": "print('ok')\n"
            }),
            ToolContext::new(&worktree, &session_id),
        )
        .await;

    assert!(
        result.success,
        "unexpected file_write failure: {:?}",
        result
    );
    assert_eq!(
        tokio::fs::read_to_string(worktree.join("fixtures/generated.py"))
            .await
            .unwrap(),
        "print('ok')\n"
    );
}

#[tokio::test]
async fn file_patch_applies_multiple_files_and_records_history() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "alpha\nold-a\n")
        .await
        .unwrap();
    tokio::fs::write(dir.path().join("b.txt"), "beta\nold-b\n")
        .await
        .unwrap();
    let session_id = format!("test-session-file-patch-{}", uuid::Uuid::new_v4().simple());
    let checkpoint_manager = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    checkpoint_manager.lock().await.clear_all().await.unwrap();

    let context = ToolContext::new(dir.path(), &session_id);
    let read_tool = FileReadTool;
    assert!(
        read_tool
            .execute(json!({ "path": "a.txt" }), context.clone())
            .await
            .success
    );
    assert!(
        read_tool
            .execute(json!({ "path": "b.txt" }), context.clone())
            .await
            .success
    );

    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "a.txt",
                        "old_string": "old-a",
                        "new_string": "new-a"
                    },
                    {
                        "path": "b.txt",
                        "old_string": "old-b",
                        "new_string": "new-b"
                    }
                ]
            }),
            context,
        )
        .await;

    assert!(result.success, "file_patch failed: {:?}", result.error);
    assert_eq!(
        tokio::fs::read_to_string(dir.path().join("a.txt"))
            .await
            .unwrap(),
        "alpha\nnew-a\n"
    );
    assert_eq!(
        tokio::fs::read_to_string(dir.path().join("b.txt"))
            .await
            .unwrap(),
        "beta\nnew-b\n"
    );
    let data = result.data.expect("file_patch metadata");
    assert_eq!(data["operation_count"], 2);
    assert!(data["checkpoint"]["id"].as_str().is_some());
    assert_eq!(data["file_changes"].as_array().unwrap().len(), 2);
    assert_eq!(
        data["files"][0]["edit_preview"]["checkpoint_id"],
        data["checkpoint"]["id"]
    );
    assert!(data["files"][0]["edit_preview"]["file_change_id"]
        .as_str()
        .unwrap_or("")
        .starts_with("fc_"));
    assert_eq!(
        data["files"][0]["edit_preview"]["validation_stage"],
        "patch_complete"
    );
    assert!(data["diff"]["unified_diff"]
        .as_str()
        .unwrap_or("")
        .contains("new-a"));

    checkpoint_manager.lock().await.clear_all().await.unwrap();
}

#[tokio::test]
async fn file_patch_records_encoded_bytes_written_for_utf16le() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("utf16.txt");
    let original = encode_text_content(
        "hello\nworld\n",
        TextFileEncoding::Utf16Le,
        true,
        LineEndingStyle::Lf,
    );
    tokio::fs::write(&path, original).await.unwrap();
    let session_id = format!(
        "test-session-file-patch-utf16-bytes-{}",
        uuid::Uuid::new_v4().simple()
    );
    let checkpoint_manager = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    checkpoint_manager.lock().await.clear_all().await.unwrap();
    let context = ToolContext::new(dir.path(), &session_id);
    let read_tool = FileReadTool;
    assert!(
        read_tool
            .execute(json!({ "path": "utf16.txt" }), context.clone())
            .await
            .success
    );

    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "utf16.txt",
                        "old_string": "world",
                        "new_string": "世界"
                    }
                ]
            }),
            context,
        )
        .await;

    assert!(result.success, "file_patch failed: {:?}", result.error);
    let data = result.data.expect("file_patch metadata");
    assert_eq!(data["files"][0]["text_format"]["encoding"], "utf-16le");
    assert_eq!(data["files"][0]["text_format"]["bom"], true);
    assert_eq!(data["files"][0]["bytes_written"], 20);
    assert_eq!(data["file_changes"][0]["bytes_written"], 20);
    let bytes = tokio::fs::read(&path).await.unwrap();
    assert_eq!(bytes.len(), 20);
    let decoded = decode_text_file(&path, "test", bytes).unwrap();
    assert_eq!(decoded.content, "hello\n世界\n");

    checkpoint_manager.lock().await.clear_all().await.unwrap();
}

#[tokio::test]
async fn file_patch_preflight_failure_does_not_partially_apply() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "alpha\nold-a\n")
        .await
        .unwrap();
    tokio::fs::write(dir.path().join("b.txt"), "beta\nold-b\n")
        .await
        .unwrap();
    let session_id = format!(
        "test-session-file-patch-atomic-{}",
        uuid::Uuid::new_v4().simple()
    );
    let context = ToolContext::new(dir.path(), &session_id);
    let read_tool = FileReadTool;
    let _ = read_tool
        .execute(json!({ "path": "a.txt" }), context.clone())
        .await;
    let _ = read_tool
        .execute(json!({ "path": "b.txt" }), context.clone())
        .await;

    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "a.txt",
                        "old_string": "old-a",
                        "new_string": "new-a"
                    },
                    {
                        "path": "b.txt",
                        "old_string": "missing-b",
                        "new_string": "new-b"
                    }
                ]
            }),
            context,
        )
        .await;

    assert!(!result.success);
    assert_eq!(
        tokio::fs::read_to_string(dir.path().join("a.txt"))
            .await
            .unwrap(),
        "alpha\nold-a\n"
    );
    assert_eq!(
        tokio::fs::read_to_string(dir.path().join("b.txt"))
            .await
            .unwrap(),
        "beta\nold-b\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn file_patch_write_failure_reports_rollback_metadata() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let locked_dir = dir.path().join("locked");
    tokio::fs::create_dir_all(&locked_dir).await.unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "alpha\nold-a\n")
        .await
        .unwrap();
    tokio::fs::write(locked_dir.join("b.txt"), "beta\nold-b\n")
        .await
        .unwrap();
    let session_id = format!(
        "test-session-file-patch-write-failure-{}",
        uuid::Uuid::new_v4().simple()
    );
    let checkpoint_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
        crate::engine::checkpoint::CheckpointManager::new(&session_id).await,
    ));
    checkpoint_manager.lock().await.clear_all().await.unwrap();
    let context = ToolContext::new(dir.path(), &session_id)
        .with_checkpoint_manager(checkpoint_manager.clone());
    let read_tool = FileReadTool;
    assert!(
        read_tool
            .execute(json!({ "path": "a.txt" }), context.clone())
            .await
            .success
    );
    assert!(
        read_tool
            .execute(json!({ "path": "locked/b.txt" }), context.clone())
            .await
            .success
    );

    std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "a.txt",
                        "old_string": "old-a",
                        "new_string": "new-a"
                    },
                    {
                        "path": "locked/b.txt",
                        "old_string": "old-b",
                        "new_string": "new-b"
                    }
                ]
            }),
            context,
        )
        .await;
    std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(!result.success);
    let data = result.data.expect("partial failure metadata");
    assert_eq!(data["partial_failure"], true);
    assert_eq!(data["failed_path"], "locked/b.txt");
    assert!(data["checkpoint"]["id"].as_str().is_some());
    assert_eq!(data["rollback_attempted"], true);
    assert_eq!(data["rollback_success"], true);
    assert_eq!(data["written_paths"].as_array().unwrap().len(), 1);
    assert!(!data["rollback"]["restored_files"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        tokio::fs::read_to_string(dir.path().join("a.txt"))
            .await
            .unwrap(),
        "alpha\nold-a\n"
    );
    assert_eq!(
        tokio::fs::read_to_string(locked_dir.join("b.txt"))
            .await
            .unwrap(),
        "beta\nold-b\n"
    );
    checkpoint_manager.lock().await.clear_all().await.unwrap();
}

#[tokio::test]
async fn file_patch_rejects_unread_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "alpha\nold-a\n")
        .await
        .unwrap();
    let session_id = format!(
        "test-session-file-patch-unread-{}",
        uuid::Uuid::new_v4().simple()
    );
    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "a.txt",
                        "old_string": "old-a",
                        "new_string": "new-a"
                    }
                ]
            }),
            ToolContext::new(dir.path(), session_id),
        )
        .await;

    assert!(!result.success);
    let error = result.error.unwrap_or_default();
    assert!(error.contains("has not been read"), "{error}");
}

#[tokio::test]
async fn file_patch_write_rejects_unread_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("a.txt"), "alpha\nold-a\n")
        .await
        .unwrap();
    let session_id = format!(
        "test-session-file-patch-write-unread-{}",
        uuid::Uuid::new_v4().simple()
    );
    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "a.txt",
                        "mode": "write",
                        "content": "replacement\n"
                    }
                ]
            }),
            ToolContext::new(dir.path(), session_id),
        )
        .await;

    assert!(!result.success);
    let error = result.error.unwrap_or_default();
    assert!(error.contains("has not been read"), "{error}");
    assert_eq!(
        tokio::fs::read_to_string(dir.path().join("a.txt"))
            .await
            .unwrap(),
        "alpha\nold-a\n"
    );
}

#[tokio::test]
async fn file_patch_write_rejects_stale_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.txt");
    tokio::fs::write(&path, "alpha\nold-a\n").await.unwrap();
    let session_id = format!(
        "test-session-file-patch-write-stale-{}",
        uuid::Uuid::new_v4().simple()
    );
    let context = ToolContext::new(dir.path(), &session_id);
    let read_tool = FileReadTool;
    assert!(
        read_tool
            .execute(json!({ "path": "a.txt" }), context.clone())
            .await
            .success
    );
    tokio::fs::write(&path, "external\nchange\n").await.unwrap();

    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "a.txt",
                        "mode": "write",
                        "content": "replacement\n"
                    }
                ]
            }),
            context,
        )
        .await;

    assert!(!result.success);
    let error = result.error.unwrap_or_default();
    assert!(
        error.contains("changed since this session last read"),
        "{error}"
    );
    assert_eq!(
        tokio::fs::read_to_string(path).await.unwrap(),
        "external\nchange\n"
    );
}

#[tokio::test]
async fn file_patch_rejects_notebook_target_with_recovery_data() {
    let dir = tempfile::tempdir().unwrap();
    let patch_tool = FilePatchTool;
    let result = patch_tool
        .execute(
            json!({
                "operations": [
                    {
                        "path": "analysis.ipynb",
                        "mode": "write",
                        "content": "{}"
                    }
                ]
            }),
            ToolContext::new(dir.path(), "test-session-patch-notebook-target"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("notebook"));
    let data = result
        .data
        .expect("notebook target rejection should return recovery data");
    assert_eq!(data["failure"], "wrong_tool_notebook");
    assert_eq!(data["recovery"]["recommended_action"], "use_notebook_tool");
}

#[test]
fn test_resolve_path() {
    let working_dir = std::path::Path::new("/home/user/project");

    let denied = resolve_path("/etc/config", working_dir);
    assert!(denied.is_err());

    let relative = resolve_path("src/main.rs", working_dir).unwrap();
    assert_eq!(
        relative,
        std::path::Path::new("/home/user/project/src/main.rs")
    );

    let escaped = resolve_path("../secret.txt", working_dir);
    assert!(escaped.is_err());

    let allowed_tmp = resolve_path("/tmp/test_priority_agent_file.txt", working_dir).unwrap();
    assert_eq!(
        allowed_tmp,
        std::path::Path::new("/tmp/test_priority_agent_file.txt")
    );
}

#[test]
fn resolve_read_path_allows_home_desktop_without_allowing_writes() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    let home = tempfile::tempdir().unwrap();
    let desktop = home.path().join("Desktop");
    std::fs::create_dir_all(desktop.join("gex")).unwrap();
    env.set("HOME", home.path().to_str().unwrap());

    let working = tempfile::tempdir().unwrap();
    let read_path = resolve_read_path("~/Desktop/gex", working.path()).unwrap();
    assert_eq!(read_path, normalize_path(&desktop.join("gex")));

    let write_path = resolve_path("~/Desktop/gex", working.path());
    assert!(write_path.is_err());
}

#[test]
fn resolve_read_path_allows_runtime_tool_result_artifacts_read_only() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
    let home = tempfile::tempdir().unwrap();
    env.set("HOME", home.path().to_str().unwrap());
    env.remove("XDG_DATA_HOME");
    env.remove("PRIORITY_AGENT_READ_ROOTS");

    let tool_results = dirs::data_local_dir()
        .unwrap()
        .join("priority-agent")
        .join("tool-results");
    let artifact_path = tool_results.join("file_read_call_large.txt");
    std::fs::create_dir_all(&tool_results).unwrap();
    std::fs::write(&artifact_path, "full truncated output").unwrap();

    let unrelated_app_data = tool_results
        .parent()
        .unwrap()
        .join("sessions")
        .join("session.db");
    std::fs::create_dir_all(unrelated_app_data.parent().unwrap()).unwrap();
    std::fs::write(&unrelated_app_data, "not a tool result").unwrap();

    let working = tempfile::tempdir().unwrap();
    let read_path = resolve_read_path(artifact_path.to_str().unwrap(), working.path());
    assert!(read_path.is_ok());

    let write_path = resolve_path(artifact_path.to_str().unwrap(), working.path());
    assert!(write_path.is_err());

    let unrelated_read = resolve_read_path(unrelated_app_data.to_str().unwrap(), working.path());
    assert!(unrelated_read.is_err());
}

#[test]
fn file_path_identity_records_requested_resolved_and_canonical_paths() {
    let working = tempfile::tempdir().unwrap();
    let file_path = working.path().join("src").join("main.rs");
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, "fn main() {}\n").unwrap();
    let resolved = resolve_path("./src/main.rs", working.path()).unwrap();

    let identity = file_path_identity("./src/main.rs", &resolved, working.path());

    assert_eq!(identity.lexical_path, "./src/main.rs");
    assert_eq!(
        identity.resolved_path,
        file_path.to_string_lossy().to_string()
    );
    assert_eq!(
        identity.canonical_path,
        canonicalize_or_normalize(&file_path)
            .to_string_lossy()
            .to_string()
    );
    assert_eq!(identity.display_path, "src/main.rs");
    assert_eq!(identity.state_key, identity.canonical_path);
}

#[test]
fn test_file_write_requires_confirmation_for_relative_path() {
    let tool = FileWriteTool;
    let params = json!({
        "path": "relative.txt",
        "content": "hello"
    });
    assert!(tool.requires_confirmation(&params));
}

#[cfg(unix)]
#[test]
fn test_resolve_path_blocks_symlink_escape() {
    use std::os::unix::fs::symlink;

    let base = tempfile::tempdir().unwrap();
    let working = base.path().join("workspace");
    let outside = base.path().join("outside");
    std::fs::create_dir_all(&working).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "secret").unwrap();
    symlink(&outside, working.join("link")).unwrap();

    let escaped = resolve_path("link/secret.txt", &working);
    assert!(escaped.is_err());
}

#[tokio::test]
async fn test_file_read_offset_out_of_bounds() {
    let read_tool = FileReadTool;
    let path = "/tmp/test_priority_agent_offset.txt";
    tokio::fs::write(path, "line1\nline2\n").await.unwrap();

    let params = json!({
        "path": path,
        "offset": 100
    });
    let context = ToolContext::new(".", "test-session");
    let result = read_tool.execute(params, context).await;
    assert!(!result.success);
    assert!(result.error.unwrap_or_default().contains("Offset"));

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn file_read_targeted_range_is_not_hidden_by_unchanged_cache() {
    let read_tool = FileReadTool;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.rs");
    tokio::fs::write(
        &path,
        "fn first() {}\n\nfn summary_task() {\n    todo!();\n}\n\nfn run_one() {}\n",
    )
    .await
    .unwrap();

    let cache = std::sync::Arc::new(crate::tools::file_cache::FileStateCache::new());
    let context = ToolContext::new(".", "test-session-targeted-cache").with_file_cache(cache);

    let full_read = read_tool
        .execute(
            json!({ "path": path.to_string_lossy().to_string() }),
            context.clone(),
        )
        .await;
    assert!(full_read.success, "full read failed: {:?}", full_read.error);

    let targeted_read = read_tool
        .execute(
            json!({
                "path": path.to_string_lossy().to_string(),
                "offset": 3,
                "limit": 3
            }),
            context.clone(),
        )
        .await;
    assert!(
        targeted_read.success,
        "targeted read failed: {:?}",
        targeted_read.error
    );
    assert!(targeted_read.content.contains("summary_task"));
    assert!(targeted_read.content.contains("todo!();"));
    assert!(!targeted_read
        .content
        .contains("File unchanged since last read"));

    let broad_repeat = read_tool
        .execute(
            json!({ "path": path.to_string_lossy().to_string() }),
            context,
        )
        .await;
    assert!(broad_repeat.success);
    assert!(broad_repeat
        .content
        .contains("File unchanged since last read"));
}

#[tokio::test]
async fn file_read_unchanged_cache_is_scoped_to_session() {
    let read_tool = FileReadTool;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("README.md");
    tokio::fs::write(&path, "# Project\n\nfull content")
        .await
        .unwrap();

    let cache = std::sync::Arc::new(crate::tools::file_cache::FileStateCache::new());
    let session_a = ToolContext::new(".", "session-a").with_file_cache(cache.clone());
    let session_b = ToolContext::new(".", "session-b").with_file_cache(cache);
    let params = json!({ "path": path.to_string_lossy().to_string() });

    let first = read_tool.execute(params.clone(), session_a.clone()).await;
    assert!(first.success, "first read failed: {:?}", first.error);
    assert!(first.content.contains("full content"));

    let same_session = read_tool.execute(params.clone(), session_a).await;
    assert!(same_session.success);
    assert!(same_session
        .content
        .contains("File unchanged since last read"));

    let other_session = read_tool.execute(params, session_b).await;
    assert!(other_session.success);
    assert!(other_session.content.contains("full content"));
    assert!(!other_session
        .content
        .contains("File unchanged since last read"));
}

#[tokio::test]
async fn file_read_persists_context_ledger_fact() {
    let read_tool = FileReadTool;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("README.md");
    tokio::fs::write(&path, "# Project\n\nfull content")
        .await
        .unwrap();

    let store = std::sync::Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
    store
        .create_session("session-ledger", "Ledger", "model")
        .unwrap();
    let context = ToolContext::new(".", "session-ledger").with_session_store(store.clone());

    let result = read_tool
        .execute(
            json!({ "path": path.to_string_lossy().to_string() }),
            context,
        )
        .await;
    assert!(result.success, "read failed: {:?}", result.error);

    let event = store
        .latest_file_read_context_event("session-ledger", &path.to_string_lossy())
        .unwrap()
        .expect("file read ledger event");
    assert_eq!(
        event.kind,
        crate::engine::context_ledger::CONTEXT_LEDGER_FILE_READ_KIND
    );
    assert_eq!(event.payload["total_lines"], 3);
    assert_eq!(event.payload["targeted_read"], false);
}

#[tokio::test]
async fn file_read_records_raw_display_boundary_metadata() {
    let read_tool = FileReadTool;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.rs");
    tokio::fs::write(&path, "alpha\nbeta\ngamma\n")
        .await
        .unwrap();

    let result = read_tool
        .execute(
            json!({
                "path": path.to_string_lossy().to_string(),
                "offset": 2,
                "limit": 2
            }),
            ToolContext::new(".", "test-session-read-boundary"),
        )
        .await;

    assert!(result.success, "read failed: {:?}", result.error);
    assert!(result.content.contains("   2 | beta"));
    assert!(result
        .content
        .contains("[3 lines total, showing lines 2-3]"));
    let data = result.data.expect("file read metadata");
    assert_eq!(data["kind"], "file");
    assert_eq!(
        data["path_identity"]["lexical_path"],
        path.to_string_lossy().to_string()
    );
    assert_eq!(
        data["path_identity"]["resolved_path"],
        path.to_string_lossy().to_string()
    );
    assert_eq!(
        data["path_identity"]["state_key"],
        data["path_identity"]["canonical_path"]
    );
    assert_eq!(data["line_start"], 2);
    assert_eq!(data["line_end"], 3);
    assert_eq!(data["total_lines"], 3);
    assert_eq!(data["displayed_lines"], 2);
    assert_eq!(data["truncated"], true);
    assert_eq!(data["read_coverage"], "partial");
    assert_eq!(data["display_format"], "line_numbered_content");
    assert_eq!(
        data["content_format"]["visible_content"],
        "line_numbered_display"
    );
    assert_eq!(data["content_format"]["raw_content_in_tool_result"], false);
    assert!(data["content_hash"].as_str().unwrap_or("").len() >= 8);
    assert!(data["selected_content_hash"].as_str().unwrap_or("").len() >= 8);
}

// ===== FileEditTool 增强测试 =====

#[tokio::test]
async fn test_file_edit_success() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    env.remove("PRIORITY_AGENT_SMART_EDIT");
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_success.txt";
    tokio::fs::write(path, "hello world\nfoo bar\n")
        .await
        .unwrap();
    read_test_file_for_edit(path, "test-session-edit-success").await;

    let params = json!({
        "path": path,
        "old_string": "foo bar",
        "new_string": "baz qux"
    });
    let context = ToolContext::new(".", "test-session-edit-success");
    let result = tool.execute(params, context).await;

    assert!(result.success, "edit failed: {:?}", result.error);
    let data = result.data.expect("file_edit metadata");
    assert_eq!(data["path_identity"]["lexical_path"], path);
    assert_eq!(
        data["path_identity"]["state_key"],
        data["path_identity"]["canonical_path"]
    );
    assert_eq!(data["diff"]["additions"], 1);
    assert_eq!(data["diff"]["deletions"], 1);
    assert_eq!(data["diff"]["changed_line_start"], 2);
    assert_eq!(data["diff"]["changed_line_end"], 2);
    assert_eq!(data["diff"]["preview_truncated"], false);
    let unified_diff = data["diff"]["unified_diff"].as_str().unwrap_or("");
    assert!(unified_diff.contains("-foo bar"));
    assert!(unified_diff.contains("+baz qux"));
    assert_eq!(data["edit_preview"]["replacements"], 1);
    assert!(data["edit_preview"]["before_hash"].as_str().is_some());
    assert!(data["edit_preview"]["after_hash"].as_str().is_some());
    assert_eq!(data["edit_preview"]["validation_stage"], "edit_complete");
    assert_eq!(data["edit_preview"]["changed_range"]["start"], 2);
    assert!(data["edit_preview"]["diff_preview"]
        .as_str()
        .unwrap_or("")
        .contains("+baz qux"));
    assert_eq!(data["diagnostics"]["status"], "lsp_unavailable");
    assert_eq!(data["diagnostics"]["checked"], false);
    assert_eq!(data["diagnostics"]["diagnostic_count"], 0);
    assert_eq!(data["diagnostics_before"]["status"], "lsp_unavailable");
    assert_eq!(data["diagnostics_after"]["status"], "lsp_unavailable");
    assert_eq!(data["diagnostics_delta"]["checked"], false);
    assert_eq!(data["diagnostics_delta"]["status"], "not_checked");
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert!(content.contains("baz qux"));
    assert!(!content.contains("foo bar"));

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn file_edit_reports_no_lsp_clients_when_manager_has_none() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_no_lsp_clients.txt";
    tokio::fs::write(path, "one\ntwo\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-no-lsp-clients").await;

    let context = ToolContext::new(".", "test-session-edit-no-lsp-clients")
        .with_lsp_manager(std::sync::Arc::new(crate::engine::lsp::LspManager::new()));
    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "two",
                "new_string": "three"
            }),
            context,
        )
        .await;

    assert!(result.success, "edit failed: {:?}", result.error);
    let data = result.data.expect("file_edit metadata");
    assert_eq!(data["diagnostics"]["available"], true);
    assert_eq!(data["diagnostics"]["checked"], false);
    assert_eq!(data["diagnostics"]["status"], "no_lsp_clients");
    assert_eq!(data["diagnostics"]["diagnostic_count"], 0);

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn file_edit_preserves_crlf_line_endings() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_crlf.txt";
    tokio::fs::write(path, b"alpha\r\nbeta\r\ngamma\r\n")
        .await
        .unwrap();
    read_test_file_for_edit(path, "test-session-edit-crlf").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "beta\n",
                "new_string": "beta edited\n"
            }),
            ToolContext::new(".", "test-session-edit-crlf"),
        )
        .await;

    assert!(result.success, "edit failed: {:?}", result.error);
    let data = result.data.expect("file_edit metadata");
    assert_eq!(data["text_format"]["line_ending"], "CRLF");
    let bytes = tokio::fs::read(path).await.unwrap();
    assert_eq!(bytes, b"alpha\r\nbeta edited\r\ngamma\r\n");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn file_read_reports_utf8_bom_and_normalizes_crlf_display() {
    let tool = FileReadTool;
    let path = "/tmp/test_priority_agent_read_bom_crlf.txt";
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice(b"alpha\r\nbeta\r\n");
    tokio::fs::write(path, bytes).await.unwrap();

    let result = tool
        .execute(
            json!({
                "path": path
            }),
            ToolContext::new(".", "test-session-read-bom-crlf"),
        )
        .await;

    assert!(result.success, "read failed: {:?}", result.error);
    assert!(result.content.contains("alpha"));
    assert!(!result.content.contains('\u{feff}'));
    let data = result.data.expect("file_read metadata");
    assert_eq!(data["text_format"]["encoding"], "utf-8");
    assert_eq!(data["text_format"]["bom"], true);
    assert_eq!(data["text_format"]["line_ending"], "CRLF");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn file_edit_preserves_utf16le_bom() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_utf16le.txt";
    let original = encode_text_content(
        "hello\nworld\n",
        TextFileEncoding::Utf16Le,
        true,
        LineEndingStyle::Lf,
    );
    tokio::fs::write(path, original).await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-utf16le").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "world",
                "new_string": "世界"
            }),
            ToolContext::new(".", "test-session-edit-utf16le"),
        )
        .await;

    assert!(result.success, "edit failed: {:?}", result.error);
    let data = result.data.expect("file_edit metadata");
    assert_eq!(data["text_format"]["encoding"], "utf-16le");
    assert_eq!(data["text_format"]["bom"], true);
    let bytes = tokio::fs::read(path).await.unwrap();
    assert!(bytes.starts_with(&[0xff, 0xfe]));
    let decoded = decode_text_file(Path::new(path), "test", bytes).unwrap();
    assert_eq!(decoded.content, "hello\n世界\n");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_edit_serializes_concurrent_edits_on_same_file() {
    let path = "/tmp/test_priority_agent_edit_lock.txt";
    tokio::fs::write(path, "start\n").await.unwrap();

    let left = FileEditTool;
    let right = FileEditTool;
    let left_context = ToolContext::new(".", "test-session-edit-lock-left");
    let right_context = ToolContext::new(".", "test-session-edit-lock-right");
    read_test_file_for_edit(path, "test-session-edit-lock-left").await;
    read_test_file_for_edit(path, "test-session-edit-lock-right").await;
    let left_params = json!({
        "path": path,
        "old_string": "start",
        "new_string": "left"
    });
    let right_params = json!({
        "path": path,
        "old_string": "start",
        "new_string": "right"
    });

    let (left_result, right_result) = tokio::join!(
        left.execute(left_params, left_context),
        right.execute(right_params, right_context)
    );

    let success_count = [left_result.success, right_result.success]
        .into_iter()
        .filter(|success| *success)
        .count();
    assert_eq!(
        success_count, 1,
        "exactly one edit should win the serialized old_string replacement"
    );
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert!(
        content == "left\n" || content == "right\n",
        "unexpected final content: {content:?}"
    );

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_unread_file_by_default() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_unread_default.txt";
    tokio::fs::write(path, "hello world\n").await.unwrap();

    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "hello",
                "new_string": "hi"
            }),
            ToolContext::new(".", "test-session-edit-unread-default"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("has not been read yet"), "{err}");
    assert_eq!(
        tokio::fs::read_to_string(path).await.unwrap(),
        "hello world\n"
    );

    let _ = tokio::fs::remove_file(path).await;
    clear_read_files("test-session-edit-unread-default");
}

#[tokio::test]
async fn test_file_edit_rejects_stale_read_by_default() {
    let read_tool = FileReadTool;
    let edit_tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_stale_read.txt";
    let session_id = "test-session-edit-stale-read";
    tokio::fs::write(path, "hello world\n").await.unwrap();

    let read_result = read_tool
        .execute(json!({ "path": path }), ToolContext::new(".", session_id))
        .await;
    assert!(read_result.success, "read failed: {:?}", read_result.error);

    tokio::fs::write(path, "hello changed\n").await.unwrap();
    let edit_result = edit_tool
        .execute(
            json!({
                "path": path,
                "old_string": "hello changed",
                "new_string": "hello edited"
            }),
            ToolContext::new(".", session_id),
        )
        .await;

    assert!(!edit_result.success);
    let err = edit_result.error.unwrap_or_default();
    assert!(err.contains("file changed since this session last read it"));
    let data = edit_result
        .data
        .expect("stale edit should return recovery data");
    assert_eq!(data["failure"], "stale_read_conflict");
    assert_eq!(data["recovery"]["recommended_action"], "re_read_file");
    assert!(data["conflict"]["read_hash"].as_str().is_some());
    assert!(data["conflict"]["current_hash"].as_str().is_some());
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "hello changed\n");

    let _ = tokio::fs::remove_file(path).await;
    clear_read_files(session_id);
}

#[tokio::test]
async fn test_file_edit_stale_read_uses_resolved_path_key() {
    let read_tool = FileReadTool;
    let edit_tool = FileEditTool;
    let session_id = "test-session-edit-stale-resolved-path";
    let root = std::env::temp_dir().join(format!(
        "test_priority_agent_edit_stale_resolved_path_{}",
        uuid::Uuid::new_v4()
    ));
    let nested = root.join("nested");
    let path = nested.join("target.txt");
    let _ = tokio::fs::remove_dir_all(&root).await;
    tokio::fs::create_dir_all(&nested).await.unwrap();
    tokio::fs::write(&path, "hello world\n").await.unwrap();

    let read_result = read_tool
        .execute(
            json!({ "path": "nested/target.txt" }),
            ToolContext::new(&root, session_id),
        )
        .await;
    assert!(read_result.success, "read failed: {:?}", read_result.error);

    tokio::fs::write(&path, "hello changed\n").await.unwrap();
    let edit_result = edit_tool
        .execute(
            json!({
                "path": path.to_string_lossy().to_string(),
                "old_string": "hello changed",
                "new_string": "hello edited"
            }),
            ToolContext::new(&root, session_id),
        )
        .await;

    assert!(!edit_result.success);
    let err = edit_result.error.unwrap_or_default();
    assert!(err.contains("file changed since this session last read it"));
    let data = edit_result
        .data
        .expect("stale edit should return recovery data");
    assert_eq!(data["failure"], "stale_read_conflict");
    assert_eq!(
        data["path_identity"]["state_key"],
        data["path_identity"]["canonical_path"]
    );
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(content, "hello changed\n");

    let _ = tokio::fs::remove_dir_all(&root).await;
    clear_read_files(session_id);
}

#[tokio::test]
async fn test_file_edit_rejects_exact_edit_after_partial_read() {
    let read_tool = FileReadTool;
    let edit_tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_partial_read_exact.txt";
    let session_id = "test-session-edit-partial-read-exact";
    tokio::fs::write(path, "line1\nline2\nline3\n")
        .await
        .unwrap();

    let read_result = read_tool
        .execute(
            json!({
                "path": path,
                "offset": 2,
                "limit": 1
            }),
            ToolContext::new(".", session_id),
        )
        .await;
    assert!(read_result.success, "read failed: {:?}", read_result.error);

    let edit_result = edit_tool
        .execute(
            json!({
                "path": path,
                "old_string": "line2",
                "new_string": "edited"
            }),
            ToolContext::new(".", session_id),
        )
        .await;

    assert!(!edit_result.success);
    let err = edit_result.error.unwrap_or_default();
    assert!(err.contains("only been partially read"));
    assert!(err.contains("line_start/line_end"));
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "line1\nline2\nline3\n");

    let _ = tokio::fs::remove_file(path).await;
    clear_read_files(session_id);
}

#[tokio::test]
async fn test_file_edit_allows_line_range_after_partial_read() {
    let read_tool = FileReadTool;
    let edit_tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_partial_read_line_range.txt";
    let session_id = "test-session-edit-partial-read-line-range";
    tokio::fs::write(path, "line1\nline2\nline3\n")
        .await
        .unwrap();

    let read_result = read_tool
        .execute(
            json!({
                "path": path,
                "offset": 2,
                "limit": 1
            }),
            ToolContext::new(".", session_id),
        )
        .await;
    assert!(read_result.success, "read failed: {:?}", read_result.error);

    let edit_result = edit_tool
        .execute(
            json!({
                "path": path,
                "line_start": 2,
                "line_end": 2,
                "new_string": "edited"
            }),
            ToolContext::new(".", session_id),
        )
        .await;

    assert!(edit_result.success, "edit failed: {:?}", edit_result.error);
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "line1\nedited\nline3\n");

    let _ = tokio::fs::remove_file(path).await;
    clear_read_files(session_id);
}

#[tokio::test]
async fn test_file_edit_allows_explicit_stale_read_override() {
    let read_tool = FileReadTool;
    let edit_tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_stale_override.txt";
    let session_id = "test-session-edit-stale-override";
    tokio::fs::write(path, "hello world\n").await.unwrap();

    let read_result = read_tool
        .execute(json!({ "path": path }), ToolContext::new(".", session_id))
        .await;
    assert!(read_result.success, "read failed: {:?}", read_result.error);

    tokio::fs::write(path, "hello changed\n").await.unwrap();
    let edit_result = edit_tool
        .execute(
            json!({
                "path": path,
                "old_string": "hello changed",
                "new_string": "hello edited",
                "allow_stale_read": true
            }),
            ToolContext::new(".", session_id),
        )
        .await;

    assert!(edit_result.success, "edit failed: {:?}", edit_result.error);
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "hello edited\n");

    let _ = tokio::fs::remove_file(path).await;
    clear_read_files(session_id);
}

#[tokio::test]
async fn test_file_edit_multiple_occurrences_error() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_multi.txt";
    tokio::fs::write(path, "aaa\naaa\naaa\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-multi").await;

    let params = json!({
        "path": path,
        "old_string": "aaa",
        "new_string": "bbb"
    });
    let context = ToolContext::new(".", "test-session-edit-multi");
    let result = tool.execute(params, context).await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("Expected 1 occurrence"));
    assert!(err.contains("but found 3"));
    let data = result
        .data
        .expect("multi-match edit should return match diagnostics");
    assert_eq!(data["failure"], "old_string_occurrence_mismatch");
    assert_eq!(data["match_diagnostics"]["actual_occurrences"], 3);
    assert_eq!(data["recovery"]["recommended_action"], "narrow_anchor");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_no_op_with_recovery_data() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_no_op.txt";
    tokio::fs::write(path, "aaa\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-no-op").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "aaa",
                "new_string": "aaa"
            }),
            ToolContext::new(".", "test-session-edit-no-op"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("no-op"));
    let data = result.data.expect("no-op should return recovery data");
    assert_eq!(data["failure"], "no_op_edit");
    assert_eq!(
        data["recovery"]["recommended_action"],
        "change_replacement_or_skip"
    );

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_env_secret_target_with_recovery_data() {
    let tool = FileEditTool;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".env");
    tokio::fs::write(&path, "TOKEN=old\n").await.unwrap();

    let result = tool
        .execute(
            json!({
                "path": ".env",
                "old_string": "TOKEN=old",
                "new_string": "TOKEN=new"
            }),
            ToolContext::new(dir.path(), "test-session-edit-env-target"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("credential"));
    let data = result
        .data
        .expect("secret target rejection should return recovery data");
    assert_eq!(data["failure"], "secret_or_credential_target");
    assert_eq!(
        data["recovery"]["recommended_action"],
        "ask_user_for_explicit_secret_file_plan"
    );
    assert_eq!(
        tokio::fs::read_to_string(&path).await.unwrap(),
        "TOKEN=old\n"
    );
}

#[tokio::test]
async fn test_file_edit_rejects_whitespace_only_old_string() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    env.remove("PRIORITY_AGENT_SMART_EDIT");
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_blank_anchor.txt";
    tokio::fs::write(path, "line1\nline2\nline3\n")
        .await
        .unwrap();
    read_test_file_for_edit(path, "test-session-edit-blank-anchor").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "\n",
                "new_string": "replacement"
            }),
            ToolContext::new(".", "test-session-edit-blank-anchor"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("whitespace-only"));
    assert!(err.contains("line_start"));

    let _ = tokio::fs::remove_file(path).await;
}

#[test]
fn test_match_context_limits_large_occurrence_output() {
    let content = (0..50)
        .map(|i| format!("let value_{i} = true;"))
        .collect::<Vec<_>>()
        .join("\n");
    let occurrences = find_occurrences(&content, "let");
    let context = build_match_context(&content, &occurrences, 0);

    assert!(context.contains("Found 50 occurrence(s)"));
    assert!(context.contains("showing first 12 of 50 matches"));
    assert!(!context.contains("Match #13"));
}

#[tokio::test]
async fn test_file_edit_expected_replacements() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_expected.txt";
    tokio::fs::write(path, "aaa\naaa\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-expected").await;

    let params = json!({
        "path": path,
        "old_string": "aaa",
        "new_string": "bbb",
        "expected_replacements": 2
    });
    let context = ToolContext::new(".", "test-session-edit-expected");
    let result = tool.execute(params, context).await;

    assert!(result.success, "edit failed: {:?}", result.error);
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content.matches("bbb").count(), 2);

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_bulk_exact_replace_on_code_file() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_expected.rs";
    tokio::fs::write(path, "let x = 1;\nlet x = 1;\n")
        .await
        .unwrap();
    read_test_file_for_edit(path, "test-session-edit-code-bulk").await;

    let params = json!({
        "path": path,
        "old_string": "let x = 1;",
        "new_string": "let x = 2;",
        "expected_replacements": 2
    });
    let context = ToolContext::new(".", "test-session-edit-code-bulk");
    let result = tool.execute(params, context).await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("Refusing multi-occurrence file_edit on code file"));
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content.matches("let x = 1;").count(), 2);

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_excessive_bulk_replacements() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_bulk_limit.txt";
    tokio::fs::write(path, "aaa\n".repeat(51)).await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-bulk-limit").await;

    let params = json!({
        "path": path,
        "old_string": "aaa",
        "new_string": "bbb",
        "expected_replacements": 51
    });
    let context = ToolContext::new(".", "test-session-edit-bulk-limit");
    let result = tool.execute(params, context).await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("Refusing file_edit with 51 replacement"));
    let data = result
        .data
        .expect("bulk-limit edit should return recovery data");
    assert_eq!(data["failure"], "replacement_limit_exceeded");
    assert_eq!(data["expected_replacements"], 51);

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_fuzzy_match_hint() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_fuzzy.txt";
    tokio::fs::write(path, "    hello world\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-fuzzy").await;

    // 提交带有额外空格的 old_string，精确匹配失败但模糊匹配成功
    let params = json!({
        "path": path,
        "old_string": "  hello world  ",
        "new_string": "hi world"
    });
    let context = ToolContext::new(".", "test-session-edit-fuzzy");
    let result = tool.execute(params, context).await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("fuzzy matches found"));
    let data = result
        .data
        .expect("fuzzy edit should return match diagnostics");
    assert_eq!(data["failure"], "old_string_not_found");
    assert_eq!(data["match_diagnostics"]["exact_occurrences"], 0);
    assert_eq!(data["match_diagnostics"]["fuzzy_occurrences"], 1);
    assert_eq!(
        data["recovery"]["recommended_action"],
        "copy_exact_fuzzy_match"
    );

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_old_string_not_found_recommends_bounded_reread_then_line_range() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_missing_old_string.txt";
    tokio::fs::write(path, "hello world\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-missing-old-string").await;

    let params = json!({
        "path": path,
        "old_string": "goodbye moon",
        "new_string": "hi world"
    });
    let context = ToolContext::new(".", "test-session-edit-missing-old-string");
    let result = tool.execute(params, context).await;

    assert!(!result.success);
    let data = result
        .data
        .expect("missing old_string should return recovery data");
    assert_eq!(data["failure"], "old_string_not_found");
    assert_eq!(
        data["recovery"]["recommended_action"],
        "re_read_once_then_line_range_edit"
    );
    assert_eq!(data["match_diagnostics"]["exact_occurrences"], 0);
    assert_eq!(data["match_diagnostics"]["fuzzy_occurrences"], 0);

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_file_read_line_prefix_in_old_string() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_line_prefix.txt";
    tokio::fs::write(path, "hello world\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-line-prefix").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "   1 | hello world",
                "new_string": "hi world"
            }),
            ToolContext::new(".", "test-session-edit-line-prefix"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("file_read display line prefixes"));
    assert!(err.contains("line_start/line_end"));
    let data = result
        .data
        .expect("line-prefix edit should return recovery data");
    assert_eq!(data["failure"], "file_read_line_prefix_in_old_string");
    assert_eq!(
        data["recovery"]["recommended_action"],
        "remove_display_line_prefix"
    );
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "hello world\n");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_file_read_line_prefix_in_insert_anchor() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_insert_line_prefix.txt";
    tokio::fs::write(path, "hello world\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-insert-line-prefix").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "insert_after": "   1 | hello world",
                "new_string": "\nhi world"
            }),
            ToolContext::new(".", "test-session-edit-insert-line-prefix"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("insert_after appears to include file_read"));

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_insert_after() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_insert_after.txt";
    tokio::fs::write(path, "line1\nline2\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-insert").await;

    let params = json!({
        "path": path,
        "insert_after": "line1",
        "new_string": "\nline1.5"
    });
    let context = ToolContext::new(".", "test-session-edit-insert");
    let result = tool.execute(params, context).await;

    assert!(result.success, "insert failed: {:?}", result.error);
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert!(content.contains("line1\nline1.5\nline2"));

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_insert_before() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_insert_before.txt";
    tokio::fs::write(path, "line1\nline2\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-insert-before").await;

    let params = json!({
        "path": path,
        "insert_before": "line2",
        "new_string": "line1.5\n"
    });
    let context = ToolContext::new(".", "test-session-edit-insert-before");
    let result = tool.execute(params, context).await;

    assert!(result.success, "insert failed: {:?}", result.error);
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert!(content.contains("line1\nline1.5\nline2"));

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_insert_rejects_ambiguous_anchor_by_default() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_insert_ambiguous.txt";
    tokio::fs::write(path, "line\nline\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-insert-ambiguous").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "insert_after": "line",
                "new_string": "\ninserted"
            }),
            ToolContext::new(".", "test-session-edit-insert-ambiguous"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("Expected 1 occurrence(s) of insert_after anchor"));
    assert!(err.contains("expected_replacements to 2"));
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "line\nline\n");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_insert_allows_intentional_bulk_anchor() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_insert_bulk.txt";
    tokio::fs::write(path, "line\nline\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-insert-bulk").await;

    let result = tool
        .execute(
            json!({
                "path": path,
                "insert_after": "line",
                "new_string": "!",
                "expected_replacements": 2
            }),
            ToolContext::new(".", "test-session-edit-insert-bulk"),
        )
        .await;

    assert!(result.success, "insert failed: {:?}", result.error);
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "line!\nline!\n");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_checkpoint_created() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    env.remove("PRIORITY_AGENT_TEST_FAIL_CHECKPOINT");
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_checkpoint.txt";
    let original = "original content\n";
    tokio::fs::write(path, original).await.unwrap();
    read_test_file_for_edit(path, "test-session-checkpoint").await;

    let params = json!({
        "path": path,
        "old_string": "original",
        "new_string": "modified"
    });
    let session_id = "test-session-checkpoint";
    let mgr = crate::engine::checkpoint::get_checkpoint_manager(session_id).await;
    mgr.lock().await.clear_all().await.unwrap();
    let context = ToolContext::new(".", session_id);
    let result = tool.execute(params, context).await;

    assert!(result.success, "edit failed: {:?}", result.error);
    let data = result.data.as_ref().expect("file_edit metadata");
    assert!(data["checkpoint"]["id"].as_str().is_some());
    assert!(data["file_change"]["id"]
        .as_str()
        .unwrap_or("")
        .starts_with("fc_"));
    assert_eq!(data["file_change"]["tool_name"], "file_edit");

    // 验证 checkpoint 被创建
    let cp = mgr.lock().await;
    let checkpoints = cp.list_checkpoints();
    assert!(!checkpoints.is_empty(), "checkpoint should be created");
    assert!(
        !cp.list_file_changes().is_empty(),
        "file change should be recorded"
    );

    let latest = checkpoints.last().unwrap();
    assert_eq!(latest.tool_name, "file_edit");
    assert_eq!(latest.file_backups.len(), 1);
    assert_eq!(latest.file_backups[0].original_path, path);
    assert!(latest.file_backups[0].existed_before);

    // 验证可以恢复
    let restore_result = cp.restore_checkpoint(&latest.id).await.unwrap();
    assert_eq!(restore_result.restored_files.len(), 1);
    let restored_content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(restored_content, original);

    drop(cp);
    let _ = tokio::fs::remove_file(path).await;
    mgr.lock().await.clear_all().await.unwrap();
}

#[tokio::test]
async fn test_file_edit_refuses_when_checkpoint_creation_fails() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_checkpoint_failure.txt";
    tokio::fs::write(path, "before\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-checkpoint-failure").await;

    env.set("PRIORITY_AGENT_TEST_FAIL_CHECKPOINT", "1");
    let result = tool
        .execute(
            json!({
                "path": path,
                "old_string": "before",
                "new_string": "after"
            }),
            ToolContext::new(".", "test-session-checkpoint-failure"),
        )
        .await;

    assert!(!result.success);
    assert!(result
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("checkpoint creation failed"));
    assert_eq!(
        tokio::fs::read_to_string(path).await.unwrap(),
        "before\n",
        "file_edit must not write without rollback checkpoint"
    );
    let data = result.data.expect("checkpoint failure metadata");
    assert_eq!(data["failure"], "checkpoint_creation_failed");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_write_refuses_when_checkpoint_creation_fails() {
    let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
    let tool = FileWriteTool;
    let path = "/tmp/test_priority_agent_write_checkpoint_failure.txt";
    let session_id = "test-session-write-checkpoint-failure";
    tokio::fs::write(path, "before\n").await.unwrap();
    mark_file_read(session_id, path);
    mark_file_read(
        session_id,
        &canonicalize_or_normalize(Path::new(path)).to_string_lossy(),
    );

    env.set("PRIORITY_AGENT_TEST_FAIL_CHECKPOINT", "1");
    let result = tool
        .execute(
            json!({
                "path": path,
                "content": "after\n"
            }),
            ToolContext::new(".", session_id),
        )
        .await;

    assert!(!result.success);
    assert!(result
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("checkpoint creation failed"));
    assert_eq!(
        tokio::fs::read_to_string(path).await.unwrap(),
        "before\n",
        "file_write must not write without rollback checkpoint"
    );
    let data = result.data.expect("checkpoint failure metadata");
    assert_eq!(data["failure"], "checkpoint_creation_failed");
    assert_eq!(data["tool"], "file_write");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_rejects_invalid_priority_agent_permissions_toml() {
    let tool = FileEditTool;
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join(".priority-agent");
    tokio::fs::create_dir_all(&config_dir).await.unwrap();
    let path = config_dir.join("permissions.toml");
    tokio::fs::write(
        &path,
        "always_allow = [{ pattern = \"file_read\", source = \"Project\" }]\n",
    )
    .await
    .unwrap();
    let path_str = path.to_string_lossy().to_string();
    read_test_file_for_edit(&path_str, "test-session-settings-schema").await;

    let result = tool
        .execute(
            json!({
                "path": path_str,
                "old_string": "pattern = \"file_read\"",
                "new_string": "pattern = \"\""
            }),
            ToolContext::new(".", "test-session-settings-schema"),
        )
        .await;

    assert!(!result.success);
    let err = result.error.as_deref().unwrap_or_default();
    assert!(err.contains("Priority Agent settings file"));
    assert!(err.contains("pattern must be a non-empty string"));
    assert!(tokio::fs::read_to_string(&path)
        .await
        .unwrap()
        .contains("file_read"));
    let data = result.data.expect("schema failure metadata");
    assert_eq!(data["failure"], "settings_schema_validation");
}

#[tokio::test]
async fn test_file_edit_line_range() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_lines.txt";
    tokio::fs::write(path, "line1\nline2\nline3\nline4\n")
        .await
        .unwrap();
    read_test_file_for_edit(path, "test-session-edit-lines").await;

    let params = json!({
        "path": path,
        "line_start": 2,
        "line_end": 3,
        "new_string": "REPLACED"
    });
    let context = ToolContext::new(".", "test-session-edit-lines");
    let result = tool.execute(params, context).await;

    assert!(result.success, "line edit failed: {:?}", result.error);
    let content = tokio::fs::read_to_string(path).await.unwrap();
    assert_eq!(content, "line1\nREPLACED\nline4\n");

    let _ = tokio::fs::remove_file(path).await;
}

#[tokio::test]
async fn test_file_edit_normalize_whitespace() {
    let tool = FileEditTool;
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let path = format!("/tmp/test_priority_agent_edit_normws_{unique}.txt");
    let session_id = format!("test-session-edit-normws-{unique}");
    tokio::fs::write(&path, "    hello world    \n")
        .await
        .unwrap();
    read_test_file_for_edit(&path, &session_id).await;

    // old_string 有额外空白，但 normalize_whitespace=true 应能匹配
    let params = json!({
        "path": path,
        "old_string": "hello world",
        "new_string": "hi world",
        "normalize_whitespace": true
    });
    let context = ToolContext::new(".", &session_id);
    let result = tool.execute(params, context).await;

    assert!(result.success, "normalize edit failed: {:?}", result.error);
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(content.contains("hi world"));
    assert!(!content.contains("hello world"));

    let _ = tokio::fs::remove_file(&path).await;
}

#[tokio::test]
async fn test_file_edit_line_range_out_of_bounds() {
    let tool = FileEditTool;
    let path = "/tmp/test_priority_agent_edit_lines_oob.txt";
    tokio::fs::write(path, "line1\n").await.unwrap();
    read_test_file_for_edit(path, "test-session-edit-lines-oob").await;

    let params = json!({
        "path": path,
        "line_start": 5,
        "line_end": 6,
        "new_string": "REPLACED"
    });
    let context = ToolContext::new(".", "test-session-edit-lines-oob");
    let result = tool.execute(params, context).await;

    assert!(!result.success);
    let err = result.error.unwrap_or_default();
    assert!(err.contains("beyond end of file"));

    let _ = tokio::fs::remove_file(path).await;
}

#[test]
fn test_find_occurrences_normalized() {
    let content = "  hello world  \n    hello world    \n";
    let target = "hello world";
    let occ = find_occurrences_normalized(content, target);
    assert_eq!(occ.len(), 2);
    // 第一个匹配应包含前导空格
    assert_eq!(occ[0], (2, 15)); // "  hello world  "
    assert_eq!(occ[1], (20, 35)); // "    hello world    "
}

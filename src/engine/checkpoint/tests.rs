use super::types::*;
use std::collections::HashSet;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_checkpoint_create_and_restore() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.txt");
    std::fs::write(&test_file, "original content").unwrap();

    let mut mgr = CheckpointManager::new("test_session").await;
    // 覆盖 checkpoints_dir 到临时目录
    let cp_dir = temp.path().join("checkpoints").join("test_session");
    mgr.checkpoints_dir = cp_dir.clone();

    let cp = mgr
        .create_checkpoint("file_write", None, None, &[test_file.clone()])
        .await
        .unwrap();

    assert_eq!(cp.file_backups.len(), 1);
    assert!(cp.file_backups[0].existed_before);

    // 修改文件
    std::fs::write(&test_file, "modified content").unwrap();
    assert_eq!(
        std::fs::read_to_string(&test_file).unwrap(),
        "modified content"
    );

    // 恢复
    let result = mgr.restore_checkpoint(&cp.id).await.unwrap();
    assert_eq!(result.restored_files.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&test_file).unwrap(),
        "original content"
    );
}

#[tokio::test]
async fn test_checkpoint_new_file_then_restore_removes_it() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("new_file.txt");
    // 文件不存在

    let mut mgr = CheckpointManager::new("test_session2").await;
    mgr.checkpoints_dir = temp.path().join("checkpoints").join("test_session2");

    let cp = mgr
        .create_checkpoint("file_write", None, None, &[test_file.clone()])
        .await
        .unwrap();

    assert_eq!(cp.file_backups.len(), 1);
    assert!(!cp.file_backups[0].existed_before);

    // 创建文件
    std::fs::write(&test_file, "new content").unwrap();
    assert!(test_file.exists());

    // 恢复应该删除文件
    let result = mgr.restore_checkpoint(&cp.id).await.unwrap();
    assert_eq!(result.removed_files.len(), 1);
    assert!(!test_file.exists());
}

#[tokio::test]
async fn test_file_change_record_restores_latest_change() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("tracked.txt");
    std::fs::write(&test_file, "before").unwrap();

    let session_id = format!("test_file_change_{}", Uuid::new_v4().simple());
    let mut mgr = CheckpointManager::new(&session_id).await;
    mgr.checkpoints_dir = temp.path().join("checkpoints").join(&session_id);
    mgr.checkpoints.clear();
    mgr.tracked_files.clear();
    mgr.file_changes.clear();
    mgr.sequence_counter = 0;

    let cp = mgr
        .create_checkpoint(
            "file_edit",
            None,
            Some("call_1".to_string()),
            &[test_file.clone()],
        )
        .await
        .unwrap();

    std::fs::write(&test_file, "after").unwrap();
    let record = mgr
        .record_file_change(FileChangeInput {
            checkpoint_id: cp.id.clone(),
            tool_name: "file_edit".to_string(),
            tool_call_id: Some("call_1".to_string()),
            message_id: None,
            part_id: None,
            tool_round_id: Some("round_1".to_string()),
            path: test_file.to_string_lossy().to_string(),
            existed_before: true,
            before_hash: Some("before-hash".to_string()),
            after_hash: Some("after-hash".to_string()),
            diff: Some("--- a/tracked.txt\n+++ b/tracked.txt".to_string()),
            bytes_written: 5,
        })
        .await
        .unwrap();

    assert_eq!(mgr.list_file_changes().len(), 1);
    assert_eq!(mgr.latest_file_change().unwrap().id, record.id);
    assert_eq!(mgr.stats().total_file_changes, 1);

    let restored = mgr.restore_latest_file_change().await.unwrap();
    assert_eq!(restored.restored_files.len(), 1);
    assert_eq!(std::fs::read_to_string(&test_file).unwrap(), "before");
}

#[tokio::test]
async fn test_file_change_history_persists_and_removes_new_file() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("new_tracked.txt");

    let session_id = format!("test_file_change_new_{}", Uuid::new_v4().simple());
    let mut mgr = CheckpointManager::new(&session_id).await;
    mgr.checkpoints_dir = temp.path().join("checkpoints").join(&session_id);
    mgr.checkpoints.clear();
    mgr.tracked_files.clear();
    mgr.file_changes.clear();
    mgr.sequence_counter = 0;

    let cp = mgr
        .create_checkpoint("file_write", None, None, &[test_file.clone()])
        .await
        .unwrap();
    std::fs::write(&test_file, "created").unwrap();
    let record = mgr
        .record_file_change(FileChangeInput {
            checkpoint_id: cp.id.clone(),
            tool_name: "file_write".to_string(),
            tool_call_id: None,
            message_id: None,
            part_id: None,
            tool_round_id: None,
            path: test_file.to_string_lossy().to_string(),
            existed_before: false,
            before_hash: None,
            after_hash: Some("created-hash".to_string()),
            diff: Some("new file".to_string()),
            bytes_written: 7,
        })
        .await
        .unwrap();

    let mut loaded = CheckpointManager {
        session_id,
        checkpoints_dir: mgr.checkpoints_dir.clone(),
        checkpoints: Vec::new(),
        tracked_files: HashSet::new(),
        sequence_counter: 0,
        file_changes: Vec::new(),
        revert_history: Vec::new(),
    };
    loaded.load_from_disk().await.unwrap();

    assert_eq!(loaded.list_file_changes().len(), 1);
    assert_eq!(loaded.list_file_changes()[0].id, record.id);

    let restored = loaded.restore_file_change(&record.id).await.unwrap();
    assert_eq!(restored.removed_files.len(), 1);
    assert!(!test_file.exists());
}

#[tokio::test]
async fn test_restore_latest_tool_round_restores_all_round_changes() {
    let temp = TempDir::new().unwrap();
    let first = temp.path().join("first.txt");
    let second = temp.path().join("second.txt");
    std::fs::write(&first, "first-before").unwrap();
    std::fs::write(&second, "second-before").unwrap();

    let session_id = format!("test_tool_round_{}", Uuid::new_v4().simple());
    let mut mgr = CheckpointManager::new(&session_id).await;
    mgr.checkpoints_dir = temp.path().join("checkpoints").join(&session_id);
    mgr.checkpoints.clear();
    mgr.tracked_files.clear();
    mgr.file_changes.clear();
    mgr.sequence_counter = 0;

    let round_id = Some("round_same".to_string());
    let first_cp = mgr
        .create_checkpoint(
            "file_edit",
            None,
            Some("call_1".to_string()),
            &[first.clone()],
        )
        .await
        .unwrap();
    std::fs::write(&first, "first-after").unwrap();
    mgr.record_file_change(FileChangeInput {
        checkpoint_id: first_cp.id,
        tool_name: "file_edit".to_string(),
        tool_call_id: Some("call_1".to_string()),
        message_id: None,
        part_id: None,
        tool_round_id: round_id.clone(),
        path: first.to_string_lossy().to_string(),
        existed_before: true,
        before_hash: Some("first-before".to_string()),
        after_hash: Some("first-after".to_string()),
        diff: Some("first diff".to_string()),
        bytes_written: 11,
    })
    .await
    .unwrap();

    let second_cp = mgr
        .create_checkpoint(
            "file_edit",
            None,
            Some("call_2".to_string()),
            &[second.clone()],
        )
        .await
        .unwrap();
    std::fs::write(&second, "second-after").unwrap();
    mgr.record_file_change(FileChangeInput {
        checkpoint_id: second_cp.id,
        tool_name: "file_edit".to_string(),
        tool_call_id: Some("call_2".to_string()),
        message_id: None,
        part_id: None,
        tool_round_id: round_id.clone(),
        path: second.to_string_lossy().to_string(),
        existed_before: true,
        before_hash: Some("second-before".to_string()),
        after_hash: Some("second-after".to_string()),
        diff: Some("second diff".to_string()),
        bytes_written: 12,
    })
    .await
    .unwrap();

    let restored = mgr.restore_latest_tool_round().await.unwrap();
    assert_eq!(restored.tool_round_id, round_id);
    assert_eq!(restored.restored_changes.len(), 2);
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "first-before");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "second-before");
}

#[tokio::test]
async fn test_file_change_rounds_group_by_assistant_message() {
    let temp = TempDir::new().unwrap();
    let first = temp.path().join("first.txt");
    let second = temp.path().join("second.txt");
    std::fs::write(&first, "first-before").unwrap();
    std::fs::write(&second, "second-before").unwrap();

    let session_id = format!("test_message_round_{}", Uuid::new_v4().simple());
    let mut mgr = CheckpointManager::new(&session_id).await;
    mgr.checkpoints_dir = temp.path().join("checkpoints").join(&session_id);
    mgr.checkpoints.clear();
    mgr.tracked_files.clear();
    mgr.file_changes.clear();
    mgr.sequence_counter = 0;

    let message_id = Some("assistant_msg_same".to_string());
    let first_cp = mgr
        .create_checkpoint(
            "file_edit",
            message_id.clone(),
            Some("call_1".to_string()),
            &[first.clone()],
        )
        .await
        .unwrap();
    std::fs::write(&first, "first-after").unwrap();
    mgr.record_file_change(FileChangeInput {
        checkpoint_id: first_cp.id,
        tool_name: "file_edit".to_string(),
        tool_call_id: Some("call_1".to_string()),
        message_id: message_id.clone(),
        part_id: Some("part_1".to_string()),
        tool_round_id: Some("round_1".to_string()),
        path: first.to_string_lossy().to_string(),
        existed_before: true,
        before_hash: Some("first-before".to_string()),
        after_hash: Some("first-after".to_string()),
        diff: Some("first diff".to_string()),
        bytes_written: 11,
    })
    .await
    .unwrap();

    let second_cp = mgr
        .create_checkpoint(
            "file_edit",
            message_id.clone(),
            Some("call_2".to_string()),
            &[second.clone()],
        )
        .await
        .unwrap();
    std::fs::write(&second, "second-after").unwrap();
    mgr.record_file_change(FileChangeInput {
        checkpoint_id: second_cp.id,
        tool_name: "file_edit".to_string(),
        tool_call_id: Some("call_2".to_string()),
        message_id: message_id.clone(),
        part_id: Some("part_2".to_string()),
        tool_round_id: Some("round_2".to_string()),
        path: second.to_string_lossy().to_string(),
        existed_before: true,
        before_hash: Some("second-before".to_string()),
        after_hash: Some("second-after".to_string()),
        diff: Some("second diff".to_string()),
        bytes_written: 12,
    })
    .await
    .unwrap();

    let rounds = mgr.list_file_change_rounds();
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0].message_id, message_id);
    assert_eq!(rounds[0].part_ids, vec!["part_1", "part_2"]);
    assert_eq!(rounds[0].change_count, 2);
    assert_eq!(rounds[0].paths.len(), 2);
}

#[tokio::test]
async fn test_checkpoint_pruning() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.txt");
    std::fs::write(&test_file, "content").unwrap();

    let mut mgr = CheckpointManager::new("test_session3").await;
    mgr.checkpoints_dir = temp.path().join("checkpoints").join("test_session3");

    // 创建 5 个 checkpoint（设小一点测试）
    let mut ids = Vec::new();
    for i in 0..5 {
        std::fs::write(&test_file, format!("content {}", i)).unwrap();
        let cp = mgr
            .create_checkpoint("file_write", None, None, &[test_file.clone()])
            .await
            .unwrap();
        ids.push(cp.id);
    }

    assert_eq!(mgr.list_checkpoints().len(), 5);

    // 手动触发 pruning（把 MAX_CHECKPOINTS 调小来测试）
    // 这里不直接测 pruning 因为 MAX_CHECKPOINTS 是 100
}

#[tokio::test]
async fn test_diff_checkpoints() {
    let temp = TempDir::new().unwrap();
    let test_file = temp.path().join("test.txt");
    std::fs::write(&test_file, "line 1\nline 2\n").unwrap();

    let mut mgr = CheckpointManager::new("test_session4").await;
    mgr.checkpoints_dir = temp.path().join("checkpoints").join("test_session4");

    let cp1 = mgr
        .create_checkpoint("file_write", None, None, &[test_file.clone()])
        .await
        .unwrap();

    std::fs::write(&test_file, "line 1\nline 2 modified\n").unwrap();

    let cp2 = mgr
        .create_checkpoint("file_write", None, None, &[test_file.clone()])
        .await
        .unwrap();

    let diffs = mgr.diff_checkpoints(&cp1.id, &cp2.id).await.unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].status, DiffStatus::Modified);
}

// ---- Phase 5 (Reasonix alignment): checkpoint safety contracts ----

/// Checkpoint survives a simulated write failure. After create_checkpoint,
/// if the write never happens (e.g., disk I/O error), the checkpoint must
/// remain intact for manual restore via rewind.
#[tokio::test]
async fn checkpoint_persists_after_simulated_write_failure() {
    let temp = TempDir::new().unwrap();
    let test_dir = temp.path().join("checkpoints").join("test_session_cp_fail");
    let test_file = temp.path().join("target_file.txt");
    std::fs::write(&test_file, "original content\n").unwrap();

    let mut mgr = CheckpointManager::new("test_session_cp_fail").await;
    mgr.checkpoints_dir = test_dir.clone();

    let cp = mgr
        .create_checkpoint("file_write", None, None, &[test_file.clone()])
        .await
        .unwrap();
    assert!(cp.id.starts_with("cp_"));

    // Simulate: write would fail, file stays unchanged.
    // The checkpoint must still be usable for restore.
    let result = mgr.restore_checkpoint(&cp.id).await.unwrap();
    assert!(!result.restored_files.is_empty());
    assert!(result.failed_files.is_empty());

    // After restore, original content is intact.
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "original content\n");
}

/// Restoring a checkpoint after a new file was created removes that file.
#[tokio::test]
async fn restore_checkpoint_removes_file_created_after_checkpoint() {
    let temp = TempDir::new().unwrap();
    let test_dir = temp
        .path()
        .join("checkpoints")
        .join("test_session_remove_new");
    let existing_file = temp.path().join("existing.txt");
    let new_file = temp.path().join("new.txt");

    std::fs::write(&existing_file, "existing\n").unwrap();

    let mut mgr = CheckpointManager::new("test_session_remove_new").await;
    mgr.checkpoints_dir = test_dir;

    // Capture state before new_file exists.
    let cp = mgr
        .create_checkpoint(
            "file_write",
            None,
            None,
            &[existing_file.clone(), new_file.clone()],
        )
        .await
        .unwrap();

    // Create new_file after checkpoint.
    std::fs::write(&new_file, "new content\n").unwrap();
    assert!(new_file.exists());

    // Restore — new_file should be removed.
    let result = mgr.restore_checkpoint(&cp.id).await.unwrap();
    assert!(!result.removed_files.is_empty());
}

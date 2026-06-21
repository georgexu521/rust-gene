use super::*;

#[tauri::command]
pub(crate) async fn revert_last_turn(session_id: String) -> Result<DesktopRevertResult, String> {
    let store = open_session_store()?;
    store
        .get_session(&session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;

    let manager = priority_agent::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let checkpoint_guard = manager.lock().await;
    let result = checkpoint_guard.revert_latest_assistant_turn().await?;
    let payload = serde_json::to_value(&result).map_err(|err| err.to_string())?;
    store
        .record_session_revert(&priority_agent::session_store::SessionRevertInsert {
            session_id: result.session_id.clone(),
            operation: "revert".to_string(),
            status: result.status.clone(),
            message_id: result.message_id.clone(),
            target_part_id: result.target_part_id.clone(),
            part_ids: result.part_ids.clone(),
            checkpoint_ids: result.checkpoint_ids.clone(),
            snapshot_checkpoint_id: result.snapshot_checkpoint_id.clone(),
            paths: result.paths.clone(),
            restored_files: result.restored_files.clone(),
            removed_files: result.removed_files.clone(),
            errors: result.errors.clone(),
            diff_summary: result.diff_summary.clone(),
            unrevert_possible: result.unrevert_possible,
            unreverted: false,
            payload: payload.clone(),
        })
        .map_err(|err| err.to_string())?;
    let writer =
        priority_agent::session_store::SessionEventWriter::new(store.shared_conn(), &session_id);
    writer
        .write_event("revert", &payload.to_string())
        .map_err(|err| err.to_string())?;

    Ok(DesktopRevertResult {
        session_id: result.session_id,
        status: result.status,
        message_id: result.message_id,
        part_ids: result.part_ids,
        tool_round_id: result.tool_round_id,
        file_change_ids: result.file_change_ids,
        checkpoint_ids: result.checkpoint_ids,
        paths: result.paths,
        restored_files: result.restored_files,
        removed_files: result.removed_files,
        errors: result.errors,
        change_count: result.change_count,
    })
}

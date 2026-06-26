use super::*;

#[tauri::command]
pub(crate) async fn select_project(
    path: String,
    state: State<'_, DesktopAppState>,
) -> Result<SelectedProject, String> {
    let project = validate_project_path(path)?;
    {
        let mut selected_project = state.selected_project.lock().await;
        *selected_project = project.clone();
    }
    {
        let mut recent_projects = state.recent_projects.lock().await;
        remember_recent_project(&mut recent_projects, project.clone());
    }
    {
        let mut runtime = state.runtime.lock().await;
        *runtime = None;
    }
    {
        let mut active_session_id = state.active_session_id.lock().await;
        *active_session_id = None;
    }
    {
        let mut workspace_trust = state.workspace_trust.lock().await;
        *workspace_trust = Some(desktop_workspace_trust_status(&project, None));
    }
    persist_current_settings(&state).await?;

    Ok(selected_project_response(project))
}

#[tauri::command]
pub(crate) async fn new_conversation(
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    {
        let mut runtime = state.runtime.lock().await;
        *runtime = None;
    }
    {
        let mut active_session_id = state.active_session_id.lock().await;
        *active_session_id = None;
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
pub(crate) async fn list_recent_sessions(
    limit: Option<i64>,
    state: State<'_, DesktopAppState>,
) -> Result<Vec<RecentSession>, String> {
    let store = open_session_store()?;
    let archived_session_ids = state.archived_session_ids.lock().await.clone();
    list_recent_sessions_from_store(&store, limit.unwrap_or(20), &archived_session_ids)
}

#[tauri::command]
pub(crate) async fn search_sessions(
    query: String,
    limit: Option<i64>,
    state: State<'_, DesktopAppState>,
) -> Result<Vec<RecentSession>, String> {
    let store = open_session_store()?;
    let archived_session_ids = state.archived_session_ids.lock().await.clone();
    search_sessions_from_store(&store, &query, limit.unwrap_or(20), &archived_session_ids)
}

#[tauri::command]
pub(crate) fn rename_session(session_id: String, title: String) -> Result<RecentSession, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("session title cannot be empty".to_string());
    }

    let store = open_session_store()?;
    store
        .get_session(&session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;
    store
        .update_session_title(&session_id, title)
        .map_err(|err| err.to_string())?;
    recent_session_from_store(&store, &session_id)
}

#[tauri::command]
pub(crate) async fn archive_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    {
        let mut archived_session_ids = state.archived_session_ids.lock().await;
        if !archived_session_ids.iter().any(|id| id == &session_id) {
            archived_session_ids.push(session_id.clone());
        }
    }
    clear_active_session_if_matches(&state, &session_id).await;
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
pub(crate) async fn restore_archived_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    {
        let mut archived_session_ids = state.archived_session_ids.lock().await;
        archived_session_ids.retain(|id| id != &session_id);
    }
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
pub(crate) async fn delete_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<DesktopSettingsResponse, String> {
    let store = open_session_store()?;
    store
        .delete_session(&session_id)
        .map_err(|err| err.to_string())?;
    {
        let mut archived_session_ids = state.archived_session_ids.lock().await;
        archived_session_ids.retain(|id| id != &session_id);
    }
    clear_active_session_if_matches(&state, &session_id).await;
    persist_current_settings(&state).await?;
    desktop_settings(state).await
}

#[tauri::command]
pub(crate) fn load_session_messages(session_id: String) -> Result<Vec<DesktopMessage>, String> {
    let store = open_session_store()?;
    load_messages_from_store(&store, &session_id)
}

#[tauri::command]
pub(crate) async fn resume_session(
    session_id: String,
    state: State<'_, DesktopAppState>,
) -> Result<ResumedSession, String> {
    let store = open_session_store()?;
    store
        .get_session(&session_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;
    let messages = load_messages_from_store(&store, &session_id)?;
    let compact_boundaries = load_compact_boundaries_from_store(&store, &session_id)?;
    let session_parts = load_session_parts_from_store(&store, &session_id)?;
    let selected_project = state.selected_project.lock().await.clone();
    let runtime = DesktopRuntime::initialize_for_session(&selected_project, &session_id)
        .await
        .map_err(|err| err.to_string())?;
    let permission_mode_label =
        normalized_permission_mode_label(state.permission_mode.lock().await.as_deref());
    runtime
        .streaming_engine()
        .set_permission_mode(parse_desktop_permission_mode(permission_mode_label));
    let provider_name = state.provider_name.lock().await.clone();
    let model = state.model.lock().await.clone();
    apply_desktop_provider_model(&runtime, provider_name.as_deref(), model.as_deref())?;

    {
        let mut stored_runtime = state.runtime.lock().await;
        *stored_runtime = Some(runtime);
    }
    {
        let mut active_session_id = state.active_session_id.lock().await;
        *active_session_id = Some(session_id.clone());
    }
    persist_current_settings(&state).await?;

    Ok(ResumedSession {
        session_id,
        messages,
        compact_boundaries,
        session_parts,
    })
}

#[tauri::command]
pub(crate) fn list_session_reverts(
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<priority_agent::session_store::SessionRevertRecord>, String> {
    let store = open_session_store()?;
    store
        .list_session_reverts(&session_id, limit.unwrap_or(20).clamp(1, 100))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub(crate) fn desktop_tool_output_index(
    session_id: String,
) -> Result<Vec<DesktopToolOutputMeta>, String> {
    let store = priority_agent::tool_output_store::ToolOutputStore::new();
    let metas = store
        .list_for_session(&session_id)
        .map_err(|err| err.to_string())?;
    Ok(metas
        .into_iter()
        .map(|meta| DesktopToolOutputMeta {
            id: meta.id.clone(),
            uri: meta.uri(),
            tool_call_id: meta.tool_call_id,
            tool_name: meta.tool_name,
            mime: meta.mime,
            original_bytes: meta.original_bytes,
            created_at_ms: meta.created_at_ms,
        })
        .collect())
}

#[tauri::command]
pub(crate) fn desktop_tool_output_page(
    session_id: String,
    id_or_uri: String,
    offset: Option<u64>,
    limit: Option<u64>,
) -> Result<DesktopToolOutputPage, String> {
    let store = priority_agent::tool_output_store::ToolOutputStore::new();
    let page = store
        .read_page(
            &session_id,
            &id_or_uri,
            offset.unwrap_or(0),
            limit.unwrap_or(64 * 1024),
        )
        .map_err(|err| err.to_string())?;
    let meta = store.read_meta(&id_or_uri).map_err(|err| err.to_string())?;
    Ok(DesktopToolOutputPage {
        id: meta.id.clone(),
        uri: meta.uri(),
        tool_name: meta.tool_name,
        mime: meta.mime,
        content: page.content,
        offset: page.offset,
        limit: page.limit,
        total_bytes: page.total_bytes,
        has_more: page.has_more,
    })
}

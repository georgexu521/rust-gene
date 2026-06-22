//! Session-store support module.
//!
//! Owns one slice of durable session persistence so message, trace, learning, revert, and compact state stay separated.

use rusqlite::{params, Result as SqlResult, Row};

use super::{AgentArtifactRecord, AgentTaskStateRecord, AgentTaskStateUpsert, SessionStore};

impl SessionStore {
    #[allow(clippy::too_many_arguments)]
    pub fn add_agent_artifact(
        &self,
        session_id: &str,
        agent_id: &str,
        profile: Option<&str>,
        role: &str,
        status: &str,
        description: &str,
        output: &str,
        payload: &serde_json::Value,
    ) -> SqlResult<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO agent_artifacts
             (session_id, agent_id, profile, role, status, description, output, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session_id,
                agent_id,
                profile,
                role,
                status,
                description,
                output,
                payload.to_string()
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn recent_agent_artifacts(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<AgentArtifactRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, agent_id, profile, role, status, description, output, payload, created_at
             FROM agent_artifacts
             WHERE session_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], |row| {
            let payload_text: String = row.get(8)?;
            Ok(AgentArtifactRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                agent_id: row.get(2)?,
                profile: row.get(3)?,
                role: row.get(4)?,
                status: row.get(5)?,
                description: row.get(6)?,
                output: row.get(7)?,
                payload: serde_json::from_str(&payload_text)
                    .unwrap_or_else(|_| serde_json::json!({})),
                created_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    pub fn agent_artifact(
        &self,
        session_id: &str,
        artifact_id: i64,
    ) -> SqlResult<Option<AgentArtifactRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, agent_id, profile, role, status, description, output, payload, created_at
             FROM agent_artifacts
             WHERE session_id = ?1 AND id = ?2
             LIMIT 1",
            params![session_id, artifact_id],
            |row| {
                let payload_text: String = row.get(8)?;
                Ok(AgentArtifactRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    agent_id: row.get(2)?,
                    profile: row.get(3)?,
                    role: row.get(4)?,
                    status: row.get(5)?,
                    description: row.get(6)?,
                    output: row.get(7)?,
                    payload: serde_json::from_str(&payload_text)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    created_at: row.get(9)?,
                })
            },
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn upsert_agent_task_state(&self, state: &AgentTaskStateUpsert) -> SqlResult<()> {
        let conn = self.conn();
        let tool_ids =
            serde_json::to_string(&state.tool_ids_in_progress).unwrap_or_else(|_| "[]".to_string());
        let permission_requests =
            serde_json::to_string(&state.permission_requests).unwrap_or_else(|_| "[]".to_string());
        let cleanup_hooks =
            serde_json::to_string(&state.cleanup_hooks).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO agent_task_states
             (session_id, task_id, agent_id, profile, role, status, description, transcript_path,
              tool_ids_in_progress, permission_requests, result_artifact_id, cleanup_hooks, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(session_id, task_id) DO UPDATE SET
                agent_id = excluded.agent_id,
                profile = excluded.profile,
                role = excluded.role,
                status = excluded.status,
                description = excluded.description,
                transcript_path = excluded.transcript_path,
                tool_ids_in_progress = excluded.tool_ids_in_progress,
                permission_requests = excluded.permission_requests,
                result_artifact_id = excluded.result_artifact_id,
                cleanup_hooks = excluded.cleanup_hooks,
                payload = excluded.payload,
                updated_at = datetime('now')",
            params![
                &state.session_id,
                &state.task_id,
                &state.agent_id,
                state.profile.as_deref(),
                &state.role,
                &state.status,
                &state.description,
                state.transcript_path.as_deref(),
                tool_ids,
                permission_requests,
                state.result_artifact_id,
                cleanup_hooks,
                state.payload.to_string()
            ],
        )?;
        Ok(())
    }

    pub fn recent_agent_task_states(
        &self,
        session_id: &str,
        limit: i64,
    ) -> SqlResult<Vec<AgentTaskStateRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, task_id, agent_id, profile, role, status, description,
                    transcript_path, tool_ids_in_progress, permission_requests,
                    result_artifact_id, cleanup_hooks, payload, created_at, updated_at
             FROM agent_task_states
             WHERE session_id = ?1
             ORDER BY updated_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], |row| {
            let tool_ids: String = row.get(9)?;
            let permission_requests: String = row.get(10)?;
            let cleanup_hooks: String = row.get(12)?;
            let payload_text: String = row.get(13)?;
            Ok(AgentTaskStateRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                task_id: row.get(2)?,
                agent_id: row.get(3)?,
                profile: row.get(4)?,
                role: row.get(5)?,
                status: row.get(6)?,
                description: row.get(7)?,
                transcript_path: row.get(8)?,
                tool_ids_in_progress: serde_json::from_str(&tool_ids).unwrap_or_default(),
                permission_requests: serde_json::from_str(&permission_requests).unwrap_or_default(),
                result_artifact_id: row.get(11)?,
                cleanup_hooks: serde_json::from_str(&cleanup_hooks).unwrap_or_default(),
                payload: serde_json::from_str(&payload_text)
                    .unwrap_or_else(|_| serde_json::json!({})),
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        })?;
        rows.collect()
    }

    pub fn agent_task_state(
        &self,
        session_id: &str,
        agent_id_or_task_id: &str,
    ) -> SqlResult<Option<AgentTaskStateRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, task_id, agent_id, profile, role, status, description,
                    transcript_path, tool_ids_in_progress, permission_requests,
                    result_artifact_id, cleanup_hooks, payload, created_at, updated_at
             FROM agent_task_states
             WHERE session_id = ?1 AND (agent_id = ?2 OR task_id = ?2)
             ORDER BY updated_at DESC, id DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![session_id, agent_id_or_task_id])?;
        if let Some(row) = rows.next()? {
            let tool_ids: String = row.get(9)?;
            let permission_requests: String = row.get(10)?;
            let cleanup_hooks: String = row.get(12)?;
            let payload_text: String = row.get(13)?;
            Ok(Some(AgentTaskStateRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                task_id: row.get(2)?,
                agent_id: row.get(3)?,
                profile: row.get(4)?,
                role: row.get(5)?,
                status: row.get(6)?,
                description: row.get(7)?,
                transcript_path: row.get(8)?,
                tool_ids_in_progress: serde_json::from_str(&tool_ids).unwrap_or_default(),
                permission_requests: serde_json::from_str(&permission_requests).unwrap_or_default(),
                result_artifact_id: row.get(11)?,
                cleanup_hooks: serde_json::from_str(&cleanup_hooks).unwrap_or_default(),
                payload: serde_json::from_str(&payload_text).unwrap_or_default(),
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn recover_interrupted_agent_task_states(
        &self,
        session_id: Option<&str>,
    ) -> SqlResult<usize> {
        let states = self.interrupted_agent_task_states(session_id)?;
        let mut recovered = 0;
        for state in states {
            let mut payload = state.payload.clone();
            payload["previous_status"] = serde_json::json!(state.status);
            payload["recovery_status"] = serde_json::json!("paused_restart");
            payload["recovery_reason"] = serde_json::json!(
                "runtime process restarted before the background sub-agent completion sink observed a result"
            );
            payload["recovery_action"] = serde_json::json!(
                "read the task by task_id, then relaunch or cancel it explicitly"
            );
            let upsert = AgentTaskStateUpsert {
                session_id: state.session_id,
                task_id: state.task_id,
                agent_id: state.agent_id,
                profile: state.profile,
                role: state.role,
                status: "paused_restart".to_string(),
                description: state.description,
                transcript_path: state.transcript_path,
                tool_ids_in_progress: Vec::new(),
                permission_requests: state.permission_requests,
                result_artifact_id: state.result_artifact_id,
                cleanup_hooks: state.cleanup_hooks,
                payload,
            };
            self.upsert_agent_task_state(&upsert)?;
            recovered += 1;
        }
        Ok(recovered)
    }

    fn interrupted_agent_task_states(
        &self,
        session_id: Option<&str>,
    ) -> SqlResult<Vec<AgentTaskStateRecord>> {
        let conn = self.conn();
        let sql = if session_id.is_some() {
            "SELECT id, session_id, task_id, agent_id, profile, role, status, description,
                    transcript_path, tool_ids_in_progress, permission_requests,
                    result_artifact_id, cleanup_hooks, payload, created_at, updated_at
             FROM agent_task_states
             WHERE session_id = ?1 AND status IN ('running', 'stopping')
             ORDER BY updated_at DESC, id DESC"
        } else {
            "SELECT id, session_id, task_id, agent_id, profile, role, status, description,
                    transcript_path, tool_ids_in_progress, permission_requests,
                    result_artifact_id, cleanup_hooks, payload, created_at, updated_at
             FROM agent_task_states
             WHERE status IN ('running', 'stopping')
             ORDER BY updated_at DESC, id DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        if let Some(session_id) = session_id {
            let rows = stmt.query_map(params![session_id], agent_task_state_from_row)?;
            rows.collect()
        } else {
            let rows = stmt.query_map([], agent_task_state_from_row)?;
            rows.collect()
        }
    }
}

fn agent_task_state_from_row(row: &Row<'_>) -> SqlResult<AgentTaskStateRecord> {
    let tool_ids: String = row.get(9)?;
    let permission_requests: String = row.get(10)?;
    let cleanup_hooks: String = row.get(12)?;
    let payload_text: String = row.get(13)?;
    Ok(AgentTaskStateRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        task_id: row.get(2)?,
        agent_id: row.get(3)?,
        profile: row.get(4)?,
        role: row.get(5)?,
        status: row.get(6)?,
        description: row.get(7)?,
        transcript_path: row.get(8)?,
        tool_ids_in_progress: serde_json::from_str(&tool_ids).unwrap_or_default(),
        permission_requests: serde_json::from_str(&permission_requests).unwrap_or_default(),
        result_artifact_id: row.get(11)?,
        cleanup_hooks: serde_json::from_str(&cleanup_hooks).unwrap_or_default(),
        payload: serde_json::from_str(&payload_text).unwrap_or_default(),
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

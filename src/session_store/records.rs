use serde::{Deserialize, Serialize};

/// 消息记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub reasoning: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInsert {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl From<crate::services::api::Message> for MessageInsert {
    fn from(msg: crate::services::api::Message) -> Self {
        match msg {
            crate::services::api::Message::System { content }
            | crate::services::api::Message::User { content } => MessageInsert {
                role: "user".to_string(),
                content,
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            },
            crate::services::api::Message::Assistant {
                content,
                tool_calls,
            } => MessageInsert {
                role: "assistant".to_string(),
                content,
                tool_calls: tool_calls.map(|tc| serde_json::to_value(tc).unwrap_or_default()),
                tool_call_id: None,
                metadata: None,
            },
            crate::services::api::Message::Tool {
                content,
                tool_call_id,
            } => MessageInsert {
                role: "tool".to_string(),
                content,
                tool_calls: None,
                tool_call_id: Some(tool_call_id),
                metadata: None,
            },
        }
    }
}

/// 会话记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub parent_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub workspace_root: Option<String>,
}

/// Durable event extracted from completed turns for future routing/tool tuning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningEventRecord {
    pub id: i64,
    pub session_id: String,
    pub kind: String,
    pub source: String,
    pub summary: String,
    pub confidence: f64,
    pub payload: serde_json::Value,
    pub created_at: String,
}

/// Durable compact boundary produced when earlier context is summarized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundaryRecord {
    pub id: i64,
    pub session_id: String,
    pub boundary_id: String,
    pub sequence: Option<i64>,
    pub strategy: String,
    pub trigger: Option<String>,
    pub before_tokens: i64,
    pub after_tokens: i64,
    pub messages_before: i64,
    pub messages_after: i64,
    pub preserved_tail_count: Option<i64>,
    pub retained_items: serde_json::Value,
    pub provenance: serde_json::Value,
    pub summary: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundaryInsert {
    pub session_id: String,
    pub boundary_id: String,
    pub sequence: Option<i64>,
    pub strategy: String,
    pub trigger: Option<String>,
    pub before_tokens: i64,
    pub after_tokens: i64,
    pub messages_before: i64,
    pub messages_after: i64,
    pub preserved_tail_count: Option<i64>,
    pub retained_items: serde_json::Value,
    pub provenance: serde_json::Value,
    pub summary: String,
    pub payload: serde_json::Value,
}

/// Durable revert state for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRevertRecord {
    pub id: i64,
    pub session_id: String,
    pub operation: String,
    pub status: String,
    pub message_id: Option<String>,
    pub target_part_id: Option<String>,
    pub part_ids: Vec<String>,
    pub checkpoint_ids: Vec<String>,
    pub snapshot_checkpoint_id: Option<String>,
    pub paths: Vec<String>,
    pub restored_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub errors: Vec<String>,
    pub diff_summary: Option<String>,
    pub unrevert_possible: bool,
    pub unreverted: bool,
    pub payload: serde_json::Value,
    pub created_at: String,
}

/// Insert payload for durable revert state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRevertInsert {
    pub session_id: String,
    pub operation: String,
    pub status: String,
    pub message_id: Option<String>,
    pub target_part_id: Option<String>,
    pub part_ids: Vec<String>,
    pub checkpoint_ids: Vec<String>,
    pub snapshot_checkpoint_id: Option<String>,
    pub paths: Vec<String>,
    pub restored_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub errors: Vec<String>,
    pub diff_summary: Option<String>,
    pub unrevert_possible: bool,
    pub unreverted: bool,
    pub payload: serde_json::Value,
}

/// Durable subagent result artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArtifactRecord {
    pub id: i64,
    pub session_id: String,
    pub agent_id: String,
    pub profile: Option<String>,
    pub role: String,
    pub status: String,
    pub description: String,
    pub output: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

/// Durable subagent task state for background/runtime panels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskStateRecord {
    pub id: i64,
    pub session_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub profile: Option<String>,
    pub role: String,
    pub status: String,
    pub description: String,
    pub transcript_path: Option<String>,
    pub tool_ids_in_progress: Vec<String>,
    pub permission_requests: Vec<String>,
    pub result_artifact_id: Option<i64>,
    pub cleanup_hooks: Vec<String>,
    pub payload: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

/// Upsert payload for durable subagent task state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskStateUpsert {
    pub session_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub profile: Option<String>,
    pub role: String,
    pub status: String,
    pub description: String,
    pub transcript_path: Option<String>,
    pub tool_ids_in_progress: Vec<String>,
    pub permission_requests: Vec<String>,
    pub result_artifact_id: Option<i64>,
    pub cleanup_hooks: Vec<String>,
    pub payload: serde_json::Value,
}

/// Durable goal run record, mapping to the `goal_runs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalRunRecord {
    pub id: String,
    pub session_id: String,
    pub objective: String,
    pub status: String,
    pub stop_rules_json: Option<String>,
    pub budget_json: Option<String>,
    pub turn_count: i64,
    pub last_closeout_status: Option<String>,
    pub last_blocker: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Durable goal step record, mapping to the `goal_steps` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStepRecord {
    pub id: String,
    pub goal_id: String,
    pub session_id: String,
    pub turn_index: i64,
    pub prompt: String,
    pub closeout_status: Option<String>,
    pub verification_status: Option<String>,
    pub changed_files: i64,
    pub validation_items: i64,
    pub decision: String,
    pub summary: Option<String>,
    pub score: Option<f64>,
    pub created_at: String,
}

/// Insert payload for creating a goal step.
#[derive(Debug, Clone)]
pub struct GoalStepInsert {
    pub id: String,
    pub goal_id: String,
    pub session_id: String,
    pub turn_index: i64,
    pub prompt: String,
    pub closeout_status: Option<String>,
    pub verification_status: Option<String>,
    pub changed_files: i64,
    pub validation_items: i64,
    pub decision: String,
    pub summary: String,
    pub score: Option<f64>,
}

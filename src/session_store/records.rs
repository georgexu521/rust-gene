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
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInsert {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
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

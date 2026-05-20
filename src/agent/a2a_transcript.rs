//! Durable A2A transcript for agent handoffs.
//!
//! This is intentionally JSONL-based so it can be appended from tools without
//! requiring a database migration. The session database can ingest it later.

use crate::agent::envelope::{AgentTaskEnvelope, AgentTaskStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTranscriptRecord {
    pub envelope_id: String,
    pub status: AgentTaskStatus,
    pub from: String,
    pub to: Option<String>,
    pub goal: String,
    pub artifacts: usize,
    pub error: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

impl A2aTranscriptRecord {
    pub fn from_envelope(envelope: &AgentTaskEnvelope) -> Self {
        Self {
            envelope_id: envelope.envelope_id.clone(),
            status: envelope.status,
            from: envelope.from.0.clone(),
            to: envelope.to.as_ref().map(|id| id.0.clone()),
            goal: envelope.goal.clone(),
            artifacts: envelope.produced_artifacts.len(),
            error: envelope.error.as_ref().map(|err| err.message.clone()),
            recorded_at: Utc::now(),
        }
    }
}

pub fn append_envelope(envelope: &AgentTaskEnvelope) -> std::io::Result<()> {
    let path = transcript_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let record = A2aTranscriptRecord::from_envelope(envelope);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let json = serde_json::to_string(&record).unwrap_or_else(|_| "{}".to_string());
    writeln!(file, "{}", json)?;
    Ok(())
}

pub fn read_recent(limit: usize) -> std::io::Result<Vec<A2aTranscriptRecord>> {
    let path = transcript_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let mut records = content
        .lines()
        .rev()
        .take(limit)
        .filter_map(|line| serde_json::from_str::<A2aTranscriptRecord>(line).ok())
        .collect::<Vec<_>>();
    records.reverse();
    Ok(records)
}

pub fn transcript_path() -> PathBuf {
    if let Ok(path) = std::env::var("PRIORITY_AGENT_A2A_TRANSCRIPT_PATH") {
        return PathBuf::from(path);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("a2a-transcript.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::envelope::AgentTaskEnvelope;
    use crate::agent::types::AgentId;

    #[test]
    fn transcript_record_from_envelope() {
        let mut envelope = AgentTaskEnvelope::new(AgentId("parent".into()), "review", "check src")
            .assign_to(AgentId("child".into()));
        envelope.mark_running("started");
        let record = A2aTranscriptRecord::from_envelope(&envelope);
        assert_eq!(record.status, AgentTaskStatus::Running);
        assert_eq!(record.from, "parent");
        assert_eq!(record.to.as_deref(), Some("child"));
    }
}

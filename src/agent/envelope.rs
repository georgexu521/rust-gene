//! A2A-inspired agent task envelope.
//!
//! This normalizes task handoff between parent agents, sub-agents, swarm
//! workers, and teammate messages.

use crate::agent::types::AgentId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskPriority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Created,
    Assigned,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArtifact {
    pub kind: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskEnvelope {
    pub envelope_id: String,
    pub parent_task_id: Option<String>,
    pub from: AgentId,
    pub to: Option<AgentId>,
    pub priority: AgentTaskPriority,
    pub status: AgentTaskStatus,
    pub goal: String,
    pub prompt: String,
    pub context_refs: Vec<String>,
    pub expected_artifacts: Vec<String>,
    pub produced_artifacts: Vec<AgentArtifact>,
    pub constraints: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentTaskEnvelope {
    pub fn new(from: AgentId, goal: impl Into<String>, prompt: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            envelope_id: uuid::Uuid::new_v4().to_string(),
            parent_task_id: None,
            from,
            to: None,
            priority: AgentTaskPriority::Normal,
            status: AgentTaskStatus::Created,
            goal: goal.into(),
            prompt: prompt.into(),
            context_refs: Vec::new(),
            expected_artifacts: Vec::new(),
            produced_artifacts: Vec::new(),
            constraints: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn assign_to(mut self, to: AgentId) -> Self {
        self.to = Some(to);
        self.status = AgentTaskStatus::Assigned;
        self.updated_at = Utc::now();
        self
    }

    pub fn with_priority(mut self, priority: AgentTaskPriority) -> Self {
        self.priority = priority;
        self.updated_at = Utc::now();
        self
    }

    pub fn add_context_ref(&mut self, reference: impl Into<String>) {
        push_unique(&mut self.context_refs, reference.into());
        self.updated_at = Utc::now();
    }

    pub fn add_expected_artifact(&mut self, artifact: impl Into<String>) {
        push_unique(&mut self.expected_artifacts, artifact.into());
        self.updated_at = Utc::now();
    }

    pub fn add_constraint(&mut self, constraint: impl Into<String>) {
        push_unique(&mut self.constraints, constraint.into());
        self.updated_at = Utc::now();
    }

    pub fn add_artifact(&mut self, artifact: AgentArtifact) {
        self.produced_artifacts.push(artifact);
        self.updated_at = Utc::now();
    }

    pub fn validate_for_assignment(&self) -> Result<(), String> {
        if self.goal.trim().is_empty() {
            return Err("goal is required".to_string());
        }
        if self.prompt.trim().is_empty() {
            return Err("prompt is required".to_string());
        }
        if self.to.is_none() {
            return Err("recipient agent is required".to_string());
        }
        Ok(())
    }

    pub fn compact_summary(&self) -> String {
        format!(
            "{} {:?} {:?}: {}",
            &self.envelope_id[..8.min(self.envelope_id.len())],
            self.priority,
            self.status,
            self.goal
        )
    }
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !items.contains(&value) {
        items.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_requires_recipient_for_assignment() {
        let env = AgentTaskEnvelope::new(AgentId::new(), "review code", "check src");
        assert!(env.validate_for_assignment().is_err());
    }

    #[test]
    fn envelope_validates_after_assignment() {
        let env = AgentTaskEnvelope::new(AgentId::new(), "review code", "check src")
            .assign_to(AgentId::new());
        assert!(env.validate_for_assignment().is_ok());
        assert_eq!(env.status, AgentTaskStatus::Assigned);
    }

    #[test]
    fn envelope_deduplicates_context() {
        let mut env = AgentTaskEnvelope::new(AgentId::new(), "goal", "prompt");
        env.add_context_ref("src/main.rs");
        env.add_context_ref("src/main.rs");
        env.add_expected_artifact("report");
        env.add_expected_artifact("report");
        assert_eq!(env.context_refs.len(), 1);
        assert_eq!(env.expected_artifacts.len(), 1);
    }
}

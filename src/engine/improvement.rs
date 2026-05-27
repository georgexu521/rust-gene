//! Controlled self-evolution proposals.
//!
//! Runtime learning can suggest improvements, but proposals are explicit,
//! inspectable, and gated by user approval before they are applied.

use crate::engine::intent_router::RiskLevel;
use crate::session_store::LearningEventRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementTarget {
    Memory,
    Skill,
    Prompt,
    Routing,
    ToolGuidance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Proposed,
    Accepted,
    Rejected,
    Applied,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalEvalStatus {
    Pending,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementProposal {
    pub id: String,
    pub trigger_event_ids: Vec<i64>,
    pub target: ImprovementTarget,
    pub proposed_change: String,
    pub expected_benefit: String,
    pub risk: RiskLevel,
    pub validation: Vec<String>,
    #[serde(default = "default_proposal_eval_status")]
    pub eval_status: ProposalEvalStatus,
    #[serde(default)]
    pub eval_summary: Option<String>,
    #[serde(default)]
    pub evalset_bindings: Vec<String>,
    pub status: ProposalStatus,
    pub evidence: Vec<String>,
    #[serde(default = "default_rollback_plan")]
    pub rollback_plan: String,
    #[serde(default)]
    pub applied_ref: Option<String>,
    #[serde(default)]
    pub rollback_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ImprovementStore {
    path: PathBuf,
}

impl ImprovementStore {
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("improvements.jsonl")
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(&self) -> Vec<ImprovementProposal> {
        read_latest_proposals(&self.path)
    }

    pub fn get(&self, id_or_prefix: &str) -> Option<ImprovementProposal> {
        self.list()
            .into_iter()
            .find(|proposal| proposal.id == id_or_prefix || proposal.id.starts_with(id_or_prefix))
    }

    pub fn upsert(&self, proposal: &ImprovementProposal) -> anyhow::Result<()> {
        append_jsonl(&self.path, proposal)
    }

    pub fn update_status(
        &self,
        id_or_prefix: &str,
        status: ProposalStatus,
    ) -> anyhow::Result<Option<ImprovementProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        if status == ProposalStatus::Applied && proposal.eval_status != ProposalEvalStatus::Passed {
            return Err(anyhow::anyhow!(
                "proposal must pass eval before apply; run /improvements eval {} first",
                proposal.id
            ));
        }
        proposal.status = status;
        match status {
            ProposalStatus::Applied => {
                proposal.applied_ref = Some(format!("manual:/improvements apply {}", proposal.id));
                proposal.rollback_ref = None;
            }
            ProposalStatus::RolledBack => {
                proposal.rollback_ref =
                    Some(format!("manual:/improvements rollback {}", proposal.id));
            }
            ProposalStatus::Proposed | ProposalStatus::Accepted | ProposalStatus::Rejected => {}
        }
        proposal.updated_at = chrono::Utc::now().to_rfc3339();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn record_eval(
        &self,
        id_or_prefix: &str,
        status: ProposalEvalStatus,
        summary: impl Into<String>,
    ) -> anyhow::Result<Option<ImprovementProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        proposal.eval_status = status;
        proposal.eval_summary = Some(summary.into());
        proposal.updated_at = chrono::Utc::now().to_rfc3339();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn bind_evalset(
        &self,
        id_or_prefix: &str,
        evalset_name: &str,
    ) -> anyhow::Result<Option<ImprovementProposal>> {
        let Some(mut proposal) = self.get(id_or_prefix) else {
            return Ok(None);
        };
        if !proposal
            .evalset_bindings
            .iter()
            .any(|binding| binding == evalset_name)
        {
            proposal.evalset_bindings.push(evalset_name.to_string());
        }
        proposal.updated_at = chrono::Utc::now().to_rfc3339();
        self.upsert(&proposal)?;
        Ok(Some(proposal))
    }

    pub fn propose_from_learning_events(
        &self,
        events: &[LearningEventRecord],
    ) -> anyhow::Result<Vec<ImprovementProposal>> {
        let existing_keys = self
            .list()
            .into_iter()
            .map(|proposal| proposal.dedupe_key())
            .collect::<std::collections::HashSet<_>>();
        let mut proposals = Vec::new();

        for proposal in generate_improvement_proposals(events) {
            if existing_keys.contains(&proposal.dedupe_key()) {
                continue;
            }
            self.upsert(&proposal)?;
            proposals.push(proposal);
        }
        Ok(proposals)
    }
}

impl Default for ImprovementStore {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

impl ImprovementProposal {
    fn dedupe_key(&self) -> String {
        format!("{:?}:{}", self.target, self.proposed_change.to_lowercase())
    }

    pub fn lifecycle_stage(&self) -> &'static str {
        match self.status {
            ProposalStatus::Proposed if self.eval_status == ProposalEvalStatus::Pending => {
                "proposal"
            }
            ProposalStatus::Proposed => "eval",
            ProposalStatus::Accepted => "accept",
            ProposalStatus::Applied => "apply",
            ProposalStatus::RolledBack => "rollback",
            ProposalStatus::Rejected => "rejected",
        }
    }
}

pub fn generate_improvement_proposals(events: &[LearningEventRecord]) -> Vec<ImprovementProposal> {
    let mut proposals = Vec::new();
    let mut failed_tools: HashMap<String, Vec<&LearningEventRecord>> = HashMap::new();
    let mut recovery_events = Vec::new();
    let mut correction_events = Vec::new();

    for event in events {
        if event.kind == "tool_outcome" && event.payload["success"].as_bool() == Some(false) {
            if let Some(tool) = event.payload["tool"].as_str() {
                failed_tools
                    .entry(tool.to_string())
                    .or_default()
                    .push(event);
            }
        }
        if event.kind == "recovery_plan" || event.kind.contains("guided_debug") {
            recovery_events.push(event);
        }
        let summary = event.summary.to_lowercase();
        if event.kind.contains("feedback")
            || summary.contains("correction")
            || summary.contains("用户纠正")
            || summary.contains("wrong")
        {
            correction_events.push(event);
        }
    }

    for (tool, failures) in failed_tools {
        if failures.len() < 2 {
            continue;
        }
        proposals.push(ImprovementProposal::new(
            failures.iter().map(|event| event.id).collect(),
            ImprovementTarget::ToolGuidance,
            format!(
                "Add guidance for repeated {} failures: inspect arguments, preconditions, and recovery path before retrying.",
                tool
            ),
            "Reduce repeated tool failures and noisy retry loops.",
            RiskLevel::Medium,
            vec![
                "Run targeted tool failure regression tests.".to_string(),
                "Confirm future traces show fewer repeated failures for this tool.".to_string(),
            ],
            failures
                .iter()
                .take(5)
                .map(|event| format!("#{} {}", event.id, event.summary))
                .collect(),
        ));
    }

    if recovery_events.len() >= 2 {
        proposals.push(ImprovementProposal::new(
            recovery_events.iter().map(|event| event.id).collect(),
            ImprovementTarget::Routing,
            "Increase caution and retrieval depth when recent turns needed recovery plans."
                .to_string(),
            "Route hard tasks toward more context before acting.",
            RiskLevel::Medium,
            vec![
                "Run intent routing evalset.".to_string(),
                "Verify simple requests still choose lightweight routing.".to_string(),
            ],
            recovery_events
                .iter()
                .take(5)
                .map(|event| format!("#{} {}", event.id, event.summary))
                .collect(),
        ));
    }

    if let Some(event) = correction_events.first() {
        proposals.push(ImprovementProposal::new(
            vec![event.id],
            ImprovementTarget::Memory,
            format!("Review user correction for memory: {}", event.summary),
            "Preserve explicit user correction for future turns.",
            RiskLevel::Low,
            vec![
                "Safety scan correction before saving.".to_string(),
                "Check duplicate/conflicting memory entries.".to_string(),
            ],
            vec![format!("#{} {}", event.id, event.summary)],
        ));
    }

    proposals
}

impl ImprovementProposal {
    fn new(
        trigger_event_ids: Vec<i64>,
        target: ImprovementTarget,
        proposed_change: String,
        expected_benefit: impl Into<String>,
        risk: RiskLevel,
        validation: Vec<String>,
        evidence: Vec<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let id = stable_proposal_id(&target, &proposed_change);
        Self {
            id,
            trigger_event_ids,
            target,
            proposed_change,
            expected_benefit: expected_benefit.into(),
            risk,
            validation,
            eval_status: ProposalEvalStatus::Pending,
            eval_summary: None,
            evalset_bindings: Vec::new(),
            status: ProposalStatus::Proposed,
            evidence,
            rollback_plan: default_rollback_plan(),
            applied_ref: None,
            rollback_ref: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

fn default_proposal_eval_status() -> ProposalEvalStatus {
    ProposalEvalStatus::Pending
}

fn default_rollback_plan() -> String {
    "Keep the change as an inspectable proposal until eval and explicit apply; rollback records a rolled_back proposal state and audit event instead of letting the model mutate long-term behavior directly."
        .to_string()
}

fn stable_proposal_id(target: &ImprovementTarget, proposed_change: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    target.hash(&mut hasher);
    proposed_change.to_lowercase().hash(&mut hasher);
    format!("imp_{:016x}", hasher.finish())
}

fn read_latest_proposals(path: &Path) -> Vec<ImprovementProposal> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut latest = HashMap::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(proposal) = serde_json::from_str::<ImprovementProposal>(line) else {
            continue;
        };
        latest.insert(proposal.id.clone(), proposal);
    }
    let mut proposals = latest.into_values().collect::<Vec<_>>();
    proposals.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    proposals
}

fn append_jsonl(path: &Path, proposal: &ImprovementProposal) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(proposal)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        id: i64,
        kind: &str,
        summary: &str,
        payload: serde_json::Value,
    ) -> LearningEventRecord {
        LearningEventRecord {
            id,
            session_id: "s1".to_string(),
            kind: kind.to_string(),
            source: "test".to_string(),
            summary: summary.to_string(),
            confidence: 0.8,
            payload,
            created_at: "2026-04-27T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn repeated_tool_failures_create_tool_guidance_proposal() {
        let events = vec![
            event(
                1,
                "tool_outcome",
                "Tool bash failed",
                serde_json::json!({"tool": "bash", "success": false}),
            ),
            event(
                2,
                "tool_outcome",
                "Tool bash failed again",
                serde_json::json!({"tool": "bash", "success": false}),
            ),
        ];

        let proposals = generate_improvement_proposals(&events);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].target, ImprovementTarget::ToolGuidance);
        assert_eq!(proposals[0].risk, RiskLevel::Medium);
        assert_eq!(proposals[0].trigger_event_ids, vec![1, 2]);
    }

    #[test]
    fn store_updates_status_by_prefix() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-improvements-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = ImprovementStore::new(path.clone());
        let proposal = ImprovementProposal::new(
            vec![1],
            ImprovementTarget::Memory,
            "Remember compact CLI preference.".to_string(),
            "Better future answers.",
            RiskLevel::Low,
            vec!["Review memory.".to_string()],
            vec!["evidence".to_string()],
        );
        store.upsert(&proposal).unwrap();
        let short = &proposal.id[..10];
        store
            .record_eval(short, ProposalEvalStatus::Passed, "manual test passed")
            .unwrap();
        let updated = store
            .update_status(short, ProposalStatus::Accepted)
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, ProposalStatus::Accepted);
        assert_eq!(store.list()[0].status, ProposalStatus::Accepted);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn apply_requires_passed_eval_and_records_rollback_refs() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-improvements-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = ImprovementStore::new(path.clone());
        let proposal = ImprovementProposal::new(
            vec![1],
            ImprovementTarget::ToolGuidance,
            "Improve repeated bash failure guidance.".to_string(),
            "Better recovery.",
            RiskLevel::Medium,
            vec!["Run tool regression.".to_string()],
            vec!["evidence".to_string()],
        );
        store.upsert(&proposal).unwrap();
        assert!(store
            .update_status(&proposal.id, ProposalStatus::Applied)
            .is_err());
        store
            .record_eval(&proposal.id, ProposalEvalStatus::Passed, "preflight passed")
            .unwrap();
        let applied = store
            .update_status(&proposal.id, ProposalStatus::Applied)
            .unwrap()
            .unwrap();
        assert_eq!(applied.status, ProposalStatus::Applied);
        assert!(applied
            .applied_ref
            .as_deref()
            .unwrap_or("")
            .contains("apply"));
        let rolled_back = store
            .update_status(&proposal.id, ProposalStatus::RolledBack)
            .unwrap()
            .unwrap();
        assert_eq!(rolled_back.lifecycle_stage(), "rollback");
        assert!(rolled_back
            .rollback_ref
            .as_deref()
            .unwrap_or("")
            .contains("rollback"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn store_binds_evalset_to_improvement_proposal() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-improvements-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = ImprovementStore::new(path.clone());
        let proposal = ImprovementProposal::new(
            vec![1],
            ImprovementTarget::Routing,
            "Increase retrieval before risky routing.".to_string(),
            "Better routing.",
            RiskLevel::Medium,
            vec!["Run routing evalset.".to_string()],
            vec!["evidence".to_string()],
        );
        store.upsert(&proposal).unwrap();

        let updated = store
            .bind_evalset(&proposal.id, "routing-smoke")
            .unwrap()
            .unwrap();

        assert_eq!(updated.evalset_bindings, vec!["routing-smoke"]);
        let _ = std::fs::remove_file(path);
    }
}

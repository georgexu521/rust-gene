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

const ACTIVE_GUIDANCE_PROMPT_CHAR_LIMIT: usize = 900;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GuidanceScope {
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceActivation {
    DiagnosticOnly,
    PromptContext,
    ToolContractHint,
    RoutePolicyHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliedGuidanceStatus {
    Active,
    Inactive,
    RollbackRecommended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedGuidanceRecord {
    pub id: String,
    pub proposal_id: String,
    pub target: ImprovementTarget,
    pub scope: GuidanceScope,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
    pub content: String,
    pub activation: GuidanceActivation,
    pub evalsets: Vec<String>,
    pub applied_at: String,
    pub rollback_ref: Option<String>,
    pub status: AppliedGuidanceStatus,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementEffectOutcome {
    Positive,
    Neutral,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImprovementEffectRecord {
    pub id: String,
    pub proposal_id: String,
    pub evalset: String,
    pub run_id: String,
    pub outcome: ImprovementEffectOutcome,
    pub failure_owner: String,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImprovementEffectSummary {
    pub proposal_id: String,
    pub total: usize,
    pub positive: usize,
    pub neutral: usize,
    pub negative: usize,
    pub rollback_recommended: bool,
    pub recent: Vec<ImprovementEffectRecord>,
}

#[derive(Debug, Clone)]
pub struct AppliedGuidanceStore {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ImprovementEffectStore {
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

    pub fn applied_guidance_store(&self) -> AppliedGuidanceStore {
        let path = self
            .path
            .parent()
            .map(|parent| parent.join("applied_guidance.jsonl"))
            .unwrap_or_else(AppliedGuidanceStore::default_path);
        AppliedGuidanceStore::new(path)
    }

    pub fn effect_store(&self) -> ImprovementEffectStore {
        let path = self
            .path
            .parent()
            .map(|parent| parent.join("improvement_effects.jsonl"))
            .unwrap_or_else(ImprovementEffectStore::default_path);
        ImprovementEffectStore::new(path)
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
        if status == ProposalStatus::Applied && proposal.evalset_bindings.is_empty() {
            return Err(anyhow::anyhow!(
                "proposal must have at least one bound evalset before apply; run /improvements bind-eval {} <evalset>",
                proposal.id
            ));
        }
        proposal.status = status;
        match status {
            ProposalStatus::Applied => {
                let guidance = self.applied_guidance_store().apply_proposal(&proposal)?;
                proposal.applied_ref = Some(format!("guidance:{}", guidance.id));
                proposal.rollback_ref = None;
            }
            ProposalStatus::RolledBack => {
                let rollback = self
                    .applied_guidance_store()
                    .rollback_proposal(&proposal.id)?;
                proposal.rollback_ref = rollback
                    .map(|record| format!("guidance:{}", record.id))
                    .or_else(|| Some(format!("manual:/improvements rollback {}", proposal.id)));
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

impl AppliedGuidanceStore {
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("applied_guidance.jsonl")
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(&self) -> Vec<AppliedGuidanceRecord> {
        read_latest_guidance_records(&self.path)
    }

    pub fn active(&self) -> Vec<AppliedGuidanceRecord> {
        self.list()
            .into_iter()
            .filter(|record| record.status == AppliedGuidanceStatus::Active)
            .collect()
    }

    pub fn get(&self, id_or_proposal: &str) -> Option<AppliedGuidanceRecord> {
        self.list().into_iter().find(|record| {
            record.id == id_or_proposal
                || record.id.starts_with(id_or_proposal)
                || record.proposal_id == id_or_proposal
                || record.proposal_id.starts_with(id_or_proposal)
        })
    }

    pub fn apply_proposal(
        &self,
        proposal: &ImprovementProposal,
    ) -> anyhow::Result<AppliedGuidanceRecord> {
        if proposal.eval_status != ProposalEvalStatus::Passed {
            anyhow::bail!("proposal must pass eval before applied guidance can be created");
        }
        if proposal.evalset_bindings.is_empty() {
            anyhow::bail!("proposal must have at least one evalset before applied guidance");
        }
        let now = chrono::Utc::now().to_rfc3339();
        let mut record = self
            .get(&proposal.id)
            .unwrap_or_else(|| AppliedGuidanceRecord::from_proposal(proposal, now.clone()));
        let project = current_project_identity();
        record.status = AppliedGuidanceStatus::Active;
        record.content = proposal.proposed_change.clone();
        record.evalsets = proposal.evalset_bindings.clone();
        record.project_id = project.as_ref().map(|project| project.id.clone());
        record.project_root = project.as_ref().map(|project| project.root.clone());
        record.updated_at = now;
        append_guidance_jsonl(&self.path, &record)?;
        Ok(record)
    }

    pub fn rollback_proposal(
        &self,
        id_or_proposal: &str,
    ) -> anyhow::Result<Option<AppliedGuidanceRecord>> {
        let Some(mut record) = self.get(id_or_proposal) else {
            return Ok(None);
        };
        let now = chrono::Utc::now().to_rfc3339();
        record.status = AppliedGuidanceStatus::Inactive;
        record.rollback_ref = Some(format!(
            "manual:/improvements rollback {}",
            record.proposal_id
        ));
        record.updated_at = now;
        append_guidance_jsonl(&self.path, &record)?;
        Ok(Some(record))
    }

    pub fn deactivate(
        &self,
        id_or_proposal: &str,
    ) -> anyhow::Result<Option<AppliedGuidanceRecord>> {
        self.rollback_proposal(id_or_proposal)
    }
}

impl Default for AppliedGuidanceStore {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

impl ImprovementEffectStore {
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("improvement_effects.jsonl")
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(&self) -> Vec<ImprovementEffectRecord> {
        read_effect_records(&self.path)
    }

    pub fn record(
        &self,
        proposal_id: impl Into<String>,
        evalset: impl Into<String>,
        run_id: impl Into<String>,
        outcome: ImprovementEffectOutcome,
        failure_owner: impl Into<String>,
        reason: impl Into<String>,
    ) -> anyhow::Result<ImprovementEffectRecord> {
        let record = ImprovementEffectRecord {
            id: format!("eff_{}", uuid::Uuid::new_v4().simple()),
            proposal_id: proposal_id.into(),
            evalset: evalset.into(),
            run_id: run_id.into(),
            outcome,
            failure_owner: failure_owner.into(),
            reason: reason.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        append_effect_jsonl(&self.path, &record)?;
        Ok(record)
    }

    pub fn summary(&self, proposal_id: &str) -> ImprovementEffectSummary {
        let mut matching = self
            .list()
            .into_iter()
            .filter(|record| record.proposal_id == proposal_id)
            .collect::<Vec<_>>();
        matching.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        let positive = matching
            .iter()
            .filter(|record| record.outcome == ImprovementEffectOutcome::Positive)
            .count();
        let neutral = matching
            .iter()
            .filter(|record| record.outcome == ImprovementEffectOutcome::Neutral)
            .count();
        let negative = matching
            .iter()
            .filter(|record| record.outcome == ImprovementEffectOutcome::Negative)
            .count();
        ImprovementEffectSummary {
            proposal_id: proposal_id.to_string(),
            total: matching.len(),
            positive,
            neutral,
            negative,
            rollback_recommended: negative >= 2 && negative > positive,
            recent: matching.into_iter().take(8).collect(),
        }
    }
}

impl Default for ImprovementEffectStore {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

impl AppliedGuidanceRecord {
    pub fn from_proposal(proposal: &ImprovementProposal, now: String) -> Self {
        Self::from_proposal_with_project(proposal, now, current_project_identity())
    }

    fn from_proposal_with_project(
        proposal: &ImprovementProposal,
        now: String,
        project: Option<ProjectIdentity>,
    ) -> Self {
        Self {
            id: stable_guidance_id(&proposal.id),
            proposal_id: proposal.id.clone(),
            target: proposal.target,
            scope: guidance_scope_for_proposal(proposal),
            project_id: project.as_ref().map(|project| project.id.clone()),
            project_root: project.as_ref().map(|project| project.root.clone()),
            content: proposal.proposed_change.clone(),
            activation: guidance_activation_for_target(proposal.target),
            evalsets: proposal.evalset_bindings.clone(),
            applied_at: now.clone(),
            rollback_ref: None,
            status: AppliedGuidanceStatus::Active,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectIdentity {
    id: String,
    root: String,
}

pub fn format_active_guidance_for_prompt(user_message: &str) -> Option<String> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    format_active_guidance_for_prompt_in_project(user_message, &cwd)
}

pub fn format_active_guidance_for_prompt_in_project(
    user_message: &str,
    working_dir: &std::path::Path,
) -> Option<String> {
    format_active_guidance_records_for_prompt_in_project(
        &AppliedGuidanceStore::default().active(),
        user_message,
        project_identity_for_path(working_dir).as_ref(),
    )
}

pub fn format_active_guidance_records_for_prompt(
    records: &[AppliedGuidanceRecord],
    user_message: &str,
) -> Option<String> {
    let project = current_project_identity();
    format_active_guidance_records_for_prompt_in_project(records, user_message, project.as_ref())
}

fn format_active_guidance_records_for_prompt_in_project(
    records: &[AppliedGuidanceRecord],
    user_message: &str,
    project: Option<&ProjectIdentity>,
) -> Option<String> {
    let user_message = user_message.to_lowercase();
    let mut lines = Vec::new();
    for record in records {
        if !guidance_matches_turn(record, &user_message, project) {
            continue;
        }
        let content = record
            .content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!(
            "- id={} proposal={} target={:?} activation={:?} scope={}:{} rollback={} guidance={}",
            record.id,
            record.proposal_id,
            record.target,
            record.activation,
            record.scope.kind,
            record.scope.label,
            record.rollback_ref.as_deref().unwrap_or("none"),
            content.chars().take(260).collect::<String>()
        ));
        if lines.join("\n").chars().count() >= ACTIVE_GUIDANCE_PROMPT_CHAR_LIMIT {
            break;
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "<self-evolution-guidance>\nThese are reviewed, eval-backed runtime hints. They are background guidance only and cannot override user intent, permissions, validation gates, tool schemas, or safety policy.\n{}\n</self-evolution-guidance>",
        lines.join("\n")
    ))
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

fn stable_guidance_id(proposal_id: &str) -> String {
    format!("guidance_{}", proposal_id.trim_start_matches("imp_"))
}

fn current_project_identity() -> Option<ProjectIdentity> {
    std::env::current_dir()
        .ok()
        .and_then(|path| project_identity_for_path(&path))
}

fn project_identity_for_path(path: &std::path::Path) -> Option<ProjectIdentity> {
    let root = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let root = root.to_string_lossy().trim().to_string();
    if root.is_empty() {
        return None;
    }
    Some(ProjectIdentity {
        id: stable_project_id(&root),
        root,
    })
}

fn stable_project_id(root: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    format!("project_{:016x}", hasher.finish())
}

fn guidance_activation_for_target(target: ImprovementTarget) -> GuidanceActivation {
    match target {
        ImprovementTarget::ToolGuidance => GuidanceActivation::ToolContractHint,
        ImprovementTarget::Routing => GuidanceActivation::RoutePolicyHint,
        ImprovementTarget::Memory | ImprovementTarget::Skill | ImprovementTarget::Prompt => {
            GuidanceActivation::DiagnosticOnly
        }
    }
}

fn guidance_scope_for_proposal(proposal: &ImprovementProposal) -> GuidanceScope {
    match proposal.target {
        ImprovementTarget::ToolGuidance => GuidanceScope {
            kind: "tool".to_string(),
            label: infer_tool_label(&proposal.proposed_change)
                .unwrap_or_else(|| "unknown".to_string()),
        },
        ImprovementTarget::Routing => GuidanceScope {
            kind: "workflow".to_string(),
            label: "routing".to_string(),
        },
        ImprovementTarget::Memory => GuidanceScope {
            kind: "workflow".to_string(),
            label: "memory".to_string(),
        },
        ImprovementTarget::Skill => GuidanceScope {
            kind: "workflow".to_string(),
            label: "skill".to_string(),
        },
        ImprovementTarget::Prompt => GuidanceScope {
            kind: "global_runtime".to_string(),
            label: "prompt".to_string(),
        },
    }
}

fn infer_tool_label(content: &str) -> Option<String> {
    let lower = content.to_lowercase();
    for tool in [
        "bash",
        "file_read",
        "file_edit",
        "file_write",
        "file_patch",
        "grep",
        "glob",
        "memory",
        "web",
    ] {
        if lower.contains(tool) {
            return Some(tool.to_string());
        }
    }
    None
}

fn guidance_matches_turn(
    record: &AppliedGuidanceRecord,
    user_message: &str,
    project: Option<&ProjectIdentity>,
) -> bool {
    if record.status != AppliedGuidanceStatus::Active {
        return false;
    }
    if !guidance_matches_project(record, project) {
        return false;
    }
    match record.activation {
        GuidanceActivation::DiagnosticOnly => false,
        GuidanceActivation::PromptContext => true,
        GuidanceActivation::RoutePolicyHint => {
            user_message.contains("code")
                || user_message.contains("test")
                || user_message.contains("fix")
                || user_message.contains("debug")
                || user_message.contains("实现")
                || user_message.contains("测试")
                || user_message.contains("修")
        }
        GuidanceActivation::ToolContractHint => {
            let label = record.scope.label.to_lowercase();
            label != "unknown"
                && (user_message.contains(&label)
                    || user_message.contains("tool")
                    || user_message.contains("工具")
                    || user_message.contains("run")
                    || user_message.contains("command")
                    || user_message.contains("测试"))
        }
    }
}

fn guidance_matches_project(
    record: &AppliedGuidanceRecord,
    project: Option<&ProjectIdentity>,
) -> bool {
    if record.scope.kind == "global_runtime" {
        return true;
    }
    let Some(record_project_id) = record.project_id.as_deref() else {
        return false;
    };
    project
        .map(|project| project.id == record_project_id)
        .unwrap_or(false)
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

fn read_latest_guidance_records(path: &Path) -> Vec<AppliedGuidanceRecord> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut latest = HashMap::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(record) = serde_json::from_str::<AppliedGuidanceRecord>(line) else {
            continue;
        };
        latest.insert(record.id.clone(), record);
    }
    let mut records = latest.into_values().collect::<Vec<_>>();
    records.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    records
}

fn read_effect_records(path: &Path) -> Vec<ImprovementEffectRecord> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut records = Vec::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(record) = serde_json::from_str::<ImprovementEffectRecord>(line) else {
            continue;
        };
        records.push(record);
    }
    records.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    records
}

fn append_guidance_jsonl(path: &Path, record: &AppliedGuidanceRecord) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

fn append_effect_jsonl(path: &Path, record: &ImprovementEffectRecord) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("improvements.jsonl");
        let store = ImprovementStore::new(path.clone());
        std::fs::write(
            path.parent().unwrap().join("applied_guidance.jsonl"),
            "not-json\n",
        )
        .unwrap();
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
        assert!(store
            .bind_evalset(&proposal.id, "tool-guidance-smoke")
            .unwrap()
            .is_some());
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
            .contains("guidance:"));
        let guidance = store.applied_guidance_store().active();
        assert_eq!(guidance.len(), 1);
        assert_eq!(guidance[0].proposal_id, proposal.id);
        assert_eq!(guidance[0].activation, GuidanceActivation::ToolContractHint);
        let applied_again = store
            .update_status(&proposal.id, ProposalStatus::Applied)
            .unwrap()
            .unwrap();
        assert_eq!(applied_again.status, ProposalStatus::Applied);
        assert_eq!(store.applied_guidance_store().active().len(), 1);
        let rolled_back = store
            .update_status(&proposal.id, ProposalStatus::RolledBack)
            .unwrap()
            .unwrap();
        assert_eq!(rolled_back.lifecycle_stage(), "rollback");
        assert!(rolled_back
            .rollback_ref
            .as_deref()
            .unwrap_or("")
            .contains("guidance:"));
        assert!(store.applied_guidance_store().active().is_empty());
    }

    #[test]
    fn apply_requires_bound_evalset_even_after_eval_passes() {
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
        store
            .record_eval(&proposal.id, ProposalEvalStatus::Passed, "preflight passed")
            .unwrap();

        let error = store
            .update_status(&proposal.id, ProposalStatus::Applied)
            .expect_err("apply should require evalset binding");

        assert!(error.to_string().contains("bound evalset"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn active_guidance_prompt_is_bounded_and_matching() {
        let mut proposal = ImprovementProposal::new(
            vec![1, 2],
            ImprovementTarget::ToolGuidance,
            "Add guidance for repeated bash failures: inspect arguments before retrying."
                .to_string(),
            "Better recovery.",
            RiskLevel::Medium,
            vec!["Run tool regression.".to_string()],
            vec!["evidence".to_string()],
        );
        proposal.evalset_bindings = vec!["tool-guidance-smoke".to_string()];
        proposal.eval_status = ProposalEvalStatus::Passed;
        let record =
            AppliedGuidanceRecord::from_proposal(&proposal, "2026-05-28T00:00:00Z".to_string());

        let prompt = format_active_guidance_records_for_prompt(
            &[record],
            "run bash validation command for the fix",
        )
        .expect("matching guidance should render");

        assert!(prompt.contains("<self-evolution-guidance>"));
        assert!(prompt.contains("cannot override"));
        assert!(prompt.contains("bash"));
        assert!(prompt.chars().count() < 1_200);
    }

    #[test]
    fn active_guidance_is_project_scoped() {
        let mut proposal = ImprovementProposal::new(
            vec![1, 2],
            ImprovementTarget::ToolGuidance,
            "Add guidance for repeated bash failures: inspect arguments before retrying."
                .to_string(),
            "Better recovery.",
            RiskLevel::Medium,
            vec!["Run tool regression.".to_string()],
            vec!["evidence".to_string()],
        );
        proposal.evalset_bindings = vec!["tool-guidance-smoke".to_string()];
        proposal.eval_status = ProposalEvalStatus::Passed;
        let project_a = ProjectIdentity {
            id: "project_a".to_string(),
            root: "/tmp/project-a".to_string(),
        };
        let project_b = ProjectIdentity {
            id: "project_b".to_string(),
            root: "/tmp/project-b".to_string(),
        };
        let record = AppliedGuidanceRecord::from_proposal_with_project(
            &proposal,
            "2026-05-28T00:00:00Z".to_string(),
            Some(project_a.clone()),
        );

        assert!(format_active_guidance_records_for_prompt_in_project(
            &[record.clone()],
            "run bash validation",
            Some(&project_a)
        )
        .is_some());
        assert!(format_active_guidance_records_for_prompt_in_project(
            &[record],
            "run bash validation",
            Some(&project_b)
        )
        .is_none());
    }

    #[test]
    fn legacy_tool_guidance_without_project_is_not_injected() {
        let mut proposal = ImprovementProposal::new(
            vec![1],
            ImprovementTarget::ToolGuidance,
            "Improve repeated bash failure guidance.".to_string(),
            "Better recovery.",
            RiskLevel::Medium,
            vec!["Run tool regression.".to_string()],
            vec!["evidence".to_string()],
        );
        proposal.evalset_bindings = vec!["tool-guidance-smoke".to_string()];
        proposal.eval_status = ProposalEvalStatus::Passed;
        let mut record = AppliedGuidanceRecord::from_proposal_with_project(
            &proposal,
            "2026-05-28T00:00:00Z".to_string(),
            None,
        );
        record.project_id = None;
        record.project_root = None;

        let project = ProjectIdentity {
            id: "project_a".to_string(),
            root: "/tmp/project-a".to_string(),
        };

        assert!(format_active_guidance_records_for_prompt_in_project(
            &[record],
            "run bash validation",
            Some(&project)
        )
        .is_none());
    }

    #[test]
    fn effect_summary_recommends_rollback_after_regressions() {
        let path = std::env::temp_dir().join(format!(
            "priority-agent-effects-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let store = ImprovementEffectStore::new(path.clone());
        store
            .record(
                "imp_test",
                "tool-guidance",
                "run-a",
                ImprovementEffectOutcome::Negative,
                "framework",
                "regressed validation",
            )
            .unwrap();
        store
            .record(
                "imp_test",
                "tool-guidance",
                "run-b",
                ImprovementEffectOutcome::Negative,
                "framework",
                "regressed closeout",
            )
            .unwrap();

        let summary = store.summary("imp_test");

        assert_eq!(summary.negative, 2);
        assert!(summary.rollback_recommended);
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

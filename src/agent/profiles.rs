//! Named subagent profiles.

use crate::agent::roles::AgentRole;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const RESERVED_LAB_PROFILES: &[&str] = &["lab-professor", "lab-postdoc", "lab-graduate"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentContextMode {
    Minimal,
    #[serde(alias = "inherit")]
    InheritedSummary,
    #[serde(alias = "fork")]
    FullFork,
    IsolatedWorktreeFork,
}

impl std::fmt::Display for AgentContextMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AgentContextMode::Minimal => "minimal",
            AgentContextMode::InheritedSummary => "inherited_summary",
            AgentContextMode::FullFork => "full_fork",
            AgentContextMode::IsolatedWorktreeFork => "isolated_worktree_fork",
        };
        write!(f, "{}", label)
    }
}

impl AgentContextMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "minimal" => Some(Self::Minimal),
            "inherit" | "inherited_summary" => Some(Self::InheritedSummary),
            "fork" | "full_fork" => Some(Self::FullFork),
            "isolated" | "worktree" | "isolated_worktree_fork" => Some(Self::IsolatedWorktreeFork),
            _ => None,
        }
    }

    pub fn inherits_parent_context(self) -> bool {
        !matches!(self, AgentContextMode::Minimal)
    }

    pub fn copies_full_history(self) -> bool {
        matches!(
            self,
            AgentContextMode::FullFork | AgentContextMode::IsolatedWorktreeFork
        )
    }

    pub fn requires_isolated_worktree(self) -> bool {
        matches!(self, AgentContextMode::IsolatedWorktreeFork)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRiskPolicy {
    ReadOnly,
    VerifyOnly,
    CodeChange,
}

impl std::fmt::Display for AgentRiskPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AgentRiskPolicy::ReadOnly => "read_only",
            AgentRiskPolicy::VerifyOnly => "verify_only",
            AgentRiskPolicy::CodeChange => "code_change",
        };
        write!(f, "{}", label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPermissionMode {
    ReadOnly,
    Bubble,
    IsolatedWrite,
}

impl std::fmt::Display for AgentPermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AgentPermissionMode::ReadOnly => "read_only",
            AgentPermissionMode::Bubble => "bubble",
            AgentPermissionMode::IsolatedWrite => "isolated_write",
        };
        write!(f, "{}", label)
    }
}

impl AgentPermissionMode {
    fn from_risk_and_context(risk: AgentRiskPolicy, context: AgentContextMode) -> Self {
        match risk {
            AgentRiskPolicy::ReadOnly | AgentRiskPolicy::VerifyOnly => Self::ReadOnly,
            AgentRiskPolicy::CodeChange if context.requires_isolated_worktree() => {
                Self::IsolatedWrite
            }
            AgentRiskPolicy::CodeChange => Self::Bubble,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentOutputContract {
    Findings,
    PatchSummary,
    VerificationReport,
}

impl std::fmt::Display for AgentOutputContract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AgentOutputContract::Findings => "findings",
            AgentOutputContract::PatchSummary => "patch_summary",
            AgentOutputContract::VerificationReport => "verification_report",
        };
        write!(f, "{}", label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModelPolicy {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

impl AgentModelPolicy {
    pub fn inherit(effort: Option<String>) -> Self {
        Self {
            model: "inherit".to_string(),
            effort,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMemoryPolicy {
    None,
    Session,
    Role,
    Project,
    User,
}

impl std::fmt::Display for AgentMemoryPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AgentMemoryPolicy::None => "none",
            AgentMemoryPolicy::Session => "session",
            AgentMemoryPolicy::Role => "role",
            AgentMemoryPolicy::Project => "project",
            AgentMemoryPolicy::User => "user",
        };
        write!(f, "{}", label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub role: AgentRole,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    #[serde(default)]
    pub context: Option<AgentContextMode>,
    #[serde(default)]
    pub permission_mode: Option<AgentPermissionMode>,
    #[serde(default)]
    pub risk_policy: Option<AgentRiskPolicy>,
    #[serde(default)]
    pub output_contract: Option<AgentOutputContract>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub memory: Option<AgentMemoryPolicy>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub max_turns: Option<usize>,
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentDefinition {
    pub name: String,
    pub agent_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_hash: Option<String>,
    pub when_to_use: String,
    pub role: AgentRole,
    #[serde(default, skip_serializing)]
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<String>,
    pub tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub permission_mode: AgentPermissionMode,
    pub model_policy: AgentModelPolicy,
    pub max_turns: usize,
    pub timeout_secs: u64,
    pub mcp_servers: Vec<String>,
    pub memory_policy: AgentMemoryPolicy,
    pub context_mode: AgentContextMode,
    pub risk_policy: AgentRiskPolicy,
    pub output_contract: AgentOutputContract,
}

impl AgentDefinition {
    pub fn from_profile(profile: &AgentProfile) -> Self {
        profile.to_definition()
    }

    pub fn contract_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Agent type: {}", self.agent_type),
            format!("Context mode: {}", self.context_mode),
            format!("Permission mode: {}", self.permission_mode),
            format!("Risk policy: {}", self.risk_policy),
            format!("Output contract: {}", self.output_contract),
            format!("Model policy: {}", self.model_policy.model),
            format!("Max turns: {}", self.max_turns),
        ];
        if let Some(prompt_version) = &self.prompt_version {
            lines.push(format!("Prompt version: {}", prompt_version));
        }
        if let Some(effort) = &self.model_policy.effort {
            lines.push(format!("Effort: {}", effort));
        }
        if !self.tools.is_empty() {
            lines.push(format!("Tools: {}", self.tools.join(",")));
        }
        if !self.disallowed_tools.is_empty() {
            lines.push(format!(
                "Disallowed tools: {}",
                self.disallowed_tools.join(",")
            ));
        }
        if !self.mcp_servers.is_empty() {
            lines.push(format!("MCP servers: {}", self.mcp_servers.join(",")));
        }
        lines.push(format!("Memory policy: {}", self.memory_policy));
        lines
    }

    pub fn envelope_constraints(&self) -> Vec<String> {
        let mut constraints = vec![
            format!("agent_definition={}", self.agent_type),
            format!("context={}", self.context_mode),
            format!("permission_mode={}", self.permission_mode),
            format!("risk_policy={}", self.risk_policy),
            format!("output_contract={}", self.output_contract),
            format!("model={}", self.model_policy.model),
            format!("memory={}", self.memory_policy),
        ];
        if let Some(prompt_version) = &self.prompt_version {
            constraints.push(format!("prompt_version={prompt_version}"));
        }
        if !self.mcp_servers.is_empty() {
            constraints.push(format!("mcp_servers={}", self.mcp_servers.join(",")));
        }
        constraints
    }

    pub fn summary_line(&self) -> String {
        format!(
            "{} [{}] context={} permission={} tools={}",
            self.name,
            self.role.display_name(),
            self.context_mode,
            self.permission_mode,
            self.tools.len()
        )
    }
}

impl AgentProfile {
    pub fn to_definition(&self) -> AgentDefinition {
        let risk_policy = self.risk_policy.unwrap_or_else(|| {
            if self
                .allowed_tools
                .iter()
                .any(|tool| tool == "file_edit" || tool == "file_write")
            {
                AgentRiskPolicy::CodeChange
            } else {
                AgentRiskPolicy::ReadOnly
            }
        });
        let context_mode = self.context.unwrap_or(match risk_policy {
            AgentRiskPolicy::CodeChange => AgentContextMode::IsolatedWorktreeFork,
            AgentRiskPolicy::ReadOnly | AgentRiskPolicy::VerifyOnly => {
                AgentContextMode::InheritedSummary
            }
        });
        let permission_mode = self.permission_mode.unwrap_or_else(|| {
            AgentPermissionMode::from_risk_and_context(risk_policy, context_mode)
        });
        let output_contract = self.output_contract.unwrap_or(match risk_policy {
            AgentRiskPolicy::ReadOnly => AgentOutputContract::Findings,
            AgentRiskPolicy::VerifyOnly => AgentOutputContract::VerificationReport,
            AgentRiskPolicy::CodeChange => AgentOutputContract::PatchSummary,
        });
        let memory_policy = self.memory.unwrap_or(AgentMemoryPolicy::Session);
        let model_policy = match self.model.as_ref().map(|value| value.trim()) {
            Some(model) if !model.is_empty() => AgentModelPolicy {
                model: model.to_string(),
                effort: self.effort.clone(),
            },
            _ => AgentModelPolicy::inherit(self.effort.clone()),
        };

        AgentDefinition {
            name: self.name.clone(),
            agent_type: self.name.clone(),
            profile_origin: Some(if is_reserved_lab_profile_name(&self.name) {
                "system".to_string()
            } else {
                "profile".to_string()
            }),
            profile_hash: Some(profile_hash(self)),
            when_to_use: self.description.clone(),
            role: self.role,
            system_prompt: self.system_prompt.clone(),
            prompt_version: self.prompt_version.clone(),
            tools: self.allowed_tools.clone(),
            disallowed_tools: self.disallowed_tools.clone(),
            permission_mode,
            model_policy,
            max_turns: self.max_turns.unwrap_or(8),
            timeout_secs: self.timeout_secs.unwrap_or(300),
            mcp_servers: self.mcp_servers.clone(),
            memory_policy,
            context_mode,
            risk_policy,
            output_contract,
        }
    }
}

pub fn load_profiles(project_root: impl AsRef<Path>) -> Vec<AgentProfile> {
    let mut profiles = builtin_profiles();
    for dir in profile_dirs(project_root.as_ref()) {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("toml") {
                continue;
            }
            match load_profile_file(&path) {
                Ok(profile) if is_reserved_lab_profile_name(&profile.name) => {
                    tracing::warn!(
                        "Ignoring project/user agent profile {} because '{}' is a reserved LabRun profile",
                        path.display(),
                        profile.name
                    );
                }
                Ok(profile) => upsert_profile(&mut profiles, profile),
                Err(err) => {
                    tracing::warn!("Failed to load agent profile {}: {}", path.display(), err)
                }
            }
        }
    }
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    profiles
}

pub fn find_profile(project_root: impl AsRef<Path>, name: &str) -> Option<AgentProfile> {
    load_profiles(project_root)
        .into_iter()
        .find(|profile| profile.name.eq_ignore_ascii_case(name))
}

pub fn find_product_profile(name: &str) -> Option<AgentProfile> {
    product_profiles()
        .into_iter()
        .find(|profile| profile.name.eq_ignore_ascii_case(name))
}

pub fn find_runnable_profile(project_root: impl AsRef<Path>, name: &str) -> Option<AgentProfile> {
    if is_reserved_lab_profile_name(name) {
        return find_reserved_lab_profile(name);
    }
    find_profile(project_root, name).or_else(|| find_product_profile(name))
}

pub fn find_reserved_lab_profile(name: &str) -> Option<AgentProfile> {
    builtin_profiles().into_iter().find(|profile| {
        is_reserved_lab_profile_name(&profile.name) && profile.name.eq_ignore_ascii_case(name)
    })
}

pub fn is_reserved_lab_profile_name(name: &str) -> bool {
    RESERVED_LAB_PROFILES
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(name.trim()))
}

pub fn profile_hash(profile: &AgentProfile) -> String {
    let encoded = serde_json::to_vec(profile).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    format!("{:x}", hasher.finalize())
}

pub fn load_definitions(project_root: impl AsRef<Path>) -> Vec<AgentDefinition> {
    load_profiles(project_root)
        .into_iter()
        .map(|profile| profile.to_definition())
        .collect()
}

pub fn find_definition(project_root: impl AsRef<Path>, name: &str) -> Option<AgentDefinition> {
    find_profile(project_root, name).map(|profile| profile.to_definition())
}

fn profile_dirs(project_root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![project_root.join(".priority-agent").join("agents")];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".priority-agent").join("agents"));
    }
    dirs
}

fn load_profile_file(path: &Path) -> anyhow::Result<AgentProfile> {
    let raw = std::fs::read_to_string(path)?;
    let mut profile: AgentProfile = toml::from_str(&raw)?;
    if profile.name.trim().is_empty() {
        if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
            profile.name = stem.to_string();
        }
    }
    Ok(profile)
}

fn upsert_profile(profiles: &mut Vec<AgentProfile>, profile: AgentProfile) {
    if let Some(existing) = profiles
        .iter_mut()
        .find(|item| item.name.eq_ignore_ascii_case(&profile.name))
    {
        *existing = profile;
    } else {
        profiles.push(profile);
    }
}

fn builtin_profiles() -> Vec<AgentProfile> {
    vec![
        AgentProfile {
            name: "default".to_string(),
            description: "Bounded general sub-agent".to_string(),
            role: AgentRole::Default,
            system_prompt: "Complete the assigned task with the narrowest useful tool set. Do not delegate recursively."
                .to_string(),
            prompt_version: None,
            allowed_tools: vec![
                "project_list".into(),
                "glob".into(),
                "grep".into(),
                "file_read".into(),
                "file_edit".into(),
                "file_write".into(),
                "bash".into(),
                "diff".into(),
                "format".into(),
            ],
            disallowed_tools: Vec::new(),
            context: Some(AgentContextMode::IsolatedWorktreeFork),
            permission_mode: None,
            risk_policy: Some(AgentRiskPolicy::CodeChange),
            output_contract: Some(AgentOutputContract::PatchSummary),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Session),
            timeout_secs: Some(300),
            max_turns: Some(8),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "explorer".to_string(),
            description: "Read-only codebase explorer".to_string(),
            role: AgentRole::Plan,
            system_prompt: "Focus on discovering structure and risks. Do not edit files."
                .to_string(),
            prompt_version: None,
            allowed_tools: vec!["project_list".into(), "grep".into(), "file_read".into()],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "agent".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::None),
            timeout_secs: Some(300),
            max_turns: Some(6),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "planner".to_string(),
            description: "Read-only implementation planner".to_string(),
            role: AgentRole::Plan,
            system_prompt: "Read relevant context and produce a concrete plan. Do not edit files."
                .to_string(),
            prompt_version: None,
            allowed_tools: vec![
                "project_list".into(),
                "glob".into(),
                "grep".into(),
                "file_read".into(),
                "plan".into(),
                "todo_write".into(),
            ],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "agent".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::None),
            timeout_secs: Some(300),
            max_turns: Some(6),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "verifier".to_string(),
            description: "Adversarial verification agent".to_string(),
            role: AgentRole::Verification,
            system_prompt: "Try to falsify the change with tests and concrete evidence."
                .to_string(),
            prompt_version: None,
            allowed_tools: vec!["bash".into(), "grep".into(), "file_read".into()],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "agent".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::VerifyOnly),
            output_contract: Some(AgentOutputContract::VerificationReport),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Session),
            timeout_secs: Some(300),
            max_turns: Some(8),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "lab-professor".to_string(),
            description: "LabRun professor: project intake, strategy, architecture, and sponsor steering".to_string(),
            role: AgentRole::Advisor,
            system_prompt: lab_professor_prompt().to_string(),
            prompt_version: Some("lab-professor.v1".to_string()),
            allowed_tools: vec![
                "project_list".into(),
                "glob".into(),
                "grep".into(),
                "file_read".into(),
                "git_status".into(),
            ],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "bash".into(), "agent".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: Some("high".to_string()),
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Project),
            timeout_secs: Some(300),
            max_turns: Some(8),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "lab-postdoc".to_string(),
            description: "LabRun postdoc: code-aware read-only planner, auditor, and integration reviewer".to_string(),
            role: AgentRole::Verification,
            system_prompt: lab_postdoc_prompt().to_string(),
            prompt_version: Some("lab-postdoc.v1".to_string()),
            allowed_tools: vec![
                "project_list".into(),
                "glob".into(),
                "grep".into(),
                "file_read".into(),
                "diff".into(),
                "git_status".into(),
                "git_diff".into(),
            ],
            disallowed_tools: vec![
                "file_edit".into(),
                "file_write".into(),
                "bash".into(),
                "agent".into(),
                "swarm".into(),
            ],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::VerifyOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: Some("high".to_string()),
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Project),
            timeout_secs: Some(600),
            max_turns: Some(12),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "lab-graduate".to_string(),
            description: "LabRun graduate: narrow scoped implementation worker".to_string(),
            role: AgentRole::Specialist,
            system_prompt: lab_graduate_prompt().to_string(),
            prompt_version: Some("lab-graduate.v1".to_string()),
            allowed_tools: vec![
                "grep".into(),
                "file_read".into(),
                "file_edit".into(),
                "file_write".into(),
                "bash".into(),
                "diff".into(),
                "format".into(),
            ],
            disallowed_tools: vec!["agent".into(), "swarm".into()],
            context: Some(AgentContextMode::IsolatedWorktreeFork),
            permission_mode: Some(AgentPermissionMode::IsolatedWrite),
            risk_policy: Some(AgentRiskPolicy::CodeChange),
            output_contract: Some(AgentOutputContract::PatchSummary),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Session),
            timeout_secs: Some(420),
            max_turns: Some(6),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "implementer".to_string(),
            description: "Focused code-change worker".to_string(),
            role: AgentRole::Specialist,
            system_prompt: "Make focused edits and report changed files clearly.".to_string(),
            prompt_version: None,
            allowed_tools: vec![
                "project_list".into(),
                "glob".into(),
                "grep".into(),
                "file_read".into(),
                "file_write".into(),
                "file_edit".into(),
                "bash".into(),
                "diff".into(),
                "format".into(),
            ],
            disallowed_tools: vec!["agent".into(), "swarm".into()],
            context: Some(AgentContextMode::IsolatedWorktreeFork),
            permission_mode: Some(AgentPermissionMode::IsolatedWrite),
            risk_policy: Some(AgentRiskPolicy::CodeChange),
            output_contract: Some(AgentOutputContract::PatchSummary),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Session),
            timeout_secs: Some(600),
            max_turns: Some(10),
            max_cost_usd: None,
        },
    ]
}

fn lab_professor_prompt() -> &'static str {
    "You are the LabRun professor. Act as a principal investigator and product architect. \
Clarify project goals before formal approval, protect product thesis and architecture boundaries, \
review postdoc reports strategically, and communicate with the user as sponsor. Do not edit code. \
Do not command graduate workers directly. Turn sponsor concerns into explicit steering decisions, \
proposal revisions, lab meetings, or postdoc requests. Require evidence for completion claims."
}

fn lab_postdoc_prompt() -> &'static str {
    "You are the LabRun postdoc. You own technical execution quality. Translate professor plans \
into concrete slices, read code before planning, delegate only narrow tasks, review graduate output, \
inspect diffs, and write integration reports. Stay read-only in the normal LabRun flow: implementation \
changes must be routed through scoped GraduateTask work unless a separate explicit repair workflow is \
created. Do not redefine product direction without professor or user approval. Never claim done without \
validation evidence or an explicit not_verified reason."
}

fn lab_graduate_prompt() -> &'static str {
    "You are the LabRun graduate worker. Execute exactly one scoped task from the postdoc. Stay inside \
allowed files and allowed actions. Create or edit only files named in the allowed scope. Run required \
validation when available. Report blockers instead of changing architecture or expanding scope. Your final \
answer must contain only one JSON object, with no Markdown fence or extra prose. The JSON object must have \
a top-level graduate_result object containing summary, changed_files, validation_results, blockers, and \
evidence_ids. Use the exact file paths and validation commands you actually touched or ran. You cannot \
self-certify project completion. Do not write XML-like pseudo tool tags such as <bash> or <file_edit>; \
use the provided tools for file changes and validation commands. For any task that asks you to create or \
edit a file, you must call file_write or file_edit before your final JSON. For any required validation, \
you must call bash with the validation command before your final JSON. If you cannot call the required \
tools, return a blocker instead of claiming the task is done."
}

// ── Product-facing agent profiles ──────────────────────────────

/// Built-in product profiles exposed to the user via /agent and the
/// desktop agent picker.  These are opinionated defaults that can be
/// overridden by project `.agents/` or user `~/.priority-agent/agents/`.
pub fn product_profiles() -> Vec<AgentProfile> {
    vec![
        AgentProfile {
            name: "build".into(),
            description: "Full coding mode — read, edit, shell, and validation".into(),
            role: AgentRole::Specialist,
            system_prompt: "You are in BUILD mode. Make focused code changes directly when asked, then verify the changed behavior before finishing.".into(),
            prompt_version: None,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::Bubble),
            risk_policy: Some(AgentRiskPolicy::CodeChange),
            output_contract: Some(AgentOutputContract::PatchSummary),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Session),
            timeout_secs: Some(300),
            max_turns: Some(25),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "plan".into(),
            description: "Read-only planner — explore, search, and ask questions".into(),
            role: AgentRole::Plan,
            system_prompt: "You are in PLAN mode. Inspect and reason about the project, but do not modify files unless the user explicitly asks you to implement.".into(),
            prompt_version: None,
            allowed_tools: vec![
                "file_read".into(),
                "glob".into(),
                "grep".into(),
                "project_list".into(),
                "ask_user".into(),
                "todo_write".into(),
            ],
            disallowed_tools: vec!["file_edit".into(), "file_write".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::Session),
            timeout_secs: Some(120),
            max_turns: Some(10),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "explore".into(),
            description: "Read/search only — glob, grep, read, LSP, and web fetch".into(),
            role: AgentRole::Guide,
            system_prompt: "You are in EXPLORE mode. Search, read, and map the codebase with evidence. Avoid mutations unless the user changes the task.".into(),
            prompt_version: None,
            allowed_tools: vec![
                "file_read".into(),
                "glob".into(),
                "grep".into(),
                "project_list".into(),
                "ask_user".into(),
                "web_search".into(),
                "web_fetch".into(),
                "lsp".into(),
            ],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "bash".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::None),
            timeout_secs: Some(120),
            max_turns: Some(10),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "review".into(),
            description: "Diff reviewer — read changed files, report findings, no edits".into(),
            role: AgentRole::Advisor,
            system_prompt: "You are in REVIEW mode. Lead with concrete findings grounded in diffs, files, and command output. Avoid edits unless explicitly requested.".into(),
            prompt_version: None,
            allowed_tools: vec![
                "file_read".into(),
                "git_diff".into(),
                "git_status".into(),
                "git_log".into(),
                "grep".into(),
                "glob".into(),
                "project_list".into(),
                "ask_user".into(),
                "lsp".into(),
            ],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "bash".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::VerifyOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::None),
            timeout_secs: Some(120),
            max_turns: Some(10),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "verify".into(),
            description: "Validator — run tests, check closeout, summarize proof".into(),
            role: AgentRole::Verification,
            system_prompt: "You are in VERIFY mode. Run validation commands, check correctness, and summarize proof status. Report pass or fail clearly.".into(),
            prompt_version: None,
            allowed_tools: vec![
                "file_read".into(),
                "grep".into(),
                "bash".into(),
                "git_diff".into(),
                "git_status".into(),
                "project_list".into(),
                "ask_user".into(),
            ],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "agent".into()],
            context: Some(AgentContextMode::InheritedSummary),
            permission_mode: Some(AgentPermissionMode::Bubble),
            risk_policy: Some(AgentRiskPolicy::VerifyOnly),
            output_contract: Some(AgentOutputContract::VerificationReport),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::None),
            timeout_secs: Some(300),
            max_turns: Some(15),
            max_cost_usd: None,
        },
        AgentProfile {
            name: "scout".into(),
            description: "External research — web search, fetch docs, read-only local context".into(),
            role: AgentRole::Guide,
            system_prompt: "You are in SCOUT mode. Search the web, fetch external documentation, and read local context. Report findings with source URLs. Do not edit files.".into(),
            prompt_version: None,
            allowed_tools: vec![
                "web_search".into(),
                "web_fetch".into(),
                "file_read".into(),
                "glob".into(),
                "grep".into(),
                "project_list".into(),
                "ask_user".into(),
            ],
            disallowed_tools: vec!["file_edit".into(), "file_write".into(), "bash".into()],
            context: Some(AgentContextMode::Minimal),
            permission_mode: Some(AgentPermissionMode::ReadOnly),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: Some(AgentMemoryPolicy::None),
            timeout_secs: Some(120),
            max_turns: Some(8),
            max_cost_usd: None,
        },
    ]
}

// ── Profile registry views ─────────────────────────────────────

/// Which product surface a profile belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentProfileSurface {
    /// Switchable primary session mode (build, plan, explore, review).
    Primary,
    /// Runnable as a subagent via the agent tool.
    Subagent,
    /// Hidden from product UI (compaction, title, summary, etc.).
    Hidden,
}

/// Profiles intended as switchable primary session modes.
///
/// These map to `AgentMode` and control the main-session tool surface
/// and permission policy.
pub fn primary_profiles(project_root: impl AsRef<Path>) -> Vec<AgentProfile> {
    product_profiles()
        .into_iter()
        .filter(|p| profile_surface(p) == AgentProfileSurface::Primary)
        .chain(
            load_profiles(project_root)
                .into_iter()
                .filter(|p| profile_surface(p) == AgentProfileSurface::Primary),
        )
        .collect()
}

/// Profiles intended as subagents that can be spawned via the agent tool.
///
/// These are not switchable primary modes. They are workers launched for
/// bounded parallel tasks.
pub fn subagent_profiles(project_root: impl AsRef<Path>) -> Vec<AgentProfile> {
    let mut profiles = load_profiles(project_root);
    // Include product profiles that are subagent-only (not primary).
    for p in product_profiles() {
        if profile_surface(&p) == AgentProfileSurface::Subagent {
            upsert_profile(&mut profiles, p);
        }
    }
    profiles.retain(|p| profile_surface(p) == AgentProfileSurface::Subagent);
    profiles
}

/// All profiles that can be used with `/agent run`.
///
/// Includes both primary profiles (which may also be runnable) and
/// subagent profiles. Excludes hidden profiles.
pub fn runnable_profiles(project_root: impl AsRef<Path>) -> Vec<AgentProfile> {
    let mut profiles = load_profiles(project_root);
    for p in product_profiles() {
        upsert_profile(&mut profiles, p);
    }
    profiles.retain(|p| profile_surface(p) != AgentProfileSurface::Hidden);
    profiles
}

pub fn profile_surface(profile: &AgentProfile) -> AgentProfileSurface {
    match profile.name.as_str() {
        "build" | "plan" | "explore" | "review" => AgentProfileSurface::Primary,
        _ => AgentProfileSurface::Subagent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles_are_available() {
        let profiles = load_profiles(".");
        assert!(profiles.iter().any(|profile| profile.name == "default"));
        assert!(profiles.iter().any(|profile| profile.name == "explorer"));
        assert!(profiles.iter().any(|profile| profile.name == "planner"));
        assert!(find_profile(".", "verifier").is_some());
        assert!(find_profile(".", "lab-professor").is_some());
        assert!(find_profile(".", "lab-postdoc").is_some());
        assert!(find_profile(".", "lab-graduate").is_some());
        let default = find_profile(".", "default").unwrap();
        assert_eq!(
            default.context,
            Some(AgentContextMode::IsolatedWorktreeFork)
        );
        let implementer = find_profile(".", "implementer").unwrap();
        assert_eq!(
            implementer.context,
            Some(AgentContextMode::IsolatedWorktreeFork)
        );
        assert_eq!(
            implementer.permission_mode,
            Some(AgentPermissionMode::IsolatedWrite)
        );
        assert_eq!(implementer.risk_policy, Some(AgentRiskPolicy::CodeChange));
        assert_eq!(
            implementer.output_contract,
            Some(AgentOutputContract::PatchSummary)
        );
    }

    #[test]
    fn product_profiles_are_runnable_by_name() {
        assert!(find_profile(".", "build").is_none());
        let build = find_runnable_profile(".", "build").unwrap();
        assert_eq!(build.name, "build");
        assert_eq!(build.role, AgentRole::Specialist);
        assert_eq!(build.risk_policy, Some(AgentRiskPolicy::CodeChange));

        let review = find_runnable_profile(".", "REVIEW").unwrap();
        assert_eq!(review.name, "review");
        assert_eq!(review.output_contract, Some(AgentOutputContract::Findings));
        assert!(!review
            .allowed_tools
            .iter()
            .any(|tool| tool == "file_edit" || tool == "file_write"));
    }

    #[test]
    fn builtin_profiles_have_role_scoped_tool_surfaces() {
        let profiles = load_profiles(".");
        for profile in profiles {
            assert!(
                !profile
                    .allowed_tools
                    .iter()
                    .any(|tool| tool == "agent" || tool == "swarm"),
                "builtin profile '{}' should not allow recursive delegation by default",
                profile.name
            );
            match profile.name.as_str() {
                "explorer" | "planner" | "verifier" => {
                    assert!(
                        !profile
                            .allowed_tools
                            .iter()
                            .any(|tool| tool == "file_write" || tool == "file_edit"),
                        "{} profile must not edit files",
                        profile.name
                    );
                }
                "lab-professor" => {
                    assert_eq!(profile.permission_mode, Some(AgentPermissionMode::ReadOnly));
                    assert_eq!(profile.prompt_version.as_deref(), Some("lab-professor.v1"));
                    assert!(!profile
                        .allowed_tools
                        .iter()
                        .any(|tool| tool == "file_write" || tool == "file_edit"));
                    assert!(profile.system_prompt.contains("principal investigator"));
                }
                "lab-postdoc" => {
                    assert_eq!(profile.permission_mode, Some(AgentPermissionMode::ReadOnly));
                    assert_eq!(profile.risk_policy, Some(AgentRiskPolicy::VerifyOnly));
                    assert!(!profile
                        .allowed_tools
                        .iter()
                        .any(|tool| tool == "file_write" || tool == "file_edit" || tool == "bash"));
                    assert!(profile
                        .system_prompt
                        .contains("technical execution quality"));
                    assert!(profile.system_prompt.contains("Stay read-only"));
                }
                "lab-graduate" => {
                    assert!(profile.disallowed_tools.contains(&"agent".to_string()));
                    assert!(profile.system_prompt.contains("one scoped task"));
                    assert!(profile.system_prompt.contains("graduate_result"));
                    assert!(profile.system_prompt.contains("no Markdown fence"));
                    assert!(profile.system_prompt.contains("pseudo tool tags"));
                    assert!(profile.allowed_tools.contains(&"file_write".to_string()));
                }
                "implementer" => {
                    assert!(profile.allowed_tools.contains(&"file_edit".to_string()));
                    assert!(profile.allowed_tools.contains(&"file_write".to_string()));
                }
                _ => {}
            }
        }
    }

    #[test]
    fn reserved_lab_profiles_cannot_be_overridden_by_project_profiles() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join(".priority-agent").join("agents");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("lab-graduate.toml"),
            r#"
name = "lab-graduate"
description = "malicious override"
allowed_tools = ["file_read", "mcp_tool"]
mcp_servers = ["github"]
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("writer.toml"),
            r#"
name = "writer"
description = "normal project profile"
allowed_tools = ["file_read"]
"#,
        )
        .unwrap();

        let graduate = find_runnable_profile(temp.path(), "lab-graduate").unwrap();
        assert_eq!(graduate.prompt_version.as_deref(), Some("lab-graduate.v1"));
        assert!(!graduate.allowed_tools.contains(&"mcp_tool".to_string()));
        assert!(graduate.mcp_servers.is_empty());

        let writer = find_runnable_profile(temp.path(), "writer").unwrap();
        assert_eq!(writer.description, "normal project profile");
        assert!(is_reserved_lab_profile_name("LAB-POSTDOC"));
        assert!(!is_reserved_lab_profile_name("writer"));
    }

    #[test]
    fn context_mode_legacy_aliases_deserialize() {
        let inherited: AgentProfile = toml::from_str(
            r#"
name = "legacy-inherit"
context = "inherit"
"#,
        )
        .unwrap();
        assert_eq!(inherited.context, Some(AgentContextMode::InheritedSummary));

        let forked: AgentProfile = toml::from_str(
            r#"
name = "legacy-fork"
context = "fork"
"#,
        )
        .unwrap();
        assert_eq!(forked.context, Some(AgentContextMode::FullFork));
    }

    #[test]
    fn context_mode_parse_accepts_runtime_aliases() {
        assert_eq!(
            AgentContextMode::parse("inherit"),
            Some(AgentContextMode::InheritedSummary)
        );
        assert_eq!(
            AgentContextMode::parse("fork"),
            Some(AgentContextMode::FullFork)
        );
        assert_eq!(
            AgentContextMode::parse("worktree"),
            Some(AgentContextMode::IsolatedWorktreeFork)
        );
        assert_eq!(AgentContextMode::parse("unknown"), None);
    }

    #[test]
    fn builtin_profiles_normalize_to_agent_definitions() {
        let definitions = load_definitions(".");
        let explorer = definitions
            .iter()
            .find(|definition| definition.name == "explorer")
            .unwrap();
        assert_eq!(explorer.context_mode, AgentContextMode::InheritedSummary);
        assert_eq!(explorer.permission_mode, AgentPermissionMode::ReadOnly);
        assert_eq!(explorer.memory_policy, AgentMemoryPolicy::None);
        assert!(!explorer.context_mode.copies_full_history());

        let implementer = definitions
            .iter()
            .find(|definition| definition.name == "implementer")
            .unwrap();
        assert_eq!(
            implementer.context_mode,
            AgentContextMode::IsolatedWorktreeFork
        );
        assert_eq!(
            implementer.permission_mode,
            AgentPermissionMode::IsolatedWrite
        );
        assert_eq!(
            implementer.output_contract,
            AgentOutputContract::PatchSummary
        );
        assert!(implementer.context_mode.copies_full_history());
        assert!(implementer.context_mode.requires_isolated_worktree());
        assert!(implementer
            .envelope_constraints()
            .contains(&"agent_definition=implementer".to_string()));

        let professor = definitions
            .iter()
            .find(|definition| definition.name == "lab-professor")
            .unwrap();
        assert_eq!(
            professor.prompt_version.as_deref(),
            Some("lab-professor.v1")
        );
        assert!(professor
            .envelope_constraints()
            .contains(&"prompt_version=lab-professor.v1".to_string()));

        let lab_roles = crate::lab::model::LabRoles::default();
        assert_eq!(
            professor.prompt_version.as_deref(),
            Some(lab_roles.professor.prompt_version.as_str())
        );
    }

    #[test]
    fn code_change_profiles_default_to_isolated_worktree_definition() {
        let profile = AgentProfile {
            name: "writer".to_string(),
            description: "writes code".to_string(),
            role: AgentRole::Specialist,
            system_prompt: String::new(),
            prompt_version: None,
            allowed_tools: vec!["file_edit".to_string(), "bash".to_string()],
            disallowed_tools: Vec::new(),
            context: None,
            permission_mode: None,
            risk_policy: None,
            output_contract: None,
            model: None,
            effort: None,
            mcp_servers: Vec::new(),
            memory: None,
            timeout_secs: None,
            max_turns: None,
            max_cost_usd: None,
        };

        let definition = profile.to_definition();

        assert_eq!(definition.risk_policy, AgentRiskPolicy::CodeChange);
        assert_eq!(
            definition.context_mode,
            AgentContextMode::IsolatedWorktreeFork
        );
        assert_eq!(
            definition.permission_mode,
            AgentPermissionMode::IsolatedWrite
        );
    }

    #[test]
    fn primary_profiles_include_only_switchable_modes() {
        let profiles = primary_profiles(".");
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"build"));
        assert!(names.contains(&"plan"));
        assert!(names.contains(&"explore"));
        assert!(names.contains(&"review"));
        // verify, default, explorer, implementer are NOT primary
        assert!(!names.contains(&"verify"));
        assert!(!names.contains(&"default"));
        assert!(!names.contains(&"explorer"));
        assert!(!names.contains(&"implementer"));
    }

    #[test]
    fn subagent_profiles_include_workers() {
        let profiles = subagent_profiles(".");
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"default"));
        assert!(names.contains(&"explorer"));
        assert!(names.contains(&"verifier"));
        assert!(names.contains(&"implementer"));
        // primary profiles are not subagent
        assert!(!names.contains(&"build"));
        assert!(!names.contains(&"plan"));
    }

    #[test]
    fn runnable_profiles_include_both_primary_and_subagent() {
        let profiles = runnable_profiles(".");
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"build"));
        assert!(names.contains(&"explorer"));
        assert!(names.contains(&"verify"));
    }

    #[test]
    fn scout_profile_is_subagent_not_primary() {
        assert_eq!(
            profile_surface(&find_runnable_profile(".", "scout").unwrap()),
            AgentProfileSurface::Subagent
        );
    }

    #[test]
    fn primary_profiles_have_non_empty_system_prompts() {
        for profile in primary_profiles(".") {
            assert!(
                !profile.system_prompt.trim().is_empty(),
                "primary profile '{}' should have a non-empty system_prompt",
                profile.name
            );
        }
    }

    #[test]
    fn primary_profiles_all_map_to_agent_mode() {
        for profile in primary_profiles(".") {
            let mode = crate::engine::agent_mode::AgentMode::parse(&profile.name);
            assert!(
                mode.is_some(),
                "primary profile '{}' should map to an AgentMode variant",
                profile.name
            );
        }
    }

    #[test]
    fn read_only_primary_profiles_do_not_allow_code_changes() {
        for profile in primary_profiles(".") {
            if profile.name != "build" {
                assert_ne!(
                    profile.risk_policy,
                    Some(AgentRiskPolicy::CodeChange),
                    "non-build primary profile '{}' should not have CodeChange risk policy",
                    profile.name
                );
            }
        }
    }
}

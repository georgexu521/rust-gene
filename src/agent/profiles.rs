//! Named subagent profiles.

use crate::agent::roles::AgentRole;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    pub when_to_use: String,
    pub role: AgentRole,
    #[serde(default, skip_serializing)]
    pub system_prompt: String,
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
            when_to_use: self.description.clone(),
            role: self.role,
            system_prompt: self.system_prompt.clone(),
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
            name: "implementer".to_string(),
            description: "Focused code-change worker".to_string(),
            role: AgentRole::Specialist,
            system_prompt: "Make focused edits and report changed files clearly.".to_string(),
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
            system_prompt: String::new(),
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
            system_prompt: String::new(),
            allowed_tools: vec!["file_read".into(), "glob".into(), "grep".into(), "project_list".into(), "ask_user".into(), "todo_write".into()],
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
            system_prompt: String::new(),
            allowed_tools: vec!["file_read".into(), "glob".into(), "grep".into(), "project_list".into(), "ask_user".into(), "web_search".into(), "web_fetch".into(), "lsp".into()],
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
            system_prompt: String::new(),
            allowed_tools: vec!["file_read".into(), "git_diff".into(), "git_status".into(), "git_log".into(), "grep".into(), "glob".into(), "project_list".into(), "ask_user".into(), "lsp".into()],
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
            system_prompt: String::new(),
            allowed_tools: vec!["file_read".into(), "grep".into(), "bash".into(), "git_diff".into(), "git_status".into(), "project_list".into(), "ask_user".into()],
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
    ]
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
                "implementer" => {
                    assert!(profile.allowed_tools.contains(&"file_edit".to_string()));
                    assert!(profile.allowed_tools.contains(&"file_write".to_string()));
                }
                _ => {}
            }
        }
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
    }

    #[test]
    fn code_change_profiles_default_to_isolated_worktree_definition() {
        let profile = AgentProfile {
            name: "writer".to_string(),
            description: "writes code".to_string(),
            role: AgentRole::Specialist,
            system_prompt: String::new(),
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
        assert!(definition.context_mode.requires_isolated_worktree());
    }
}

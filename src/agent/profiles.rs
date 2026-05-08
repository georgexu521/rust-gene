//! Named subagent profiles.

use crate::agent::roles::AgentRole;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentContextMode {
    Inherit,
    Fork,
    Minimal,
}

impl std::fmt::Display for AgentContextMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AgentContextMode::Inherit => "inherit",
            AgentContextMode::Fork => "fork",
            AgentContextMode::Minimal => "minimal",
        };
        write!(f, "{}", label)
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
    pub context: Option<AgentContextMode>,
    #[serde(default)]
    pub risk_policy: Option<AgentRiskPolicy>,
    #[serde(default)]
    pub output_contract: Option<AgentOutputContract>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub max_turns: Option<usize>,
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
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
            context: Some(AgentContextMode::Inherit),
            risk_policy: Some(AgentRiskPolicy::CodeChange),
            output_contract: Some(AgentOutputContract::PatchSummary),
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
            context: Some(AgentContextMode::Inherit),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
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
            context: Some(AgentContextMode::Inherit),
            risk_policy: Some(AgentRiskPolicy::ReadOnly),
            output_contract: Some(AgentOutputContract::Findings),
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
            context: Some(AgentContextMode::Inherit),
            risk_policy: Some(AgentRiskPolicy::VerifyOnly),
            output_contract: Some(AgentOutputContract::VerificationReport),
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
            context: Some(AgentContextMode::Fork),
            risk_policy: Some(AgentRiskPolicy::CodeChange),
            output_contract: Some(AgentOutputContract::PatchSummary),
            timeout_secs: Some(600),
            max_turns: Some(10),
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
        let implementer = find_profile(".", "implementer").unwrap();
        assert_eq!(implementer.context, Some(AgentContextMode::Fork));
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
}

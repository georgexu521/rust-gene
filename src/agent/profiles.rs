//! Named subagent profiles.

use crate::agent::roles::AgentRole;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    pub context: Option<String>,
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
            name: "explorer".to_string(),
            description: "Read-only codebase explorer".to_string(),
            role: AgentRole::Plan,
            system_prompt: "Focus on discovering structure and risks. Do not edit files."
                .to_string(),
            allowed_tools: vec!["project_list".into(), "grep".into(), "file_read".into()],
            context: Some("inherit".to_string()),
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
            context: Some("inherit".to_string()),
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
                "grep".into(),
                "file_read".into(),
                "file_edit".into(),
                "bash".into(),
            ],
            context: Some("inherit".to_string()),
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
        assert!(profiles.iter().any(|profile| profile.name == "explorer"));
        assert!(find_profile(".", "verifier").is_some());
    }
}

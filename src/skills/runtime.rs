//! Runtime skill registry used by the Priority Agent CLI and tools.

use super::registry::SkillRegistry;
use super::types::Skill;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SkillRuntime {
    skills: HashMap<String, Skill>,
    project_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SkillInvocation {
    pub name: String,
    pub prompt: String,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub context: Option<String>,
}

impl SkillRuntime {
    pub fn load(project_root: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        let skills = load_skills(&project_root);
        Self {
            skills,
            project_root,
        }
    }

    pub fn reload(&mut self) -> usize {
        self.skills = load_skills(&self.project_root);
        self.skills.len()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        let normalized = normalize_skill_name(name);
        self.skills.get(&normalized)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names = self.skills.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn list(&self) -> Vec<&Skill> {
        let mut skills = self.skills.values().collect::<Vec<_>>();
        skills.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
        skills
    }

    pub fn search(&self, query: &str) -> Vec<&Skill> {
        let keywords = query
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        if keywords.is_empty() {
            return self.list();
        }
        let mut matches = self
            .skills
            .values()
            .filter(|skill| skill.matches(&keywords))
            .collect::<Vec<_>>();
        matches.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
        matches
    }

    pub fn discovery_summary(&self, query: &str, limit: usize) -> String {
        let skills = self.search(query);
        if skills.is_empty() {
            return "No matching skills found.".to_string();
        }

        let lines = skills
            .into_iter()
            .take(limit)
            .map(|skill| skill.discovery_summary())
            .collect::<Vec<_>>();
        format!("Skills ({} shown):\n{}", lines.len(), lines.join("\n"))
    }

    pub fn invocation(&self, name: &str, task: &str) -> Option<SkillInvocation> {
        let skill = self.get(name)?;
        if !skill.meta.user_invocable {
            return None;
        }
        Some(SkillInvocation {
            name: skill.meta.name.clone(),
            prompt: render_skill_invocation(skill, task),
            allowed_tools: skill.meta.allowed_tools.clone(),
            disallowed_tools: skill.meta.disallowed_tools.clone(),
            model: skill.meta.model.clone(),
            effort: skill.meta.effort.clone(),
            context: skill.meta.context.clone(),
        })
    }

    pub fn invocation_prompt(&self, name: &str, task: &str) -> Option<String> {
        self.invocation(name, task)
            .map(|invocation| invocation.prompt)
    }
}

fn load_skills(project_root: &Path) -> HashMap<String, Skill> {
    let mut registry = SkillRegistry::new().with_default_paths(project_root);
    registry.load_bundled();
    registry.discover_and_load();

    registry
        .list()
        .into_iter()
        .map(|skill| (normalize_skill_name(&skill.meta.name), skill.clone()))
        .collect()
}

fn normalize_skill_name(name: &str) -> String {
    name.trim_start_matches('/')
        .replace('_', "-")
        .to_ascii_lowercase()
}

fn render_skill_invocation(skill: &Skill, task: &str) -> String {
    let mut out = String::new();
    out.push_str(&skill.to_injection());
    if !skill.meta.allowed_tools.is_empty() {
        out.push_str("\nAllowed tools for this skill: ");
        out.push_str(&skill.meta.allowed_tools.join(", "));
        out.push('\n');
    }
    if !skill.meta.disallowed_tools.is_empty() {
        out.push_str("Disallowed tools for this skill: ");
        out.push_str(&skill.meta.disallowed_tools.join(", "));
        out.push('\n');
    }
    if let Some(model) = &skill.meta.model {
        out.push_str(&format!("Preferred model: {}\n", model));
    }
    if let Some(effort) = &skill.meta.effort {
        out.push_str(&format!("Preferred effort: {}\n", effort));
    }
    if let Some(context) = &skill.meta.context {
        out.push_str(&format!("Context mode: {}\n", context));
    }
    if !task.trim().is_empty() {
        out.push_str("\nUser task:\n");
        out.push_str(task.trim());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_skill_is_directly_invocable() {
        let runtime = SkillRuntime::load(".");
        let prompt = runtime
            .invocation_prompt("karpathy-guidelines", "review this")
            .unwrap();
        assert!(prompt.contains("review this"));
        assert!(prompt.contains("Skill:"));
        assert!(prompt.contains("not user instruction text"));
    }

    #[test]
    fn underscore_and_slash_names_are_normalized() {
        let runtime = SkillRuntime::load(".");
        assert!(runtime.get("/review_pr").is_some());
    }
}

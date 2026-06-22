//! Agent tool support module.
//!
//! Holds result-state and support helpers for delegated agent execution without widening the public tool contract.

use super::*;
use std::path::{Path, PathBuf};

/// 加载文件上下文
pub(super) async fn load_file_context(files: &[String], working_dir: &Path) -> String {
    let mut context = String::new();
    for file in files {
        let path = match crate::tools::file_tool::resolve_path(file, working_dir) {
            Ok(p) => p,
            Err(e) => {
                warn!("Access denied for context file '{}': {}", file, e);
                context.push_str(&format!("\n## File: {}\n(access denied: {})\n", file, e));
                continue;
            }
        };
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                context.push_str(&format!("\n## File: {}\n```\n{}\n```\n", file, content));
            }
            Err(e) => {
                warn!("Failed to load context file {}: {}", path.display(), e);
                context.push_str(&format!(
                    "\n## File: {}\n(error loading file: {})\n",
                    file, e
                ));
            }
        }
    }
    context
}

/// 构建完整的系统提示词
pub(super) fn build_system_prompt(
    template: Option<AgentTemplate>,
    role: AgentRole,
    description: &str,
    prompt: &str,
    file_context: &str,
) -> String {
    let role_prefix = role.system_prompt_prefix();
    let base_prompt = match template {
        Some(t) => t.build_system_prompt(description, prompt),
        None => format!(
            "You are a sub-agent tasked with: {}\n\n\
             Instructions: {}\n\n\
             Complete this task and report back with your findings. Be thorough and detailed in your response.",
            description, prompt
        ),
    };

    if file_context.is_empty() {
        format!("{}\n\n{}", role_prefix, base_prompt)
    } else {
        format!(
            "{}\n\n{}\n\nRelevant files are provided below:\n{}",
            role_prefix, base_prompt, file_context
        )
    }
}

#[derive(Debug, Clone)]
pub(super) struct IsolatedAgentWorktree {
    pub(super) path: PathBuf,
    pub(super) branch: String,
}

pub(super) async fn create_isolated_agent_worktree(
    context: &ToolContext,
    description: &str,
) -> anyhow::Result<IsolatedAgentWorktree> {
    let manager = context
        .worktree_manager
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("isolated_worktree_fork requires a WorktreeManager"))?;
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let suffix = &uuid[..8];
    let slug = isolated_worktree_slug(description);
    let name = format!("agent-{}-{}", slug, suffix);
    let branch = format!("codex/agent-{}", suffix);
    let path = manager.create(&name, Some(&branch)).await?;
    Ok(IsolatedAgentWorktree { path, branch })
}

pub(super) fn isolated_worktree_slug(description: &str) -> String {
    let mut slug = String::new();
    for ch in description.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= 32 {
            break;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "worker".to_string()
    } else {
        slug.to_string()
    }
}

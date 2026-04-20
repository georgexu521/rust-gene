//! Layered instruction loader for AGENTS.md.
//!
//! Load order (low to high priority):
//! 1. Global: ~/.priority-agent/AGENTS.md
//! 2. Project root: <repo-root>/AGENTS.md
//! 3. Directory-specific: <repo-root>/.../<cwd>/AGENTS.md

use std::path::{Path, PathBuf};
use tracing::debug;

const FILE_NAME: &str = "AGENTS.md";
const PER_LAYER_CHAR_LIMIT: usize = 4_000;
const TOTAL_CHAR_LIMIT: usize = 16_000;

#[derive(Debug, Clone)]
pub struct InstructionLayer {
    pub source: String,
    pub path: PathBuf,
    pub content: String,
}

fn global_agents_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".priority-agent").join(FILE_NAME))
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join(".git").exists() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

fn read_layer(path: &Path, source: impl Into<String>) -> Option<InstructionLayer> {
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    let clipped: String = trimmed.chars().take(PER_LAYER_CHAR_LIMIT).collect();
    Some(InstructionLayer {
        source: source.into(),
        path: path.to_path_buf(),
        content: clipped,
    })
}

fn load_instruction_layers_internal(
    working_dir: &Path,
    global_override: Option<&Path>,
) -> Vec<InstructionLayer> {
    let mut layers = Vec::new();

    let global_path = global_override
        .map(|p| p.to_path_buf())
        .or_else(global_agents_path);
    if let Some(path) = global_path {
        if path.exists() {
            if let Some(layer) = read_layer(&path, "global") {
                layers.push(layer);
            }
        }
    }

    let root = find_project_root(working_dir).unwrap_or_else(|| working_dir.to_path_buf());

    let root_agents = root.join(FILE_NAME);
    if root_agents.exists() {
        if let Some(layer) = read_layer(&root_agents, "project") {
            layers.push(layer);
        }
    }

    // Directory-specific AGENTS.md from root child to cwd.
    let mut ancestors = Vec::new();
    let mut cur = working_dir.to_path_buf();
    loop {
        ancestors.push(cur.clone());
        if cur == root {
            break;
        }
        if !cur.pop() {
            break;
        }
    }
    ancestors.reverse();
    for dir in ancestors {
        if dir == root {
            continue;
        }
        let p = dir.join(FILE_NAME);
        if p.exists() {
            if let Some(layer) = read_layer(&p, "directory") {
                layers.push(layer);
            }
        }
    }

    layers
}

pub fn load_instruction_layers(working_dir: &Path) -> Vec<InstructionLayer> {
    load_instruction_layers_internal(working_dir, None)
}

pub fn compose_system_prompt(base_prompt: &str, working_dir: &Path) -> String {
    let layers = load_instruction_layers(working_dir);
    if layers.is_empty() {
        debug!(
            "No AGENTS.md layers found for working_dir={}",
            working_dir.display()
        );
        return base_prompt.to_string();
    }

    debug!(
        "Loaded {} AGENTS.md layer(s) for {}",
        layers.len(),
        working_dir.display()
    );

    let mut out = String::from(base_prompt);
    out.push_str("\n\n## Layered Instructions (AGENTS.md)\n");
    out.push_str(
        "Apply these instructions in order; later layers override earlier ones when conflicts exist.\n",
    );

    let mut used = 0usize;
    for layer in layers {
        debug!("Applying AGENTS.md layer: [{}] {}", layer.source, layer.path.display());
        if used >= TOTAL_CHAR_LIMIT {
            debug!(
                "AGENTS.md total char limit reached ({}), truncating remaining layers",
                TOTAL_CHAR_LIMIT
            );
            break;
        }
        let remaining = TOTAL_CHAR_LIMIT.saturating_sub(used);
        let clipped: String = layer.content.chars().take(remaining).collect();
        used += clipped.chars().count();
        out.push_str(&format!(
            "\n### [{}] {}\n{}\n",
            layer.source,
            layer.path.display(),
            clipped
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compose_without_layers() {
        let tmp = std::env::temp_dir().join(format!("agents-none-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let base = "base prompt";
        let got = compose_system_prompt(base, &tmp);
        assert_eq!(got, base);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_layer_order() {
        let base = std::env::temp_dir().join(format!("agents-order-{}", uuid::Uuid::new_v4()));
        let repo = base.join("repo");
        let sub = repo.join("src").join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();

        let global = base.join("global-agents.md");
        std::fs::write(&global, "global rules").unwrap();
        std::fs::write(repo.join(FILE_NAME), "project rules").unwrap();
        std::fs::write(repo.join("src").join(FILE_NAME), "src rules").unwrap();
        std::fs::write(sub.join(FILE_NAME), "nested rules").unwrap();

        let layers = load_instruction_layers_internal(&sub, Some(&global));
        let contents: Vec<String> = layers.into_iter().map(|l| l.content).collect();
        assert_eq!(
            contents,
            vec![
                "global rules".to_string(),
                "project rules".to_string(),
                "src rules".to_string(),
                "nested rules".to_string()
            ]
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_layer_char_limit_applied() {
        let base = std::env::temp_dir().join(format!("agents-limit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(base.join(".git")).unwrap();
        let long = "x".repeat(PER_LAYER_CHAR_LIMIT + 500);
        std::fs::write(base.join(FILE_NAME), long).unwrap();

        let layers = load_instruction_layers(&base);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].content.chars().count(), PER_LAYER_CHAR_LIMIT);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_compose_includes_layer_header() {
        let base = std::env::temp_dir().join(format!("agents-compose-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(base.join(".git")).unwrap();
        std::fs::write(base.join(FILE_NAME), "project directives").unwrap();

        let prompt = compose_system_prompt("base prompt", &base);
        assert!(prompt.contains("Layered Instructions (AGENTS.md)"));
        assert!(prompt.contains("project directives"));

        let _ = std::fs::remove_dir_all(&base);
    }
}

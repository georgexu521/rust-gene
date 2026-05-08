//! Layered instruction loader for AGENTS.md.
//!
//! Load order (low to high priority):
//! 1. Global: `~/.priority-agent/AGENTS.md`
//! 2. Project root: `<repo-root>/AGENTS.md`
//! 3. Directory-specific: `<repo-root>/.../<cwd>/AGENTS.md`

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tracing::debug;

const FILE_NAME: &str = "AGENTS.md";
const PER_LAYER_CHAR_LIMIT: usize = 4_000;
const TOTAL_CHAR_LIMIT: usize = 16_000;
const RUNTIME_GUIDANCE_HEADING: &str = "Agent Runtime Guidance";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionLayerSelection {
    RuntimeGuidanceSection,
    FullFileFallback,
    FullFileEnvOverride,
}

impl InstructionLayerSelection {
    pub fn label(self) -> &'static str {
        match self {
            Self::RuntimeGuidanceSection => "runtime-guidance",
            Self::FullFileFallback => "full-file-fallback",
            Self::FullFileEnvOverride => "full-file-env",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstructionLayer {
    pub source: String,
    pub path: PathBuf,
    pub content: String,
    pub selection: InstructionLayerSelection,
    pub truncated: bool,
    pub original_chars: usize,
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

fn full_agents_loading_enabled() -> bool {
    matches!(
        std::env::var("PRIORITY_AGENT_AGENTS_MD_FULL")
            .ok()
            .as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 {
        return None;
    }
    let rest = trimmed.get(level..)?;
    if !rest.starts_with(' ') {
        return None;
    }
    Some((level, rest.trim()))
}

fn extract_runtime_guidance_section(content: &str) -> Option<String> {
    let mut in_section = false;
    let mut selected = Vec::new();

    for line in content.lines() {
        if let Some((level, title)) = markdown_heading(line) {
            if in_section && level <= 2 {
                break;
            }
            if level == 2 && title == RUNTIME_GUIDANCE_HEADING {
                in_section = true;
                selected.push(line);
                continue;
            }
        }
        if in_section {
            selected.push(line);
        }
    }

    if selected.is_empty() {
        None
    } else {
        Some(selected.join("\n"))
    }
}

fn select_prompt_visible_content(content: &str) -> (Cow<'_, str>, InstructionLayerSelection) {
    if full_agents_loading_enabled() {
        return (
            Cow::Borrowed(content),
            InstructionLayerSelection::FullFileEnvOverride,
        );
    }

    if let Some(section) = extract_runtime_guidance_section(content) {
        return (
            Cow::Owned(section),
            InstructionLayerSelection::RuntimeGuidanceSection,
        );
    }

    (
        Cow::Borrowed(content),
        InstructionLayerSelection::FullFileFallback,
    )
}

fn read_layer(path: &Path, source: impl Into<String>) -> Option<InstructionLayer> {
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (selected, selection) = select_prompt_visible_content(trimmed);
    let selected = selected.trim();
    if selected.is_empty() {
        return None;
    }
    let original_chars = selected.chars().count();
    let clipped: String = selected.chars().take(PER_LAYER_CHAR_LIMIT).collect();
    let truncated = original_chars > PER_LAYER_CHAR_LIMIT;
    Some(InstructionLayer {
        source: source.into(),
        path: path.to_path_buf(),
        content: clipped,
        selection,
        truncated,
        original_chars,
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
    let workspace = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());
    let mut out = String::from(base_prompt);
    out.push_str("\n\n## Workspace Boundary\n");
    out.push_str(&format!("- Current workspace: `{}`\n", workspace.display()));
    out.push_str(
        "- Treat this directory as the active project root for this session.\n\
         - Resolve relative paths against this workspace.\n\
         - Do not read, write, or inspect files outside this workspace unless the user explicitly asks for that path.\n\
         - If a remembered or suggested absolute path points outside this workspace, re-check the current workspace instead of using it.\n",
    );

    if layers.is_empty() {
        debug!(
            "No AGENTS.md layers found for working_dir={}",
            working_dir.display()
        );
        return out;
    }

    debug!(
        "Loaded {} AGENTS.md layer(s) for {}",
        layers.len(),
        working_dir.display()
    );

    out.push_str("\n\n## Layered Instructions (AGENTS.md)\n");
    out.push_str(
        "Apply these instructions in order; later layers override earlier ones when conflicts exist.\n",
    );

    let mut used = 0usize;
    for layer in layers {
        debug!(
            "Applying AGENTS.md layer: [{}:{}] {} truncated={} selected_chars={}",
            layer.source,
            layer.selection.label(),
            layer.path.display(),
            layer.truncated,
            layer.original_chars
        );
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
        let truncated_label = if layer.truncated { " truncated" } else { "" };
        out.push_str(&format!(
            "\n### [{}:{}{}] {}\n{}\n",
            layer.source,
            layer.selection.label(),
            truncated_label,
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
        assert!(got.starts_with(base));
        assert!(got.contains("Workspace Boundary"));
        assert!(got.contains("Current workspace"));
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
    fn test_runtime_guidance_section_is_preferred() {
        let base = std::env::temp_dir().join(format!("agents-section-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(base.join(".git")).unwrap();
        std::fs::write(
            base.join(FILE_NAME),
            "# Project Notes\n\
             archived intro\n\n\
             ## Agent Runtime Guidance\n\
             runtime rule\n\
             ### Nested Detail\n\
             keep nested detail\n\n\
             ## Archive\n\
             old doctrine",
        )
        .unwrap();

        let layers = load_instruction_layers(&base);

        assert_eq!(layers.len(), 1);
        assert_eq!(
            layers[0].selection,
            InstructionLayerSelection::RuntimeGuidanceSection
        );
        assert!(layers[0].content.contains("runtime rule"));
        assert!(layers[0].content.contains("keep nested detail"));
        assert!(!layers[0].content.contains("archived intro"));
        assert!(!layers[0].content.contains("old doctrine"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_full_file_fallback_when_runtime_section_absent() {
        let base = std::env::temp_dir().join(format!("agents-fallback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(base.join(".git")).unwrap();
        std::fs::write(base.join(FILE_NAME), "# Project Notes\nproject rules").unwrap();

        let layers = load_instruction_layers(&base);

        assert_eq!(layers.len(), 1);
        assert_eq!(
            layers[0].selection,
            InstructionLayerSelection::FullFileFallback
        );
        assert!(layers[0].content.contains("project rules"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn project_agents_runtime_guide_stays_under_prompt_budget() {
        let root_agents = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_NAME);
        let content = std::fs::read_to_string(&root_agents).unwrap();
        let chars = content.trim().chars().count();

        assert!(
            chars <= PER_LAYER_CHAR_LIMIT,
            "{} has {chars} chars; keep prompt-visible project guidance under {PER_LAYER_CHAR_LIMIT}",
            root_agents.display()
        );
    }

    #[test]
    fn project_agents_runtime_guide_excludes_archived_doctrine() {
        let root_agents = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_NAME);
        let layer = read_layer(&root_agents, "project").unwrap();
        let forbidden = [
            "完整的 Socratic 执行流程",
            "高密度思考 = 高密度提问-解答",
            "总计 25 个工具",
            "Phase 4 进行中",
        ];

        for phrase in forbidden {
            assert!(
                !layer.content.contains(phrase),
                "archived project-history phrase leaked into prompt-visible AGENTS.md: {phrase}"
            );
        }
    }

    #[test]
    fn project_agents_uses_runtime_guidance_section() {
        let root_agents = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_NAME);
        let layer = read_layer(&root_agents, "project").unwrap();

        assert_eq!(
            layer.selection,
            InstructionLayerSelection::RuntimeGuidanceSection
        );
        assert!(layer.content.contains("## Agent Runtime Guidance"));
        assert!(!layer
            .content
            .contains("Only the `## Agent Runtime Guidance` section"));
    }

    #[test]
    fn archived_agents_project_guide_remains_available() {
        let archive = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs")
            .join("archive")
            .join("AGENTS_PROJECT_GUIDE_PRE_RUNTIME_DIET_2026-05-08.md");
        let content = std::fs::read_to_string(&archive).unwrap();

        assert!(content.contains("完整的 Socratic 执行流程"));
        assert!(content.contains("开发记录"));
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

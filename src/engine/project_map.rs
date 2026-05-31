//! Compact project map loading for runtime navigation context.
//!
//! `docs/PROJECT_MAP.md` is a human-maintained orientation layer. Runtime only
//! injects its bounded agent section, so the map can reduce broad scans without
//! becoming another large prompt blob.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const PROJECT_MAP_PATH: &str = "docs/PROJECT_MAP.md";
pub const DEFAULT_PROJECT_MAP_MAX_CHARS: usize = 3_200;

const AGENT_CONTEXT_START: &str = "<!-- agent-context:start -->";
const AGENT_CONTEXT_END: &str = "<!-- agent-context:end -->";
const PROJECT_MAP_ENV: &str = "PRIORITY_AGENT_PROJECT_MAP";
const PROJECT_MAP_MAX_CHARS_ENV: &str = "PRIORITY_AGENT_PROJECT_MAP_MAX_CHARS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectMapFreshness {
    Current,
    Stale { newer_files: usize },
    Unknown,
}

impl ProjectMapFreshness {
    pub fn label(&self) -> String {
        match self {
            Self::Current => "current".to_string(),
            Self::Stale { newer_files } => format!("stale: {newer_files} watched file(s) newer"),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMapZone {
    pub source: PathBuf,
    pub content: String,
    pub chars: usize,
    pub truncated: bool,
    pub freshness: ProjectMapFreshness,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectSymbolIndex {
    pub schema_version: u8,
    pub files: Vec<ProjectIndexedFile>,
    pub total_symbols: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectIndexedFile {
    pub path: String,
    pub hash: String,
    pub lines: usize,
    pub summary: String,
    pub symbols: Vec<ProjectIndexedSymbol>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectIndexedSymbol {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub signature: String,
}

pub fn project_map_runtime_enabled() -> bool {
    match std::env::var(PROJECT_MAP_ENV) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => !cfg!(test),
    }
}

pub fn configured_project_map_max_chars() -> usize {
    std::env::var(PROJECT_MAP_MAX_CHARS_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value >= 800)
        .unwrap_or(DEFAULT_PROJECT_MAP_MAX_CHARS)
}

pub fn load_project_map_zone(root: &Path) -> Option<ProjectMapZone> {
    load_project_map_zone_with_limit(root, configured_project_map_max_chars())
}

pub fn load_project_map_zone_with_limit(root: &Path, max_chars: usize) -> Option<ProjectMapZone> {
    let source = root.join(PROJECT_MAP_PATH);
    let raw = std::fs::read_to_string(&source).ok()?;
    let agent_context = extract_agent_context(&raw).unwrap_or(raw.as_str());
    let (snippet, truncated) = truncate_chars(agent_context.trim(), max_chars);
    let freshness = project_map_freshness(root, &source);
    let content = format!(
        "Project map source: {PROJECT_MAP_PATH}\nFreshness: {}\nPolicy: use this as navigation before broad repo scans; verify exact code with file_read/symbol_query before editing. If module/file responsibilities change, update docs/PROJECT_MAP.md in the same change.\n\n{}{}",
        freshness.label(),
        snippet.trim(),
        if truncated {
            "\n\n[project map truncated by runtime budget]"
        } else {
            ""
        }
    );
    Some(ProjectMapZone {
        source,
        chars: content.chars().count(),
        content,
        truncated,
        freshness,
    })
}

pub fn build_project_symbol_index(
    root: &Path,
    max_files: usize,
    max_symbols_per_file: usize,
) -> ProjectSymbolIndex {
    let mut index = crate::engine::symbol_index::SymbolIndex::new();
    index.index_project(root);

    let mut by_file: BTreeMap<PathBuf, Vec<&crate::engine::symbol_index::Symbol>> = BTreeMap::new();
    for symbol in index.all_symbols() {
        by_file.entry(symbol.file.clone()).or_default().push(symbol);
    }

    let mut truncated = by_file.len() > max_files;
    let mut files = Vec::new();
    for (path, symbols) in by_file.into_iter().take(max_files) {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .to_string();
        let total_symbols = symbols.len();
        if total_symbols > max_symbols_per_file {
            truncated = true;
        }
        let indexed_symbols = symbols
            .into_iter()
            .take(max_symbols_per_file)
            .map(|symbol| ProjectIndexedSymbol {
                name: symbol.name.clone(),
                kind: symbol_kind_label(&symbol.kind).to_string(),
                line: symbol.line + 1,
                signature: symbol.signature.clone().unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        files.push(ProjectIndexedFile {
            path: relative,
            hash: format!("{:x}", md5::compute(content.as_bytes())),
            lines: content.lines().count(),
            summary: file_symbol_summary(total_symbols, &indexed_symbols),
            symbols: indexed_symbols,
        });
    }

    ProjectSymbolIndex {
        schema_version: 1,
        total_symbols: index.len(),
        files,
        truncated,
    }
}

fn extract_agent_context(raw: &str) -> Option<&str> {
    let start = raw.find(AGENT_CONTEXT_START)? + AGENT_CONTEXT_START.len();
    let end = raw[start..].find(AGENT_CONTEXT_END)? + start;
    Some(&raw[start..end])
}

fn symbol_kind_label(kind: &crate::engine::symbol_index::SymbolKind) -> &'static str {
    match kind {
        crate::engine::symbol_index::SymbolKind::Function => "function",
        crate::engine::symbol_index::SymbolKind::Struct => "struct",
        crate::engine::symbol_index::SymbolKind::Enum => "enum",
        crate::engine::symbol_index::SymbolKind::Trait => "trait",
        crate::engine::symbol_index::SymbolKind::Impl => "impl",
        crate::engine::symbol_index::SymbolKind::Module => "module",
        crate::engine::symbol_index::SymbolKind::Variable => "variable",
        crate::engine::symbol_index::SymbolKind::TypeAlias => "type_alias",
        crate::engine::symbol_index::SymbolKind::Macro => "macro",
        crate::engine::symbol_index::SymbolKind::Unknown => "unknown",
    }
}

fn file_symbol_summary(total_symbols: usize, symbols: &[ProjectIndexedSymbol]) -> String {
    if symbols.is_empty() {
        return "no indexed symbols".to_string();
    }
    let names = symbols
        .iter()
        .take(5)
        .map(|symbol| format!("{} {}", symbol.kind, symbol.name))
        .collect::<Vec<_>>()
        .join(", ");
    if total_symbols > symbols.len() {
        format!("{total_symbols} symbols; first indexed: {names}")
    } else {
        format!("{total_symbols} symbols: {names}")
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> (String, bool) {
    let total = input.chars().count();
    if total <= max_chars {
        return (input.to_string(), false);
    }
    (input.chars().take(max_chars).collect(), true)
}

fn project_map_freshness(root: &Path, map_path: &Path) -> ProjectMapFreshness {
    let Ok(map_meta) = std::fs::metadata(map_path) else {
        return ProjectMapFreshness::Unknown;
    };
    let Ok(map_mtime) = map_meta.modified() else {
        return ProjectMapFreshness::Unknown;
    };

    let mut newer_files = 0usize;
    for watched in watched_project_paths(root) {
        count_newer_files(&watched, map_path, map_mtime, &mut newer_files, 2_500);
    }

    if newer_files == 0 {
        ProjectMapFreshness::Current
    } else {
        ProjectMapFreshness::Stale { newer_files }
    }
}

fn watched_project_paths(root: &Path) -> Vec<PathBuf> {
    [
        "src",
        "Cargo.toml",
        "Cargo.lock",
        "scripts",
        "AGENTS.md",
        "docs/PROJECT_STATUS.md",
    ]
    .iter()
    .map(|path| root.join(path))
    .collect()
}

fn count_newer_files(
    path: &Path,
    map_path: &Path,
    map_mtime: SystemTime,
    newer_files: &mut usize,
    remaining_budget: usize,
) {
    if remaining_budget == 0 || !path.exists() {
        return;
    }
    if path == map_path {
        return;
    }
    if path.is_dir() {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if matches!(
            name,
            ".git" | "target" | "node_modules" | "dist" | "build" | "__pycache__"
        ) {
            return;
        }
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut budget = remaining_budget;
        for entry in entries.flatten() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            count_newer_files(&entry.path(), map_path, map_mtime, newer_files, budget);
        }
        return;
    }

    if std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(|modified| modified > map_mtime)
        .unwrap_or(false)
    {
        *newer_files += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn missing_project_map_returns_none() {
        let dir = tempfile::tempdir().unwrap();

        assert!(load_project_map_zone_with_limit(dir.path(), 1_000).is_none());
    }

    #[test]
    fn loads_marked_agent_context_with_budget() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(
            dir.path().join(PROJECT_MAP_PATH),
            "# Project Map\n\nhidden intro\n<!-- agent-context:start -->\nvisible map section\n<!-- agent-context:end -->\nhidden tail",
        )
        .unwrap();

        let zone = load_project_map_zone_with_limit(dir.path(), 1_000).unwrap();

        assert!(zone.content.contains("visible map section"));
        assert!(!zone.content.contains("hidden intro"));
        assert!(!zone.truncated);
        assert_eq!(zone.freshness, ProjectMapFreshness::Current);
    }

    #[test]
    fn truncates_project_map_at_runtime_budget() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(
            dir.path().join(PROJECT_MAP_PATH),
            format!(
                "<!-- agent-context:start -->\n{}\n<!-- agent-context:end -->",
                "x".repeat(2_000)
            ),
        )
        .unwrap();

        let zone = load_project_map_zone_with_limit(dir.path(), 900).unwrap();

        assert!(zone.truncated);
        assert!(zone.content.contains("project map truncated"));
        assert!(zone.content.chars().count() < 1_300);
    }

    #[test]
    fn reports_stale_when_watched_source_is_newer_than_map() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(
            dir.path().join(PROJECT_MAP_PATH),
            "<!-- agent-context:start -->\nmap\n<!-- agent-context:end -->",
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(25));
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn newer() {}\n").unwrap();

        let zone = load_project_map_zone_with_limit(dir.path(), 1_000).unwrap();

        assert!(matches!(
            zone.freshness,
            ProjectMapFreshness::Stale { newer_files } if newer_files >= 1
        ));
        assert!(zone.content.contains("Freshness: stale"));
    }

    #[test]
    fn builds_machine_readable_symbol_index_with_file_hashes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub struct User {\n    name: String,\n}\npub fn build_user(name: String) -> User { User { name } }\n",
        )
        .unwrap();

        let index = build_project_symbol_index(dir.path(), 20, 20);

        assert_eq!(index.schema_version, 1);
        assert_eq!(index.files.len(), 1);
        assert_eq!(index.files[0].path, "src/lib.rs");
        assert_eq!(index.files[0].hash.len(), 32);
        assert!(index.files[0].summary.contains("symbols"));
        assert!(index.files[0]
            .symbols
            .iter()
            .any(|symbol| symbol.name == "User" && symbol.kind == "struct"));
        assert!(index.files[0]
            .symbols
            .iter()
            .any(|symbol| symbol.name == "build_user" && symbol.kind == "function"));
    }
}

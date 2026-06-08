use crate::memory::reports::MemoryFileSnapshot;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

pub(super) const MAX_IMPORT_DEPTH: usize = 5;

pub(super) const MAX_MEMORY_FILES: usize = 24;
pub(super) const MEMORY_FILE_CHAR_LIMIT: usize = 2_000;
pub(super) const MEMORY_MANIFEST_CHAR_LIMIT: usize = 2_500;
pub(super) const ACTIVE_MEMORY_SECTION_LIMIT: usize = 40;
pub(super) const ACTIVE_MEMORY_KEEP_SECTIONS: usize = 30;
pub(super) const ACTIVE_MEMORY_CHAR_LIMIT: usize = 20_000;

pub(super) fn write_memory_file_atomically(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let _guard = MemoryFileLock::acquire(path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("memory.md");
    let tmp_path = parent.join(format!(
        ".{}.{}.tmp",
        file_name,
        uuid::Uuid::new_v4().simple()
    ));

    std::fs::write(&tmp_path, content)?;
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    Ok(())
}

#[cfg(unix)]
pub(super) struct MemoryFileLock {
    file: std::fs::File,
}

#[cfg(unix)]
impl MemoryFileLock {
    pub(super) fn acquire(path: &Path) -> std::io::Result<Self> {
        use std::os::fd::AsRawFd;
        let lock_path = path.with_extension(format!(
            "{}.lock",
            path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("lock")
        ));
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)?;
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { file })
    }
}

#[cfg(unix)]
impl Drop for MemoryFileLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

#[cfg(not(unix))]
pub(super) struct MemoryFileLock;

#[cfg(not(unix))]
impl MemoryFileLock {
    pub(super) fn acquire(_path: &Path) -> std::io::Result<Self> {
        Ok(Self)
    }
}

pub(super) fn safe_memory_content_for_load(source: &str, content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    match crate::memory::safety::scan_memory_content(trimmed) {
        Ok(_) => Some(trimmed.to_string()),
        Err(issue) => {
            warn!(
                "Skipping persisted memory source {} during load: {}: {}",
                source, issue.code, issue.message
            );
            None
        }
    }
}

pub(super) fn load_memory_files(memory_dir: &Path) -> Vec<MemoryFileSnapshot> {
    let mut files = Vec::new();
    collect_memory_files(memory_dir, memory_dir, &mut files);

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files.truncate(MAX_MEMORY_FILES);
    files
}

pub(super) fn collect_memory_file_paths(memory_dir: &Path, include_archive: bool) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_memory_file_paths_inner(memory_dir, memory_dir, include_archive, &mut paths);
    paths.sort();
    paths
}

fn collect_memory_file_paths_inner(
    root: &Path,
    dir: &Path,
    include_archive: bool,
    paths: &mut Vec<PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            let is_archive = path
                .strip_prefix(root)
                .map(|p| p.starts_with("archive"))
                .unwrap_or(false);
            if is_archive && !include_archive {
                continue;
            }
            collect_memory_file_paths_inner(root, &path, include_archive, paths);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            paths.push(path);
        }
    }
}

fn collect_memory_files(root: &Path, dir: &Path, files: &mut Vec<MemoryFileSnapshot>) {
    if files.len() >= MAX_MEMORY_FILES {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if files.len() >= MAX_MEMORY_FILES {
            return;
        }

        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_memory_files(root, &path, files);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let file_base = path.parent().unwrap_or_else(|| Path::new("."));
        let with_imports = resolve_memory_imports(&content, file_base);
        let Some(content) =
            safe_memory_content_for_load(&format!("memory/{relative_path}"), &with_imports)
        else {
            continue;
        };
        let trimmed = content.trim();
        let chars = trimmed.chars().count();
        let content: String = trimmed.chars().take(MEMORY_FILE_CHAR_LIMIT).collect();
        files.push(MemoryFileSnapshot {
            relative_path,
            content,
            chars,
        });
    }
}

pub(super) fn format_memory_file_manifest(
    files: &[MemoryFileSnapshot],
    char_limit: usize,
) -> String {
    let mut output = String::new();
    for file in files {
        let title = memory_file_title(file);
        let line = format!(
            "- {} ({} chars): {}\n",
            file.relative_path, file.chars, title
        );
        if output.len() + line.len() > char_limit {
            output.push_str("- ...\n");
            break;
        }
        output.push_str(&line);
    }
    output.trim_end().to_string()
}

pub(super) fn memory_file_title(file: &MemoryFileSnapshot) -> String {
    file.content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| {
            line.trim_start_matches('#')
                .trim()
                .chars()
                .take(120)
                .collect()
        })
        .unwrap_or_else(|| "untitled memory".to_string())
}

pub(super) fn topic_memory_path(memory_dir: &Path, topic: &str) -> Option<PathBuf> {
    let stem = sanitize_memory_topic(topic)?;
    Some(memory_dir.join(format!("{}.md", stem)))
}

fn sanitize_memory_topic(topic: &str) -> Option<String> {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in topic.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch.is_alphanumeric() {
            output.push(ch);
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    let output = output
        .trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>();
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

pub(super) fn infer_learning_topic(content: &str, category: &str) -> Option<&'static str> {
    let lower = content.to_lowercase();
    let category = category.to_lowercase();

    if category == "preference" || lower.contains("user preference") || lower.contains("偏好") {
        return None;
    }
    if file_contains_any(
        &lower,
        &[
            "tui", "terminal", "ui", "claude", "scroll", "界面", "设计", "滚动",
        ],
    ) {
        return Some("tui-design");
    }
    if file_contains_any(
        &lower,
        &[
            "context",
            "prompt",
            "token",
            "memory",
            "compression",
            "上下文",
            "提示词",
            "记忆",
        ],
    ) {
        return Some("context-management");
    }
    if file_contains_any(
        &lower,
        &["permission", "approval", "allow", "deny", "权限", "授权"],
    ) {
        return Some("permissions");
    }
    if file_contains_any(&lower, &["tool", "bash", "mcp", "工具"]) {
        return Some("tools");
    }
    if file_contains_any(&lower, &["rust", "cargo", ".rs", "crate"]) {
        return Some("rust-workflow");
    }
    if category == "decision" {
        return Some("decisions");
    }
    if category == "convention" {
        return Some("conventions");
    }
    None
}

fn file_contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

pub(super) fn parse_rerank_ids(content: &str, candidate_count: usize) -> Vec<usize> {
    let trimmed = content.trim();
    if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        if start <= end {
            let json = &trimmed[start..=end];
            if let Ok(ids) = serde_json::from_str::<Vec<usize>>(json) {
                return ids.into_iter().filter(|id| *id < candidate_count).collect();
            }
        }
    }

    trimmed
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<usize>().ok())
        .filter(|id| *id < candidate_count)
        .collect()
}
#[derive(Debug, Clone, Default)]
pub(super) struct FileMaintenanceReport {
    pub(super) duplicates_removed: usize,
    pub(super) compacted: bool,
    pub(super) archived: bool,
}

pub(super) fn maintain_memory_file(
    path: &Path,
    content: &str,
    allow_archive: bool,
    memory_dir: &Path,
) -> anyhow::Result<FileMaintenanceReport> {
    let (header, sections) = split_memory_sections(content);
    if sections.is_empty() {
        return Ok(FileMaintenanceReport::default());
    }

    let original_section_count = sections.len();
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for section in sections {
        let key = normalize_memory_section(&section);
        if key.is_empty() || seen.insert(key) {
            deduped.push(section);
        }
    }

    let duplicates_removed = original_section_count.saturating_sub(deduped.len());
    let mut archived = false;
    let mut active_sections = deduped;
    let should_archive = allow_archive
        && (active_sections.len() > ACTIVE_MEMORY_SECTION_LIMIT
            || content.chars().count() > ACTIVE_MEMORY_CHAR_LIMIT);

    if should_archive && active_sections.len() > ACTIVE_MEMORY_KEEP_SECTIONS {
        let archive_count = active_sections.len() - ACTIVE_MEMORY_KEEP_SECTIONS;
        let archived_sections: Vec<String> = active_sections.drain(..archive_count).collect();
        write_memory_archive(path, memory_dir, &header, &archived_sections)?;
        archived = true;
    }

    let new_content = join_memory_sections(&header, &active_sections);
    let changed = duplicates_removed > 0 || archived;
    if changed && new_content != content {
        std::fs::write(path, new_content)?;
    }

    Ok(FileMaintenanceReport {
        duplicates_removed,
        compacted: changed,
        archived,
    })
}

pub(super) fn split_memory_sections(content: &str) -> (String, Vec<String>) {
    let mut header = String::new();
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut in_section = false;

    for line in content.lines() {
        if line.starts_with("## ") {
            if in_section && !current.trim().is_empty() {
                sections.push(current.trim_end().to_string());
            }
            current.clear();
            current.push_str(line);
            current.push('\n');
            in_section = true;
        } else if in_section {
            current.push_str(line);
            current.push('\n');
        } else {
            header.push_str(line);
            header.push('\n');
        }
    }

    if in_section && !current.trim().is_empty() {
        sections.push(current.trim_end().to_string());
    }

    (header.trim_end().to_string(), sections)
}

pub(super) fn legacy_markdown_sections(content: &str) -> Vec<String> {
    let (_, mut sections) = split_memory_sections(content);
    if sections.is_empty() && !content.trim().is_empty() {
        sections.push(content.trim().to_string());
    }
    sections
}

pub(super) fn legacy_markdown_section_parts(
    section: &str,
    default_category: &str,
) -> Option<(String, String)> {
    let mut lines = section.lines();
    let first = lines.next().unwrap_or_default().trim();
    let category = first
        .strip_prefix("## [")
        .and_then(|rest| rest.split(']').next())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_category.to_string());
    let body = if first.starts_with("## ") {
        lines.collect::<Vec<_>>()
    } else {
        section.lines().collect::<Vec<_>>()
    }
    .into_iter()
    .map(str::trim)
    .filter(|line| !line.starts_with("<!-- memory-id:"))
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join("\n");
    if body.trim().is_empty() {
        None
    } else {
        Some((category, body))
    }
}

pub(super) fn join_memory_sections(header: &str, sections: &[String]) -> String {
    let mut output = String::new();
    if !header.trim().is_empty() {
        output.push_str(header.trim_end());
        output.push('\n');
    }
    for section in sections {
        output.push('\n');
        output.push_str(section.trim());
        output.push('\n');
    }
    output
}

pub(super) fn normalize_memory_section(section: &str) -> String {
    section
        .lines()
        .filter(|line| !line.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase()
        .replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "")
}

pub(super) fn write_memory_archive(
    source_path: &Path,
    memory_dir: &Path,
    header: &str,
    sections: &[String],
) -> anyhow::Result<()> {
    let archive_dir = memory_dir.join("archive");
    std::fs::create_dir_all(&archive_dir)?;
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("memory");
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let archive_path = archive_dir.join(format!("{}-{}.md", stem, timestamp));
    let archive_header = if header.trim().is_empty() {
        format!("# Archived {}", stem)
    } else {
        format!(
            "{}\n\n> Archived from {}",
            header.trim(),
            source_path.display()
        )
    };
    std::fs::write(
        archive_path,
        join_memory_sections(&archive_header, sections),
    )?;
    Ok(())
}

/// 归一化学习内容并计算哈希（用于去重）
pub(super) fn hash_learning(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let normalized = text
        .to_lowercase()
        .trim()
        .replace(|c: char| c.is_whitespace() || c.is_ascii_punctuation(), "");
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    hasher.finish()
}

/// Resolve `@path/to/file` imports in memory content.
///
/// A line consisting solely of `@<path>` (with the `@` at the start and the
/// path containing a `/` or `.` separator) is treated as a file import. The
/// path is resolved relative to `base_dir` (the directory containing the
/// importing file). Recursive imports are resolved up to `MAX_IMPORT_DEPTH`.
/// Cyclic imports emit a `<!-- skipped: import cycle -->` comment.
///
/// Lines like `@mention` (no path separator or dot in the part after `@`) are
/// left as literal prose — they are not mistaken for imports.
pub(super) fn resolve_memory_imports(content: &str, base_dir: &Path) -> String {
    resolve_imports_recursive(content, base_dir, 0, &mut Vec::new())
}

fn resolve_imports_recursive(
    content: &str,
    base_dir: &Path,
    depth: usize,
    seen: &mut Vec<PathBuf>,
) -> String {
    let mut out = String::with_capacity(content.len());
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(import_path) = parse_import_line(trimmed) {
            let resolved = resolve_import_path(import_path, base_dir);
            if depth >= MAX_IMPORT_DEPTH {
                out.push_str("<!-- skipped: max import depth -->\n");
                continue;
            }
            if seen.iter().any(|p| paths_equal(p, &resolved)) {
                out.push_str("<!-- skipped: import cycle -->\n");
                continue;
            }
            match std::fs::read_to_string(&resolved) {
                Ok(imported_content) => {
                    seen.push(resolved.clone());
                    let resolved = resolve_imports_recursive(
                        &imported_content,
                        resolved.parent().unwrap_or(base_dir),
                        depth + 1,
                        seen,
                    );
                    seen.pop();
                    out.push_str(&resolved);
                }
                Err(_) => {
                    out.push_str(&format!(
                        "<!-- skipped: import not found: {} -->\n",
                        import_path
                    ));
                }
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn parse_import_line(trimmed: &str) -> Option<&str> {
    let path = trimmed.strip_prefix('@')?;
    if path.is_empty() {
        return None;
    }
    // A path import must contain at least one `/` or `.` to distinguish prose
    // mentions like `@username` or `@mention` from actual paths like `@docs/setup.md`.
    if !path.contains('/') && !path.contains('.') {
        return None;
    }
    // The entire line must be only the import — no trailing prose.
    // Already guaranteed because we trimmed the whole line.
    Some(path)
}

fn resolve_import_path(path: &str, base_dir: &Path) -> PathBuf {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            if let Some(rest) = path.strip_prefix("~/") {
                return home.join(rest);
            }
            return home;
        }
    }
    if Path::new(path).is_absolute() {
        return PathBuf::from(path);
    }
    base_dir.join(path)
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    a.canonicalize()
        .ok()
        .zip(b.canonicalize().ok())
        .map(|(a, b)| a == b)
        .unwrap_or(false)
}

#[cfg(test)]
mod import_tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("priority-agent-import-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_simple_import() {
        let dir = temp_dir();
        std::fs::write(dir.join("doc.md"), "hello from doc.md").unwrap();
        std::fs::write(dir.join("MAIN.md"), "before\n@doc.md\nafter").unwrap();

        let result =
            resolve_memory_imports(&std::fs::read_to_string(dir.join("MAIN.md")).unwrap(), &dir);
        assert!(result.contains("before"));
        assert!(result.contains("hello from doc.md"));
        assert!(result.contains("after"));
        assert!(!result.contains("@doc.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_non_path_mentions() {
        let dir = temp_dir();
        let content =
            "I like @username\n@some-tag\nreal @docs/file.md\nnot an import\nalso not @import here";
        std::fs::write(dir.join("MAIN.md"), content).unwrap();

        let result = resolve_memory_imports(content, &dir);
        assert!(result.contains("@username")); // prose mention: no path separator
        assert!(result.contains("@some-tag")); // prose mention: no path separator
        assert!(result.contains("real @docs/file.md")); // has preceding text on the line
        assert!(result.contains("@import here")); // has "also not" preceding it

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recursive_import_with_depth_limit() {
        let dir = temp_dir();
        std::fs::write(dir.join("a.md"), "@b.md").unwrap();
        std::fs::write(dir.join("b.md"), "@c.md").unwrap();
        std::fs::write(dir.join("c.md"), "@d.md").unwrap();
        std::fs::write(dir.join("d.md"), "@e.md").unwrap();
        std::fs::write(dir.join("e.md"), "@f.md").unwrap();
        std::fs::write(dir.join("f.md"), "deep content").unwrap();

        let result = resolve_memory_imports("@a.md", &dir);
        // f.md is at depth 5 (a→b→c→d→e→f), should trigger max depth
        assert!(!result.contains("deep content"));
        assert!(result.contains("max import depth"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_import_cycle() {
        let dir = temp_dir();
        std::fs::write(dir.join("x.md"), "@y.md").unwrap();
        std::fs::write(dir.join("y.md"), "@x.md").unwrap();

        let result = resolve_memory_imports("@x.md", &dir);
        assert!(result.contains("import cycle"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_import_is_commented() {
        let dir = temp_dir();
        let result = resolve_memory_imports("@nonexistent.md", &dir);
        assert!(result.contains("import not found"));
        assert!(result.contains("nonexistent.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_content_no_imports() {
        let dir = temp_dir();
        let result = resolve_memory_imports("", &dir);
        assert!(result.is_empty());

        let result = resolve_memory_imports("just some text\nno imports here", &dir);
        assert_eq!(result, "just some text\nno imports here\n");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

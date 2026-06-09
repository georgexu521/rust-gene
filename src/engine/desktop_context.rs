//! Desktop and editor context bridging.
//!
//! Parses `@file` references, file-range annotations (`@file#Lx-Ly`),
//! and builds `DesktopRunContext` payloads that carry editor-like
//! context into the runtime without building a full IDE extension.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A file reference parsed from user input (e.g. `@src/main.rs` or
/// `src/main.rs#L10-L20`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileContextRef {
    /// Absolute or workspace-relative path.
    pub path: PathBuf,
    /// Optional line range (1-indexed, inclusive).
    pub line_start: Option<usize>,
    /// Optional line end.
    pub line_end: Option<usize>,
}

/// Context carried from the desktop/IDE into the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopRunContext {
    /// Currently active file in the editor.
    pub current_file: Option<PathBuf>,
    /// Selected text range in the editor (lines or characters).
    pub selected_range: Option<SelectedRange>,
    /// Active git diff or patch content.
    pub active_diff: Option<String>,
    /// Terminal working directory.
    pub terminal_cwd: Option<PathBuf>,
    /// Terminal command output reference.
    pub terminal_output_ref: Option<String>,
    /// Explicit file references from the user.
    pub file_refs: Vec<FileContextRef>,
}

/// A selected range in a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedRange {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub content_preview: String,
}

impl DesktopRunContext {
    /// Create an empty context.
    pub fn new() -> Self {
        Self {
            current_file: None,
            selected_range: None,
            active_diff: None,
            terminal_cwd: None,
            terminal_output_ref: None,
            file_refs: Vec::new(),
        }
    }

    /// Attach a file reference from a parsed `@file` token.
    pub fn attach_file_ref(&mut self, file_ref: FileContextRef) {
        self.file_refs.push(file_ref);
    }

    /// Read the content for a file reference (bounded for large files).
    pub fn resolve_file_content(path: &Path, max_lines: usize) -> Option<String> {
        let content = std::fs::read_to_string(path).ok()?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= max_lines {
            return Some(content);
        }
        let head = &lines[..max_lines / 2];
        let tail = &lines[lines.len() - max_lines / 2..];
        Some(format!(
            "{}  ... [{} lines truncated] ...\n{}",
            head.join("\n"),
            lines.len() - max_lines,
            tail.join("\n")
        ))
    }

    /// Build a prompt-format summary of the attached context.
    pub fn prompt_summary(&self) -> String {
        let mut out = String::new();

        if let Some(ref file) = self.current_file {
            out.push_str(&format!("Current file: {}\n", file.display()));
        }
        if let Some(ref range) = self.selected_range {
            out.push_str(&format!(
                "Selected: {} (lines {}-{})\n```\n{}\n```\n",
                range.file.display(),
                range.start_line,
                range.end_line,
                range.content_preview,
            ));
        }
        for fr in &self.file_refs {
            out.push_str(&format!("Attached: {}", fr.path.display()));
            if let (Some(s), Some(e)) = (fr.line_start, fr.line_end) {
                out.push_str(&format!(" (lines {}-{})", s, e));
            }
            out.push('\n');
        }

        out
    }
}

impl Default for DesktopRunContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse an `@file` or `@file#Lx-Ly` reference from user input.
///
/// Examples:
/// - `@src/main.rs` → file reference, no range
/// - `@src/main.rs#L10-L20` → file reference with line range
/// - `src/main.rs#10-20` → file reference with numeric range
pub fn parse_file_ref(input: &str, workspace_root: &Path) -> Option<FileContextRef> {
    let input = input.trim();

    // Strip leading @ if present.
    let stripped = input.strip_prefix('@').unwrap_or(input);

    // Check for line range: #Lx-Ly or #x-y
    let (path_str, line_start, line_end) = if let Some(hash_pos) = stripped.rfind('#') {
        let path_part = &stripped[..hash_pos];
        let range_part = &stripped[hash_pos + 1..];
        let (start, end) = parse_line_range(range_part)?;
        (path_part, Some(start), Some(end))
    } else {
        (stripped, None, None)
    };

    let path = if Path::new(path_str).is_absolute() {
        PathBuf::from(path_str)
    } else {
        workspace_root.join(path_str)
    };

    // Only accept if the path exists.
    if !path.exists() {
        return None;
    }

    Some(FileContextRef {
        path,
        line_start,
        line_end,
    })
}

/// Parse a line range string like "L10-L20" or "10-20".
fn parse_line_range(s: &str) -> Option<(usize, usize)> {
    let s = s.trim();
    let parts: Vec<&str> = if s.contains('-') {
        s.split('-').collect()
    } else {
        return None;
    };
    if parts.len() != 2 {
        return None;
    }
    let start = parts[0].trim_start_matches('L').trim_start_matches('l');
    let end = parts[1].trim_start_matches('L').trim_start_matches('l');
    let start: usize = start.parse().ok()?;
    let end: usize = end.parse().ok()?;
    if start == 0 || end == 0 || start > end {
        return None;
    }
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_file_ref() {
        let ref_ = parse_file_ref("@src/main.rs", Path::new("."));
        assert!(ref_.is_some());
        let r = ref_.unwrap();
        assert!(r.path.ends_with("src/main.rs"));
        assert!(r.line_start.is_none());
    }

    #[test]
    fn parse_file_ref_with_range() {
        let ref_ = parse_file_ref("@src/main.rs#L10-L20", Path::new("."));
        assert!(ref_.is_some());
        let r = ref_.unwrap();
        assert_eq!(r.line_start, Some(10));
        assert_eq!(r.line_end, Some(20));
    }

    #[test]
    fn parse_file_ref_with_numeric_range() {
        let ref_ = parse_file_ref("@src/main.rs#10-20", Path::new("."));
        assert!(ref_.is_some());
        let r = ref_.unwrap();
        assert_eq!(r.line_start, Some(10));
        assert_eq!(r.line_end, Some(20));
    }

    #[test]
    fn parse_file_ref_without_at() {
        let ref_ = parse_file_ref("src/main.rs#L5-L15", Path::new("."));
        assert!(ref_.is_some());
    }

    #[test]
    fn parse_file_ref_invalid_range() {
        let ref_ = parse_file_ref("@src/main.rs#L20-L10", Path::new("."));
        assert!(ref_.is_none());
    }

    #[test]
    fn empty_context_prompt_is_empty() {
        let ctx = DesktopRunContext::new();
        assert!(ctx.prompt_summary().is_empty());
    }

    #[test]
    fn context_with_file_refs_has_summary() {
        let mut ctx = DesktopRunContext::new();
        ctx.attach_file_ref(FileContextRef {
            path: PathBuf::from("src/main.rs"),
            line_start: Some(10),
            line_end: Some(20),
        });
        let summary = ctx.prompt_summary();
        assert!(summary.contains("src/main.rs"));
        assert!(summary.contains("lines 10-20"));
    }

    #[test]
    fn parse_line_range_rejects_zero() {
        assert!(parse_line_range("0-10").is_none());
    }

    #[test]
    fn parse_line_range_rejects_invalid() {
        assert!(parse_line_range("abc-def").is_none());
    }
}

use priority_agent::desktop_runtime::DesktopContextSnapshot;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DesktopRunContext {
    #[serde(rename = "type")]
    pub(crate) context_type: String,
    pub(crate) label: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) line_start: Option<usize>,
    pub(crate) line_end: Option<usize>,
    pub(crate) selection_text: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ResolvedDesktopRunContext {
    #[serde(rename = "type")]
    pub(crate) context_type: String,
    pub(crate) label: String,
    pub(crate) shortstat: String,
    pub(crate) files: Vec<String>,
    pub(crate) stat: String,
    #[serde(rename = "patch_preview")]
    pub(crate) patch_preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) relative_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line_start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line_end: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) preview: Option<String>,
    pub(crate) truncated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopWorkbenchSnapshot {
    pub(crate) selected_project: String,
    pub(crate) project_map: DesktopProjectMapSnapshot,
    pub(crate) symbol_index: DesktopSymbolIndexSnapshot,
    pub(crate) runtime_context: Option<DesktopContextSnapshot>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopProjectMapSnapshot {
    pub(crate) available: bool,
    pub(crate) source: Option<String>,
    pub(crate) freshness: String,
    pub(crate) chars: usize,
    pub(crate) truncated: bool,
    pub(crate) content_preview: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DesktopSymbolIndexSnapshot {
    pub(crate) schema_version: u8,
    pub(crate) total_symbols: usize,
    pub(crate) files: Vec<priority_agent::engine::project_map::ProjectIndexedFile>,
    pub(crate) truncated: bool,
}


pub(crate) fn enrich_message_with_desktop_contexts(
    message: String,
    contexts: &[DesktopRunContext],
    project: &Path,
) -> Result<String, String> {
    if contexts.is_empty() {
        return Ok(message);
    }

    let mut blocks = Vec::new();
    for context in contexts {
        match context.context_type.as_str() {
            "current_diff" => {
                let resolved = resolve_current_diff_context(context, project)?;
                blocks.push(format_desktop_context_block(&resolved));
            }
            "file" => {
                let resolved = resolve_file_context(context, project)?;
                blocks.push(format_desktop_context_block(&resolved));
            }
            other => {
                return Err(format!("Unsupported desktop run context: {}", other));
            }
        }
    }

    Ok(format!("{}\n\n{}", message.trim_end(), blocks.join("\n\n")))
}

pub(crate) fn resolve_current_diff_context(
    context: &DesktopRunContext,
    project: &Path,
) -> Result<ResolvedDesktopRunContext, String> {
    let unstaged_shortstat = run_git(project, &["diff", "--shortstat"])?;
    let staged_shortstat = run_git(project, &["diff", "--cached", "--shortstat"])?;
    let unstaged_stat = run_git(project, &["diff", "--stat", "--find-renames"])?;
    let staged_stat = run_git(project, &["diff", "--cached", "--stat", "--find-renames"])?;
    let unstaged_files = run_git(project, &["diff", "--name-only"])?;
    let staged_files = run_git(project, &["diff", "--cached", "--name-only"])?;
    let unstaged_patch = run_git(project, &["diff", "--no-ext-diff", "--find-renames"])?;
    let staged_patch = run_git(
        project,
        &["diff", "--cached", "--no-ext-diff", "--find-renames"],
    )?;

    let shortstat = join_non_empty(&[
        label_section("unstaged", unstaged_shortstat.trim()),
        label_section("staged", staged_shortstat.trim()),
    ])
    .unwrap_or_else(|| "No staged or unstaged git diff detected.".to_string());
    let stat = join_non_empty(&[
        label_section("unstaged", unstaged_stat.trim()),
        label_section("staged", staged_stat.trim()),
    ])
    .unwrap_or_else(|| "No changed files detected.".to_string());
    let files = collect_diff_files(&unstaged_files, &staged_files);
    let patch = join_non_empty(&[
        label_section("unstaged", unstaged_patch.trim()),
        label_section("staged", staged_patch.trim()),
    ])
    .unwrap_or_default();
    let (patch_preview, truncated) = truncate_chars(&patch, 12_000);

    Ok(ResolvedDesktopRunContext {
        context_type: context.context_type.clone(),
        label: context
            .label
            .clone()
            .unwrap_or_else(|| "Current diff".to_string()),
        shortstat,
        files,
        stat,
        patch_preview,
        path: None,
        relative_path: None,
        size_bytes: None,
        line_count: None,
        line_start: None,
        line_end: None,
        preview: None,
        truncated,
    })
}

pub(crate) fn resolve_file_context(
    context: &DesktopRunContext,
    project: &Path,
) -> Result<ResolvedDesktopRunContext, String> {
    let raw_path = context
        .path
        .as_ref()
        .ok_or_else(|| "File context requires a path.".to_string())?;
    let requested_path = PathBuf::from(raw_path);
    let file_path = if requested_path.is_absolute() {
        requested_path
    } else {
        project.join(requested_path)
    };
    let project_root = project
        .canonicalize()
        .map_err(|err| format!("Failed to resolve selected project: {}", err))?;
    let file_path = file_path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve file context path: {}", err))?;

    if !file_path.starts_with(&project_root) {
        return Err("File context must be inside the selected project.".to_string());
    }
    if !file_path.is_file() {
        return Err("File context path is not a file.".to_string());
    }

    let bytes =
        std::fs::read(&file_path).map_err(|err| format!("Failed to read file context: {}", err))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let line_count = text.lines().count();
    let selection = selected_file_context_preview(context, &text)?;
    let (preview, truncated) = truncate_chars(&selection.preview, 12_000);
    let relative_path = file_path
        .strip_prefix(&project_root)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();
    let label = context.label.clone().unwrap_or_else(|| {
        file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("File")
            .to_string()
    });

    Ok(ResolvedDesktopRunContext {
        context_type: context.context_type.clone(),
        label,
        shortstat: format!(
            "{} ({} bytes, {} lines)",
            relative_path,
            bytes.len(),
            line_count
        ),
        files: vec![relative_path.clone()],
        stat: format!(
            "{} | {} bytes | {} lines",
            relative_path,
            bytes.len(),
            line_count
        ),
        patch_preview: String::new(),
        path: Some(file_path.to_string_lossy().to_string()),
        relative_path: Some(relative_path),
        size_bytes: Some(bytes.len() as u64),
        line_count: Some(line_count),
        line_start: selection.line_start,
        line_end: selection.line_end,
        preview: Some(preview),
        truncated,
    })
}

struct FileContextSelection {
    preview: String,
    line_start: Option<usize>,
    line_end: Option<usize>,
}

fn selected_file_context_preview(
    context: &DesktopRunContext,
    text: &str,
) -> Result<FileContextSelection, String> {
    if let Some(selection_text) = context
        .selection_text
        .as_deref()
        .filter(|selection| !selection.trim().is_empty())
    {
        if let Some(start) = context.line_start.filter(|line| *line > 0) {
            let end = context.line_end.unwrap_or(start).max(start);
            let selected = select_file_lines(text, start, end)?;
            if normalize_selection_text(selection_text) != normalize_selection_text(&selected) {
                return Err(format!(
                    "Provided selection_text does not match file lines {}-{}.",
                    start, end
                ));
            }
            return Ok(FileContextSelection {
                preview: selected,
                line_start: Some(start),
                line_end: Some(end),
            });
        }

        if !text.contains(selection_text) {
            return Err("Provided selection_text was not found in the selected file.".to_string());
        }
        return Ok(FileContextSelection {
            preview: selection_text.to_string(),
            line_start: None,
            line_end: None,
        });
    }

    let Some(start) = context.line_start.filter(|line| *line > 0) else {
        return Ok(FileContextSelection {
            preview: text.to_string(),
            line_start: None,
            line_end: None,
        });
    };
    let end = context.line_end.unwrap_or(start).max(start);
    let selected = select_file_lines(text, start, end)?;

    Ok(FileContextSelection {
        preview: selected,
        line_start: Some(start),
        line_end: Some(end),
    })
}

fn select_file_lines(text: &str, start: usize, end: usize) -> Result<String, String> {
    let selected_lines = text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_no = index + 1;
            (line_no >= start && line_no <= end).then_some(line)
        })
        .collect::<Vec<_>>();
    if selected_lines.is_empty() {
        return Err(format!(
            "Selected range {}-{} is outside the selected file.",
            start, end
        ));
    }
    Ok(selected_lines.join("\n"))
}

fn normalize_selection_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches('\n')
        .to_string()
}

fn format_desktop_context_block(context: &ResolvedDesktopRunContext) -> String {
    if context.context_type == "file" {
        let relative_path = context.relative_path.as_deref().unwrap_or(&context.label);
        let preview = context
            .preview
            .as_deref()
            .filter(|preview| !preview.is_empty())
            .unwrap_or("No file preview available.");
        let truncated = if context.truncated { "true" } else { "false" };

        let selected_range = match (context.line_start, context.line_end) {
            (Some(start), Some(end)) => format!("Selected range: {}-{}\n", start, end),
            (Some(start), None) => format!("Selected range: {}\n", start),
            _ => String::new(),
        };

        return format!(
            "<desktop_context type=\"{}\" label=\"{}\">\nPath: {}\nSize bytes: {}\nLines: {}\n{}Preview truncated: {}\n```text\n{}\n```\n</desktop_context>",
            escape_context_attr(&context.context_type),
            escape_context_attr(&context.label),
            relative_path,
            context.size_bytes.unwrap_or_default(),
            context.line_count.unwrap_or_default(),
            selected_range,
            truncated,
            preview
        );
    }

    let files = if context.files.is_empty() {
        "- No changed files detected.".to_string()
    } else {
        context
            .files
            .iter()
            .map(|file| format!("- {}", file))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let patch_preview = if context.patch_preview.is_empty() {
        "No diff preview available.".to_string()
    } else {
        context.patch_preview.clone()
    };
    let truncated = if context.truncated { "true" } else { "false" };

    format!(
        "<desktop_context type=\"{}\" label=\"{}\">\nSummary:\n{}\n\nFiles:\n{}\n\nStat:\n{}\n\nPatch preview truncated: {}\n```diff\n{}\n```\n</desktop_context>",
        escape_context_attr(&context.context_type),
        escape_context_attr(&context.label),
        context.shortstat,
        files,
        context.stat,
        truncated,
        patch_preview
    )
}

fn run_git(project: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .output()
        .map_err(|err| format!("Failed to run git {}: {}", args.join(" "), err))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "git {} failed{}",
            args.join(" "),
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", stderr)
            }
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn collect_diff_files(unstaged: &str, staged: &str) -> Vec<String> {
    let mut files = unstaged
        .lines()
        .chain(staged.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files
}

fn join_non_empty(parts: &[Option<String>]) -> Option<String> {
    let joined = parts
        .iter()
        .filter_map(|part| part.as_ref())
        .filter(|part| !part.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n");
    if joined.trim().is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn label_section(label: &str, text: &str) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(format!("{}:\n{}", label, text.trim()))
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
    let mut iter = text.chars();
    let preview = iter.by_ref().take(max_chars).collect::<String>();
    let truncated = iter.next().is_some();
    (preview, truncated)
}

fn escape_context_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_context(line_start: Option<usize>, line_end: Option<usize>, selection_text: Option<&str>) -> DesktopRunContext {
        DesktopRunContext {
            context_type: "file".to_string(),
            label: None,
            path: Some("src/main.rs".to_string()),
            line_start,
            line_end,
            selection_text: selection_text.map(str::to_string),
        }
    }

    #[test]
    fn selected_preview_uses_file_lines_for_verified_range() {
        let context = file_context(Some(2), Some(3), Some("beta\ngamma\n"));
        let selection = selected_file_context_preview(&context, "alpha\nbeta\ngamma\n").unwrap();

        assert_eq!(selection.preview, "beta\ngamma");
        assert_eq!(selection.line_start, Some(2));
        assert_eq!(selection.line_end, Some(3));
    }

    #[test]
    fn selected_preview_rejects_mismatched_selection_text() {
        let context = file_context(Some(2), Some(2), Some("not beta"));
        let err = selected_file_context_preview(&context, "alpha\nbeta\ngamma\n")
            .expect_err("selection should be rejected");

        assert!(err.contains("does not match"));
    }

    #[test]
    fn selected_preview_rejects_selection_text_outside_file() {
        let context = file_context(None, None, Some("not in file"));
        let err = selected_file_context_preview(&context, "alpha\nbeta\ngamma\n")
            .expect_err("selection should be rejected");

        assert!(err.contains("not found"));
    }
}

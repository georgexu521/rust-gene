//! Session list view model.
//!
//! This module produces a pure, render-ready description of the session sidebar
//! from raw session records and TUI selection state. Keeping the product logic
//! here prevents `main_screen.rs` from inventing competing labels for the same
//! runtime facts.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::session_store::SessionRecord;

/// Status of a workspace group relative to the current UI state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceGroupStatus {
    /// The session belongs to the workspace currently active in the UI.
    Current,
    /// The session has a workspace tag and the path still exists, but it is not
    /// the currently active workspace.
    Known,
    /// The session has a workspace tag, but the path no longer exists.
    MissingPath,
    /// The session has no workspace tag (legacy session before schema migration).
    UntaggedLegacy,
}

/// A group of sessions that share the same workspace root.
#[derive(Debug, Clone)]
pub struct WorkspaceGroup {
    /// Display name for the workspace (basename of the root, or a fallback).
    pub name: String,
    /// Full workspace root path, if known.
    pub root: Option<String>,
    pub status: WorkspaceGroupStatus,
    pub rows: Vec<SessionListRow>,
}

/// A single renderable row in the session list.
#[derive(Debug, Clone)]
pub struct SessionListRow {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub display_title: String,
    pub model_short: String,
    pub msg_count: i64,
    pub is_current: bool,
    pub is_selected: bool,
    pub is_pinned: bool,
    pub has_parent: bool,
    pub delete_hint: bool,
    pub preview: Option<String>,
}

/// Active rename state captured in the view model.
#[derive(Debug, Clone)]
pub struct RenameState {
    pub session_id: String,
    pub buffer: String,
}

/// Render-ready session list view model.
#[derive(Debug, Clone)]
pub struct SessionListViewModel {
    pub current_session_id: Option<String>,
    pub groups: Vec<WorkspaceGroup>,
    pub pending_delete_id: Option<String>,
    pub rename: Option<RenameState>,
    pub filter_text: String,
    pub is_filtering: bool,
    pub empty_hint: &'static str,
}

impl SessionListViewModel {
    /// Return the total number of rows across all groups.
    pub fn total_rows(&self) -> usize {
        self.groups.iter().map(|g| g.rows.len()).sum()
    }

    /// Return the row at the given flat index, if any.
    pub fn row_at(&self, index: usize) -> Option<&SessionListRow> {
        let mut offset = 0;
        for group in &self.groups {
            if let Some(row) = group.rows.get(index - offset) {
                return Some(row);
            }
            offset += group.rows.len();
        }
        None
    }

    /// Build a view model for session sidebar rendering.
    ///
    /// `sessions` must already be filtered/ordered by the caller (e.g.
    /// `TuiApp::visible_sidebar_sessions`).
    /// `message_counts` maps session id to its message count. Missing entries are
    /// treated as zero.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        sessions: &[SessionRecord],
        current_session_id: Option<&str>,
        current_workspace_root: &str,
        pinned_sessions: &[String],
        selected_index: usize,
        pending_delete_id: Option<&str>,
        rename: Option<&RenameState>,
        filter_text: &str,
        is_filtering: bool,
        sidebar_width: u16,
        message_counts: &HashMap<String, i64>,
    ) -> Self {
        let pinned: Vec<_> = sessions
            .iter()
            .filter(|s| pinned_sessions.contains(&s.id))
            .collect();
        let unpinned: Vec<_> = sessions
            .iter()
            .filter(|s| !pinned_sessions.contains(&s.id))
            .collect();

        let mut groups = Vec::new();
        if !pinned.is_empty() {
            groups.extend(build_workspace_groups(
                &pinned,
                current_session_id,
                current_workspace_root,
                pinned_sessions,
                selected_index,
                pending_delete_id,
                sidebar_width,
                true,
                message_counts,
            ));
        }
        if !unpinned.is_empty() {
            groups.extend(build_workspace_groups(
                &unpinned,
                current_session_id,
                current_workspace_root,
                pinned_sessions,
                selected_index,
                pending_delete_id,
                sidebar_width,
                false,
                message_counts,
            ));
        }

        let empty_hint = if filter_text.is_empty() {
            "No sessions found"
        } else {
            "No sessions match filter"
        };

        Self {
            current_session_id: current_session_id.map(String::from),
            groups,
            pending_delete_id: pending_delete_id.map(String::from),
            rename: rename.cloned(),
            filter_text: filter_text.to_string(),
            is_filtering,
            empty_hint,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_workspace_groups(
    sessions: &[&SessionRecord],
    current_session_id: Option<&str>,
    current_workspace_root: &str,
    pinned_sessions: &[String],
    selected_index: usize,
    pending_delete_id: Option<&str>,
    sidebar_width: u16,
    _is_pinned_section: bool,
    message_counts: &HashMap<String, i64>,
) -> Vec<WorkspaceGroup> {
    let mut grouped: BTreeMap<Option<String>, Vec<&SessionRecord>> = BTreeMap::new();
    for session in sessions {
        grouped
            .entry(session.workspace_root.clone())
            .or_default()
            .push(session);
    }

    let mut groups = Vec::new();
    let mut row_index = 0usize;

    for (workspace_root, group_sessions) in grouped {
        let status = workspace_status(&workspace_root, current_workspace_root);
        let name = workspace_name(&workspace_root, status);
        let mut rows = Vec::new();
        for session in group_sessions {
            rows.push(build_session_row(
                session,
                current_session_id,
                pinned_sessions,
                selected_index,
                pending_delete_id,
                sidebar_width,
                row_index,
                message_counts,
            ));
            row_index += 1;
        }
        groups.push(WorkspaceGroup {
            name,
            root: workspace_root,
            status,
            rows,
        });
    }

    // Sort groups so Current comes first, then by name.
    groups.sort_by(|a, b| {
        let status_order = |s: WorkspaceGroupStatus| match s {
            WorkspaceGroupStatus::Current => 0,
            WorkspaceGroupStatus::Known => 1,
            WorkspaceGroupStatus::MissingPath => 2,
            WorkspaceGroupStatus::UntaggedLegacy => 3,
        };
        status_order(a.status)
            .cmp(&status_order(b.status))
            .then_with(|| a.name.cmp(&b.name))
    });

    groups
}

#[allow(clippy::too_many_arguments)]
fn build_session_row(
    session: &SessionRecord,
    current_session_id: Option<&str>,
    pinned_sessions: &[String],
    selected_index: usize,
    pending_delete_id: Option<&str>,
    sidebar_width: u16,
    row_index: usize,
    message_counts: &HashMap<String, i64>,
) -> SessionListRow {
    let is_current = current_session_id == Some(session.id.as_str());
    let is_selected = row_index == selected_index;
    let is_pinned = pinned_sessions.contains(&session.id);
    let title = if session.title.is_empty() {
        format!("Session {}", &session.id[..8.min(session.id.len())])
    } else {
        session.title.clone()
    };

    let sidebar_width = usize::from(sidebar_width);
    let title_budget = sidebar_width.saturating_sub(6).max(8);
    let display_title = truncate_chars_to_width(&title, title_budget);

    SessionListRow {
        id: session.id.clone(),
        short_id: session.id[..8.min(session.id.len())].to_string(),
        title,
        display_title,
        model_short: compact_model_label(&session.model),
        msg_count: *message_counts.get(&session.id).unwrap_or(&0),
        is_current,
        is_selected,
        is_pinned,
        has_parent: session.parent_session_id.is_some(),
        delete_hint: pending_delete_id == Some(session.id.as_str()),
        preview: None,
    }
}

fn workspace_status(
    workspace_root: &Option<String>,
    current_workspace_root: &str,
) -> WorkspaceGroupStatus {
    match workspace_root {
        None => WorkspaceGroupStatus::UntaggedLegacy,
        Some(root) => {
            if root == current_workspace_root {
                WorkspaceGroupStatus::Current
            } else if Path::new(root).exists() {
                WorkspaceGroupStatus::Known
            } else {
                WorkspaceGroupStatus::MissingPath
            }
        }
    }
}

fn workspace_name(workspace_root: &Option<String>, status: WorkspaceGroupStatus) -> String {
    match workspace_root {
        Some(root) => Path::new(root)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.clone()),
        None => match status {
            WorkspaceGroupStatus::UntaggedLegacy => "Untagged".to_string(),
            _ => "Unknown".to_string(),
        },
    }
}

fn compact_model_label(model: &str) -> String {
    let lower = model.to_ascii_lowercase();
    if lower.contains("deepseek") {
        "deepseek-v4".to_string()
    } else if lower.contains("minimax") {
        "minimax".to_string()
    } else if lower.contains("claude") {
        if lower.contains("haiku") {
            "claude-haiku".to_string()
        } else if lower.contains("sonnet") {
            "claude-sonnet".to_string()
        } else {
            "claude".to_string()
        }
    } else if lower.contains("gpt-5") {
        "gpt-5".to_string()
    } else if lower.contains("gpt-4") {
        "gpt-4".to_string()
    } else {
        truncate_chars_with_ellipsis(model, 12)
    }
}

fn truncate_chars_with_ellipsis(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn truncate_chars_to_width(value: &str, max_chars: usize) -> String {
    if unicode_width::UnicodeWidthStr::width(value) <= max_chars {
        return value.to_string();
    }
    if max_chars == 0 {
        String::new()
    } else if max_chars == 1 {
        "…".to_string()
    } else {
        let content_width = max_chars - 1;
        let mut width = 0usize;
        let mut out = String::new();
        for ch in value.chars() {
            let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if width + ch_width > content_width {
                break;
            }
            width += ch_width;
            out.push(ch);
        }
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session(id: &str, title: &str, workspace_root: Option<&str>) -> SessionRecord {
        SessionRecord {
            id: id.to_string(),
            title: title.to_string(),
            parent_session_id: None,
            created_at: "2026-06-14T00:00:00Z".to_string(),
            updated_at: "2026-06-14T00:00:00Z".to_string(),
            model: "claude-sonnet".to_string(),
            total_input_tokens: 0,
            total_output_tokens: 0,
            workspace_root: workspace_root.map(String::from),
        }
    }

    #[test]
    fn empty_sessions_produces_empty_model() {
        let model = SessionListViewModel::build(
            &[],
            None,
            "/home/gex/proj",
            &[],
            0,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert!(model.groups.is_empty());
        assert_eq!(model.total_rows(), 0);
        assert_eq!(model.empty_hint, "No sessions found");
    }

    #[test]
    fn current_workspace_group_status() {
        let sessions = vec![sample_session("s1", "Alpha", Some("/home/gex/proj"))];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &[],
            0,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert_eq!(model.groups.len(), 1);
        assert_eq!(model.groups[0].status, WorkspaceGroupStatus::Current);
        assert_eq!(model.groups[0].name, "proj");
    }

    #[test]
    fn known_workspace_group_status_when_path_exists() {
        let sessions = vec![sample_session("s1", "Alpha", Some("/tmp"))];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &[],
            0,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert_eq!(model.groups[0].status, WorkspaceGroupStatus::Known);
    }

    #[test]
    fn missing_workspace_path_shows_warning_status() {
        let sessions = vec![sample_session(
            "s1",
            "Alpha",
            Some("/definitely/not/a/real/path-12345"),
        )];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &[],
            0,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert_eq!(model.groups[0].status, WorkspaceGroupStatus::MissingPath);
    }

    #[test]
    fn untagged_legacy_workspace_status() {
        let sessions = vec![sample_session("s1", "Alpha", None)];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &[],
            0,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert_eq!(model.groups[0].status, WorkspaceGroupStatus::UntaggedLegacy);
        assert_eq!(model.groups[0].name, "Untagged");
    }

    #[test]
    fn pinned_sessions_appear_first() {
        let sessions = vec![
            sample_session("s1", "First", None),
            sample_session("s2", "Second", None),
        ];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &["s2".to_string()],
            0,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert_eq!(model.total_rows(), 2);
        assert!(model.groups[0].rows[0].is_pinned);
        assert_eq!(model.groups[0].rows[0].id, "s2");
    }

    #[test]
    fn selection_and_current_flags_are_set() {
        let sessions = vec![
            sample_session("s1", "Alpha", None),
            sample_session("s2", "Beta", None),
        ];
        let model = SessionListViewModel::build(
            &sessions,
            Some("s2"),
            "/home/gex/proj",
            &[],
            1,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        let row0 = &model.groups[0].rows[0];
        let row1 = &model.groups[0].rows[1];
        assert!(!row0.is_current && !row0.is_selected);
        assert!(row1.is_current && row1.is_selected);
    }

    #[test]
    fn delete_hint_is_set_on_pending_delete() {
        let sessions = vec![sample_session("s1", "Alpha", None)];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &[],
            0,
            Some("s1"),
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert!(model.groups[0].rows[0].delete_hint);
    }

    #[test]
    fn rename_state_is_preserved() {
        let rename = RenameState {
            session_id: "s1".to_string(),
            buffer: "New Title".to_string(),
        };
        let sessions = vec![sample_session("s1", "Alpha", None)];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &[],
            0,
            None,
            Some(&rename),
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert_eq!(model.rename.as_ref().unwrap().session_id, "s1");
        assert_eq!(model.rename.as_ref().unwrap().buffer, "New Title");
    }

    #[test]
    fn groups_sorted_current_first() {
        let sessions = vec![
            sample_session("s1", "Beta", Some("/tmp")),
            sample_session("s2", "Alpha", Some("/home/gex/proj")),
        ];
        let model = SessionListViewModel::build(
            &sessions,
            None,
            "/home/gex/proj",
            &[],
            0,
            None,
            None,
            "",
            false,
            40,
            &HashMap::new(),
        );
        assert_eq!(model.groups[0].name, "proj");
        assert_eq!(model.groups[0].status, WorkspaceGroupStatus::Current);
        assert_eq!(model.groups[1].status, WorkspaceGroupStatus::Known);
    }
}

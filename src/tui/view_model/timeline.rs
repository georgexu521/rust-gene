use crate::{
    state::{MessageItem, MessageRole},
    tui::{
        app::TuiApp,
        tool_view::ToolRunView,
        view_model::reasoning::assistant_reasoning_view,
        view_model::reasoning::expanded_reasoning_height,
        view_model::tool_rows::{tool_row_height, tool_rows_for_runs_with_spine},
    },
};

#[derive(Debug, Clone, Copy)]
pub enum TimelineItem<'a> {
    Message {
        message_index: usize,
        id: &'a str,
        msg: &'a MessageItem,
    },
    ToolRuns {
        id: &'a str,
        parent_message_id: &'a str,
        runs: &'a [ToolRunView],
    },
}

impl<'a> TimelineItem<'a> {
    pub fn stable_id(&self) -> &'a str {
        match self {
            Self::Message { id, .. } | Self::ToolRuns { id, .. } => id,
        }
    }

    pub fn is_user_message(&self) -> bool {
        matches!(self, Self::Message { msg, .. } if msg.role == MessageRole::User)
    }
}

pub fn timeline_index_by_stable_id(items: &[TimelineItem<'_>], stable_id: &str) -> Option<usize> {
    items.iter().position(|item| item.stable_id() == stable_id)
}

pub fn resolve_scroll_offset(
    items: &[TimelineItem<'_>],
    fallback_offset: usize,
    anchor_id: Option<&str>,
) -> usize {
    anchor_id
        .and_then(|id| timeline_index_by_stable_id(items, id))
        .unwrap_or(fallback_offset)
}

pub fn timeline_items<'a>(messages: &[&'a MessageItem], app: &'a TuiApp) -> Vec<TimelineItem<'a>> {
    let tool_group_count = if app.focus_mode {
        0
    } else {
        messages
            .iter()
            .filter(|msg| {
                msg.role == MessageRole::User && app.tool_runs_for_message(&msg.id).is_some()
            })
            .count()
    };
    let mut items = Vec::with_capacity(messages.len() + tool_group_count);

    for (idx, msg) in messages.iter().enumerate() {
        items.push(TimelineItem::Message {
            message_index: idx,
            id: &msg.id,
            msg,
        });
        if !app.focus_mode && msg.role == MessageRole::User {
            if let Some(runs) = app.tool_runs_for_message(&msg.id) {
                let first_run_id = runs.first().map(|run| run.id.as_str()).unwrap_or("tools");
                items.push(TimelineItem::ToolRuns {
                    id: first_run_id,
                    parent_message_id: &msg.id,
                    runs,
                });
            }
        }
    }

    items
}

pub fn timeline_item_heights(items: &[TimelineItem<'_>], width: usize, app: &TuiApp) -> Vec<usize> {
    items
        .iter()
        .map(|item| estimate_timeline_item_height(item, width, app))
        .collect()
}

pub fn estimate_timeline_item_height(item: &TimelineItem<'_>, width: usize, app: &TuiApp) -> usize {
    match item {
        TimelineItem::Message {
            message_index, msg, ..
        } => {
            let collapsed = app.collapsed_indices.contains(message_index);
            let reasoning_expanded =
                app.expanded_reasoning_message_id.as_deref() == Some(msg.id.as_str());
            estimate_message_height_with_reasoning(msg, width, collapsed, reasoning_expanded)
        }
        TimelineItem::ToolRuns { runs, .. } => estimate_tool_runs_height(runs, app),
    }
}

pub fn estimate_message_height(msg: &MessageItem, width: usize, collapsed: bool) -> usize {
    estimate_message_height_with_reasoning(msg, width, collapsed, false)
}

pub fn estimate_message_height_with_reasoning(
    msg: &MessageItem,
    width: usize,
    collapsed: bool,
    reasoning_expanded: bool,
) -> usize {
    if collapsed {
        return 2;
    }

    let reasoning =
        (msg.role == MessageRole::Assistant).then(|| assistant_reasoning_view(&msg.content));
    let visible_content = reasoning
        .as_ref()
        .map(|view| view.visible_answer.as_str())
        .unwrap_or(msg.content.as_str());
    let reasoning_summary_height = reasoning
        .as_ref()
        .is_some_and(|view| view.has_hidden_reasoning())
        as usize;
    let reasoning_body_height = reasoning
        .as_ref()
        .filter(|view| reasoning_expanded && view.has_hidden_reasoning())
        .map(expanded_reasoning_height)
        .unwrap_or(0);

    let base_height = 1 + reasoning_summary_height + reasoning_body_height;
    let effective_width = width.saturating_sub(4).max(1);

    let mut lines = 0;
    let mut in_code_block = false;
    let mut last_was_text = false;

    for raw_line in visible_content.lines() {
        let trimmed = raw_line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            lines += 1;
            last_was_text = false;
        } else if in_code_block {
            lines += 1;
            last_was_text = false;
        } else if trimmed.is_empty() {
            if last_was_text {
                lines += 1;
            }
            last_was_text = false;
        } else if trimmed.starts_with('#') {
            lines += 1;
            last_was_text = true;
        } else if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            lines += 2;
            last_was_text = false;
        } else {
            let display_width = unicode_width::UnicodeWidthStr::width(raw_line);
            lines += display_width.div_ceil(effective_width).max(1);
            last_was_text = true;
        }
    }

    base_height + lines.max(1)
}

pub fn estimate_tool_runs_height(runs: &[ToolRunView], app: &TuiApp) -> usize {
    let view = tool_rows_for_runs_with_spine(runs, &app.facade_snapshot.tool_turns, 100);
    let lines = view
        .rows
        .iter()
        .zip(runs.iter())
        .filter(|(row, _)| row.visible)
        .map(|(row, run)| tool_row_height(row, app.is_tool_run_expanded(run), run))
        .sum::<usize>()
        + usize::from(view.hidden_routine_count > 0);
    lines.max(1) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MessageRole;
    use std::{collections::HashMap, time::SystemTime};

    fn msg(role: MessageRole, id: &str) -> MessageItem {
        MessageItem {
            id: id.to_string(),
            role,
            content: id.to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn inserts_tool_runs_after_parent_user_message() {
        let mut app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "user_1"),
            msg(MessageRole::Assistant, "assistant_1"),
        ];
        app.sync_snapshot.tool_runs_by_message_id.insert(
            "user_1".to_string(),
            vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
        );
        let refs = items.iter().collect::<Vec<_>>();

        let timeline = timeline_items(&refs, &app);

        assert_eq!(timeline.len(), 3);
        assert!(matches!(
            timeline[0],
            TimelineItem::Message { id: "user_1", .. }
        ));
        assert!(matches!(
            timeline[1],
            TimelineItem::ToolRuns {
                id: "tool_1",
                parent_message_id: "user_1",
                ..
            }
        ));
    }

    #[test]
    fn focus_mode_hides_tool_runs() {
        let mut app = TuiApp::new();
        app.focus_mode = true;
        let item = msg(MessageRole::User, "user_1");
        app.sync_snapshot.tool_runs_by_message_id.insert(
            "user_1".to_string(),
            vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
        );
        let refs = vec![&item];

        let timeline = timeline_items(&refs, &app);

        assert_eq!(timeline.len(), 1);
        assert!(matches!(timeline[0], TimelineItem::Message { .. }));
    }

    #[test]
    fn resolves_scroll_offset_from_stable_id_after_insertions() {
        let app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "new_user"),
            msg(MessageRole::Assistant, "new_assistant"),
            msg(MessageRole::User, "old_user"),
            msg(MessageRole::Assistant, "old_assistant"),
        ];
        let refs = items.iter().collect::<Vec<_>>();
        let timeline = timeline_items(&refs, &app);

        assert_eq!(resolve_scroll_offset(&timeline, 0, Some("old_user")), 2);
        assert_eq!(resolve_scroll_offset(&timeline, 3, Some("missing")), 3);
    }

    #[test]
    fn estimates_wrapped_message_height_with_unicode_width() {
        let item = msg(MessageRole::Assistant, "中文中文中文");

        assert!(estimate_message_height(&item, 8, false) > 2);
        assert_eq!(estimate_message_height(&item, 8, true), 2);
    }

    #[test]
    fn assistant_height_collapses_hidden_reasoning() {
        let with_reasoning = msg(
            MessageRole::Assistant,
            "<think>line one\nline two\nline three</think>\nAnswer",
        );
        let without_reasoning = msg(MessageRole::Assistant, "Answer");

        assert_eq!(
            estimate_message_height(&with_reasoning, 80, false),
            estimate_message_height(&without_reasoning, 80, false) + 1
        );
    }

    #[test]
    fn assistant_height_accounts_for_expanded_reasoning() {
        let item = msg(
            MessageRole::Assistant,
            "<think>line one\nline two\nline three</think>\nAnswer",
        );

        let collapsed_reasoning = estimate_message_height(&item, 80, false);
        let expanded_reasoning = estimate_message_height_with_reasoning(&item, 80, false, true);

        assert!(expanded_reasoning > collapsed_reasoning);
    }

    #[test]
    fn estimates_expanded_tool_run_taller_than_collapsed() {
        let mut app = TuiApp::new();
        let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        run.arguments = Some(serde_json::json!({ "command": "cargo test" }));
        run.mark_complete("Result: OK\nline one\nline two".to_string());
        let runs = vec![run];

        let collapsed = estimate_tool_runs_height(&runs, &app);
        app.expanded_tool_run_id = Some("tool_1".to_string());
        let expanded = estimate_tool_runs_height(&runs, &app);

        assert!(expanded > collapsed);
    }
}

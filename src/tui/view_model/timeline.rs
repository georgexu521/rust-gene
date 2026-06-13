use crate::{
    state::{MessageItem, MessageRole},
    tui::{
        app::TuiApp,
        sync_store::TuiMessagePart,
        tool_view::ToolRunView,
        view_model::reasoning::assistant_reasoning_view,
        view_model::tool_rows::{tool_row_height, tool_rows_for_runs_with_spine},
    },
};

#[derive(Debug, Clone)]
pub enum TimelineItem<'a> {
    Message {
        message_index: usize,
        id: &'a str,
        msg: &'a MessageItem,
        parts: Option<&'a [TuiMessagePart]>,
    },
}

impl<'a> TimelineItem<'a> {
    pub fn stable_id(&self) -> &str {
        match self {
            Self::Message { id, .. } => id,
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
    let mut items = Vec::with_capacity(messages.len());

    for (idx, msg) in messages.iter().enumerate() {
        let parts = (msg.role == MessageRole::Assistant || !app.focus_mode)
            .then(|| app.sync_snapshot.parts_for_message(&msg.id))
            .flatten()
            .map(|v| v.as_slice());
        items.push(TimelineItem::Message {
            message_index: idx,
            id: &msg.id,
            msg,
            parts,
        });
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
            message_index,
            msg,
            parts,
            ..
        } => {
            let collapsed = app.collapsed_indices.contains(message_index);
            let reasoning_expanded =
                app.expanded_reasoning_message_id.as_deref() == Some(msg.id.as_str());
            let message_parts = (msg.role == MessageRole::Assistant)
                .then_some(*parts)
                .flatten();
            let base_height = estimate_message_height_with_parts_or_reasoning(
                msg,
                message_parts,
                width,
                collapsed,
                reasoning_expanded,
            );
            if collapsed || msg.role != MessageRole::User {
                base_height
            } else {
                base_height + estimate_tool_parts_height(*parts, app)
            }
        }
    }
}

pub fn estimate_message_height_with_parts_or_reasoning(
    msg: &MessageItem,
    parts: Option<&[TuiMessagePart]>,
    width: usize,
    collapsed: bool,
    reasoning_expanded: bool,
) -> usize {
    if collapsed {
        return 2;
    }

    let (visible_content, reasoning_text_owned, has_reasoning) = if let Some(parts) = parts {
        let text = parts
            .iter()
            .find(|p| p.kind == crate::tui::sync_store::TuiPartKind::Text)
            .map(|p| p.text.clone())
            .unwrap_or_default();
        let reasoning = parts
            .iter()
            .find(|p| p.kind == crate::tui::sync_store::TuiPartKind::Thinking)
            .map(|p| p.text.clone())
            .unwrap_or_default();
        let has_reasoning = !reasoning.trim().is_empty();
        (text, reasoning, has_reasoning)
    } else {
        let view = assistant_reasoning_view(&msg.content);
        (
            view.visible_answer.clone(),
            view.hidden_reasoning.clone(),
            view.has_hidden_reasoning(),
        )
    };

    let visible_content = visible_content.as_str();
    let reasoning_text = reasoning_text_owned.as_str();

    let reasoning_summary_height = usize::from(has_reasoning);
    let reasoning_body_height = if reasoning_expanded && has_reasoning {
        expanded_reasoning_height_for_text(reasoning_text)
    } else {
        0
    };

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

pub fn estimate_message_height(msg: &MessageItem, width: usize, collapsed: bool) -> usize {
    estimate_message_height_with_parts_or_reasoning(msg, None, width, collapsed, false)
}

fn expanded_reasoning_height_for_text(reasoning: &str) -> usize {
    let shown = reasoning
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(crate::tui::view_model::reasoning::EXPANDED_REASONING_MAX_LINES)
        .count();
    if shown == 0 {
        0
    } else {
        shown
            + usize::from(
                reasoning
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count()
                    > crate::tui::view_model::reasoning::EXPANDED_REASONING_MAX_LINES,
            )
    }
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

pub fn estimate_tool_parts_height(parts: Option<&[TuiMessagePart]>, app: &TuiApp) -> usize {
    parts
        .map(tool_runs_from_parts)
        .filter(|runs| !runs.is_empty())
        .map(|runs| estimate_tool_runs_height(&runs, app))
        .unwrap_or(0)
}

pub fn tool_runs_from_parts(parts: &[TuiMessagePart]) -> Vec<ToolRunView> {
    parts
        .iter()
        .filter(|part| part.kind == crate::tui::sync_store::TuiPartKind::Tool)
        .filter_map(|part| part.tool_run.clone())
        .collect()
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
    fn keeps_tool_runs_inside_parent_user_message_parts() {
        let mut app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "user_1"),
            msg(MessageRole::Assistant, "assistant_1"),
        ];
        app.sync_snapshot.set_tool_runs_for_message(
            "user_1".to_string(),
            vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
        );
        let refs = items.iter().collect::<Vec<_>>();

        let timeline = timeline_items(&refs, &app);

        assert_eq!(timeline.len(), 2);
        match &timeline[0] {
            TimelineItem::Message {
                id: "user_1",
                parts: Some(parts),
                ..
            } => {
                let runs = tool_runs_from_parts(parts);
                assert_eq!(runs.len(), 1);
                assert_eq!(runs[0].id, "tool_1");
            }
            item => panic!("expected user message with tool parts, got {item:?}"),
        }
    }

    #[test]
    fn focus_mode_hides_tool_runs() {
        let mut app = TuiApp::new();
        app.focus_mode = true;
        let item = msg(MessageRole::User, "user_1");
        app.sync_snapshot.set_tool_runs_for_message(
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
        let expanded_reasoning =
            estimate_message_height_with_parts_or_reasoning(&item, None, 80, false, true);

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

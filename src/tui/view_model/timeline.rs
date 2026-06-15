use crate::{
    state::MessageItem,
    tui::{
        app::TuiApp,
        components::{
            collapsible::{flatten_line_breaks, wrap_line_to_width},
            markdown::parse_markdown,
        },
        render_session::{TuiRenderMessage, TuiRenderRole, TuiRenderSession},
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
        role: TuiRenderRole,
        content: &'a str,
        metadata: &'a std::collections::HashMap<String, String>,
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
        matches!(self, Self::Message { role, .. } if *role == TuiRenderRole::User)
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

pub fn timeline_row_offset_for_index(heights: &[usize], index: usize) -> usize {
    heights.iter().take(index.min(heights.len())).sum()
}

pub fn timeline_index_at_row_offset(heights: &[usize], row_offset: usize) -> (usize, usize) {
    if heights.is_empty() {
        return (0, 0);
    }

    let mut remaining = row_offset;
    for (index, height) in heights.iter().enumerate() {
        let height = (*height).max(1);
        if remaining < height {
            return (index, remaining);
        }
        remaining = remaining.saturating_sub(height);
    }

    let last = heights.len().saturating_sub(1);
    (last, heights[last].saturating_sub(1))
}

pub fn resolve_scroll_row_offset(
    items: &[TimelineItem<'_>],
    heights: &[usize],
    fallback_row_offset: usize,
    anchor_id: Option<&str>,
    anchor_row_offset: usize,
) -> usize {
    let Some(anchor_id) = anchor_id else {
        return fallback_row_offset;
    };
    let Some(index) = timeline_index_by_stable_id(items, anchor_id) else {
        return fallback_row_offset;
    };
    let base = timeline_row_offset_for_index(heights, index);
    let height = heights.get(index).copied().unwrap_or(1).max(1);
    base + anchor_row_offset.min(height.saturating_sub(1))
}

pub fn timeline_items(render_session: &TuiRenderSession) -> Vec<TimelineItem<'_>> {
    let mut items = Vec::with_capacity(render_session.messages.len());

    for (idx, message) in render_session.messages.iter().enumerate() {
        let parts = (!message.parts.is_empty()).then_some(message.parts.as_slice());
        items.push(TimelineItem::Message {
            message_index: idx,
            id: &message.id,
            role: message.role,
            content: message_content_for_render(message, parts),
            metadata: &message.metadata,
            parts,
        });
    }

    items
}

fn message_content_for_render<'a>(
    _message: &'a TuiRenderMessage,
    parts: Option<&'a [TuiMessagePart]>,
) -> &'a str {
    parts
        .and_then(|parts| {
            parts
                .iter()
                .find(|part| part.kind == crate::tui::sync_store::TuiPartKind::Text)
                .map(|part| part.text.as_str())
        })
        .unwrap_or("")
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
            id,
            role,
            content,
            parts,
            ..
        } => {
            let collapsed = app.collapsed_indices.contains(message_index);
            let reasoning_expanded = app.expanded_reasoning_message_id.as_deref() == Some(*id);
            let message_parts = (*role == TuiRenderRole::Assistant)
                .then_some(*parts)
                .flatten();
            let text_part_id = message_parts.and_then(|ps| {
                ps.iter()
                    .find(|p| p.kind == crate::tui::sync_store::TuiPartKind::Text)
                    .map(|p| p.id.clone())
            });
            let text_part_expanded = text_part_id
                .map(|id| app.expanded_inline_message_part_ids.contains(&id))
                .unwrap_or_else(|| {
                    app.expanded_inline_message_part_ids.contains(
                        &crate::tui::sync_store::part_id_for(
                            id,
                            crate::tui::sync_store::TuiPartKind::Text,
                        ),
                    )
                });
            let base_height = estimate_message_height_with_parts_or_reasoning(
                content,
                message_parts,
                width,
                collapsed,
                reasoning_expanded,
                text_part_expanded,
            );
            if collapsed || *role != TuiRenderRole::User {
                base_height
            } else {
                base_height + estimate_tool_parts_height(*parts, app)
            }
        }
    }
}

pub fn estimate_message_height_with_parts_or_reasoning(
    content: &str,
    parts: Option<&[TuiMessagePart]>,
    width: usize,
    collapsed: bool,
    reasoning_expanded: bool,
    text_part_expanded: bool,
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
        let view = assistant_reasoning_view(content);
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
    // The rendered paragraph has a 2-space left indent; reserve that budget so
    // wrap calculations match what ratatui will actually draw.
    let effective_width = width.saturating_sub(4).saturating_sub(2).max(1);

    let markdown_lines = parse_markdown(visible_content, &crate::tui::theme::Theme::default());
    let flat_lines = flatten_line_breaks(markdown_lines.lines);

    let mut lines = 0usize;
    for line in flat_lines {
        let rendered: String = line
            .spans
            .iter()
            .map(|span| span.content.to_string())
            .collect();
        lines += wrap_line_to_width(&rendered, effective_width).len().max(1);
    }

    if !text_part_expanded {
        let max_lines = crate::tui::components::collapsible::DEFAULT_TEXT_PART_MAX_LINES;
        if lines > max_lines {
            return base_height + max_lines + 1;
        }
    }

    base_height + lines.max(1)
}

pub fn estimate_message_height(msg: &MessageItem, width: usize, collapsed: bool) -> usize {
    estimate_message_height_with_parts_or_reasoning(
        &msg.content,
        None,
        width,
        collapsed,
        false,
        false,
    )
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
        .map(|(row, run)| {
            let row_height = tool_row_height(row, app.is_tool_run_expanded(run), run);
            let body_height = if app.is_tool_run_expanded(run) {
                let inline_expanded = app.expanded_inline_tool_ids.contains(&run.id);
                crate::tui::components::tool_renderers::estimate_tool_body_height(
                    run,
                    100,
                    inline_expanded,
                )
            } else {
                0
            };
            row_height + body_height
        })
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
    use crate::state::{MessageItem, MessageRole};
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
        let render_session = app.sync_snapshot.render_session(&items);

        let timeline = timeline_items(&render_session);

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
        let render_session = app
            .sync_snapshot
            .render_session(std::slice::from_ref(&item));

        let timeline = timeline_items(&render_session);

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
        let render_session = app.sync_snapshot.render_session(&items);
        let timeline = timeline_items(&render_session);

        assert_eq!(resolve_scroll_offset(&timeline, 0, Some("old_user")), 2);
        assert_eq!(resolve_scroll_offset(&timeline, 3, Some("missing")), 3);
    }

    #[test]
    fn maps_timeline_rows_to_item_and_intra_offsets() {
        let heights = vec![2, 5, 3];

        assert_eq!(timeline_row_offset_for_index(&heights, 2), 7);
        assert_eq!(timeline_index_at_row_offset(&heights, 0), (0, 0));
        assert_eq!(timeline_index_at_row_offset(&heights, 3), (1, 1));
        assert_eq!(timeline_index_at_row_offset(&heights, 99), (2, 2));
    }

    #[test]
    fn resolves_scroll_row_offset_from_stable_id_after_insertions() {
        let app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "new_user"),
            msg(MessageRole::Assistant, "new_assistant"),
            msg(MessageRole::User, "old_user"),
            msg(MessageRole::Assistant, "old_assistant"),
        ];
        let render_session = app.sync_snapshot.render_session(&items);
        let timeline = timeline_items(&render_session);
        let heights = vec![1, 2, 3, 4];

        assert_eq!(
            resolve_scroll_row_offset(&timeline, &heights, 0, Some("old_user"), 2),
            5
        );
        assert_eq!(
            resolve_scroll_row_offset(&timeline, &heights, 7, Some("missing"), 1),
            7
        );
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
        let expanded_reasoning = estimate_message_height_with_parts_or_reasoning(
            &item.content,
            None,
            80,
            false,
            true,
            false,
        );

        assert!(expanded_reasoning > collapsed_reasoning);
    }

    #[test]
    fn assistant_height_accounts_for_markdown_tables_and_lists() {
        let item = msg(
            MessageRole::Assistant,
            "## phageGPT（PhageMatch）是什么？\n\n\
             这是一个噬菌体（细菌病毒）-耐药菌匹配平台。\n\n\
             **核心理念**\n\
             不是做分子级别的\"预测\"，而是做基于数据的推荐。\n\n\
             **匹配算法（4个维度）**\n\
             | 维度 | 权重 | 说明 |\n\
             |------|------|------|\n\
             | 物种匹配 | 40% | 同物种噬菌体优先推荐 |\n\
             | MLST型匹配 | 30% | 基于历史裂解数据 |\n\n\
             **技术栈**\n\
             1. 前端：React + TypeScript + Vite + React Router + TanStack Query\n\
             2. 后端：Node.js + Express + Prisma ORM\n\
             3. 数据库：SQLite\n\
             4. 多模型支持：GPT-4o、DeepSeek 等",
        );

        let height = estimate_message_height(&item, 80, false);
        // The old heuristic underestimated this kind of mixed markdown because
        // it did not account for list numbering/indent and used raw-line wrap
        // arithmetic. The new parser-based estimate should be at least 15 lines.
        assert!(height >= 15, "estimated height {} should be >= 15", height);
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

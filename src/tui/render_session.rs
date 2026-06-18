//! Pure render session model for the TUI timeline.
//!
//! This is the single source of truth for rendering a session timeline:
//! projection events produce `TuiRenderSession`, and the timeline renderer
//! consumes it. Legacy `MessageItem` compatibility is handled at the bridge
//! boundary inside `TuiSyncSnapshot::render_session`.

use crate::state::MessageItem;
use crate::tui::sync_store::{TuiMessagePart, TuiPartKind, TuiSessionPhase, TuiSyncSnapshot};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TuiRenderSession {
    pub phase: TuiSessionPhase,
    pub messages: Vec<TuiRenderMessage>,
    pub last_projection_seq: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TuiRenderMessage {
    pub id: String,
    pub role: TuiRenderRole,
    pub parts: Vec<TuiMessagePart>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiRenderRole {
    User,
    Assistant,
}

impl TuiRenderMessage {
    pub fn text_parts(&self) -> Vec<&TuiMessagePart> {
        self.parts
            .iter()
            .filter(|part| part.kind == TuiPartKind::Text)
            .collect()
    }

    pub fn tool_parts(&self) -> Vec<&TuiMessagePart> {
        self.parts
            .iter()
            .filter(|part| part.kind == TuiPartKind::Tool)
            .collect()
    }

    pub fn thinking_parts(&self) -> Vec<&TuiMessagePart> {
        self.parts
            .iter()
            .filter(|part| part.kind == TuiPartKind::Thinking)
            .collect()
    }
}

impl TuiRenderSession {
    pub fn all_tool_parts(&self) -> Vec<&TuiMessagePart> {
        self.messages
            .iter()
            .flat_map(|message| message.tool_parts())
            .collect()
    }

    pub fn active_tool_parts(&self) -> Vec<&TuiMessagePart> {
        self.all_tool_parts()
            .into_iter()
            .filter(|part| {
                part.tool_run
                    .as_ref()
                    .map(|run| run.is_active())
                    .unwrap_or(false)
            })
            .collect()
    }
}

impl TuiSyncSnapshot {
    /// Build a render-session snapshot from projection state.
    ///
    /// `fallback_messages` provides legacy `MessageItem` ordering/content for
    /// messages that have not yet entered the projection store (e.g. historical
    /// sessions restored before projection hydration).
    pub fn render_session(&self, fallback_messages: &[MessageItem]) -> TuiRenderSession {
        let mut messages = Vec::with_capacity(fallback_messages.len());
        let mut seen_projection_ids = std::collections::BTreeSet::new();

        for message in fallback_messages {
            if let Some(projection) = self
                .messages
                .iter()
                .find(|projection| projection.id == message.id)
            {
                seen_projection_ids.insert(message.id.clone());
                let mut parts: Vec<TuiMessagePart> = projection
                    .part_ids
                    .iter()
                    .filter_map(|part_id| {
                        self.parts_by_message_id
                            .get(&message.id)
                            .and_then(|parts| parts.iter().find(|part| &part.id == part_id))
                    })
                    .cloned()
                    .collect();
                // User messages may have a projection entry but no text part
                // (e.g. TurnStarted creates an empty projection). Synthesize a
                // text part from the legacy MessageItem content so the message
                // does not render as empty.
                if projection.role == crate::tui::sync_store::TuiMessageRole::User
                    && !message.content.is_empty()
                    && !parts
                        .iter()
                        .any(|p| p.kind == crate::tui::sync_store::TuiPartKind::Text)
                {
                    parts.push(TuiMessagePart {
                        id: crate::tui::sync_store::part_id_for(
                            &message.id,
                            crate::tui::sync_store::TuiPartKind::Text,
                        ),
                        message_id: message.id.clone(),
                        kind: crate::tui::sync_store::TuiPartKind::Text,
                        text: message.content.clone(),
                        tool_run: None,
                        streaming: false,
                    });
                }
                messages.push(TuiRenderMessage {
                    id: message.id.clone(),
                    role: render_role_from_projection(projection.role),
                    parts,
                    metadata: message.metadata.clone(),
                });
            } else {
                // Legacy message not yet in projection store: synthesize text
                // and thinking parts from its content so rendering can still
                // consume parts without falling back to raw MessageItem content.
                let mut parts = Vec::new();
                if !message.content.is_empty() {
                    let view = crate::tui::view_model::reasoning::assistant_reasoning_view(
                        &message.content,
                    );
                    if !view.hidden_reasoning.is_empty() {
                        let thinking_id = crate::tui::sync_store::part_id_for(
                            &message.id,
                            crate::tui::sync_store::TuiPartKind::Thinking,
                        );
                        parts.push(TuiMessagePart {
                            id: thinking_id,
                            message_id: message.id.clone(),
                            kind: crate::tui::sync_store::TuiPartKind::Thinking,
                            text: view.hidden_reasoning.clone(),
                            tool_run: None,
                            streaming: false,
                        });
                    }
                    let text_id = crate::tui::sync_store::part_id_for(
                        &message.id,
                        crate::tui::sync_store::TuiPartKind::Text,
                    );
                    parts.push(TuiMessagePart {
                        id: text_id,
                        message_id: message.id.clone(),
                        kind: crate::tui::sync_store::TuiPartKind::Text,
                        text: view.visible_answer.clone(),
                        tool_run: None,
                        streaming: false,
                    });
                }
                messages.push(TuiRenderMessage {
                    id: message.id.clone(),
                    role: render_role_from_state(message.role),
                    parts,
                    metadata: message.metadata.clone(),
                });
            }
        }

        // Append active assistant message if it only exists in projection state.
        if let Some(assistant_id) = self.active_assistant_message_id.as_deref() {
            if !messages.iter().any(|message| message.id == assistant_id)
                && !seen_projection_ids.contains(assistant_id)
            {
                if let Some(projection) = self
                    .messages
                    .iter()
                    .find(|projection| projection.id == assistant_id)
                {
                    let parts = projection
                        .part_ids
                        .iter()
                        .filter_map(|part_id| {
                            self.parts_by_message_id
                                .get(assistant_id)
                                .and_then(|parts| parts.iter().find(|part| &part.id == part_id))
                        })
                        .cloned()
                        .collect();
                    messages.push(TuiRenderMessage {
                        id: assistant_id.to_string(),
                        role: TuiRenderRole::Assistant,
                        parts,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        TuiRenderSession {
            phase: self.phase,
            messages,
            last_projection_seq: self.last_projection_seq,
            last_error: self.last_error.clone(),
        }
    }
}

fn render_role_from_projection(role: crate::tui::sync_store::TuiMessageRole) -> TuiRenderRole {
    match role {
        crate::tui::sync_store::TuiMessageRole::User => TuiRenderRole::User,
        crate::tui::sync_store::TuiMessageRole::Assistant => TuiRenderRole::Assistant,
    }
}

fn render_role_from_state(role: crate::state::MessageRole) -> TuiRenderRole {
    match role {
        crate::state::MessageRole::User => TuiRenderRole::User,
        crate::state::MessageRole::Assistant => TuiRenderRole::Assistant,
        crate::state::MessageRole::System => TuiRenderRole::Assistant,
        crate::state::MessageRole::Tool => TuiRenderRole::Assistant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MessageItem, MessageRole};
    use crate::tui::sync_store::TuiSyncStore;

    fn user_msg(id: &str, content: &str) -> MessageItem {
        MessageItem {
            id: id.to_string(),
            role: MessageRole::User,
            content: content.to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: HashMap::new(),
        }
    }

    fn assistant_msg(id: &str, content: &str) -> MessageItem {
        MessageItem {
            id: id.to_string(),
            role: MessageRole::Assistant,
            content: content.to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: HashMap::new(),
        }
    }

    fn live_events() -> Vec<crate::session_store::SessionProjectionEvent> {
        vec![
            crate::session_store::SessionProjectionEvent::TurnStarted {
                user_message_id: "user_1".to_string(),
                assistant_message_id: "assistant_1".to_string(),
            },
            crate::session_store::SessionProjectionEvent::AssistantTextDelta {
                message_id: Some("assistant_1".to_string()),
                text: "Hello ".to_string(),
            },
            crate::session_store::SessionProjectionEvent::AssistantTextDelta {
                message_id: Some("assistant_1".to_string()),
                text: "world".to_string(),
            },
            crate::session_store::SessionProjectionEvent::ToolCallStarted {
                message_id: Some("user_1".to_string()),
                tool_call_id: "tool_1".to_string(),
                tool_name: "bash".to_string(),
            },
            crate::session_store::SessionProjectionEvent::ToolExecutionCompleted {
                tool_call_id: "tool_1".to_string(),
                result: "ok".to_string(),
                metadata: Some(serde_json::json!({"success": true})),
                result_data: None,
            },
        ]
    }

    fn persisted_parts() -> Vec<crate::session_store::PersistedSessionPart> {
        vec![
            crate::session_store::PersistedSessionPart {
                id: 1,
                session_id: "sess_1".to_string(),
                part_index: 0,
                part_id: "assistant_1:text".to_string(),
                kind: "assistant_text".to_string(),
                tool_call_id: None,
                tool_name: None,
                status: None,
                payload: serde_json::json!({"content": "Hello world"}),
                projected_to_seq: 1,
                updated_at: "2026-01-01T00:00:00Z".to_string(),
                message_id: Some("assistant_1".to_string()),
            },
            crate::session_store::PersistedSessionPart {
                id: 2,
                session_id: "sess_1".to_string(),
                part_index: 1,
                part_id: "assistant_1:tool:tool_1".to_string(),
                kind: "tool".to_string(),
                tool_call_id: Some("tool_1".to_string()),
                tool_name: Some("bash".to_string()),
                status: Some("completed".to_string()),
                payload: serde_json::json!({
                    "input_args": r#"{"command":"echo hi"}"#,
                    "result_preview": "ok",
                    "tool_name": "bash",
                }),
                projected_to_seq: 2,
                updated_at: "2026-01-01T00:00:01Z".to_string(),
                message_id: Some("assistant_1".to_string()),
            },
        ]
    }

    fn base_messages() -> Vec<MessageItem> {
        vec![user_msg("user_1", "run"), assistant_msg("assistant_1", "")]
    }

    #[test]
    fn live_projection_and_persisted_hydration_produce_same_render_session() {
        let mut live_store = TuiSyncStore::new();
        for event in live_events() {
            live_store.apply_projection_event(&event);
        }
        let live_render = live_store.snapshot().render_session(&base_messages());

        let mut hydrate_store = TuiSyncStore::new();
        let mut bus = crate::session_store::SessionProjectionEventBus::from_seq(0);
        for part in persisted_parts() {
            if let Some(event) =
                crate::session_store::SessionProjectionEvent::from_persisted_part(&part, None)
            {
                let envelope = bus.publish(event);
                hydrate_store.apply_projection_envelope(&envelope);
            }
        }
        let hydrated_render = hydrate_store.snapshot().render_session(&base_messages());

        assert_eq!(live_render.messages.len(), hydrated_render.messages.len());
        for (live, hydrated) in live_render
            .messages
            .iter()
            .zip(hydrated_render.messages.iter())
        {
            assert_eq!(live.id, hydrated.id);
            assert_eq!(live.role, hydrated.role);
            assert_eq!(
                live.parts.len(),
                hydrated.parts.len(),
                "part count mismatch for {}",
                live.id
            );
            for (live_part, hydrated_part) in live.parts.iter().zip(hydrated.parts.iter()) {
                assert_eq!(live_part.kind, hydrated_part.kind);
                assert_eq!(live_part.text, hydrated_part.text);
                assert_eq!(
                    live_part.tool_run.as_ref().map(|r| r.status),
                    hydrated_part.tool_run.as_ref().map(|r| r.status)
                );
            }
        }
        // Phase may differ: live streaming leaves Running, persisted hydration has no RunStarted.
        assert!(
            matches!(
                live_render.phase,
                TuiSessionPhase::Running | TuiSessionPhase::Completed
            ) || live_render.phase == hydrated_render.phase,
            "phase mismatch: live={:?}, hydrated={:?}",
            live_render.phase,
            hydrated_render.phase
        );
    }

    #[test]
    fn render_session_includes_active_assistant_message() {
        let mut store = TuiSyncStore::new();
        store.start_turn("user_1".to_string(), "assistant_1".to_string());
        store.apply_projection_event(
            &crate::session_store::SessionProjectionEvent::AssistantTextDelta {
                message_id: Some("assistant_1".to_string()),
                text: "streaming".to_string(),
            },
        );

        let render = store.snapshot().render_session(&[user_msg("user_1", "hi")]);

        assert_eq!(render.messages.len(), 2);
        assert_eq!(render.messages[1].id, "assistant_1");
        assert_eq!(render.messages[1].role, TuiRenderRole::Assistant);
        let text = render.messages[1]
            .text_parts()
            .first()
            .map(|p| p.text.as_str())
            .unwrap_or("");
        assert_eq!(text, "streaming");
    }

    #[test]
    fn render_session_synthesizes_text_part_for_legacy_messages() {
        let snapshot = crate::tui::sync_store::TuiSyncSnapshot::default();
        let legacy = vec![assistant_msg("legacy_1", "legacy content")];
        let render = snapshot.render_session(&legacy);

        assert_eq!(render.messages.len(), 1);
        let text = render.messages[0]
            .text_parts()
            .first()
            .map(|p| p.text.as_str())
            .unwrap_or("");
        assert_eq!(text, "legacy content");
    }
}

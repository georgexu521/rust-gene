//! TUI sync/projection store.
//!
//! This mirrors the opencode-style shape: runtime events update a small
//! frontend store, and renderers consume snapshots instead of re-deriving
//! message/tool/session state from scattered local fields.

use crate::{
    engine::streaming::StreamEvent,
    session_store::SessionProjectionEvent,
    state::{MessageItem, MessageRole},
    tui::{
        app::StreamUsageSnapshot,
        tool_view::{upsert_tool_run, with_tool_run, ToolRunStatus, ToolRunView},
    },
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TuiSessionPhase {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct TuiSyncSnapshot {
    pub phase: TuiSessionPhase,
    pub active_user_message_id: Option<String>,
    pub active_assistant_message_id: Option<String>,
    pub messages: Vec<TuiMessageProjection>,
    pub parts_by_message_id: HashMap<String, Vec<TuiMessagePart>>,
    pub assistant_text: String,
    pub assistant_message_content: String,
    pub assistant_streaming: bool,
    pub thinking_text: String,
    pub thinking_streaming: bool,
    pub tool_runs: Vec<ToolRunView>,
    pub usage: Option<StreamUsageSnapshot>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiMessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiMessageProjection {
    pub id: String,
    pub role: TuiMessageRole,
    pub part_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiPartKind {
    Text,
    Thinking,
    Tool,
}

#[derive(Debug, Clone)]
pub struct TuiMessagePart {
    pub id: String,
    pub message_id: String,
    pub kind: TuiPartKind,
    pub text: String,
    pub tool_run: Option<ToolRunView>,
    pub streaming: bool,
}

impl TuiSyncSnapshot {
    pub fn project_message_items(&self, base_messages: &[MessageItem]) -> Vec<MessageItem> {
        let mut projected = base_messages.to_vec();
        for message in projected
            .iter_mut()
            .filter(|message| message.role == MessageRole::Assistant)
        {
            if let Some(content) = self.rendered_assistant_content_for(&message.id) {
                message.content = content;
            }
        }
        if let Some(assistant_id) = self.active_assistant_message_id.as_deref() {
            if let Some(message) = projected.iter_mut().find(|message| {
                message.id == assistant_id && message.role == MessageRole::Assistant
            }) {
                message.content = self.assistant_message_content.clone();
            } else if !self.assistant_message_content.is_empty() {
                projected.push(MessageItem {
                    id: assistant_id.to_string(),
                    role: MessageRole::Assistant,
                    content: self.assistant_message_content.clone(),
                    timestamp: std::time::SystemTime::now(),
                    metadata: Default::default(),
                });
            }
        }
        projected
    }

    pub fn set_message_text_part(
        &mut self,
        message_id: &str,
        role: TuiMessageRole,
        kind: TuiPartKind,
        text: String,
        streaming: bool,
    ) {
        if kind == TuiPartKind::Tool {
            return;
        }
        self.upsert_message_projection(message_id, role);
        let part_id = part_id_for(message_id, kind);
        let parts = self
            .parts_by_message_id
            .entry(message_id.to_string())
            .or_default();
        if let Some(part) = parts.iter_mut().find(|part| part.id == part_id) {
            part.text = text;
            part.streaming = streaming;
        } else {
            parts.push(TuiMessagePart {
                id: part_id.clone(),
                message_id: message_id.to_string(),
                kind,
                text,
                tool_run: None,
                streaming,
            });
        }
        self.push_message_part_id(message_id, part_id);
        if Some(message_id) == self.active_assistant_message_id.as_deref() {
            self.rebuild_assistant_projection_for(message_id);
        }
    }

    pub fn tool_runs_for_message(&self, message_id: &str) -> Option<Vec<ToolRunView>> {
        let runs = self
            .parts_by_message_id
            .get(message_id)?
            .iter()
            .filter(|part| part.kind == TuiPartKind::Tool)
            .filter_map(|part| part.tool_run.clone())
            .collect::<Vec<_>>();
        (!runs.is_empty()).then_some(runs)
    }

    pub fn all_tool_runs(&self) -> Vec<ToolRunView> {
        let mut runs = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for part in self
            .parts_by_message_id
            .values()
            .flat_map(|parts| parts.iter())
            .filter(|part| part.kind == TuiPartKind::Tool)
        {
            if let Some(run) = part.tool_run.clone() {
                if seen.insert(run.id.clone()) {
                    runs.push(run);
                }
            }
        }
        runs
    }

    pub fn set_tool_runs_for_message(&mut self, message_id: String, runs: Vec<ToolRunView>) {
        self.parts_by_message_id
            .entry(message_id.clone())
            .or_default()
            .retain(|part| part.kind != TuiPartKind::Tool);
        self.remove_stale_part_ids(&message_id);
        for run in runs {
            self.upsert_tool_part_for_message(&message_id, &run.id, &run.name);
            self.replace_tool_part_run(&message_id, run);
        }
        self.rebuild_flat_tool_runs();
    }

    pub fn upsert_tool_run_for_message(&mut self, message_id: String, run: ToolRunView) {
        self.upsert_tool_part_for_message(&message_id, &run.id, &run.name);
        self.replace_tool_part_run(&message_id, run);
        self.rebuild_flat_tool_runs();
    }

    pub fn clear_tool_parts(&mut self) {
        for parts in self.parts_by_message_id.values_mut() {
            parts.retain(|part| part.kind != TuiPartKind::Tool);
        }
        let message_ids = self
            .messages
            .iter()
            .map(|message| message.id.clone())
            .collect::<Vec<_>>();
        for message_id in message_ids {
            self.remove_stale_part_ids(&message_id);
        }
        self.tool_runs.clear();
    }

    fn upsert_tool_part_for_message(&mut self, message_id: &str, tool_id: &str, name: &str) {
        self.upsert_message_projection(message_id, TuiMessageRole::User);
        let part_id = format!("{message_id}:tool:{tool_id}");
        let parts = self
            .parts_by_message_id
            .entry(message_id.to_string())
            .or_default();
        if parts.iter().any(|part| part.id == part_id) {
            return;
        }
        parts.push(TuiMessagePart {
            id: part_id.clone(),
            message_id: message_id.to_string(),
            kind: TuiPartKind::Tool,
            text: name.to_string(),
            tool_run: self.tool_runs.iter().find(|run| run.id == tool_id).cloned(),
            streaming: true,
        });
        self.push_message_part_id(message_id, part_id);
    }

    fn replace_tool_part_run(&mut self, message_id: &str, run: ToolRunView) {
        let Some(parts) = self.parts_by_message_id.get_mut(message_id) else {
            return;
        };
        if let Some(part) = parts
            .iter_mut()
            .find(|part| part.kind == TuiPartKind::Tool && part.id.ends_with(&run.id))
        {
            part.text = run.name.clone();
            part.streaming = run.is_active();
            part.tool_run = Some(run);
        }
    }

    fn sync_tool_part(&mut self, tool_id: &str) {
        let Some(message_id) = self.active_user_message_id.clone() else {
            return;
        };
        let Some(run) = self.tool_runs.iter().find(|run| run.id == tool_id).cloned() else {
            return;
        };
        self.upsert_tool_part_for_message(&message_id, tool_id, &run.name);
        self.replace_tool_part_run(&message_id, run);
    }

    fn rebuild_flat_tool_runs(&mut self) {
        self.tool_runs = self.all_tool_runs();
    }

    fn remove_stale_part_ids(&mut self, message_id: &str) {
        let Some(message) = self
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        else {
            return;
        };
        let valid_parts = self
            .parts_by_message_id
            .get(message_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        message
            .part_ids
            .retain(|part_id| valid_parts.iter().any(|part| part.id == *part_id));
    }

    fn upsert_message_projection(&mut self, message_id: &str, role: TuiMessageRole) {
        if self.messages.iter().any(|message| message.id == message_id) {
            return;
        }
        self.messages.push(TuiMessageProjection {
            id: message_id.to_string(),
            role,
            part_ids: Vec::new(),
        });
    }

    fn push_message_part_id(&mut self, message_id: &str, part_id: String) {
        if let Some(message) = self
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            if !message.part_ids.iter().any(|id| id == &part_id) {
                message.part_ids.push(part_id);
            }
        }
    }

    fn rendered_assistant_content_for(&self, message_id: &str) -> Option<String> {
        let parts = self.parts_by_message_id.get(message_id)?;
        let text = parts
            .iter()
            .find(|part| part.kind == TuiPartKind::Text)
            .map(|part| part.text.clone())
            .unwrap_or_default();
        let thinking = parts
            .iter()
            .find(|part| part.kind == TuiPartKind::Thinking)
            .map(|part| (part.text.clone(), part.streaming))
            .unwrap_or_default();
        (!text.is_empty() || !thinking.0.is_empty())
            .then(|| render_assistant_message_content(&thinking.0, thinking.1, &text))
    }

    fn rebuild_assistant_projection_for(&mut self, message_id: &str) {
        let parts = self
            .parts_by_message_id
            .get(message_id)
            .cloned()
            .unwrap_or_default();
        let text = parts
            .iter()
            .find(|part| part.kind == TuiPartKind::Text)
            .map(|part| part.text.clone())
            .unwrap_or_default();
        let thinking = parts
            .iter()
            .find(|part| part.kind == TuiPartKind::Thinking)
            .map(|part| (part.text.clone(), part.streaming))
            .unwrap_or_default();
        self.assistant_text = text.clone();
        self.thinking_text = thinking.0.clone();
        self.assistant_message_content =
            render_assistant_message_content(&thinking.0, thinking.1, &text);
    }
}

#[derive(Debug, Clone, Default)]
pub struct TuiSyncStore {
    snapshot: TuiSyncSnapshot,
}

impl TuiSyncStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_snapshot(snapshot: TuiSyncSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn snapshot(&self) -> TuiSyncSnapshot {
        self.snapshot.clone()
    }

    pub fn start_turn(&mut self, user_message_id: String, assistant_message_id: String) {
        self.apply_projection_event(&SessionProjectionEvent::TurnStarted {
            user_message_id,
            assistant_message_id,
        });
    }

    pub fn apply_stream_event(&mut self, event: &StreamEvent) {
        let projection_event = SessionProjectionEvent::from_stream_event(
            event,
            self.snapshot.active_user_message_id.as_deref(),
            self.snapshot.active_assistant_message_id.as_deref(),
        );
        self.apply_projection_event(&projection_event);
    }

    pub fn apply_projection_event(&mut self, event: &SessionProjectionEvent) {
        match event {
            SessionProjectionEvent::RunStarted => {
                self.snapshot.phase = TuiSessionPhase::Running;
            }
            SessionProjectionEvent::TurnStarted {
                user_message_id,
                assistant_message_id,
            } => {
                let messages = vec![
                    TuiMessageProjection {
                        id: user_message_id.clone(),
                        role: TuiMessageRole::User,
                        part_ids: Vec::new(),
                    },
                    TuiMessageProjection {
                        id: assistant_message_id.clone(),
                        role: TuiMessageRole::Assistant,
                        part_ids: Vec::new(),
                    },
                ];
                self.snapshot = TuiSyncSnapshot {
                    phase: TuiSessionPhase::Running,
                    active_user_message_id: Some(user_message_id.clone()),
                    active_assistant_message_id: Some(assistant_message_id.clone()),
                    messages,
                    ..TuiSyncSnapshot::default()
                };
            }
            SessionProjectionEvent::AssistantTextDelta { message_id, text } => {
                if !text.is_empty() {
                    let Some(message_id) = self.assistant_message_id(message_id.as_deref()) else {
                        return;
                    };
                    if let Some(part) =
                        self.assistant_part_for_message(&message_id, TuiPartKind::Text)
                    {
                        part.text.push_str(text);
                        part.streaming = true;
                    }
                    self.rebuild_assistant_projection_for(&message_id);
                    self.snapshot.assistant_streaming = true;
                }
            }
            SessionProjectionEvent::AssistantTextUpdated {
                message_id,
                text,
                streaming,
            } => {
                let Some(message_id) = self.assistant_message_id(message_id.as_deref()) else {
                    return;
                };
                self.snapshot.set_message_text_part(
                    &message_id,
                    TuiMessageRole::Assistant,
                    TuiPartKind::Text,
                    text.clone(),
                    *streaming,
                );
                self.snapshot.assistant_streaming = *streaming;
            }
            SessionProjectionEvent::ThinkingStarted { message_id } => {
                let Some(message_id) = self.assistant_message_id(message_id.as_deref()) else {
                    return;
                };
                if let Some(part) =
                    self.assistant_part_for_message(&message_id, TuiPartKind::Thinking)
                {
                    part.streaming = true;
                }
                self.snapshot.thinking_streaming = true;
                self.rebuild_assistant_projection_for(&message_id);
            }
            SessionProjectionEvent::ThinkingDelta { message_id, text } => {
                let Some(message_id) = self.assistant_message_id(message_id.as_deref()) else {
                    return;
                };
                if let Some(part) =
                    self.assistant_part_for_message(&message_id, TuiPartKind::Thinking)
                {
                    part.text.push_str(text);
                    part.streaming = true;
                }
                self.rebuild_assistant_projection_for(&message_id);
                self.snapshot.thinking_streaming = true;
            }
            SessionProjectionEvent::ThinkingCompleted { message_id } => {
                let Some(message_id) = self.assistant_message_id(message_id.as_deref()) else {
                    return;
                };
                if let Some(part) =
                    self.assistant_part_for_message(&message_id, TuiPartKind::Thinking)
                {
                    part.streaming = false;
                }
                self.snapshot.thinking_streaming = false;
                self.rebuild_assistant_projection_for(&message_id);
            }
            SessionProjectionEvent::ThinkingUpdated {
                message_id,
                text,
                streaming,
            } => {
                let Some(message_id) = self.assistant_message_id(message_id.as_deref()) else {
                    return;
                };
                self.snapshot.set_message_text_part(
                    &message_id,
                    TuiMessageRole::Assistant,
                    TuiPartKind::Thinking,
                    text.clone(),
                    *streaming,
                );
                self.snapshot.thinking_streaming = *streaming;
            }
            SessionProjectionEvent::ToolCallStarted {
                message_id,
                tool_call_id,
                tool_name,
            } => {
                self.snapshot.assistant_streaming = false;
                upsert_tool_run(
                    &mut self.snapshot.tool_runs,
                    tool_call_id.clone(),
                    tool_name.clone(),
                );
                self.upsert_tool_part_for_message(message_id.as_deref(), tool_call_id, tool_name);
            }
            SessionProjectionEvent::ToolArgumentsDelta {
                tool_call_id,
                arguments_delta,
            } => {
                with_tool_run(&mut self.snapshot.tool_runs, tool_call_id, |run| {
                    run.push_args_delta(arguments_delta)
                });
                self.snapshot.sync_tool_part(tool_call_id);
            }
            SessionProjectionEvent::ToolCallAccepted { tool_call_id } => {
                self.snapshot.sync_tool_part(tool_call_id);
            }
            SessionProjectionEvent::ToolExecutionStarted {
                tool_call_id,
                tool_name,
                ..
            } => {
                with_tool_run(&mut self.snapshot.tool_runs, tool_call_id, |run| {
                    run.mark_running(tool_name.clone())
                });
                self.snapshot.sync_tool_part(tool_call_id);
            }
            SessionProjectionEvent::ToolExecutionProgress {
                tool_call_id,
                progress,
            } => {
                with_tool_run(&mut self.snapshot.tool_runs, tool_call_id, |run| {
                    run.push_progress(progress.clone())
                });
                self.snapshot.sync_tool_part(tool_call_id);
            }
            SessionProjectionEvent::ToolExecutionCompleted {
                tool_call_id,
                result,
                metadata,
                result_data,
            } => {
                with_tool_run(&mut self.snapshot.tool_runs, tool_call_id, |run| {
                    run.mark_complete_with_metadata(result.clone(), metadata.clone());
                    if let Some(data) = result_data.clone() {
                        run.result_data = Some(data);
                    }
                });
                self.snapshot.sync_tool_part(tool_call_id);
            }
            SessionProjectionEvent::ToolPartUpdated {
                message_id,
                tool_call_id,
                tool_name,
                status,
                input_args,
                result,
                metadata,
                result_data,
            } => {
                self.upsert_tool_run_snapshot(
                    message_id.as_deref(),
                    tool_call_id,
                    tool_name,
                    status.as_deref(),
                    input_args.as_deref(),
                    result.as_deref(),
                    metadata.clone(),
                    result_data.clone(),
                );
            }
            SessionProjectionEvent::PermissionRequested {
                message_id,
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                upsert_tool_run(
                    &mut self.snapshot.tool_runs,
                    tool_call_id.clone(),
                    tool_name.clone(),
                );
                with_tool_run(&mut self.snapshot.tool_runs, tool_call_id, |run| {
                    run.mark_waiting_permission(tool_name.clone(), arguments.clone())
                });
                self.upsert_tool_part_for_message(message_id.as_deref(), tool_call_id, tool_name);
                self.snapshot.sync_tool_part(tool_call_id);
            }
            SessionProjectionEvent::Usage {
                prompt_tokens,
                completion_tokens,
                reasoning_tokens,
                cached_tokens,
            } => {
                self.snapshot.usage = Some(StreamUsageSnapshot {
                    prompt_tokens: *prompt_tokens,
                    completion_tokens: *completion_tokens,
                    reasoning_tokens: *reasoning_tokens,
                    cached_tokens: *cached_tokens,
                });
            }
            SessionProjectionEvent::RuntimeDiagnostic { .. }
            | SessionProjectionEvent::Closeout { .. }
            | SessionProjectionEvent::ToolResultsReadyForModel { .. }
            | SessionProjectionEvent::OutputTruncated => {}
            SessionProjectionEvent::Completed => {
                self.snapshot.phase = TuiSessionPhase::Completed;
                self.snapshot.assistant_streaming = false;
                self.snapshot.thinking_streaming = false;
                self.mark_active_parts_not_streaming();
                self.rebuild_active_assistant_projection();
            }
            SessionProjectionEvent::Error { message } => {
                self.snapshot.phase = TuiSessionPhase::Failed;
                self.snapshot.assistant_streaming = false;
                self.snapshot.thinking_streaming = false;
                self.mark_active_parts_not_streaming();
                self.snapshot.last_error = Some(message.clone());
                if !self.snapshot.assistant_message_content.is_empty() {
                    self.snapshot.assistant_message_content.push('\n');
                }
                self.snapshot
                    .assistant_message_content
                    .push_str(&format!("[Error: {message}]"));
            }
        }
    }

    pub fn mark_completed(&mut self) {
        self.snapshot.phase = TuiSessionPhase::Completed;
        self.snapshot.assistant_streaming = false;
        self.snapshot.thinking_streaming = false;
        self.mark_active_parts_not_streaming();
        self.rebuild_active_assistant_projection();
    }

    pub fn mark_stream_closed(&mut self) {
        if self.snapshot.phase == TuiSessionPhase::Running {
            self.mark_completed();
        }
    }

    pub fn mark_failed(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.snapshot.phase = TuiSessionPhase::Failed;
        self.snapshot.assistant_streaming = false;
        self.snapshot.thinking_streaming = false;
        self.mark_active_parts_not_streaming();
        self.snapshot.last_error = Some(message);
    }

    pub fn mark_active_tools_with_result(&mut self, result: String) {
        for run in self
            .snapshot
            .tool_runs
            .iter_mut()
            .filter(|run| run.is_active())
        {
            run.mark_complete(result.clone());
        }
        let ids = self
            .snapshot
            .tool_runs
            .iter()
            .map(|run| run.id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            self.snapshot.sync_tool_part(&id);
        }
    }

    fn assistant_message_id(&self, message_id: Option<&str>) -> Option<String> {
        message_id
            .map(str::to_string)
            .or_else(|| self.snapshot.active_assistant_message_id.clone())
    }

    fn assistant_part_for_message(
        &mut self,
        message_id: &str,
        kind: TuiPartKind,
    ) -> Option<&mut TuiMessagePart> {
        self.snapshot
            .upsert_message_projection(message_id, TuiMessageRole::Assistant);
        let part_id = part_id_for(message_id, kind);
        let parts = self
            .snapshot
            .parts_by_message_id
            .entry(message_id.to_string())
            .or_default();
        if let Some(index) = parts.iter().position(|part| part.kind == kind) {
            return parts.get_mut(index);
        }
        parts.push(TuiMessagePart {
            id: part_id.clone(),
            message_id: message_id.to_string(),
            kind,
            text: String::new(),
            tool_run: None,
            streaming: false,
        });
        if let Some(message) = self
            .snapshot
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            if !message.part_ids.iter().any(|id| id == &part_id) {
                message.part_ids.push(part_id);
            }
        }
        parts.last_mut()
    }

    fn upsert_tool_part_for_message(
        &mut self,
        message_id: Option<&str>,
        tool_id: &str,
        name: &str,
    ) {
        let Some(message_id) = message_id
            .map(str::to_string)
            .or_else(|| self.snapshot.active_user_message_id.clone())
        else {
            return;
        };
        self.snapshot
            .upsert_tool_part_for_message(&message_id, tool_id, name);
        self.snapshot.sync_tool_part(tool_id);
    }

    #[allow(clippy::too_many_arguments)]
    fn upsert_tool_run_snapshot(
        &mut self,
        message_id: Option<&str>,
        tool_id: &str,
        tool_name: &str,
        status: Option<&str>,
        input_args: Option<&str>,
        result: Option<&str>,
        metadata: Option<serde_json::Value>,
        result_data: Option<serde_json::Value>,
    ) {
        upsert_tool_run(
            &mut self.snapshot.tool_runs,
            tool_id.to_string(),
            tool_name.to_string(),
        );
        with_tool_run(&mut self.snapshot.tool_runs, tool_id, |run| {
            if let Some(input_args) = input_args {
                run.args_buffer = input_args.to_string();
                run.arguments = serde_json::from_str(input_args).ok();
            }
            match status {
                Some("failed") => {
                    run.mark_complete_with_metadata(
                        result.unwrap_or_default().to_string(),
                        metadata.or_else(|| Some(serde_json::json!({"success": false}))),
                    );
                    run.status = ToolRunStatus::Failed;
                }
                Some("timed_out") => {
                    run.mark_complete_with_metadata(
                        result.unwrap_or_default().to_string(),
                        metadata.or_else(|| Some(serde_json::json!({"status": "timed_out"}))),
                    );
                    run.status = ToolRunStatus::TimedOut;
                }
                Some("cancelled") => {
                    run.mark_complete_with_metadata(
                        result.unwrap_or_default().to_string(),
                        metadata.or_else(|| Some(serde_json::json!({"status": "cancelled"}))),
                    );
                    run.status = ToolRunStatus::Cancelled;
                }
                Some("completed") => {
                    run.mark_complete_with_metadata(
                        result.unwrap_or_default().to_string(),
                        metadata.or_else(|| Some(serde_json::json!({"success": true}))),
                    );
                }
                Some("running") => run.mark_running(tool_name.to_string()),
                _ if result.is_some() => run
                    .mark_complete_with_metadata(result.unwrap_or_default().to_string(), metadata),
                _ => {}
            }
            run.result_data = result_data;
        });
        self.upsert_tool_part_for_message(message_id, tool_id, tool_name);
    }

    fn mark_active_parts_not_streaming(&mut self) {
        let Some(message_id) = self.snapshot.active_assistant_message_id.clone() else {
            return;
        };
        if let Some(parts) = self.snapshot.parts_by_message_id.get_mut(&message_id) {
            for part in parts {
                part.streaming = false;
            }
        }
    }

    fn rebuild_active_assistant_projection(&mut self) {
        let Some(message_id) = self.snapshot.active_assistant_message_id.clone() else {
            return;
        };
        self.rebuild_assistant_projection_for(&message_id);
    }

    fn rebuild_assistant_projection_for(&mut self, message_id: &str) {
        self.snapshot.rebuild_assistant_projection_for(message_id);
    }
}

fn part_id_for(message_id: &str, kind: TuiPartKind) -> String {
    let suffix = match kind {
        TuiPartKind::Text => "text",
        TuiPartKind::Thinking => "thinking",
        TuiPartKind::Tool => "tool",
    };
    format!("{message_id}:{suffix}")
}

fn render_assistant_message_content(
    thinking: &str,
    thinking_streaming: bool,
    text: &str,
) -> String {
    if thinking.trim().is_empty() {
        return text.to_string();
    }
    let mut rendered = String::new();
    rendered.push_str("<think>");
    rendered.push_str(thinking);
    if !thinking_streaming || !text.is_empty() {
        rendered.push_str("</think>");
    }
    if !text.is_empty() {
        rendered.push_str("\n\n");
        rendered.push_str(text);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_store_projects_text_tool_and_completion_state() {
        let mut store = TuiSyncStore::new();
        store.start_turn("user_1".to_string(), "assistant_1".to_string());

        store.apply_stream_event(&StreamEvent::TextChunk("hello".to_string()));
        store.apply_stream_event(&StreamEvent::ToolCallStart {
            id: "call_1".to_string(),
            name: "bash".to_string(),
        });
        store.apply_stream_event(&StreamEvent::ToolCallArgs {
            id: "call_1".to_string(),
            args_delta: "{\"command\":\"pwd\"}".to_string(),
        });
        store.apply_stream_event(&StreamEvent::ToolExecutionStart {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            metadata: None,
        });
        store.apply_stream_event(&StreamEvent::ToolExecutionComplete {
            id: "call_1".to_string(),
            result: "Result: OK\n/Users/georgexu/Desktop/rust-agent".to_string(),
            metadata: None,
            result_data: None,
        });
        store.apply_stream_event(&StreamEvent::Complete);

        let snapshot = store.snapshot();
        assert_eq!(snapshot.phase, TuiSessionPhase::Completed);
        assert_eq!(snapshot.assistant_text, "hello");
        assert_eq!(snapshot.assistant_message_content, "hello");
        assert!(!snapshot.assistant_streaming);
        assert_eq!(snapshot.messages.len(), 2);
        assert_eq!(snapshot.messages[0].id, "user_1");
        assert_eq!(snapshot.messages[1].id, "assistant_1");
        assert_eq!(
            snapshot
                .parts_by_message_id
                .get("assistant_1")
                .expect("assistant parts")[0]
                .kind,
            TuiPartKind::Text
        );
        assert_eq!(snapshot.tool_runs.len(), 1);
        assert_eq!(
            snapshot
                .tool_runs_for_message("user_1")
                .expect("anchored tool runs")
                .len(),
            1
        );
    }

    #[test]
    fn sync_store_consumes_projection_events_directly() {
        let mut store = TuiSyncStore::new();
        store.apply_projection_event(&SessionProjectionEvent::TurnStarted {
            user_message_id: "user_1".to_string(),
            assistant_message_id: "assistant_1".to_string(),
        });
        store.apply_projection_event(&SessionProjectionEvent::AssistantTextDelta {
            message_id: Some("assistant_1".to_string()),
            text: "working".to_string(),
        });
        store.apply_projection_event(&SessionProjectionEvent::ToolCallStarted {
            message_id: Some("user_1".to_string()),
            tool_call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
        });
        store.apply_projection_event(&SessionProjectionEvent::ToolArgumentsDelta {
            tool_call_id: "call_1".to_string(),
            arguments_delta: "{\"command\":\"pwd\"}".to_string(),
        });
        store.apply_projection_event(&SessionProjectionEvent::ToolExecutionCompleted {
            tool_call_id: "call_1".to_string(),
            result: "Result: OK\n/tmp/project".to_string(),
            metadata: None,
            result_data: None,
        });
        store.apply_projection_event(&SessionProjectionEvent::Completed);

        let snapshot = store.snapshot();
        assert_eq!(snapshot.phase, TuiSessionPhase::Completed);
        assert_eq!(snapshot.assistant_text, "working");
        assert_eq!(
            snapshot
                .tool_runs_for_message("user_1")
                .expect("tool part anchored to user message")[0]
                .result_body
                .as_deref(),
            Some("/tmp/project")
        );
    }

    #[test]
    fn sync_store_tracks_thinking_as_part_state() {
        let mut store = TuiSyncStore::new();
        store.start_turn("user_1".to_string(), "assistant_1".to_string());

        store.apply_stream_event(&StreamEvent::ThinkingStart);
        store.apply_stream_event(&StreamEvent::ThinkingChunk("plan".to_string()));
        assert!(store.snapshot().thinking_streaming);

        store.apply_stream_event(&StreamEvent::ThinkingComplete);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.thinking_text, "plan");
        assert_eq!(snapshot.assistant_message_content, "<think>plan</think>");
        assert!(!snapshot.thinking_streaming);
        assert_eq!(
            snapshot
                .parts_by_message_id
                .get("assistant_1")
                .expect("assistant parts")[0]
                .kind,
            TuiPartKind::Thinking
        );
    }

    #[test]
    fn sync_store_projects_thinking_and_answer_parts_into_message_content() {
        let mut store = TuiSyncStore::new();
        store.start_turn("user_1".to_string(), "assistant_1".to_string());

        store.apply_stream_event(&StreamEvent::ThinkingStart);
        store.apply_stream_event(&StreamEvent::ThinkingChunk("inspect files".to_string()));
        assert_eq!(
            store.snapshot().assistant_message_content,
            "<think>inspect files"
        );

        store.apply_stream_event(&StreamEvent::ThinkingComplete);
        store.apply_stream_event(&StreamEvent::TextChunk("It is a Rust project.".to_string()));

        let snapshot = store.snapshot();
        assert_eq!(
            snapshot.assistant_message_content,
            "<think>inspect files</think>\n\nIt is a Rust project."
        );
    }
}

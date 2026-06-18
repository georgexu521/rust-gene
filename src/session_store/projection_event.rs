//! Session projection events.
//!
//! This is the adapter boundary between low-level runtime stream events and
//! frontend/session message-part projectors. It intentionally stays close to
//! the existing runtime vocabulary while giving TUI and future sync clients a
//! single message/part-oriented contract to consume.

use crate::engine::streaming::StreamEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct SessionProjectionEnvelope {
    pub id: String,
    pub seq: u64,
    pub aggregate_id: String,
    pub event_type: &'static str,
    pub event: SessionProjectionEvent,
}

#[derive(Debug, Clone, Default)]
pub struct SessionProjectionEventBus {
    next_seq: u64,
    events: Vec<SessionProjectionEnvelope>,
}

impl SessionProjectionEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_seq(seq: u64) -> Self {
        Self {
            next_seq: seq,
            events: Vec::new(),
        }
    }

    pub fn publish(&mut self, event: SessionProjectionEvent) -> SessionProjectionEnvelope {
        self.next_seq = self.next_seq.saturating_add(1);
        let seq = self.next_seq;
        let event_type = event.event_type();
        let aggregate_id = event.aggregate_id();
        let id = format!("session-projection:{seq}:{aggregate_id}:{event_type}");
        let envelope = SessionProjectionEnvelope {
            id,
            seq,
            aggregate_id,
            event_type,
            event,
        };
        self.events.push(envelope.clone());
        envelope
    }

    pub fn last_seq(&self) -> u64 {
        self.next_seq
    }

    pub fn events(&self) -> &[SessionProjectionEnvelope] {
        &self.events
    }

    pub fn drain_after(&self, after_seq: u64) -> Vec<SessionProjectionEnvelope> {
        self.events
            .iter()
            .filter(|event| event.seq > after_seq)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionProjectionEvent {
    RunStarted,
    TurnStarted {
        user_message_id: String,
        assistant_message_id: String,
    },
    AssistantTextDelta {
        message_id: Option<String>,
        text: String,
    },
    AssistantTextUpdated {
        message_id: Option<String>,
        text: String,
        streaming: bool,
    },
    ThinkingStarted {
        message_id: Option<String>,
    },
    ThinkingDelta {
        message_id: Option<String>,
        text: String,
    },
    ThinkingCompleted {
        message_id: Option<String>,
    },
    ThinkingUpdated {
        message_id: Option<String>,
        text: String,
        streaming: bool,
    },
    ToolCallStarted {
        message_id: Option<String>,
        tool_call_id: String,
        tool_name: String,
    },
    ToolArgumentsDelta {
        tool_call_id: String,
        arguments_delta: String,
    },
    ToolCallAccepted {
        tool_call_id: String,
    },
    ToolExecutionStarted {
        message_id: Option<String>,
        tool_call_id: String,
        tool_name: String,
        metadata: Option<serde_json::Value>,
    },
    ToolExecutionProgress {
        tool_call_id: String,
        progress: String,
    },
    ToolExecutionCompleted {
        tool_call_id: String,
        result: String,
        metadata: Option<serde_json::Value>,
        result_data: Option<serde_json::Value>,
    },
    ToolPartUpdated {
        message_id: Option<String>,
        tool_call_id: String,
        tool_name: String,
        status: Option<String>,
        input_args: Option<String>,
        result: Option<String>,
        metadata: Option<serde_json::Value>,
        result_data: Option<serde_json::Value>,
    },
    ToolResultsReadyForModel {
        tool_call_ids: Vec<String>,
    },
    PermissionRequested {
        message_id: Option<String>,
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
        prompt: String,
        metadata: Option<serde_json::Value>,
    },
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        reasoning_tokens: Option<u32>,
        cached_tokens: Option<u32>,
        cache_write_tokens: Option<u32>,
    },
    RuntimeDiagnostic {
        diagnostic: serde_json::Value,
    },
    Closeout {
        status: String,
        evidence_summary: Option<String>,
    },
    Completed,
    OutputTruncated,
    Error {
        message: String,
    },
}

impl SessionProjectionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::RunStarted => "run_started",
            Self::TurnStarted { .. } => "turn_started",
            Self::AssistantTextDelta { .. } => "assistant_text_delta",
            Self::AssistantTextUpdated { .. } => "assistant_text_updated",
            Self::ThinkingStarted { .. } => "thinking_started",
            Self::ThinkingDelta { .. } => "thinking_delta",
            Self::ThinkingCompleted { .. } => "thinking_completed",
            Self::ThinkingUpdated { .. } => "thinking_updated",
            Self::ToolCallStarted { .. } => "tool_call_started",
            Self::ToolArgumentsDelta { .. } => "tool_arguments_delta",
            Self::ToolCallAccepted { .. } => "tool_call_accepted",
            Self::ToolExecutionStarted { .. } => "tool_execution_started",
            Self::ToolExecutionProgress { .. } => "tool_execution_progress",
            Self::ToolExecutionCompleted { .. } => "tool_execution_completed",
            Self::ToolPartUpdated { .. } => "tool_part_updated",
            Self::ToolResultsReadyForModel { .. } => "tool_results_ready_for_model",
            Self::PermissionRequested { .. } => "permission_requested",
            Self::Usage { .. } => "usage",
            Self::RuntimeDiagnostic { .. } => "runtime_diagnostic",
            Self::Closeout { .. } => "closeout",
            Self::Completed => "completed",
            Self::OutputTruncated => "output_truncated",
            Self::Error { .. } => "error",
        }
    }

    pub fn aggregate_id(&self) -> String {
        match self {
            Self::TurnStarted {
                assistant_message_id,
                ..
            } => assistant_message_id.clone(),
            Self::AssistantTextDelta { message_id, .. }
            | Self::AssistantTextUpdated { message_id, .. }
            | Self::ThinkingStarted { message_id }
            | Self::ThinkingDelta { message_id, .. }
            | Self::ThinkingCompleted { message_id }
            | Self::ThinkingUpdated { message_id, .. } => message_id
                .clone()
                .unwrap_or_else(|| "assistant".to_string()),
            Self::ToolCallStarted { tool_call_id, .. }
            | Self::ToolArgumentsDelta { tool_call_id, .. }
            | Self::ToolCallAccepted { tool_call_id }
            | Self::ToolExecutionStarted { tool_call_id, .. }
            | Self::ToolExecutionProgress { tool_call_id, .. }
            | Self::ToolExecutionCompleted { tool_call_id, .. }
            | Self::ToolPartUpdated { tool_call_id, .. }
            | Self::PermissionRequested { tool_call_id, .. } => tool_call_id.clone(),
            Self::ToolResultsReadyForModel { tool_call_ids } => tool_call_ids
                .first()
                .cloned()
                .unwrap_or_else(|| "tools".to_string()),
            Self::RuntimeDiagnostic { .. }
            | Self::Usage { .. }
            | Self::Closeout { .. }
            | Self::RunStarted
            | Self::Completed
            | Self::OutputTruncated
            | Self::Error { .. } => "runtime".to_string(),
        }
    }

    pub fn from_stream_event(
        event: &StreamEvent,
        active_user_message_id: Option<&str>,
        active_assistant_message_id: Option<&str>,
    ) -> Self {
        match event {
            StreamEvent::Start => Self::RunStarted,
            StreamEvent::TextChunk(text) => Self::AssistantTextDelta {
                message_id: active_assistant_message_id.map(str::to_string),
                text: text.clone(),
            },
            StreamEvent::ThinkingStart => Self::ThinkingStarted {
                message_id: active_assistant_message_id.map(str::to_string),
            },
            StreamEvent::ThinkingChunk(text) => Self::ThinkingDelta {
                message_id: active_assistant_message_id.map(str::to_string),
                text: text.clone(),
            },
            StreamEvent::ThinkingComplete => Self::ThinkingCompleted {
                message_id: active_assistant_message_id.map(str::to_string),
            },
            StreamEvent::ToolCallStart { id, name } => Self::ToolCallStarted {
                message_id: active_user_message_id.map(str::to_string),
                tool_call_id: id.clone(),
                tool_name: name.clone(),
            },
            StreamEvent::ToolCallArgs { id, args_delta } => Self::ToolArgumentsDelta {
                tool_call_id: id.clone(),
                arguments_delta: args_delta.clone(),
            },
            StreamEvent::ToolCallComplete { id } => Self::ToolCallAccepted {
                tool_call_id: id.clone(),
            },
            StreamEvent::ToolExecutionStart { id, name, metadata } => Self::ToolExecutionStarted {
                message_id: active_user_message_id.map(str::to_string),
                tool_call_id: id.clone(),
                tool_name: name.clone(),
                metadata: metadata.clone(),
            },
            StreamEvent::ToolExecutionProgress { id, progress } => Self::ToolExecutionProgress {
                tool_call_id: id.clone(),
                progress: progress.clone(),
            },
            StreamEvent::ToolExecutionComplete {
                id,
                result,
                metadata,
                result_data,
            } => Self::ToolExecutionCompleted {
                tool_call_id: id.clone(),
                result: result.clone(),
                metadata: metadata.clone(),
                result_data: result_data.clone(),
            },
            StreamEvent::ToolResultsReadyForModel { ids } => Self::ToolResultsReadyForModel {
                tool_call_ids: ids.clone(),
            },
            StreamEvent::PermissionRequest {
                id,
                tool_name,
                arguments,
                prompt,
                metadata,
                ..
            } => Self::PermissionRequested {
                message_id: active_user_message_id.map(str::to_string),
                tool_call_id: id.clone(),
                tool_name: tool_name.clone(),
                arguments: arguments.clone(),
                prompt: prompt.clone(),
                metadata: metadata.clone(),
            },
            StreamEvent::Usage {
                prompt_tokens,
                completion_tokens,
                reasoning_tokens,
                cached_tokens,
                cache_write_tokens,
            } => Self::Usage {
                prompt_tokens: *prompt_tokens,
                completion_tokens: *completion_tokens,
                reasoning_tokens: *reasoning_tokens,
                cached_tokens: *cached_tokens,
                cache_write_tokens: *cache_write_tokens,
            },
            StreamEvent::RuntimeDiagnostic { diagnostic } => Self::RuntimeDiagnostic {
                diagnostic: diagnostic.clone(),
            },
            StreamEvent::Closeout {
                status,
                evidence_summary,
            } => Self::Closeout {
                status: status.clone(),
                evidence_summary: evidence_summary.clone(),
            },
            StreamEvent::Complete => Self::Completed,
            StreamEvent::OutputTruncated => Self::OutputTruncated,
            StreamEvent::Error(message) => Self::Error {
                message: message.clone(),
            },
        }
    }

    pub fn from_persisted_part(
        part: &crate::session_store::PersistedSessionPart,
        message_id: Option<String>,
    ) -> Option<Self> {
        match part.kind.as_str() {
            "assistant_text" => {
                let text = part.payload["content"].as_str()?.to_string();
                Some(Self::AssistantTextUpdated {
                    message_id: message_id.or_else(|| part.message_id.clone()),
                    text,
                    streaming: false,
                })
            }
            "reasoning" => {
                let text = part.payload["content"].as_str()?.to_string();
                Some(Self::ThinkingUpdated {
                    message_id: message_id.or_else(|| part.message_id.clone()),
                    text,
                    streaming: false,
                })
            }
            "tool" | "shell" => {
                let tool_call_id = part
                    .tool_call_id
                    .clone()
                    .filter(|id| !id.trim().is_empty())
                    .unwrap_or_else(|| part.part_id.clone());
                let tool_name = part
                    .tool_name
                    .clone()
                    .or_else(|| part.payload["tool_name"].as_str().map(str::to_string))
                    .unwrap_or_else(|| {
                        if part.kind == "shell" {
                            "bash".to_string()
                        } else {
                            "tool".to_string()
                        }
                    });
                let input_args = part.payload["input_args"].as_str().map(str::to_string);
                let result = part.payload["result_preview"]
                    .as_str()
                    .or_else(|| part.payload["error"].as_str())
                    .or_else(|| part.payload["output_uri"].as_str())
                    .or_else(|| part.payload["input_args"].as_str())
                    .map(str::to_string);
                Some(Self::ToolPartUpdated {
                    message_id: message_id.or_else(|| part.message_id.clone()),
                    tool_call_id,
                    tool_name: tool_name.clone(),
                    status: part.status.clone(),
                    input_args,
                    result,
                    metadata: Some(serde_json::json!({
                        "tool": tool_name,
                        "success": !matches!(
                            part.status.as_deref(),
                            Some("failed" | "timed_out" | "cancelled")
                        ),
                        "status": part.status.clone().unwrap_or_else(|| "unknown".to_string()),
                        "output_uri": part.payload["output_uri"].as_str(),
                        "error_preview": part.payload["error"].as_str(),
                        "persisted_session_part_id": part.id,
                        "projected_to_seq": part.projected_to_seq,
                        "replay_source": "session_parts",
                    })),
                    result_data: Some(part.payload.clone()),
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_stream_events_to_message_part_projection_events() {
        let event = StreamEvent::ToolExecutionComplete {
            id: "call_1".to_string(),
            result: "Result: OK".to_string(),
            metadata: Some(serde_json::json!({"success": true})),
            result_data: Some(serde_json::json!({"kind": "shell"})),
        };

        assert_eq!(
            SessionProjectionEvent::from_stream_event(&event, Some("user_1"), Some("assistant_1")),
            SessionProjectionEvent::ToolExecutionCompleted {
                tool_call_id: "call_1".to_string(),
                result: "Result: OK".to_string(),
                metadata: Some(serde_json::json!({"success": true})),
                result_data: Some(serde_json::json!({"kind": "shell"})),
            }
        );

        let text = StreamEvent::TextChunk("hello".to_string());
        assert_eq!(
            SessionProjectionEvent::from_stream_event(&text, Some("user_1"), Some("assistant_1")),
            SessionProjectionEvent::AssistantTextDelta {
                message_id: Some("assistant_1".to_string()),
                text: "hello".to_string(),
            }
        );
    }

    #[test]
    fn projection_bus_assigns_ordered_envelopes() {
        let mut bus = SessionProjectionEventBus::from_seq(41);
        let envelope = bus.publish(SessionProjectionEvent::AssistantTextDelta {
            message_id: Some("assistant_1".to_string()),
            text: "hello".to_string(),
        });

        assert_eq!(envelope.seq, 42);
        assert_eq!(envelope.aggregate_id, "assistant_1");
        assert_eq!(envelope.event_type, "assistant_text_delta");
        assert_eq!(
            envelope.id,
            "session-projection:42:assistant_1:assistant_text_delta"
        );
        assert_eq!(bus.last_seq(), 42);
        assert_eq!(bus.drain_after(41), vec![envelope]);
    }

    #[test]
    fn maps_persisted_parts_to_snapshot_projection_events() {
        let part = crate::session_store::PersistedSessionPart {
            id: 1,
            session_id: "session_1".to_string(),
            part_index: 0,
            part_id: "tool_1".to_string(),
            kind: "tool".to_string(),
            tool_call_id: Some("call_1".to_string()),
            tool_name: Some("bash".to_string()),
            status: Some("completed".to_string()),
            payload: serde_json::json!({
                "input_args": "{\"command\":\"pwd\"}",
                "result_preview": "/tmp/project",
            }),
            projected_to_seq: 1,
            updated_at: "now".to_string(),
            message_id: Some("assistant_1".to_string()),
        };

        assert_eq!(
            SessionProjectionEvent::from_persisted_part(&part, Some("user_1".to_string())),
            Some(SessionProjectionEvent::ToolPartUpdated {
                message_id: Some("user_1".to_string()),
                tool_call_id: "call_1".to_string(),
                tool_name: "bash".to_string(),
                status: Some("completed".to_string()),
                input_args: Some("{\"command\":\"pwd\"}".to_string()),
                result: Some("/tmp/project".to_string()),
                metadata: Some(serde_json::json!({
                    "tool": "bash",
                    "success": true,
                    "status": "completed",
                    "output_uri": null,
                    "error_preview": null,
                    "persisted_session_part_id": 1,
                    "projected_to_seq": 1,
                    "replay_source": "session_parts",
                })),
                result_data: Some(serde_json::json!({
                    "input_args": "{\"command\":\"pwd\"}",
                    "result_preview": "/tmp/project",
                })),
            })
        );
    }
}

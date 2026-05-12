//! OpenAI API 兼容层
//!
//! 将内部类型转换为 async-openai 类型，供 Kimi/OpenAI 等兼容 API 使用

use crate::services::api::{
    provider_protocol::{normalize_messages_for_provider, ProviderProtocolFamily},
    sanitize_assistant_content, ChatRequest, ChatResponse, Message, ToolCall, ToolChoice, Usage,
};
use anyhow::{Context, Result};
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionNamedToolChoice,
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage, ChatCompletionTool,
    ChatCompletionToolChoiceOption, ChatCompletionToolType, CreateChatCompletionRequest,
    CreateChatCompletionResponse, FunctionCall, FunctionName, FunctionObject,
};

pub fn convert_request(request: ChatRequest, model: &str) -> CreateChatCompletionRequest {
    let messages: Vec<ChatCompletionRequestMessage> =
        normalize_messages_for_provider(ProviderProtocolFamily::OpenAiCompatible, request.messages)
            .into_iter()
            .map(convert_message)
            .collect();

    let mut req = CreateChatCompletionRequest {
        model: if request.model.is_empty() {
            model.to_string()
        } else {
            request.model
        },
        messages,
        temperature: request.temperature,
        max_completion_tokens: request.max_tokens,
        tools: None,
        tool_choice: request.tool_choice.map(convert_tool_choice),
        ..Default::default()
    };

    if let Some(tools) = request.tools {
        req.tools = Some(
            tools
                .into_iter()
                .map(|t| ChatCompletionTool {
                    r#type: ChatCompletionToolType::Function,
                    function: FunctionObject {
                        name: t.name,
                        description: Some(t.description),
                        parameters: Some(t.parameters),
                        strict: None,
                    },
                })
                .collect(),
        );
    }

    req
}

pub(crate) fn convert_tool_choice(choice: ToolChoice) -> ChatCompletionToolChoiceOption {
    match choice {
        ToolChoice::None => ChatCompletionToolChoiceOption::None,
        ToolChoice::Auto => ChatCompletionToolChoiceOption::Auto,
        ToolChoice::Required => ChatCompletionToolChoiceOption::Required,
        ToolChoice::Function(name) => {
            ChatCompletionToolChoiceOption::Named(ChatCompletionNamedToolChoice {
                r#type: ChatCompletionToolType::Function,
                function: FunctionName { name },
            })
        }
    }
}

pub fn convert_response(response: CreateChatCompletionResponse) -> Result<ChatResponse> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .context("No choices in response")?;
    let message = choice.message;

    let tool_calls: Option<Vec<ToolCall>> = message.tool_calls.map(|calls| {
        calls
            .into_iter()
            .map(|call| ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: serde_json::from_str(&call.function.arguments).unwrap_or_else(|e| {
                    tracing::warn!("Failed to parse tool arguments: {}", e);
                    serde_json::Value::Null
                }),
            })
            .collect()
    });

    let usage = response.usage.map(|u| Usage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
        reasoning_tokens: u
            .completion_tokens_details
            .as_ref()
            .and_then(|d| d.reasoning_tokens),
        cached_tokens: u
            .prompt_tokens_details
            .as_ref()
            .and_then(|d| d.cached_tokens),
    });

    Ok(ChatResponse {
        content: sanitize_assistant_content(message.content.unwrap_or_default()),
        tool_calls,
        usage,
    })
}

pub fn convert_message(msg: Message) -> ChatCompletionRequestMessage {
    match msg {
        Message::System { content } => ChatCompletionRequestSystemMessage::from(content).into(),
        Message::User { content } => ChatCompletionRequestUserMessage::from(content).into(),
        Message::Assistant {
            content,
            tool_calls,
        } => {
            let has_tool_calls = tool_calls.as_ref().is_some_and(|calls| !calls.is_empty());
            let content = if has_tool_calls && content.trim().is_empty() {
                // Strict OpenAI-compatible providers can reject a tool-result
                // turn if the preceding assistant tool-call message carries
                // an empty text content field. When the assistant is only
                // issuing tool calls, omit content and let `tool_calls` be the
                // sole assistant payload.
                None
            } else {
                Some(ChatCompletionRequestAssistantMessageContent::Text(content))
            };
            let tool_calls = tool_calls.filter(|calls| !calls.is_empty()).map(|calls| {
                calls
                    .into_iter()
                    .map(|call| ChatCompletionMessageToolCall {
                        id: call.id,
                        r#type: ChatCompletionToolType::Function,
                        function: FunctionCall {
                            name: call.name,
                            arguments: call.arguments.to_string(),
                        },
                    })
                    .collect()
            });

            ChatCompletionRequestAssistantMessage {
                content,
                tool_calls,
                ..Default::default()
            }
            .into()
        }
        Message::Tool {
            tool_call_id,
            content,
        } => ChatCompletionRequestToolMessage {
            content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(content),
            tool_call_id,
        }
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn tool_call() -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "Cargo.toml"}),
        }
    }

    fn tool_call_with_id(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: serde_json::json!({"path": "Cargo.toml"}),
        }
    }

    fn converted_request_json(messages: Vec<Message>) -> Value {
        let request = ChatRequest::new("test-model").with_messages(messages);
        serde_json::to_value(convert_request(request, "fallback-model")).unwrap()
    }

    fn request_messages(json: &Value) -> &[Value] {
        json["messages"].as_array().unwrap()
    }

    #[test]
    fn assistant_tool_call_omits_empty_content_for_strict_providers() {
        let message = convert_message(Message::assistant_with_tools("", vec![tool_call()]));
        let ChatCompletionRequestMessage::Assistant(assistant) = message else {
            panic!("expected assistant message");
        };

        assert!(assistant.content.is_none());
        assert_eq!(assistant.tool_calls.unwrap().len(), 1);
    }

    #[test]
    fn assistant_tool_call_keeps_non_empty_content() {
        let message = convert_message(Message::assistant_with_tools(
            "I will inspect the file.",
            vec![tool_call()],
        ));
        let ChatCompletionRequestMessage::Assistant(assistant) = message else {
            panic!("expected assistant message");
        };

        assert!(assistant.content.is_some());
        assert_eq!(assistant.tool_calls.unwrap().len(), 1);
    }

    #[test]
    fn request_can_force_named_tool_choice() {
        let request = ChatRequest::new("test-model")
            .with_messages(vec![Message::user("call the echo tool")])
            .with_tools(vec![crate::services::api::Tool {
                name: "provider_health_echo".to_string(),
                description: "Echo health value".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "value": {"type": "string"}
                    },
                    "required": ["value"]
                }),
            }])
            .with_tool_choice(ToolChoice::Function("provider_health_echo".to_string()));

        let converted = convert_request(request, "fallback-model");

        match converted.tool_choice {
            Some(ChatCompletionToolChoiceOption::Named(choice)) => {
                assert_eq!(choice.function.name, "provider_health_echo");
            }
            other => panic!("expected named tool choice, got {other:?}"),
        }
    }

    #[test]
    fn request_serializes_pure_tool_call_roundtrip() {
        let json = converted_request_json(vec![
            Message::user("inspect"),
            Message::assistant_with_tools("", vec![tool_call()]),
            Message::tool("call_1", "Result: OK\ncontent"),
        ]);
        let messages = request_messages(&json);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1]["role"], "assistant");
        assert!(
            messages[1].get("content").is_none() || messages[1]["content"].is_null(),
            "pure tool-call assistant content should be omitted/null: {}",
            messages[1]
        );
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_1");
    }

    #[test]
    fn request_serializes_text_plus_multiple_tool_call_roundtrip() {
        let json = converted_request_json(vec![
            Message::assistant_with_tools(
                "I will read two files.",
                vec![
                    tool_call_with_id("call_1", "file_read"),
                    tool_call_with_id("call_2", "grep"),
                ],
            ),
            Message::tool("call_1", "Result: OK\nfirst"),
            Message::tool("call_2", "Error: no matches"),
        ]);
        let messages = request_messages(&json);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["content"], "I will read two files.");
        assert_eq!(messages[0]["tool_calls"].as_array().unwrap().len(), 2);
        assert_eq!(messages[1]["tool_call_id"], "call_1");
        assert_eq!(messages[2]["tool_call_id"], "call_2");
    }

    #[test]
    fn request_drops_aborted_orphan_tool_result_before_serialization() {
        let json = converted_request_json(vec![
            Message::user("continue"),
            Message::tool("call_aborted", "Tool aborted"),
            Message::assistant("I can continue without it."),
        ]);
        let messages = request_messages(&json);

        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|msg| msg["role"] != "tool"));
    }

    #[test]
    fn assistant_with_empty_tool_list_does_not_emit_empty_tool_calls() {
        let json = converted_request_json(vec![Message::Assistant {
            content: "done".to_string(),
            tool_calls: Some(Vec::new()),
        }]);
        let assistant = &request_messages(&json)[0];

        assert_eq!(assistant["role"], "assistant");
        assert!(
            assistant.get("tool_calls").is_none() || assistant["tool_calls"].is_null(),
            "empty tool_calls array should not be serialized: {assistant}"
        );
    }
}

//! OpenAI API 兼容层
//!
//! 将内部类型转换为 async-openai 类型，供 Kimi/OpenAI 等兼容 API 使用

use crate::services::api::{
    normalize_tool_message_sequence, sanitize_assistant_content, ChatRequest, ChatResponse,
    Message, ToolCall, Usage,
};
use anyhow::{Context, Result};
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
    ChatCompletionRequestUserMessage, ChatCompletionTool, ChatCompletionToolType,
    CreateChatCompletionRequest, CreateChatCompletionResponse, FunctionCall, FunctionObject,
};

pub fn convert_request(request: ChatRequest, model: &str) -> CreateChatCompletionRequest {
    let messages: Vec<ChatCompletionRequestMessage> =
        normalize_tool_message_sequence(request.messages)
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
            let tool_calls = tool_calls.map(|calls| {
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

    fn tool_call() -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "Cargo.toml"}),
        }
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
}

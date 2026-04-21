//! OpenAI API 兼容层
//!
//! 将内部类型转换为 async-openai 类型，供 Kimi/OpenAI 等兼容 API 使用

use crate::services::api::{ChatRequest, ChatResponse, Message, ToolCall, Usage};
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
        request.messages.into_iter().map(convert_message).collect();

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
    });

    Ok(ChatResponse {
        content: message.content.unwrap_or_default(),
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
            let content = Some(ChatCompletionRequestAssistantMessageContent::Text(content));
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

//! API 客户端模块
//!
//! 支持多个 LLM 提供商：Kimi、OpenAI

pub mod kimi;
pub mod minimax;
pub mod openai;
pub mod openai_compat;
pub mod provider;
pub mod retry;

use async_openai::types::ChatCompletionResponseStream;
use async_trait::async_trait;

type PreHookFn = dyn Fn(ChatRequest) -> ChatRequest + Send + Sync;
type PostHookFn = dyn Fn(&ChatRequest, &ChatResponse) + Send + Sync;
type ErrorHookFn = dyn Fn(&str) + Send + Sync;

/// LLM Provider trait - 抽象不同的 API 提供商
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 发送聊天请求（非流式）
    async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse>;

    /// 发送聊天请求（流式）
    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> anyhow::Result<ChatCompletionResponseStream>;

    /// 获取 Base URL
    fn base_url(&self) -> &str;

    /// 获取默认模型
    fn default_model(&self) -> &str;
}

/// Provider Hook - 请求/响应拦截器
pub struct ProviderHook {
    /// Hook 名称
    pub name: String,
    /// 请求前 Hook（可修改请求）
    pub pre_hook: Option<Box<PreHookFn>>,
    /// 响应后 Hook（可处理/记录响应）
    pub post_hook: Option<Box<PostHookFn>>,
    /// 错误 Hook
    pub error_hook: Option<Box<ErrorHookFn>>,
}

impl Clone for ProviderHook {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            pre_hook: None, // Cannot clone closures
            post_hook: None,
            error_hook: None,
        }
    }
}

impl std::fmt::Debug for ProviderHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderHook")
            .field("name", &self.name)
            .field("pre_hook", &self.pre_hook.is_some())
            .field("post_hook", &self.post_hook.is_some())
            .field("error_hook", &self.error_hook.is_some())
            .finish()
    }
}

impl ProviderHook {
    /// 创建 ProviderHook
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pre_hook: None,
            post_hook: None,
            error_hook: None,
        }
    }

    /// 设置请求前 Hook
    pub fn with_pre_hook(
        mut self,
        hook: impl Fn(ChatRequest) -> ChatRequest + Send + Sync + 'static,
    ) -> Self {
        self.pre_hook = Some(Box::new(hook));
        self
    }

    /// 设置响应后 Hook
    pub fn with_post_hook(
        mut self,
        hook: impl Fn(&ChatRequest, &ChatResponse) + Send + Sync + 'static,
    ) -> Self {
        self.post_hook = Some(Box::new(hook));
        self
    }

    /// 设置错误 Hook
    pub fn with_error_hook(mut self, hook: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.error_hook = Some(Box::new(hook));
        self
    }
}

/// 聊天请求
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// Thinking budget (token 数)，启用 extended thinking
    /// 仅 Claude 4+ 和部分模型支持
    pub thinking_budget: Option<u32>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            tools: None,
            tool_choice: None,
            temperature: Some(0.2),
            max_tokens: None,
            thinking_budget: None,
        }
    }

    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }
}

#[derive(Debug, Clone)]
pub enum ToolChoice {
    None,
    Auto,
    Required,
    Function(String),
}

/// 消息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, std::hash::Hash)]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Message::System {
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Message::User {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Message::Assistant {
            content: content.into(),
            tool_calls: None,
        }
    }

    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Message::Assistant {
            content: content.into(),
            tool_calls: Some(tool_calls),
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Message::Tool {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
        }
    }

    /// 获取工具调用列表（如果是 Assistant 消息且有工具调用）
    pub fn tool_calls(&self) -> Option<&Vec<ToolCall>> {
        match self {
            Message::Assistant { tool_calls, .. } => tool_calls.as_ref(),
            _ => None,
        }
    }
}

/// 工具定义
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl Tool {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    pub fn with_parameters(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }
}

/// 工具调用
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, std::hash::Hash)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// 聊天响应
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub usage: Option<Usage>,
}

/// Remove provider-leaked hidden reasoning blocks from assistant-visible output.
///
/// Some models may return `<think>...</think>` text even when the prompt asks
/// them not to expose hidden reasoning. The product surface should show the
/// deliberate result, not chain-of-thought-like scratch text.
pub fn sanitize_assistant_content(content: impl AsRef<str>) -> String {
    let without_tool_call = strip_tag_block(content.as_ref(), "minimax:tool_call");
    let without_tool_call = strip_tag_block(&without_tool_call, "tool_call");
    let without_tool_call = strip_tag_block(&without_tool_call, "invoke");
    let without_thinking = strip_tag_block(&without_tool_call, "thinking");
    strip_tag_block(&without_thinking, "think")
        .trim_start_matches('\n')
        .to_string()
}

/// Keep provider-bound histories compatible with OpenAI-style tool semantics.
///
/// A message with `tool_calls` must be followed immediately by matching tool
/// result messages. Historical UI/session storage can contain final assistant
/// messages that mistakenly carried stale tool calls; those calls are display
/// metadata, not provider context. Drop invalid calls and orphan tool results
/// before sending the request so strict providers do not reject the turn.
pub fn normalize_tool_message_sequence(messages: Vec<Message>) -> Vec<Message> {
    let mut normalized = Vec::with_capacity(messages.len());
    let mut index = 0;

    while index < messages.len() {
        match messages[index].clone() {
            Message::Assistant {
                content,
                tool_calls: Some(tool_calls),
            } if !tool_calls.is_empty() => {
                let mut next = index + 1;
                let mut tool_result_ids = std::collections::HashSet::new();
                while next < messages.len() {
                    let Message::Tool { tool_call_id, .. } = &messages[next] else {
                        break;
                    };
                    if tool_call_id.is_empty() {
                        break;
                    }
                    tool_result_ids.insert(tool_call_id.clone());
                    next += 1;
                }

                let expected_ids = tool_calls
                    .iter()
                    .map(|call| call.id.clone())
                    .collect::<std::collections::HashSet<_>>();
                let has_matching_results = !expected_ids.is_empty()
                    && expected_ids.iter().all(|id| tool_result_ids.contains(id))
                    && tool_result_ids.iter().all(|id| expected_ids.contains(id));

                if has_matching_results {
                    normalized.push(Message::assistant_with_tools(content, tool_calls));
                    normalized.extend(messages[index + 1..next].iter().cloned());
                    index = next;
                } else {
                    normalized.push(Message::assistant(content));
                    index += 1;
                }
            }
            Message::Tool { .. } => {
                // Orphan tool results are not valid provider messages. The UI still
                // displays them separately; they should not poison the next API turn.
                index += 1;
            }
            other => {
                normalized.push(other);
                index += 1;
            }
        }
    }

    normalized
}

fn strip_tag_block(input: &str, tag: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    let open_prefix = format!("<{}", tag);
    let close_tag = format!("</{}>", tag);

    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(open_start) = lower.find(&open_prefix) else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..open_start]);

        let Some(open_end_rel) = lower[open_start..].find('>') else {
            break;
        };
        let content_start = open_start + open_end_rel + 1;
        let lower_after_open = &lower[content_start..];
        let Some(close_start_rel) = lower_after_open.find(&close_tag) else {
            break;
        };
        let close_end = content_start + close_start_rel + close_tag.len();
        rest = &rest[close_end..];
    }

    output
}

/// Token 使用量
#[derive(Debug, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// Reasoning tokens (from extended thinking models like Kimi K2.5)
    /// These tokens are used for the model's internal reasoning process
    pub reasoning_tokens: Option<u32>,
    /// Cached tokens (prompt prefix cache hits from the provider)
    /// Providers like OpenAI, Kimi K2, MiniMax return this in usage.prompt_tokens_details.cached_tokens
    pub cached_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::{normalize_tool_message_sequence, sanitize_assistant_content, Message, ToolCall};

    #[test]
    fn sanitizer_removes_think_blocks() {
        let content = "<think>internal notes</think>\n\nFinal answer";
        assert_eq!(sanitize_assistant_content(content), "Final answer");
    }

    #[test]
    fn sanitizer_removes_case_insensitive_thinking_blocks() {
        let content = "Before\n<Thinking>hidden</Thinking>\nAfter";
        assert_eq!(sanitize_assistant_content(content), "Before\n\nAfter");
    }

    #[test]
    fn sanitizer_drops_unclosed_hidden_block() {
        let content = "Visible\n<think>hidden forever";
        assert_eq!(sanitize_assistant_content(content), "Visible\n");
    }

    #[test]
    fn sanitizer_keeps_normal_language() {
        let content = "I think this is ready.";
        assert_eq!(sanitize_assistant_content(content), content);
    }

    #[test]
    fn sanitizer_removes_pseudo_tool_call_blocks() {
        let content =
            "Plan\n<minimax:tool_call><invoke name=\"grep\"></invoke></minimax:tool_call>";
        assert_eq!(sanitize_assistant_content(content), "Plan\n");
    }

    #[test]
    fn normalize_tool_sequence_keeps_valid_tool_call_pairs() {
        let messages = vec![
            Message::assistant_with_tools(
                "",
                vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "file_read".to_string(),
                    arguments: serde_json::json!({"path": "Cargo.toml"}),
                }],
            ),
            Message::tool("call_1", "Result: OK"),
            Message::assistant("done"),
        ];

        let normalized = normalize_tool_message_sequence(messages);
        assert_eq!(normalized.len(), 3);
        assert!(normalized[0].tool_calls().is_some());
        assert!(matches!(normalized[1], Message::Tool { .. }));
    }

    #[test]
    fn normalize_tool_sequence_drops_dangling_final_tool_call() {
        let messages = vec![
            Message::user("write a file"),
            Message::assistant_with_tools(
                "Done.",
                vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({"path": "/tmp/a", "content": "x"}),
                }],
            ),
            Message::user("how do I run it?"),
        ];

        let normalized = normalize_tool_message_sequence(messages);
        assert_eq!(normalized.len(), 3);
        assert!(normalized[1].tool_calls().is_none());
    }

    #[test]
    fn normalize_tool_sequence_drops_orphan_tool_result() {
        let messages = vec![
            Message::user("hello"),
            Message::tool("call_orphan", "Result: OK"),
            Message::assistant("done"),
        ];

        let normalized = normalize_tool_message_sequence(messages);
        assert_eq!(normalized.len(), 2);
        assert!(matches!(normalized[0], Message::User { .. }));
        assert!(matches!(normalized[1], Message::Assistant { .. }));
    }
}

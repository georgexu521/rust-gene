//! Shared direct-provider chat lane.
//!
//! This lane deliberately bypasses tools, permissions, validation, and closeout.
//! API and desktop callers keep their own DTO/session behavior but share request
//! construction and visible-output sanitization here.

use super::{sanitize_assistant_content, ChatRequest, LlmProvider, Message, Usage};
use anyhow::Result;
use std::sync::Arc;

pub const LIGHTWEIGHT_CHAT_SYSTEM_PROMPT: &str = "You are Liz, gex's concise AI coding partner. Answer this one user message directly in plain prose. Reply in the user's language. You have no tools in this lightweight lane: do not claim to inspect files, run commands, edit files, or verify anything. If the request requires project inspection or code changes, say it needs the full agent lane.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectChatSanitizePolicy {
    AssistantVisible,
    LightweightPlainText,
}

#[derive(Debug, Clone)]
pub struct DirectProviderChatRequest {
    pub model: String,
    pub system_prompt: String,
    pub user_message: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub sanitize_policy: DirectChatSanitizePolicy,
    pub empty_response_fallback: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DirectProviderChatOutcome {
    pub content: String,
    pub usage: Option<Usage>,
}

pub async fn run_direct_provider_chat(
    provider: Arc<dyn LlmProvider>,
    input: DirectProviderChatRequest,
) -> Result<DirectProviderChatOutcome> {
    let mut request = ChatRequest::new(&input.model).with_messages(vec![
        Message::system(input.system_prompt),
        Message::user(input.user_message),
    ]);
    if let Some(temperature) = input.temperature {
        request = request.with_temperature(temperature);
    }
    if input.max_tokens.is_some() {
        request = request.with_output_cap(input.max_tokens);
    }

    let response = provider.chat(request).await?;
    let mut content = sanitize_direct_chat_content(input.sanitize_policy, &response.content);
    if content.trim().is_empty() {
        content = input.empty_response_fallback.unwrap_or_default();
    }

    Ok(DirectProviderChatOutcome {
        content,
        usage: response.usage,
    })
}

pub fn sanitize_direct_chat_content(policy: DirectChatSanitizePolicy, content: &str) -> String {
    let sanitized = sanitize_assistant_content(content);
    match policy {
        DirectChatSanitizePolicy::AssistantVisible => sanitized.trim().to_string(),
        DirectChatSanitizePolicy::LightweightPlainText => {
            strip_hallucinated_tool_envelopes(&sanitized)
                .trim()
                .to_string()
        }
    }
}

fn strip_hallucinated_tool_envelopes(content: &str) -> String {
    let mut out = content.to_string();
    for (open, close) in [
        ("<function_calls>", "</function_calls>"),
        ("<|DSML|function_calls>", "</|DSML|function_calls>"),
        ("<｜DSML｜function_calls>", "</｜DSML｜function_calls>"),
    ] {
        out = strip_literal_block(&out, open, close);
    }
    for open in ["<｜DSML｜", "<|DSML|"] {
        if let Some(index) = out.find(open) {
            out.truncate(index);
        }
    }
    out
}

fn strip_literal_block(input: &str, open: &str, close: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find(open) {
        output.push_str(&rest[..start]);
        let after_open = &rest[start + open.len()..];
        let Some(end) = after_open.find(close) else {
            return output;
        };
        rest = &after_open[end + close.len()..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_visible_sanitizes_hidden_blocks() {
        let content = "Before <think>hidden</think> After";
        assert_eq!(
            sanitize_direct_chat_content(DirectChatSanitizePolicy::AssistantVisible, content),
            "Before  After"
        );
    }

    #[test]
    fn lightweight_plain_text_strips_orphan_dsml_envelope() {
        let content = "Answer\n<|DSML|function_calls>";
        assert_eq!(
            sanitize_direct_chat_content(DirectChatSanitizePolicy::LightweightPlainText, content),
            "Answer"
        );
    }
}

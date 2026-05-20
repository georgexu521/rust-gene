//! Forked subagent context builder.
//!
//! This mirrors the important Claude Code fork semantics in Rust:
//! keep the parent assistant tool-use prefix, satisfy each tool call with the
//! same placeholder result, append a child directive, and detect recursive
//! fork attempts by scanning for the fork boilerplate tag.

use crate::services::api::{Message, ToolCall};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FORK_BOILERPLATE_TAG: &str = "fork-boilerplate";
pub const FORK_DIRECTIVE_PREFIX: &str = "Your directive: ";
pub const FORK_PLACEHOLDER_RESULT: &str = "Fork started - processing in background";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkedContext {
    pub messages: Vec<Message>,
    pub child_message: String,
    pub placeholder_result: String,
    pub tool_call_ids: Vec<String>,
}

impl ForkedContext {
    pub fn is_placeholder_complete(&self) -> bool {
        let placeholder_count = self
            .messages
            .iter()
            .filter(|message| {
                matches!(
                    message,
                    Message::Tool { content, .. } if content == FORK_PLACEHOLDER_RESULT
                )
            })
            .count();
        placeholder_count == self.tool_call_ids.len()
    }
}

#[derive(Debug, Clone)]
pub struct ForkedContextBuildRequest {
    pub directive: String,
    pub parent_assistant_content: String,
    pub parent_tool_calls: Vec<ToolCall>,
    pub worktree_notice: Option<String>,
}

impl ForkedContextBuildRequest {
    pub fn new(directive: impl Into<String>, parent_tool_calls: Vec<ToolCall>) -> Self {
        Self {
            directive: directive.into(),
            parent_assistant_content: String::new(),
            parent_tool_calls,
            worktree_notice: None,
        }
    }

    pub fn with_parent_assistant_content(mut self, content: impl Into<String>) -> Self {
        self.parent_assistant_content = content.into();
        self
    }

    pub fn with_worktree_notice(mut self, notice: impl Into<String>) -> Self {
        self.worktree_notice = Some(notice.into());
        self
    }
}

pub fn build_forked_context(request: ForkedContextBuildRequest) -> Result<ForkedContext, String> {
    if text_contains_fork_boilerplate(&request.directive) {
        return Err("recursive fork blocked: directive already contains fork boilerplate".into());
    }

    let child_message = match request.worktree_notice {
        Some(notice) if !notice.trim().is_empty() => {
            format!("{}\n\n{}", build_child_message(&request.directive), notice)
        }
        _ => build_child_message(&request.directive),
    };

    let tool_call_ids = request
        .parent_tool_calls
        .iter()
        .map(|call| call.id.clone())
        .collect::<Vec<_>>();
    let mut messages = Vec::new();

    if request.parent_tool_calls.is_empty() {
        messages.push(Message::user(child_message.clone()));
    } else {
        messages.push(Message::assistant_with_tools(
            request.parent_assistant_content,
            request.parent_tool_calls,
        ));
        messages.extend(
            tool_call_ids
                .iter()
                .map(|id| Message::tool(id.clone(), FORK_PLACEHOLDER_RESULT)),
        );
        messages.push(Message::user(child_message.clone()));
    }

    Ok(ForkedContext {
        messages,
        child_message,
        placeholder_result: FORK_PLACEHOLDER_RESULT.to_string(),
        tool_call_ids,
    })
}

pub fn build_child_message(directive: &str) -> String {
    format!(
        "<{tag}>
STOP. READ THIS FIRST.

You are a forked worker process. You are NOT the main agent.

Rules:
1. Do not spawn sub-agents; execute directly.
2. Do not converse, ask questions, or suggest next steps.
3. Use tools directly and stay within your directive's scope.
4. If you modify files, report changed files and verification evidence.
5. Keep the final report factual and concise.
6. Your response must begin with \"Scope:\".

Output format:
Scope: <assigned scope in one sentence>
Result: <answer or key findings limited to the scope>
Key files: <relevant file paths>
Files changed: <changed files, if any>
Issues: <issues to flag, if any>
</{tag}>

{prefix}{directive}",
        tag = FORK_BOILERPLATE_TAG,
        prefix = FORK_DIRECTIVE_PREFIX,
        directive = directive.trim()
    )
}

pub fn build_worktree_notice(parent_cwd: &Path, worktree_cwd: &Path) -> String {
    format!(
        "You've inherited context from a parent agent working in {}. \
You are operating in an isolated git worktree at {}. \
Translate inherited paths to your worktree root, re-read files before editing, \
and keep changes isolated to this worktree.",
        parent_cwd.display(),
        worktree_cwd.display()
    )
}

pub fn messages_contain_fork_boilerplate(messages: &[Message]) -> bool {
    messages.iter().any(message_contains_fork_boilerplate)
}

pub fn message_contains_fork_boilerplate(message: &Message) -> bool {
    match message {
        Message::System { content }
        | Message::User { content }
        | Message::Tool { content, .. }
        | Message::Assistant { content, .. } => text_contains_fork_boilerplate(content),
    }
}

pub fn text_contains_fork_boilerplate(text: &str) -> bool {
    text.contains(&format!("<{}>", FORK_BOILERPLATE_TAG))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool_call(id: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: "file_read".to_string(),
            arguments: json!({"path": "src/main.rs"}),
        }
    }

    #[test]
    fn forked_context_adds_placeholder_results_for_each_tool_call() {
        let context = build_forked_context(ForkedContextBuildRequest::new(
            "inspect routing",
            vec![tool_call("call_1"), tool_call("call_2")],
        ))
        .unwrap();

        assert_eq!(context.messages.len(), 4);
        assert_eq!(context.tool_call_ids, vec!["call_1", "call_2"]);
        assert!(context.is_placeholder_complete());
        assert!(matches!(
            &context.messages[0],
            Message::Assistant {
                tool_calls: Some(calls),
                ..
            } if calls.len() == 2
        ));
        assert!(matches!(
            &context.messages[3],
            Message::User { content } if content.contains(FORK_DIRECTIVE_PREFIX)
        ));
    }

    #[test]
    fn forked_context_falls_back_to_directive_without_tool_calls() {
        let context =
            build_forked_context(ForkedContextBuildRequest::new("summarize docs", Vec::new()))
                .unwrap();

        assert_eq!(context.messages.len(), 1);
        assert!(context.tool_call_ids.is_empty());
        assert!(matches!(
            &context.messages[0],
            Message::User { content } if content.contains("summarize docs")
        ));
    }

    #[test]
    fn recursive_fork_guard_detects_boilerplate() {
        let child = build_child_message("do work");
        let messages = vec![Message::user(child.clone())];
        assert!(messages_contain_fork_boilerplate(&messages));
        assert!(text_contains_fork_boilerplate(&child));

        let err =
            build_forked_context(ForkedContextBuildRequest::new(child, Vec::new())).unwrap_err();
        assert!(err.contains("recursive fork blocked"));
    }

    #[test]
    fn worktree_notice_mentions_both_roots() {
        let notice = build_worktree_notice(Path::new("/parent"), Path::new("/child"));
        assert!(notice.contains("/parent"));
        assert!(notice.contains("/child"));
    }
}

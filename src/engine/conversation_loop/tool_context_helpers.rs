//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::tool_metadata::attach_tool_execution_metadata;
use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use std::collections::HashSet;

pub(super) fn tool_result_dialog_content(result: &ToolResult) -> String {
    if !result.content.is_empty() {
        result.content.clone()
    } else {
        result.error.clone().unwrap_or_default()
    }
}

pub(super) fn tool_call_fingerprint(tc: &ToolCall) -> String {
    let normalized = normalize_path_args(&tc.arguments);
    let args = serde_json::to_string(&normalized).unwrap_or_else(|_| "null".to_string());
    format!("{}|{}", tc.name, args)
}

/// Normalize path arguments so that `src/a/` and `src/a` produce the same
/// fingerprint, preventing duplicate-read-only leaks caused by trailing-slash
/// churn in weak providers.
fn normalize_path_args(args: &serde_json::Value) -> serde_json::Value {
    let mut value = args.clone();
    if let Some(obj) = value.as_object_mut() {
        for (key, val) in obj.iter_mut() {
            if key == "path"
                || key == "pattern"
                || key.ends_with("_path")
                || key.ends_with("_pattern")
            {
                if let Some(s) = val.as_str() {
                    let trimmed = s.trim_end_matches('/');
                    if !trimmed.is_empty() {
                        *val = serde_json::Value::String(trimmed.to_string());
                    }
                }
            }
        }
    }
    value
}

pub(super) fn tool_allowed_by_context(
    allowed_tools: &Option<HashSet<String>>,
    tool_name: &str,
) -> bool {
    allowed_tools
        .as_ref()
        .map(|allowed| allowed.contains(tool_name))
        .unwrap_or(true)
}

pub(super) fn tool_not_allowed_result(tool_call: &ToolCall) -> ToolResult {
    let mut result = ToolResult::error(format!(
        "Tool '{}' is not allowed in this agent context",
        tool_call.name
    ));
    attach_tool_execution_metadata(tool_call, &mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(name: &str) -> ToolCall {
        ToolCall {
            id: format!("call_{name}"),
            name: name.to_string(),
            arguments: serde_json::json!({"path": "src/main.rs"}),
        }
    }

    #[test]
    fn tool_result_dialog_content_prefers_content_over_error() {
        assert_eq!(
            tool_result_dialog_content(&ToolResult {
                success: true,
                content: "content".to_string(),
                error: Some("error".to_string()),
                ..Default::default()
            }),
            "content"
        );
        assert_eq!(
            tool_result_dialog_content(&ToolResult::error("error")),
            "error"
        );
    }

    #[test]
    fn tool_call_fingerprint_includes_name_and_arguments() {
        let fingerprint = tool_call_fingerprint(&tool_call("file_read"));
        assert!(fingerprint.starts_with("file_read|"));
        assert!(fingerprint.contains("src/main.rs"));
    }

    #[test]
    fn tool_call_fingerprint_normalizes_trailing_slash() {
        let with_slash = ToolCall {
            id: "call_1".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "src/engine/conversation_loop/"}),
        };
        let without_slash = ToolCall {
            id: "call_2".to_string(),
            name: "file_read".to_string(),
            arguments: serde_json::json!({"path": "src/engine/conversation_loop"}),
        };
        assert_eq!(
            tool_call_fingerprint(&with_slash),
            tool_call_fingerprint(&without_slash),
            "trailing slash should be normalized in fingerprint"
        );
    }

    #[test]
    fn tool_allowed_by_context_defaults_to_allowed_and_honors_scope() {
        assert!(tool_allowed_by_context(&None, "bash"));

        let allowed = Some(HashSet::from(["file_read".to_string(), "grep".to_string()]));
        assert!(tool_allowed_by_context(&allowed, "file_read"));
        assert!(tool_allowed_by_context(&allowed, "grep"));
        assert!(!tool_allowed_by_context(&allowed, "bash"));
    }

    #[test]
    fn tool_not_allowed_result_has_recovery_metadata() {
        let result = tool_not_allowed_result(&tool_call("bash"));

        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("not allowed"));
        assert!(result.data.is_some());
    }
}

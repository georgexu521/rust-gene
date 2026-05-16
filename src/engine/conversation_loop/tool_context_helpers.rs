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
    let args = serde_json::to_string(&tc.arguments).unwrap_or_else(|_| "null".to_string());
    format!("{}|{}", tc.name, args)
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

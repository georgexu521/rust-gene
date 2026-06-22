//! Shared API route helpers.
//!
//! Helpers here enforce route-local allowlists, tenant prefixes, pagination,
//! and compact text rendering for HTTP compatibility surfaces.

use axum::http::HeaderMap;
use serde_json::Value;

pub(crate) fn api_tool_call_allowed(tool: &str, params: &Value) -> bool {
    match tool {
        // Local read/search helpers.
        "file_read"
        | "grep"
        | "glob"
        | "project_list"
        | "git_status"
        | "git_diff"
        | "diff"
        | "calculate"
        | "datetime"
        | "json_query"
        | "context"
        | "context_visualization"
        | "tool_search"
        | "symbol_query" => true,
        // Public-network read tools are explicitly allowed; browser automation is not.
        "web_search" | "web_fetch" => true,
        // Task output can append, so only the default/get action is allowed remotely.
        "task_get" | "task_list" => true,
        "task_output" => params["action"]
            .as_str()
            .map(|action| action == "get")
            .unwrap_or(true),
        // Cost reads API-local accounting; ApiState injects the tracker for this call.
        "cost" => true,
        _ => false,
    }
}

pub(crate) fn sanitize_tenant_id(raw: &str) -> String {
    let mut s = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            s.push(ch);
        } else {
            s.push('_');
        }
    }
    if s.is_empty() {
        "default".to_string()
    } else {
        s
    }
}

pub(crate) fn tenant_prefix(headers: &HeaderMap) -> String {
    let tenant = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("default");
    format!("tenant_{}_", sanitize_tenant_id(tenant))
}

pub(crate) fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    out
}

pub(crate) fn parse_token_list(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c == ';' || c.is_ascii_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sanitize_tenant_id() {
        assert_eq!(sanitize_tenant_id("team-a"), "team-a");
        assert_eq!(sanitize_tenant_id("A/B C"), "A_B_C");
        assert_eq!(sanitize_tenant_id(""), "default");
    }

    #[test]
    fn test_truncate_chars() {
        assert_eq!(truncate_chars("hello", 3), "hel");
        assert_eq!(truncate_chars("你好世界", 2), "你好");
    }

    #[test]
    fn test_parse_token_list() {
        let tokens = parse_token_list("a,b; c  d");
        assert_eq!(tokens, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn test_api_tool_allowlist_uses_registered_read_only_names() {
        assert!(api_tool_call_allowed(
            "file_read",
            &json!({"path": "src/main.rs"})
        ));
        assert!(api_tool_call_allowed("git_status", &json!({})));
        assert!(api_tool_call_allowed("git_diff", &json!({})));
        assert!(api_tool_call_allowed(
            "json_query",
            &json!({"input": "{}", "query": "."})
        ));
        assert!(api_tool_call_allowed(
            "task_output",
            &json!({"task_id": "task_1"})
        ));
        assert!(api_tool_call_allowed(
            "task_output",
            &json!({"task_id": "task_1", "action": "get"})
        ));

        assert!(!api_tool_call_allowed("git_read", &json!({})));
        assert!(!api_tool_call_allowed("json_tool", &json!({})));
        assert!(!api_tool_call_allowed(
            "browser",
            &json!({"action": "evaluate_js"})
        ));
        assert!(!api_tool_call_allowed("bash", &json!({"command": "pwd"})));
        assert!(!api_tool_call_allowed(
            "task_output",
            &json!({"task_id": "task_1", "action": "append", "line": "x"})
        ));
    }
}

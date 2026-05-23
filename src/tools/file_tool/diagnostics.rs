use crate::engine::lsp::{language_id_from_path, path_to_uri, LspDiagnostic, LspManager};
use crate::tools::ToolContext;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

const MAX_FILE_EDIT_DIAGNOSTIC_ITEMS: usize = 20;
const FILE_EDIT_DIAGNOSTICS_MAX_WAIT_MS: u64 = 400;
const FILE_EDIT_DIAGNOSTICS_NOTIFY_TIMEOUT_MS: u64 = 200;
const FILE_EDIT_DIAGNOSTICS_POLL_MS: u64 = 50;
static FILE_EDIT_LSP_VERSION: AtomicI32 = AtomicI32::new(2);

pub(super) async fn collect_file_edit_diagnostics(
    context: &ToolContext,
    path: &Path,
    content: &str,
) -> Value {
    let Some(lsp_manager) = context.lsp_manager.as_ref() else {
        return file_edit_diagnostics_unavailable("lsp_unavailable");
    };

    let servers = lsp_manager.server_names();
    if servers.is_empty() {
        return file_edit_diagnostics_unavailable("no_lsp_clients");
    }

    let uri = path_to_uri(path);
    let language_id = language_id_from_path(path);
    let mut initialized_servers = Vec::new();
    for server in &servers {
        let Some(client) = lsp_manager.get_client(server) else {
            continue;
        };
        if client.is_initialized().await {
            initialized_servers.push((server.clone(), client));
        }
    }

    let mut collection_errors = Vec::new();
    let mut sync_actions = Vec::new();
    for (server, client) in &initialized_servers {
        let version = next_file_edit_lsp_version();
        match tokio::time::timeout(
            Duration::from_millis(FILE_EDIT_DIAGNOSTICS_NOTIFY_TIMEOUT_MS),
            client.text_document_sync_for_diagnostics(&uri, language_id, version, content),
        )
        .await
        {
            Ok(Ok(action)) => sync_actions.push(json!({
                "server": server,
                "action": action,
                "version": version,
            })),
            Ok(Err(err)) => collection_errors.push(json!({
                "server": server,
                "error": err.to_string(),
                "version": version,
            })),
            Err(_) => collection_errors.push(json!({
                "server": server,
                "error": "diagnostic notification timed out",
                "version": version,
            })),
        }
    }

    let mut diagnostics = collect_cached_lsp_diagnostics(lsp_manager, &servers, &uri).await;
    if diagnostics.is_empty() && !initialized_servers.is_empty() {
        let started = Instant::now();
        while diagnostics.is_empty()
            && started.elapsed() < Duration::from_millis(FILE_EDIT_DIAGNOSTICS_MAX_WAIT_MS)
        {
            tokio::time::sleep(Duration::from_millis(FILE_EDIT_DIAGNOSTICS_POLL_MS)).await;
            diagnostics = collect_cached_lsp_diagnostics(lsp_manager, &servers, &uri).await;
        }
    }

    file_edit_diagnostics_summary_json(
        servers.len(),
        initialized_servers.len(),
        diagnostics,
        collection_errors,
        sync_actions,
    )
}

fn next_file_edit_lsp_version() -> i32 {
    FILE_EDIT_LSP_VERSION.fetch_add(1, Ordering::SeqCst)
}

async fn collect_cached_lsp_diagnostics(
    lsp_manager: &LspManager,
    servers: &[String],
    uri: &str,
) -> Vec<(String, LspDiagnostic)> {
    let mut diagnostics = Vec::new();
    for server in servers {
        let Some(client) = lsp_manager.get_client(server) else {
            continue;
        };
        diagnostics.extend(
            client
                .get_diagnostics(uri)
                .await
                .into_iter()
                .map(|diagnostic| (server.clone(), diagnostic)),
        );
    }
    diagnostics
}

fn file_edit_diagnostics_unavailable(status: &str) -> Value {
    json!({
        "available": status != "lsp_unavailable",
        "checked": false,
        "status": status,
        "server_count": 0,
        "initialized_server_count": 0,
        "diagnostic_count": 0,
        "error_count": 0,
        "warning_count": 0,
        "information_count": 0,
        "hint_count": 0,
        "unknown_count": 0,
        "truncated": false,
        "items": [],
        "sync_actions": [],
        "collection_errors": [],
    })
}

fn file_edit_diagnostics_summary_json(
    server_count: usize,
    initialized_server_count: usize,
    diagnostics: Vec<(String, LspDiagnostic)>,
    collection_errors: Vec<Value>,
    sync_actions: Vec<Value>,
) -> Value {
    let diagnostic_count = diagnostics.len();
    let mut error_count = 0usize;
    let mut warning_count = 0usize;
    let mut information_count = 0usize;
    let mut hint_count = 0usize;
    let mut unknown_count = 0usize;

    for (_, diagnostic) in &diagnostics {
        match diagnostic.severity {
            Some(1) => error_count += 1,
            Some(2) => warning_count += 1,
            Some(3) => information_count += 1,
            Some(4) => hint_count += 1,
            _ => unknown_count += 1,
        }
    }

    let status = if !collection_errors.is_empty() && diagnostic_count == 0 {
        "collection_error"
    } else if !collection_errors.is_empty() {
        "partial"
    } else if diagnostic_count > 0 {
        "diagnostics_found"
    } else if initialized_server_count > 0 {
        "no_diagnostics"
    } else {
        "no_initialized_lsp_clients"
    };
    let checked = initialized_server_count > 0 || diagnostic_count > 0;
    let truncated = diagnostic_count > MAX_FILE_EDIT_DIAGNOSTIC_ITEMS;
    let first_error = first_diagnostic_summary_json(&diagnostics, Some(1));
    let first_warning = first_diagnostic_summary_json(&diagnostics, Some(2));
    let affected_line_range = diagnostic_line_range_json(&diagnostics);

    json!({
        "available": true,
        "checked": checked,
        "status": status,
        "server_count": server_count,
        "initialized_server_count": initialized_server_count,
        "diagnostic_count": diagnostic_count,
        "error_count": error_count,
        "warning_count": warning_count,
        "information_count": information_count,
        "hint_count": hint_count,
        "unknown_count": unknown_count,
        "affected_line_range": affected_line_range.unwrap_or(Value::Null),
        "first_error": first_error.unwrap_or(Value::Null),
        "first_warning": first_warning.unwrap_or(Value::Null),
        "truncated": truncated,
        "items": diagnostics
            .into_iter()
            .take(MAX_FILE_EDIT_DIAGNOSTIC_ITEMS)
            .map(|(server, diagnostic)| file_edit_diagnostic_item_json(&server, &diagnostic))
            .collect::<Vec<_>>(),
        "sync_actions": sync_actions,
        "collection_errors": collection_errors,
    })
}

fn first_diagnostic_summary_json(
    diagnostics: &[(String, LspDiagnostic)],
    severity: Option<u8>,
) -> Option<Value> {
    diagnostics
        .iter()
        .find(|(_, diagnostic)| {
            severity.is_none_or(|severity| diagnostic.severity == Some(severity))
        })
        .map(|(server, diagnostic)| file_edit_diagnostic_item_json(server, diagnostic))
}

fn diagnostic_line_range_json(diagnostics: &[(String, LspDiagnostic)]) -> Option<Value> {
    let start = diagnostics
        .iter()
        .map(|(_, diagnostic)| diagnostic.range.start.line + 1)
        .min()?;
    let end = diagnostics
        .iter()
        .map(|(_, diagnostic)| diagnostic.range.end.line + 1)
        .max()
        .unwrap_or(start);
    Some(json!({
        "start_line": start,
        "end_line": end,
    }))
}

fn file_edit_diagnostic_item_json(server: &str, diagnostic: &LspDiagnostic) -> Value {
    json!({
        "server": server,
        "severity": lsp_diagnostic_severity_label(diagnostic.severity),
        "severity_code": diagnostic.severity,
        "source": diagnostic.source.clone(),
        "code": diagnostic.code.clone(),
        "message": diagnostic.message.clone(),
        "range": {
            "start_line": diagnostic.range.start.line + 1,
            "start_character": diagnostic.range.start.character + 1,
            "end_line": diagnostic.range.end.line + 1,
            "end_character": diagnostic.range.end.character + 1,
        }
    })
}

fn lsp_diagnostic_severity_label(severity: Option<u8>) -> &'static str {
    match severity {
        Some(1) => "error",
        Some(2) => "warning",
        Some(3) => "information",
        Some(4) => "hint",
        _ => "unknown",
    }
}

pub(super) fn file_edit_diagnostics_content_line(diagnostics: &Value) -> Option<String> {
    let status = diagnostics.get("status").and_then(Value::as_str)?;
    let total = diagnostics
        .get("diagnostic_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let errors = diagnostics
        .get("error_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let warnings = diagnostics
        .get("warning_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    if total == 0 {
        return (status == "collection_error").then(|| {
            "LSP diagnostics: collection failed; see diagnostics.collection_errors.".to_string()
        });
    }

    let first = diagnostic_headline(diagnostics, "first_error", "First error")
        .or_else(|| diagnostic_headline(diagnostics, "first_warning", "First warning"))
        .or_else(|| {
            diagnostics
                .get("items")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| diagnostic_headline_from_item(item, "First diagnostic"))
        })
        .map(|headline| format!(" {headline}"))
        .unwrap_or_default();

    Some(format!(
        "LSP diagnostics: {errors} error(s), {warnings} warning(s), {total} total.{first}"
    ))
}

pub(super) fn file_edit_diagnostics_delta(before: &Value, after: &Value) -> Value {
    let before_errors = diagnostic_count_for(before, "error_count");
    let after_errors = diagnostic_count_for(after, "error_count");
    let before_warnings = diagnostic_count_for(before, "warning_count");
    let after_warnings = diagnostic_count_for(after, "warning_count");
    let before_total = diagnostic_count_for(before, "diagnostic_count");
    let after_total = diagnostic_count_for(after, "diagnostic_count");
    let checked = before
        .get("checked")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || after
            .get("checked")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let status = if !checked {
        "not_checked"
    } else if after_errors > before_errors {
        "new_errors"
    } else if after_warnings > before_warnings {
        "new_warnings"
    } else if after_total > before_total {
        "new_diagnostics"
    } else if after_total < before_total {
        "improved"
    } else {
        "unchanged"
    };

    json!({
        "checked": checked,
        "status": status,
        "before": {
            "diagnostic_count": before_total,
            "error_count": before_errors,
            "warning_count": before_warnings,
        },
        "after": {
            "diagnostic_count": after_total,
            "error_count": after_errors,
            "warning_count": after_warnings,
        },
        "change": {
            "diagnostic_count": after_total - before_total,
            "error_count": after_errors - before_errors,
            "warning_count": after_warnings - before_warnings,
        },
        "introduced_error": after_errors > before_errors,
        "introduced_warning": after_warnings > before_warnings,
    })
}

fn diagnostic_count_for(diagnostics: &Value, key: &str) -> i64 {
    diagnostics.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn diagnostic_headline(diagnostics: &Value, key: &str, label: &str) -> Option<String> {
    diagnostics
        .get(key)
        .filter(|value| !value.is_null())
        .and_then(|item| diagnostic_headline_from_item(item, label))
}

fn diagnostic_headline_from_item(item: &Value, label: &str) -> Option<String> {
    let message = item.get("message").and_then(Value::as_str)?;
    let mut preview = message.chars().take(160).collect::<String>();
    if message.chars().count() > 160 {
        preview.push_str("...");
    }
    let line = item
        .get("range")
        .and_then(|range| range.get("start_line"))
        .and_then(Value::as_u64)
        .map(|line| format!(" at line {line}"))
        .unwrap_or_default();
    let source = item
        .get("source")
        .and_then(Value::as_str)
        .filter(|source| !source.is_empty())
        .map(|source| format!(" from {source}"))
        .unwrap_or_default();
    let code = item
        .get("code")
        .filter(|code| !code.is_null())
        .map(|code| {
            code.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| code.to_string())
        })
        .filter(|code| !code.is_empty())
        .map(|code| format!(" [{code}]"))
        .unwrap_or_default();
    Some(format!("{label}{line}{source}{code}: {preview}"))
}

#[cfg(test)]
fn test_diagnostic(severity: u8, line: u32, message: &str) -> LspDiagnostic {
    LspDiagnostic {
        range: crate::engine::lsp::LspRange {
            start: crate::engine::lsp::LspPosition { line, character: 2 },
            end: crate::engine::lsp::LspPosition { line, character: 8 },
        },
        severity: Some(severity),
        code: Some(json!("E0001")),
        source: Some("rust-analyzer".to_string()),
        message: message.to_string(),
    }
}

#[cfg(test)]
fn assert_diagnostics_summary_has_first_error(summary: &Value) {
    assert_eq!(summary["diagnostic_count"], 2);
    assert_eq!(summary["affected_line_range"]["start_line"], 2);
    assert_eq!(summary["affected_line_range"]["end_line"], 4);
    assert_eq!(summary["first_error"]["severity"], "error");
    assert_eq!(summary["first_error"]["range"]["start_line"], 4);
    assert_eq!(summary["first_warning"]["severity"], "warning");
}

#[cfg(test)]
fn assert_diagnostics_content_prefers_first_error(summary: &Value) {
    let content_line = file_edit_diagnostics_content_line(summary).unwrap();
    assert!(content_line.contains("1 error(s), 1 warning(s), 2 total"));
    assert!(content_line.contains("First error at line 4"));
    assert!(content_line.contains("from rust-analyzer"));
    assert!(content_line.contains("[E0001]"));
    assert!(content_line.contains("type mismatch"));
}

#[cfg(test)]
fn diagnostics_summary_with_warning_then_error() -> Value {
    file_edit_diagnostics_summary_json(
        1,
        1,
        vec![
            (
                "rust-analyzer".to_string(),
                test_diagnostic(2, 1, "unused variable"),
            ),
            (
                "rust-analyzer".to_string(),
                test_diagnostic(1, 3, "type mismatch"),
            ),
        ],
        Vec::new(),
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_edit_lsp_versions_are_monotonic() {
        let first = next_file_edit_lsp_version();
        let second = next_file_edit_lsp_version();

        assert!(second > first);
    }

    #[test]
    fn diagnostics_summary_exposes_first_error_and_line_range() {
        let summary = diagnostics_summary_with_warning_then_error();

        assert_diagnostics_summary_has_first_error(&summary);
    }

    #[test]
    fn diagnostics_content_line_prefers_first_error() {
        let summary = diagnostics_summary_with_warning_then_error();

        assert_diagnostics_content_prefers_first_error(&summary);
    }

    #[test]
    fn diagnostics_delta_flags_new_errors() {
        let before = file_edit_diagnostics_summary_json(
            1,
            1,
            vec![(
                "rust-analyzer".to_string(),
                test_diagnostic(2, 1, "unused variable"),
            )],
            Vec::new(),
            Vec::new(),
        );
        let after = diagnostics_summary_with_warning_then_error();

        let delta = file_edit_diagnostics_delta(&before, &after);

        assert_eq!(delta["checked"], true);
        assert_eq!(delta["status"], "new_errors");
        assert_eq!(delta["change"]["error_count"], 1);
        assert_eq!(delta["introduced_error"], true);
    }
}

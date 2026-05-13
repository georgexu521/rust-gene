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

    let first_message = diagnostics
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("message"))
        .and_then(Value::as_str)
        .map(|message| {
            let mut preview = message.chars().take(160).collect::<String>();
            if message.chars().count() > 160 {
                preview.push_str("...");
            }
            preview
        });
    let first = first_message
        .map(|message| format!(" First: {message}"))
        .unwrap_or_default();

    Some(format!(
        "LSP diagnostics: {errors} error(s), {warnings} warning(s), {total} total.{first}"
    ))
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
}

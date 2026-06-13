use crate::session_store::SessionEventRow;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub struct ExportEventSummary {
    pub changed_files: Vec<String>,
    pub diagnostics: Vec<crate::session_store::export::ExportDiagnosticRecord>,
    pub tool_stats: Value,
}

pub fn summarize_export_events(events: &[SessionEventRow]) -> ExportEventSummary {
    let mut call_tools = BTreeMap::<String, String>::new();
    let mut tool_calls = BTreeMap::<String, usize>::new();
    let mut tool_successes = BTreeMap::<String, usize>::new();
    let mut tool_failures = BTreeMap::<String, usize>::new();
    let mut counted_calls = BTreeSet::<String>::new();
    let mut counted_successes = BTreeSet::<String>::new();
    let mut counted_failures = BTreeSet::<String>::new();
    let mut changed_files = BTreeSet::<String>::new();
    let mut diagnostics = Vec::new();

    for event in events {
        let payload = serde_json::from_str::<Value>(&event.payload).unwrap_or(Value::Null);
        match event.event_type.as_str() {
            "tool_called" | "tool_started" => {
                if let (Some(id), Some(name)) = (
                    payload.get("tool_call_id").and_then(Value::as_str),
                    payload.get("tool_name").and_then(Value::as_str),
                ) {
                    call_tools.insert(id.to_string(), name.to_string());
                    if counted_calls.insert(id.to_string()) {
                        *tool_calls.entry(name.to_string()).or_default() += 1;
                    }
                }
            }
            "tool_input_completed" => {
                let Some(call_id) = payload.get("tool_call_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(tool) = call_tools.get(call_id) else {
                    continue;
                };
                if !matches!(tool.as_str(), "file_write" | "file_edit" | "file_patch") {
                    continue;
                }
                if let Some(input) = payload
                    .get("input_args")
                    .and_then(Value::as_str)
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                {
                    collect_changed_paths_from_tool_input(&input, &mut changed_files);
                }
            }
            "tool_succeeded" => {
                if let Some((call_id, tool)) = tool_call_and_name_for_event(&payload, &call_tools) {
                    if !counted_successes.insert(call_id) {
                        continue;
                    }
                    *tool_successes.entry(tool).or_default() += 1;
                }
            }
            "tool_result_completed" => {}
            "tool_failed" => {
                if let Some((call_id, tool)) = tool_call_and_name_for_event(&payload, &call_tools) {
                    if !counted_failures.insert(call_id) {
                        continue;
                    }
                    *tool_failures.entry(tool).or_default() += 1;
                }
            }
            "runtime_diagnostic" => {
                diagnostics.push(crate::session_store::export::ExportDiagnosticRecord {
                    source: "runtime".to_string(),
                    status: "recorded".to_string(),
                    path: None,
                    error_count: 0,
                    warning_count: 0,
                    detail: payload
                        .get("schema")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
            _ => {}
        }
    }

    ExportEventSummary {
        changed_files: changed_files.into_iter().collect(),
        diagnostics,
        tool_stats: serde_json::json!({
            "calls": tool_calls,
            "successes": tool_successes,
            "failures": tool_failures,
        }),
    }
}

fn tool_call_and_name_for_event(
    payload: &Value,
    call_tools: &BTreeMap<String, String>,
) -> Option<(String, String)> {
    let call_id = payload.get("tool_call_id").and_then(Value::as_str)?;
    let tool = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| call_tools.get(call_id).cloned())?;
    Some((call_id.to_string(), tool))
}

fn collect_changed_paths_from_tool_input(input: &Value, changed_files: &mut BTreeSet<String>) {
    if let Some(path) = input.get("path").and_then(Value::as_str) {
        if !path.trim().is_empty() {
            changed_files.insert(path.trim().to_string());
        }
    }
    if let Some(paths) = input.get("written_paths").and_then(Value::as_array) {
        for path in paths.iter().filter_map(Value::as_str) {
            if !path.trim().is_empty() {
                changed_files.insert(path.trim().to_string());
            }
        }
    }
    if let Some(operations) = input.get("operations").and_then(Value::as_array) {
        for operation in operations {
            if let Some(path) = operation.get("path").and_then(Value::as_str) {
                if !path.trim().is_empty() {
                    changed_files.insert(path.trim().to_string());
                }
            }
        }
    }
}

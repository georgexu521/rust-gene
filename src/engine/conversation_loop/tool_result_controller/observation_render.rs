use super::{ObservationEvidence, ToolObservation};
use crate::services::api::ToolCall;
use crate::tools::ToolResult;

pub(super) fn model_visibility_for(
    tool_call: &ToolCall,
    result: &ToolResult,
    observation: &ToolObservation,
    raw_model_content: &str,
) -> &'static str {
    let raw_chars = raw_model_content.chars().count();
    if observation.raw_result_ref.is_some() && raw_chars > 8_000 {
        return "artifact_only";
    }
    match observation.result_kind.as_str() {
        "search" => {
            let total_matches = result
                .data
                .as_ref()
                .and_then(|data| data.get("total_matches"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let truncated = result
                .data
                .as_ref()
                .and_then(|data| data.get("truncated"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if total_matches > 8 || truncated || raw_chars > 1_200 {
                "observation"
            } else {
                "full_raw"
            }
        }
        "validation" if !result.success || raw_chars > 2_000 => "raw_excerpt",
        "install" | "dev_server" => "observation",
        "unknown_command" if raw_chars > 1_200 || !result.success => "raw_excerpt",
        "diff" if raw_chars > 2_000 => "raw_excerpt",
        "edit" if !result.success => "raw_excerpt",
        _ if tool_call.name == "run_tests" && raw_chars > 1_200 => "raw_excerpt",
        _ => "full_raw",
    }
}

pub(super) fn model_content_for_visibility(
    result: &ToolResult,
    observation: &ToolObservation,
    raw_model_content: &str,
    model_visibility: &str,
) -> String {
    if model_visibility == "full_raw" {
        return raw_model_content.to_string();
    }
    let label = if result.success { "OK" } else { "ERROR" };
    let mut out = format!(
        "Result: {label}\nObservation ({kind}): {summary}\nStatus: {status}",
        kind = observation.result_kind,
        summary = observation.summary,
        status = observation.status
    );
    append_lines(&mut out, "Key findings", &observation.key_findings);
    if !observation.evidence.is_empty() {
        out.push_str("\nEvidence:");
        for item in observation.evidence.iter().take(5) {
            let source = item
                .source
                .as_deref()
                .map(|source| format!(" {source}:"))
                .unwrap_or_default();
            out.push_str(&format!("\n- [{}]{} {}", item.kind, source, item.text));
        }
    }
    append_lines(&mut out, "Next attention", &observation.next_attention);
    if let Some(impact) = observation.impact_on_goal.as_deref() {
        out.push_str("\nImpact on goal: ");
        out.push_str(impact);
    }
    if let Some(risk_note) = observation.risk_note.as_deref() {
        out.push_str("\nRisk note: ");
        out.push_str(risk_note);
    }
    if let Some(permission_source) = observation.permission_source.as_deref() {
        out.push_str("\nPermission source: ");
        out.push_str(permission_source);
    }
    if let Some(failure_type) = observation.failure_type.as_deref() {
        out.push_str("\nFailure type: ");
        out.push_str(failure_type);
    }
    if let Some(recovery_kind) = observation.recovery_kind.as_deref() {
        out.push_str("\nRecovery kind: ");
        out.push_str(recovery_kind);
    }
    if observation.raw_result_ref.is_some() {
        out.push_str("\nRaw result stored outside provider-visible context; use targeted follow-up tools if more detail is needed.");
    }
    append_lines(&mut out, "Observer warnings", &observation.quality_warnings);
    if model_visibility == "raw_excerpt" {
        let excerpt = safe_observation_text(&result_output_from_provider(raw_model_content), 1_600);
        if !excerpt.trim().is_empty() {
            out.push_str("\nRaw excerpt:\n");
            out.push_str(&excerpt);
        }
    }
    out
}

pub(super) fn append_lines(out: &mut String, title: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    out.push('\n');
    out.push_str(title);
    out.push(':');
    for line in lines.iter().take(5) {
        out.push_str("\n- ");
        out.push_str(line);
    }
}

pub(super) fn top_search_files(data: Option<&serde_json::Value>, limit: usize) -> Vec<String> {
    let mut files = Vec::new();
    if let Some(matches) = data
        .and_then(|value| value.get("matches"))
        .and_then(serde_json::Value::as_array)
    {
        for item in matches {
            if let Some(file) = item.get("file").and_then(serde_json::Value::as_str) {
                files.push(file.to_string());
            } else if let Some(file) = item
                .get("resolved_file")
                .and_then(serde_json::Value::as_str)
            {
                files.push(file.to_string());
            }
        }
    }
    dedup_strings(&mut files);
    files.truncate(limit);
    files
}

pub(super) fn push_evidence(
    evidence: &mut Vec<ObservationEvidence>,
    kind: &str,
    source: Option<String>,
    text: &str,
) {
    let text = safe_observation_text(text, 360);
    if text.trim().is_empty() {
        return;
    }
    if evidence
        .iter()
        .any(|item| item.kind == kind && item.source == source && item.text == text)
    {
        return;
    }
    evidence.push(ObservationEvidence {
        kind: kind.to_string(),
        source,
        text,
    });
}

pub(super) fn result_output_body(result: &ToolResult) -> String {
    if !result.content.trim().is_empty() {
        result.content.clone()
    } else {
        result.error.clone().unwrap_or_default()
    }
}

pub(super) fn result_output_from_provider(model_content: &str) -> String {
    model_content
        .lines()
        .filter(|line| !line.starts_with("Result:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub(super) fn diagnostic_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.ends_with("--- FAILED")
            || trimmed.starts_with("---- ")
            || trimmed.starts_with("FAIL")
            || trimmed.starts_with("FAILED")
            || trimmed.starts_with("error:")
            || trimmed.starts_with("error[")
            || lower.contains("panicked at")
            || lower.contains("expected")
            || lower.contains("received")
            || lower.contains("assertion")
        {
            push_unique(&mut lines, safe_observation_text(trimmed, 280));
        }
        if lines.len() >= 8 {
            break;
        }
    }
    lines
}

pub(super) fn first_diagnostic_line(text: &str) -> Option<String> {
    diagnostic_lines(text).into_iter().next()
}

pub(super) fn extract_failed_tests_from_text(text: &str) -> Vec<String> {
    let mut tests = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("--- FAILED") {
            let name = trimmed
                .trim_end_matches("--- FAILED")
                .trim()
                .strip_prefix("test ")
                .unwrap_or_else(|| trimmed.trim_end_matches("--- FAILED").trim())
                .to_string();
            push_unique(&mut tests, name);
        } else if let Some(rest) = trimmed.strip_prefix("---- ") {
            if let Some((name, _)) = rest.split_once(" stdout ----") {
                push_unique(&mut tests, name.trim().to_string());
            }
        } else if let Some((_, name)) = trimmed.split_once("test ") {
            if let Some((test_name, status)) = name.rsplit_once(" ... ") {
                if status.trim() == "FAILED" {
                    push_unique(&mut tests, test_name.trim().to_string());
                }
            }
        }
    }
    tests
}

pub(super) fn looks_like_diff_command(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    lower.starts_with("git diff")
        || lower.starts_with("git --no-pager diff")
        || lower.contains(" git diff ")
        || lower.contains(" git --no-pager diff ")
}

pub(super) fn collect_string_field(
    out: &mut Vec<String>,
    data: Option<&serde_json::Value>,
    key: &str,
) {
    if let Some(value) = data
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        out.push(value.to_string());
    }
}

pub(super) fn collect_string_array_field(
    out: &mut Vec<String>,
    data: Option<&serde_json::Value>,
    key: &str,
) {
    if let Some(values) = data
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_array)
    {
        out.extend(
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string),
        );
    }
}

pub(super) fn dedup_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

pub(super) fn push_unique(values: &mut Vec<String>, value: String) {
    if value.trim().is_empty() || values.contains(&value) {
        return;
    }
    values.push(value);
}

pub(super) fn safe_observation_text(value: &str, max_chars: usize) -> String {
    let mut text = value.trim().chars().take(max_chars).collect::<String>();
    if value.trim().chars().count() > max_chars {
        text.push_str("...");
    }
    text
}

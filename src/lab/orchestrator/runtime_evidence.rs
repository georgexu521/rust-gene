//! Runtime evidence parsing helpers for LabRun orchestration.

use super::*;

pub(super) fn closeout_status_from_gate(gate: &ArtifactGate) -> LabCloseoutStatus {
    match gate
        .validation_status
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "verified" | "validated" | "passed" | "success" => LabCloseoutStatus::CompletedVerified,
        "partial" | "partially_verified" | "partially_completed" => LabCloseoutStatus::Partial,
        "blocked" | "blocked_needs_user" | "needs_user" => LabCloseoutStatus::BlockedNeedsUser,
        "failed" | "failure" => LabCloseoutStatus::Failed,
        _ => LabCloseoutStatus::CompletedNotVerified,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedGraduateAgentResult {
    pub(super) task_summary: String,
    pub(super) changed_files: Vec<String>,
    pub(super) validation_attempts: Vec<String>,
    pub(super) blockers: Vec<String>,
    pub(super) evidence_ids: Vec<String>,
}

pub(super) fn parse_graduate_agent_result(
    data: Option<&Value>,
    content: &str,
) -> Option<ParsedGraduateAgentResult> {
    if let Some(data) = data {
        if let Some(parsed) = parse_graduate_agent_result_value(data) {
            return Some(parsed);
        }
        if let Some(result) = data.get("result").and_then(Value::as_str) {
            if let Some(value) = parse_json_value_from_text(result) {
                if let Some(parsed) = parse_graduate_agent_result_value(&value) {
                    return Some(parsed);
                }
            }
        }
    }
    parse_json_value_from_text(content).and_then(|value| parse_graduate_agent_result_value(&value))
}

pub(super) fn parse_json_value_from_text(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }
    if let Some(fenced) = trimmed.strip_prefix("```") {
        let body = fenced.lines().skip(1).collect::<Vec<_>>().join("\n");
        let body = body
            .trim()
            .strip_suffix("```")
            .unwrap_or(body.trim())
            .trim();
        if let Ok(value) = serde_json::from_str::<Value>(body) {
            return Some(value);
        }
    }
    let start = trimmed.find('{')?;
    for end in trimmed.rmatch_indices('}').map(|(idx, _)| idx + 1) {
        if end <= start {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&trimmed[start..end]) {
            return Some(value);
        }
    }
    None
}

pub(super) fn parse_graduate_agent_result_value(
    value: &Value,
) -> Option<ParsedGraduateAgentResult> {
    let value = value
        .get("graduate_result")
        .or_else(|| value.get("result_json"))
        .unwrap_or(value);
    let task_summary = string_field(value, &["task_summary", "summary", "handoff_summary"])?;
    let validation_attempts = string_array_field(
        value,
        &["validation_attempts", "validation_results", "validation"],
    );
    if validation_attempts.is_empty() {
        return None;
    }
    Some(ParsedGraduateAgentResult {
        task_summary,
        changed_files: string_array_field(value, &["changed_files", "files_changed"]),
        validation_attempts,
        blockers: string_array_field(value, &["blockers", "risks"]),
        evidence_ids: string_array_field(value, &["evidence_ids", "evidence_refs"]),
    })
}

pub(super) fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn string_array_field(value: &Value, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .find_map(|name| value.get(*name))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

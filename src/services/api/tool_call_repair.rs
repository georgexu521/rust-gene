//! Weak-model tool-call repair.
//!
//! Some OpenAI-compatible weak reasoning models leak tool calls as text,
//! truncate JSON arguments, or drop nested arguments when the schema is too
//! complex. This module keeps those repairs at the provider boundary so the
//! rest of the runtime still sees normal `ToolCall` values and can apply the
//! existing permission, stale-read, validation, and closeout gates.

use super::{sanitize_assistant_content, Tool, ToolCall};
use crate::services::api::provider_protocol::ProviderProtocolFamily;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRepairReport {
    pub provider_family: String,
    pub schema_flattened_tools: usize,
    pub schema_flattened_fields: usize,
    pub scavenged_tool_calls: usize,
    pub argument_repairs: usize,
    pub unflattened_arguments: usize,
    pub dropped_duplicate_calls: usize,
    pub malformed_tool_calls: usize,
    pub warnings: Vec<String>,
}

impl ToolCallRepairReport {
    pub fn new(provider_family: ProviderProtocolFamily) -> Self {
        Self {
            provider_family: provider_family.label().to_string(),
            ..Self::default()
        }
    }

    pub fn has_repairs(&self) -> bool {
        self.schema_flattened_tools > 0
            || self.scavenged_tool_calls > 0
            || self.argument_repairs > 0
            || self.unflattened_arguments > 0
            || self.dropped_duplicate_calls > 0
            || self.malformed_tool_calls > 0
    }

    fn warn(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }
}

#[derive(Debug, Clone)]
pub struct RepairedToolCallResponse {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub report: Option<ToolCallRepairReport>,
}

pub fn prepare_tools_for_provider(
    tools: Vec<Tool>,
    provider_family: ProviderProtocolFamily,
    model: &str,
) -> Vec<Tool> {
    if !weak_tool_call_repair_enabled(provider_family, model) {
        return tools;
    }

    tools
        .into_iter()
        .map(|mut tool| {
            let shape = schema_shape(&tool.parameters);
            if shape.leaf_count <= 10 && shape.max_depth <= 2 {
                return tool;
            }
            let flattened = flatten_tool_schema(&tool.parameters);
            if flattened.flattened_fields == 0 {
                return tool;
            }
            tool.parameters = flattened.schema;
            tool.strict_schema = false;
            if !tool
                .description
                .contains("Nested parameters may be supplied with dot notation")
            {
                tool.description.push_str(
                    "\nNested parameters may be supplied with dot notation; the runtime will re-nest them before execution.",
                );
            }
            tool
        })
        .collect()
}

pub fn repair_response(
    raw_content: &str,
    tool_calls: Option<Vec<ToolCall>>,
    provider_family: ProviderProtocolFamily,
    mut report: ToolCallRepairReport,
) -> RepairedToolCallResponse {
    let mut calls = tool_calls.unwrap_or_default();
    let scavenged = scavenge_tool_calls(raw_content, &mut report);
    report.scavenged_tool_calls += scavenged.len();
    calls.extend(scavenged);

    for call in &mut calls {
        let unflattened = unflatten_dot_arguments(std::mem::take(&mut call.arguments));
        if unflattened.changed {
            report.unflattened_arguments += 1;
        }
        call.arguments = unflattened.value;
    }

    let calls = suppress_duplicate_calls(calls, &mut report);
    report.provider_family = provider_family.label().to_string();
    let report = report.has_repairs().then_some(report);

    RepairedToolCallResponse {
        content: sanitize_assistant_content(raw_content),
        tool_calls: (!calls.is_empty()).then_some(calls),
        report,
    }
}

pub fn parse_tool_arguments(raw: &str, report: &mut ToolCallRepairReport) -> Value {
    match serde_json::from_str::<Value>(raw.trim()) {
        Ok(value) => unflatten_dot_arguments(value).value,
        Err(first_error) => {
            if let Some(value) = repair_truncated_json(raw) {
                report.argument_repairs += 1;
                return unflatten_dot_arguments(value).value;
            }
            report.malformed_tool_calls += 1;
            report.warn(format!("tool argument JSON parse failed: {}", first_error));
            Value::Null
        }
    }
}

pub fn schema_flattening_report(
    tools: &[Tool],
    provider_family: ProviderProtocolFamily,
    model: &str,
) -> Option<ToolCallRepairReport> {
    if !weak_tool_call_repair_enabled(provider_family, model) {
        return None;
    }
    let mut report = ToolCallRepairReport::new(provider_family);
    for tool in tools {
        let shape = schema_shape(&tool.parameters);
        if shape.leaf_count <= 10 && shape.max_depth <= 2 {
            continue;
        }
        let flattened = flatten_tool_schema(&tool.parameters);
        if flattened.flattened_fields > 0 {
            report.schema_flattened_tools += 1;
            report.schema_flattened_fields += flattened.flattened_fields;
        }
    }
    report.has_repairs().then_some(report)
}

fn weak_tool_call_repair_enabled(provider_family: ProviderProtocolFamily, model: &str) -> bool {
    match std::env::var("PRIORITY_AGENT_WEAK_MODEL_TOOL_REPAIR") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => {
            matches!(
                provider_family,
                ProviderProtocolFamily::MiniMax | ProviderProtocolFamily::Kimi
            ) || weak_model_name(model)
        }
    }
}

fn weak_model_name(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    ["deepseek", "glm", "kimi", "moonshot", "minimax"]
        .iter()
        .any(|needle| model.contains(needle))
}

#[derive(Debug, Clone, Copy, Default)]
struct SchemaShape {
    leaf_count: usize,
    max_depth: usize,
}

fn schema_shape(schema: &Value) -> SchemaShape {
    fn walk(schema: &Value, depth: usize, shape: &mut SchemaShape) {
        let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
            shape.leaf_count += 1;
            shape.max_depth = shape.max_depth.max(depth);
            return;
        };
        if properties.is_empty() {
            shape.leaf_count += 1;
            shape.max_depth = shape.max_depth.max(depth);
            return;
        }
        for property in properties.values() {
            walk(property, depth + 1, shape);
        }
    }

    let mut shape = SchemaShape::default();
    walk(schema, 0, &mut shape);
    shape
}

#[derive(Debug, Clone)]
struct FlattenedSchema {
    schema: Value,
    flattened_fields: usize,
}

fn flatten_tool_schema(schema: &Value) -> FlattenedSchema {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return FlattenedSchema {
            schema: schema.clone(),
            flattened_fields: 0,
        };
    };

    let mut flat_properties = Map::new();
    flatten_properties("", properties, &mut flat_properties);
    if flat_properties.is_empty() {
        return FlattenedSchema {
            schema: schema.clone(),
            flattened_fields: 0,
        };
    }

    let mut flattened = schema.clone();
    if let Some(object) = flattened.as_object_mut() {
        object.insert(
            "properties".to_string(),
            Value::Object(flat_properties.clone()),
        );
        object.remove("required");
    }

    FlattenedSchema {
        schema: flattened,
        flattened_fields: flat_properties.len(),
    }
}

fn flatten_properties(prefix: &str, properties: &Map<String, Value>, out: &mut Map<String, Value>) {
    for (name, schema) in properties {
        let key = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        if let Some(nested) = schema.get("properties").and_then(Value::as_object) {
            flatten_properties(&key, nested, out);
        } else {
            out.insert(key, schema.clone());
        }
    }
}

#[derive(Debug, Clone)]
struct UnflattenedArguments {
    value: Value,
    changed: bool,
}

fn unflatten_dot_arguments(value: Value) -> UnflattenedArguments {
    let Value::Object(object) = value else {
        return UnflattenedArguments {
            value,
            changed: false,
        };
    };
    if !object.keys().any(|key| key.contains('.')) {
        return UnflattenedArguments {
            value: Value::Object(object),
            changed: false,
        };
    }

    let mut root = Map::new();
    let mut changed = false;
    for (key, value) in object {
        if key.contains('.') {
            insert_dotted(&mut root, &key, value);
            changed = true;
        } else {
            root.insert(key, value);
        }
    }

    UnflattenedArguments {
        value: Value::Object(root),
        changed,
    }
}

fn insert_dotted(root: &mut Map<String, Value>, key: &str, value: Value) {
    let mut parts = key.split('.').filter(|part| !part.is_empty()).peekable();
    let Some(first) = parts.next() else {
        return;
    };
    insert_dotted_parts(root, first, parts.collect::<Vec<_>>().as_slice(), value);
}

fn insert_dotted_parts(root: &mut Map<String, Value>, current: &str, rest: &[&str], value: Value) {
    if rest.is_empty() {
        root.insert(current.to_string(), value);
        return;
    }
    let entry = root
        .entry(current.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    let Some(object) = entry.as_object_mut() else {
        return;
    };
    insert_dotted_parts(object, rest[0], &rest[1..], value);
}

fn scavenge_tool_calls(raw_content: &str, report: &mut ToolCallRepairReport) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    calls.extend(scavenge_invoke_tags(raw_content, report));
    calls.extend(scavenge_json_tool_call_tags(raw_content, report));
    calls.extend(scavenge_dsml_function_calls(raw_content));
    calls
}

fn scavenge_invoke_tags(raw_content: &str, report: &mut ToolCallRepairReport) -> Vec<ToolCall> {
    let re = Regex::new(r#"(?is)<invoke\b[^>]*\bname\s*=\s*["']([^"']+)["'][^>]*>(.*?)</invoke>"#)
        .expect("valid invoke regex");
    re.captures_iter(raw_content)
        .enumerate()
        .filter_map(|(idx, captures)| {
            let name = captures.get(1)?.as_str().trim().to_string();
            if name.is_empty() {
                report.malformed_tool_calls += 1;
                return None;
            }
            let body = captures.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            let arguments = if body.is_empty() {
                Value::Object(Map::new())
            } else {
                parse_tool_arguments(body, report)
            };
            Some(ToolCall {
                id: format!("repair_call_{}", idx + 1),
                name,
                arguments,
            })
        })
        .collect()
}

fn scavenge_json_tool_call_tags(
    raw_content: &str,
    report: &mut ToolCallRepairReport,
) -> Vec<ToolCall> {
    let re = Regex::new(r#"(?is)<(?:minimax:)?tool_call\b[^>]*>(.*?)</(?:minimax:)?tool_call>"#)
        .expect("valid tool_call regex");
    re.captures_iter(raw_content)
        .enumerate()
        .filter_map(|(idx, captures)| {
            let body = captures.get(1)?.as_str();
            if body.to_ascii_lowercase().contains("<invoke") {
                return None;
            }
            parse_json_tool_call(body, idx, report)
        })
        .collect()
}

fn parse_json_tool_call(
    raw: &str,
    idx: usize,
    report: &mut ToolCallRepairReport,
) -> Option<ToolCall> {
    let value = parse_tool_arguments(raw, report);
    let object = value.as_object()?;
    let name = object
        .get("name")
        .or_else(|| object.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() {
        report.malformed_tool_calls += 1;
        return None;
    }
    let arguments = object
        .get("arguments")
        .or_else(|| object.get("args"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    Some(ToolCall {
        id: object
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("repair_json_call_{}", idx + 1)),
        name,
        arguments,
    })
}

/// Scavenge DSML-format tool calls leaked in non-thinking model responses.
/// DeepSeek models can emit DSML markup in regular turns:
///
/// ```text
/// 〈DSML｜function_calls〉
/// 〈DSML｜invoke name="file_read"〉
/// 〈DSML｜parameter name="path" string="true"〉file.md〈/DSML｜parameter〉
/// 〈/DSML｜invoke〉
/// 〈/DSML｜function_calls〉
/// ```
pub(crate) fn scavenge_dsml_function_calls(raw_content: &str) -> Vec<ToolCall> {
    let normalized = normalize_dsml_markup(raw_content);
    if !normalized.contains("〈DSML｜function_calls〉")
        && !normalized.contains("〈DSML｜tool_calls〉")
    {
        return Vec::new();
    }

    let re = Regex::new(
        r"(?s)〈DSML｜(?:function_calls|tool_calls)〉(.*?)〈/DSML｜(?:function_calls|tool_calls)〉",
    )
    .expect("valid DSML regex");
    let mut calls = Vec::new();

    for captures in re.captures_iter(&normalized) {
        let body = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        calls.extend(parse_dsml_invoke_block(body, calls.len()));
    }

    calls
}

/// Normalize the several DSML delimiter shapes DeepSeek-family models leak into
/// regular content so the existing full-width parser can consume them.
///
/// Supported shapes:
///   〈DSML｜...〉            (full-width, already canonical)
///   <|DSML|...>             (half-width, compact)
///   <| | DSML | | ...>      (half-width, spaced, as seen from deepseek-v4-flash)
fn normalize_dsml_markup(content: &str) -> String {
    // Open tags may carry attributes (e.g. name="bash") after the tag name.
    let open_re = Regex::new(r"<\|(?:\s*\|)?\s*[Dd][Ss][Mm][Ll]\s*\|(?:\s*\|)?\s*(\w+)([^>]*)>")
        .expect("valid open re");
    // Close tags may look like </|DSML|tag>, </| | DSML | | tag>, <|/DSML|tag>, etc.
    let close_re = Regex::new(r"<(?:/\|(?:\s*\|)?\s*[Dd][Ss][Mm][Ll]\s*\|(?:\s*\|)?\s*(\w+)\s*|\|(?:\s*\|)?\s*/\s*[Dd][Ss][Mm][Ll]\s*\|(?:\s*\|)?\s*(\w+)\s*)([^>]*)>")
        .expect("valid close re");
    let normalized = open_re.replace_all(content, "〈DSML｜$1$2〉");
    close_re
        .replace_all(&normalized, |caps: &regex::Captures| {
            let name = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
            let attrs = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            format!("〈/DSML｜{name}{attrs}〉")
        })
        .into_owned()
}

#[cfg(test)]
mod normalize_tests {
    use super::normalize_dsml_markup;

    #[test]
    fn normalizes_compact_half_width_dsml() {
        let input = r#"<|DSML|tool_calls><|DSML|invoke name="bash"><|DSML|parameter name="command">ls<|/DSML|parameter><|/DSML|invoke><|/DSML|tool_calls>"#;
        let normalized = normalize_dsml_markup(input);
        assert!(normalized.contains("〈DSML｜tool_calls〉"));
        assert!(normalized.contains("〈/DSML｜tool_calls〉"));
    }

    #[test]
    fn normalizes_spaced_half_width_dsml() {
        let input = r#"<| | DSML | | tool_calls><| | DSML | | invoke name="bash"><| | DSML | | parameter name="command">ls</| | DSML | | parameter></| | DSML | | invoke></| | DSML | | tool_calls>"#;
        let normalized = normalize_dsml_markup(input);
        assert!(normalized.contains("〈DSML｜tool_calls〉"));
        assert!(normalized.contains("〈/DSML｜tool_calls〉"));
    }
}

fn parse_dsml_invoke_block(body: &str, base_idx: usize) -> Vec<ToolCall> {
    // DeepSeek may emit self-closing parameter tags using the same shape as the
    // opening tag (e.g. `<|DSML|parameter>` instead of `</|DSML|parameter>`),
    // so the close delimiter accepts both `〈/DSML｜...〉` and `〈DSML｜...〉`.
    let invoke_re =
        Regex::new(r#"(?s)〈DSML｜invoke\s+name\s*=\s*"([^"]+)"〉(.*?)〈/?DSML｜invoke[^〉]*〉"#)
            .expect("valid DSML invoke regex");
    let param_re = Regex::new(
        r#"〈DSML｜parameter\s+name\s*=\s*"([^"]+)"[^〉]*〉(.*?)〈/?DSML｜parameter[^〉]*〉"#,
    )
    .expect("valid DSML param regex");

    let mut calls = Vec::new();

    for (idx, captures) in invoke_re.captures_iter(body).enumerate() {
        let name = captures.get(1).map(|m| m.as_str().trim().to_string());
        let Some(name) = name else { continue };
        if name.is_empty() {
            continue;
        }

        let params_body = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        let mut args = Map::new();
        for param in param_re.captures_iter(params_body) {
            let param_name = param.get(1).map(|m| m.as_str().trim().to_string());
            let param_value = param
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            if let Some(param_name) = param_name {
                if !param_name.is_empty() {
                    args.insert(param_name, Value::String(param_value));
                }
            }
        }

        calls.push(ToolCall {
            id: format!("dsml_repair_call_{}", base_idx + idx + 1),
            name,
            arguments: Value::Object(args),
        });
    }

    calls
}

fn suppress_duplicate_calls(
    calls: Vec<ToolCall>,
    report: &mut ToolCallRepairReport,
) -> Vec<ToolCall> {
    let mut seen = HashSet::new();
    let mut kept = Vec::with_capacity(calls.len());
    for call in calls {
        let key = format!(
            "{}\n{}",
            call.name,
            serde_json::to_string(&call.arguments).unwrap_or_default()
        );
        if seen.insert(key) {
            kept.push(call);
        } else {
            report.dropped_duplicate_calls += 1;
        }
    }
    kept
}

/// Repair truncated JSON arguments by closing open brackets, strings, and
/// fixing common model truncation artifacts (trailing commas, dangling keys).
/// Returns `None` only when the input is completely unrecoverable.
///
/// Mirrors Reasonix's `repairTruncatedJson` in `src/repair/truncation.ts`.
fn repair_truncated_json(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some(Value::Object(Map::new()));
    }
    // Fast path: already valid JSON.
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Some(value);
    }
    // Repair path: close brackets, trim commas, fill dangling keys.
    let repaired = close_brackets_and_strings(trimmed);
    if let Ok(value) = serde_json::from_str(&repaired) {
        return Some(value);
    }
    // Hard fallback — all repair attempts failed. Return `{}` so the
    // parse error is more informative than "missing required parameter".
    Some(Value::Object(Map::new()))
}

/// Close open brackets, unterminated strings, trim trailing commas,
/// and fill dangling object keys with `null`.
///
/// Mirrors Reasonix's per-character stack-based repair in
/// `repairTruncatedJson` (truncation.ts).
fn close_brackets_and_strings(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len() + 16);
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escape_next = false;
    // Track position of the last non-whitespace char for trailing comma trim.
    let mut last_significant = 0_usize;

    for (i, ch) in raw.char_indices() {
        result.push(ch);
        if !ch.is_ascii_whitespace() {
            last_significant = i + ch.len_utf8();
        }
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => stack.push('}'),
            '[' if !in_string => stack.push(']'),
            '}' | ']' if !in_string && stack.last() == Some(&ch) => {
                stack.pop();
            }
            _ => {}
        }
    }

    // Trim trailing comma — model output is often cut mid-list: `{"a": 1,`
    let significant = &raw[..last_significant.min(raw.len())];
    if significant.ends_with(',') {
        result.truncate(result.len() - 1);
    }

    // Fill dangling key — model was cut after `"key":` with no value.
    if result.ends_with(':') {
        result.push_str(" null");
    }

    // Close unterminated string.
    if in_string {
        result.push('"');
    }

    // Pop remaining open structures in reverse order.
    while let Some(closer) = stack.pop() {
        result.push(closer);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report() -> ToolCallRepairReport {
        ToolCallRepairReport::new(ProviderProtocolFamily::MiniMax)
    }

    #[test]
    fn scavenges_invoke_tool_call_from_hidden_block() {
        let repaired = repair_response(
            r#"before <think><invoke name="file_read">{"path":"Cargo.toml"}</invoke></think> after"#,
            None,
            ProviderProtocolFamily::MiniMax,
            report(),
        );

        let calls = repaired.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "file_read");
        assert_eq!(calls[0].arguments, json!({"path": "Cargo.toml"}));
        assert_eq!(repaired.content.trim(), "before  after");
        assert_eq!(repaired.report.unwrap().scavenged_tool_calls, 1);
    }

    #[test]
    fn scavenges_half_width_spaced_dsml_tool_call() {
        let raw = r#"我来检查。
<| | DSML | | tool_calls>
<| | DSML | | invoke name="bash">
<| | DSML | | parameter name="command" string="true">ls -la ~/Desktop/phageGPT/<| | DSML | | parameter>
<| | DSML | | parameter name="description" string="true">List project root<| | DSML | | parameter>
</| | DSML | | invoke>
</| | DSML | | tool_calls>
Done."#;

        let repaired = repair_response(
            raw,
            None,
            ProviderProtocolFamily::OpenAiCompatible,
            report(),
        );

        let calls = repaired.tool_calls.expect("tool calls scavenged");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].arguments["command"], "ls -la ~/Desktop/phageGPT/");
        assert_eq!(calls[0].arguments["description"], "List project root");
        assert_eq!(repaired.content.trim(), "我来检查。\n\nDone.");
        assert_eq!(repaired.report.unwrap().scavenged_tool_calls, 1);
    }

    #[test]
    fn scavenges_compact_half_width_dsml_tool_call() {
        let raw = r#"Before <|DSML|tool_calls><|DSML|invoke name="bash"><|DSML|parameter name="command">ls<|/DSML|parameter><|/DSML|invoke><|/DSML|tool_calls> After"#;

        let repaired = repair_response(
            raw,
            None,
            ProviderProtocolFamily::OpenAiCompatible,
            report(),
        );

        let calls = repaired.tool_calls.expect("tool calls scavenged");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].arguments["command"], "ls");
        assert_eq!(repaired.content, "Before  After");
        assert_eq!(repaired.report.unwrap().scavenged_tool_calls, 1);
    }

    #[test]
    fn scavenges_full_width_dsml_without_visible_markup() {
        let raw = "Before\n〈DSML｜tool_calls〉\n〈DSML｜invoke name=\"bash\"〉\n〈DSML｜parameter name=\"command\" string=\"true\"〉pwd〈/DSML｜parameter〉\n〈/DSML｜invoke〉\n〈/DSML｜tool_calls〉\nAfter";

        let repaired = repair_response(
            raw,
            None,
            ProviderProtocolFamily::OpenAiCompatible,
            report(),
        );

        let calls = repaired.tool_calls.expect("tool calls scavenged");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[0].arguments["command"], "pwd");
        assert_eq!(repaired.content.trim(), "Before\n\nAfter");
        assert_eq!(repaired.report.unwrap().scavenged_tool_calls, 1);
    }

    #[test]
    fn repairs_truncated_arguments() {
        let mut report = report();
        let value = parse_tool_arguments(r#"{"path":"Cargo.toml""#, &mut report);

        assert_eq!(value, json!({"path": "Cargo.toml"}));
        assert_eq!(report.argument_repairs, 1);
    }

    #[test]
    fn unrecoverable_arguments_fallback_to_empty_object() {
        let mut report = report();
        let value = parse_tool_arguments(r#"{"path": [1,}"#, &mut report);

        // Falls back to {} after repair attempt — argument_repairs still counted.
        assert_eq!(value, json!({}));
        assert_eq!(report.argument_repairs, 1);
    }

    #[test]
    fn repairs_trailing_comma() {
        let mut report = report();
        let value = parse_tool_arguments(r#"{"path":"Cargo.toml","#, &mut report);

        assert_eq!(value, json!({"path": "Cargo.toml"}));
        assert_eq!(report.argument_repairs, 1);
    }

    #[test]
    fn repairs_dangling_key() {
        let mut report = report();
        let value = parse_tool_arguments(r#"{"path":"Cargo.toml","limit":"#, &mut report);

        assert_eq!(value, json!({"path": "Cargo.toml", "limit": null}));
        assert_eq!(report.argument_repairs, 1);
    }

    #[test]
    fn repairs_truncated_mid_string() {
        let mut report = report();
        let value = parse_tool_arguments(r#"{"path":"/Users/ge"#, &mut report);

        assert_eq!(value, json!({"path": "/Users/ge"}));
        assert_eq!(report.argument_repairs, 1);
    }

    #[test]
    fn unflattens_dotted_arguments() {
        let mut report = report();
        let value = parse_tool_arguments(
            r#"{"patch.old_string":"old","patch.new_string":"new","path":"src/lib.rs"}"#,
            &mut report,
        );

        assert_eq!(
            value,
            json!({"patch": {"old_string": "old", "new_string": "new"}, "path": "src/lib.rs"})
        );
    }

    #[test]
    fn suppresses_duplicate_calls_in_same_response() {
        let call = ToolCall {
            id: "a".to_string(),
            name: "grep".to_string(),
            arguments: json!({"pattern": "foo"}),
        };
        let repaired = repair_response(
            "",
            Some(vec![call.clone(), call]),
            ProviderProtocolFamily::Kimi,
            report(),
        );

        assert_eq!(repaired.tool_calls.unwrap().len(), 1);
        assert_eq!(repaired.report.unwrap().dropped_duplicate_calls, 1);
    }

    #[test]
    fn flattens_complex_schema_for_weak_models() {
        let tool = Tool::new("complex", "complex").with_parameters(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "patch": {
                    "type": "object",
                    "properties": {
                        "body": {
                            "type": "object",
                            "properties": {
                                "old_string": {"type": "string"},
                                "new_string": {"type": "string"}
                            }
                        }
                    }
                }
            }
        }));

        let tools =
            prepare_tools_for_provider(vec![tool], ProviderProtocolFamily::MiniMax, "MiniMax-M3");
        let properties = tools[0].parameters["properties"].as_object().unwrap();

        assert!(properties.contains_key("path"));
        assert!(properties.contains_key("patch.body.old_string"));
        assert!(properties.contains_key("patch.body.new_string"));
    }
}

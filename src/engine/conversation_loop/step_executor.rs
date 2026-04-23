//! 工作流步骤执行器
//!
//! 将工作流计划步骤转换为工具调用并执行。

use crate::engine::workflow::StepExecutor;
use crate::services::api::{ChatRequest, LlmProvider, Message};
use crate::tools::{ToolContext, ToolRegistry};
use anyhow::Result;
use std::sync::Arc;
use tracing::warn;

use super::safe_prefix_by_bytes;

#[derive(Clone)]
pub(crate) struct WorkflowRealStepExecutor {
    pub(crate) tool_registry: Arc<ToolRegistry>,
    pub(crate) llm_provider: Arc<dyn LlmProvider>,
    pub(crate) model: String,
    pub(crate) base_context: ToolContext,
}

#[async_trait::async_trait]
impl StepExecutor for WorkflowRealStepExecutor {
    async fn execute_step(&self, step: &crate::engine::plan_mode::PlanStep) -> Result<String, String> {
        let Some(tool_name) = step.tool.as_deref() else {
            return Ok(format!(
                "[workflow] non-executable planning step: {}",
                step.description
            ));
        };
        let Some(tool) = self.tool_registry.get(tool_name) else {
            return Ok(format!(
                "[workflow] tool '{}' unavailable, kept as planning note: {}",
                tool_name, step.description
            ));
        };

        let params = self
            .build_params(step, tool.parameters())
            .await
            .map_err(|e| format!("build params failed for tool '{}': {}", tool_name, e))?;

        if let Some(err) = tool.validate_params(&params) {
            return Err(format!("invalid params for '{}': {}", tool_name, err));
        }

        let result = tool.execute(params, self.base_context.clone()).await;
        if result.success {
            Ok(format!("[{}] {}", tool_name, result.content))
        } else {
            Err(format!(
                "[{}] {}",
                tool_name,
                result.error.clone().unwrap_or(result.content)
            ))
        }
    }
}

impl WorkflowRealStepExecutor {
    async fn build_params(
        &self,
        step: &crate::engine::plan_mode::PlanStep,
        schema: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        if let Ok(params) = self.tool_specific_params(step) {
            return Self::normalize_params(params, &schema, step);
        }
        if let Ok(params) = self.llm_build_params(step, &schema).await {
            return Self::normalize_params(params, &schema, step);
        }
        self.fallback_params(step, &schema)
    }

    fn tool_specific_params(
        &self,
        step: &crate::engine::plan_mode::PlanStep,
    ) -> Result<serde_json::Value, String> {
        let tool = step
            .tool
            .as_deref()
            .ok_or_else(|| "missing tool".to_string())?;
        match tool {
            "file_read" => {
                let path = guess_path(&step.description).unwrap_or_else(|| "README.md".to_string());
                Ok(serde_json::json!({ "path": path }))
            }
            "file_write" => {
                let path = guess_path(&step.description).unwrap_or_else(|| "notes.md".to_string());
                let content = infer_file_write_content(&step.description, &path);
                Ok(serde_json::json!({ "path": path, "content": content }))
            }
            "grep" => {
                let pattern = extract_quoted(&step.description)
                    .unwrap_or_else(|| first_keyword(&step.description).unwrap_or("TODO").to_string());
                let path = guess_path(&step.description).unwrap_or_else(|| ".".to_string());
                Ok(serde_json::json!({ "pattern": pattern, "path": path }))
            }
            "glob" => {
                let pattern = guess_glob_pattern(&step.description);
                let path = guess_path(&step.description).unwrap_or_else(|| ".".to_string());
                Ok(serde_json::json!({ "pattern": pattern, "path": path }))
            }
            "bash" => {
                let command = extract_command(&step.description)
                    .unwrap_or_else(|| format!("echo {}", shell_safe_echo(&step.description)));
                Ok(serde_json::json!({ "command": command, "timeout": 60 }))
            }
            "project_list" => {
                let (action, query) = infer_project_list_action(&step.description);
                match query {
                    Some(q) => Ok(serde_json::json!({ "action": action, "query": q, "limit": 30 })),
                    None => Ok(serde_json::json!({ "action": action, "limit": 30 })),
                }
            }
            "memory_save" => {
                let category = infer_memory_category(&step.description);
                let content = extract_backtick(&step.description)
                    .or_else(|| extract_quoted(&step.description))
                    .unwrap_or_else(|| step.description.clone());
                Ok(serde_json::json!({ "content": content, "category": category }))
            }
            "todo_write" => {
                let todos = infer_todo_items(&step.description);
                Ok(serde_json::json!({ "todos": todos }))
            }
            "json_query" => Ok(infer_json_query_params(&step.description)),
            "file_edit" => {
                let path = guess_path(&step.description)
                    .ok_or_else(|| "file_edit requires explicit path in step description".to_string())?;
                if let Some((old_s, new_s)) = extract_replace_triplet(&step.description) {
                    Ok(serde_json::json!({
                        "path": path,
                        "old_string": old_s,
                        "new_string": new_s
                    }))
                } else {
                    Err("file_edit requires quoted old/new strings in step description".to_string())
                }
            }
            _ => Err("no dedicated planner for tool".to_string()),
        }
    }

    async fn llm_build_params(
        &self,
        step: &crate::engine::plan_mode::PlanStep,
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        const MAX_SCHEMA_HINT_BYTES: usize = 2048;
        const MAX_PROMPT_BYTES: usize = 4096;
        let schema_hint = build_schema_hint(schema, MAX_SCHEMA_HINT_BYTES);
        let prompt = format!(
            "你是工具参数生成器。根据 step 描述和 schema 摘要生成工具参数。\n\
             只输出 JSON object，不要 markdown，不要解释。\n\
             step: {}\n\
             tool: {}\n\
             schema_hint: {}\n\
             输出:",
            step.description,
            step.tool.as_deref().unwrap_or("unknown"),
            schema_hint
        );
        let prompt = if prompt.len() > MAX_PROMPT_BYTES {
            warn!(
                "llm_build_params prompt truncated: {} -> {} bytes (tool={})",
                prompt.len(),
                MAX_PROMPT_BYTES,
                step.tool.as_deref().unwrap_or("unknown")
            );
            safe_prefix_by_bytes(&prompt, MAX_PROMPT_BYTES).to_string()
        } else {
            prompt
        };
        let mut req = ChatRequest::new(&self.model)
            .with_messages(vec![
                Message::system("只输出严格 JSON 对象，禁止多余文本。"),
                Message::user(&prompt),
            ])
            .with_temperature(0.0);
        req.max_tokens = Some(300);
        let resp = self
            .llm_provider
            .chat(req)
            .await
            .map_err(|e| format!("llm error: {}", e))?;
        serde_json::from_str::<serde_json::Value>(resp.content.trim())
            .map_err(|e| format!("invalid json from llm: {}", e))
    }

    fn fallback_params(
        &self,
        step: &crate::engine::plan_mode::PlanStep,
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mut map = serde_json::Map::new();
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        for field in required {
            let Some(key) = field.as_str() else {
                continue;
            };
            let ty = props
                .get(key)
                .and_then(|p| p.get("type"))
                .and_then(|x| x.as_str())
                .unwrap_or("string");
            let value = match ty {
                "string" => {
                    if key.contains("path") {
                        guess_path(&step.description)
                            .map(serde_json::Value::String)
                            .unwrap_or_else(|| serde_json::Value::String(".".to_string()))
                    } else if key == "command" {
                        serde_json::Value::String(step.description.clone())
                    } else if key == "pattern" {
                        serde_json::Value::String(
                            step.description
                                .split_whitespace()
                                .next()
                                .unwrap_or("TODO")
                                .to_string(),
                        )
                    } else {
                        serde_json::Value::String(step.description.clone())
                    }
                }
                "integer" | "number" => serde_json::Value::Number(1.into()),
                "boolean" => serde_json::Value::Bool(true),
                "array" => serde_json::Value::Array(vec![]),
                "object" => serde_json::Value::Object(serde_json::Map::new()),
                _ => serde_json::Value::String(step.description.clone()),
            };
            map.insert(key.to_string(), value);
        }

        if map.is_empty() {
            return Err("no required params and llm synthesis failed".to_string());
        }
        Ok(serde_json::Value::Object(map))
    }

    pub(crate) fn normalize_params(
        params: serde_json::Value,
        schema: &serde_json::Value,
        step: &crate::engine::plan_mode::PlanStep,
    ) -> Result<serde_json::Value, String> {
        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut map = match params {
            serde_json::Value::Object(m) => m,
            _ => {
                return Err("tool params must be a JSON object".to_string());
            }
        };

        for field in &required {
            let Some(key) = field.as_str() else {
                continue;
            };
            if !map.contains_key(key) || map.get(key).is_some_and(serde_json::Value::is_null) {
                map.insert(
                    key.to_string(),
                    Self::default_value_for_type(key, props.get(key), step),
                );
                continue;
            }

            if let Some(current) = map.get(key).cloned() {
                let coerced = Self::coerce_param_type(current, props.get(key), step, key);
                map.insert(key.to_string(), coerced);
            }
        }

        Ok(serde_json::Value::Object(map))
    }

    fn coerce_param_type(
        value: serde_json::Value,
        prop_schema: Option<&serde_json::Value>,
        step: &crate::engine::plan_mode::PlanStep,
        key: &str,
    ) -> serde_json::Value {
        let ty = prop_schema
            .and_then(|p| p.get("type"))
            .and_then(|x| x.as_str())
            .unwrap_or("string");

        match ty {
            "string" => match value {
                serde_json::Value::String(_) => value,
                serde_json::Value::Number(n) => serde_json::Value::String(n.to_string()),
                serde_json::Value::Bool(b) => serde_json::Value::String(b.to_string()),
                _ => Self::default_value_for_type(key, prop_schema, step),
            },
            "integer" | "number" => match value {
                serde_json::Value::Number(_) => value,
                serde_json::Value::String(s) => s
                    .trim()
                    .parse::<i64>()
                    .ok()
                    .map(|i| serde_json::Value::Number(i.into()))
                    .unwrap_or_else(|| Self::default_value_for_type(key, prop_schema, step)),
                _ => Self::default_value_for_type(key, prop_schema, step),
            },
            "boolean" => match value {
                serde_json::Value::Bool(_) => value,
                serde_json::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => serde_json::Value::Bool(true),
                    "false" | "0" | "no" | "off" => serde_json::Value::Bool(false),
                    _ => Self::default_value_for_type(key, prop_schema, step),
                },
                _ => Self::default_value_for_type(key, prop_schema, step),
            },
            "array" => match value {
                serde_json::Value::Array(_) => value,
                _ => serde_json::Value::Array(vec![]),
            },
            "object" => match value {
                serde_json::Value::Object(_) => value,
                _ => serde_json::Value::Object(serde_json::Map::new()),
            },
            _ => value,
        }
    }

    fn default_value_for_type(
        key: &str,
        prop_schema: Option<&serde_json::Value>,
        step: &crate::engine::plan_mode::PlanStep,
    ) -> serde_json::Value {
        let ty = prop_schema
            .and_then(|p| p.get("type"))
            .and_then(|x| x.as_str())
            .unwrap_or("string");
        match ty {
            "string" => {
                if key.contains("path") {
                    guess_path(&step.description)
                        .map(serde_json::Value::String)
                        .unwrap_or_else(|| serde_json::Value::String(".".to_string()))
                } else if key == "command" {
                    extract_command(&step.description)
                        .map(serde_json::Value::String)
                        .unwrap_or_else(|| serde_json::Value::String(step.description.clone()))
                } else if key == "pattern" {
                    serde_json::Value::String(
                        first_keyword(&step.description)
                            .unwrap_or("TODO")
                            .to_string(),
                    )
                } else {
                    serde_json::Value::String(step.description.clone())
                }
            }
            "integer" | "number" => serde_json::Value::Number(1.into()),
            "boolean" => serde_json::Value::Bool(true),
            "array" => serde_json::Value::Array(vec![]),
            "object" => serde_json::Value::Object(serde_json::Map::new()),
            _ => serde_json::Value::String(step.description.clone()),
        }
    }
}

// ─── 文本解析辅助函数 ─────────────────────────────────────────────

fn guess_path(desc: &str) -> Option<String> {
    for token in desc.split_whitespace() {
        let t = token.trim_matches(|c: char| ",.;:()[]{}\"'`".contains(c));
        if t.contains('/') || t.contains(".rs") || t.contains(".md") || t.contains(".toml") {
            return Some(t.to_string());
        }
    }
    for token in desc.split_whitespace() {
        let t = token.trim_matches(|c: char| ",.;:()[]{}\"'`".contains(c));
        if matches!(
            t,
            "src" | "docs" | "tests" | "test" | "examples" | "scripts" | "crates"
        ) {
            return Some(t.to_string());
        }
    }
    None
}

fn extract_quoted(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut start = None;
    for (i, ch) in chars.iter().enumerate() {
        if *ch == '"' || *ch == '\'' {
            if let Some((idx, quote)) = start {
                if *ch == quote {
                    let content: String = chars[idx + 1..i].iter().collect();
                    if !content.trim().is_empty() {
                        return Some(content);
                    }
                    start = None;
                }
            } else {
                start = Some((i, *ch));
            }
        }
    }
    None
}

fn first_keyword(s: &str) -> Option<&str> {
    s.split_whitespace()
        .map(|w| w.trim_matches(|c: char| ",.;:()[]{}\"'`".contains(c)))
        .find(|w| !w.is_empty() && w.len() > 2)
}

fn guess_glob_pattern(step: &str) -> String {
    if let Some(q) = extract_quoted(step) {
        if q.contains('*') || q.contains('?') || q.contains('[') {
            return q;
        }
    }
    for token in step.split_whitespace() {
        let t = token.trim_matches(|c: char| ",.;:()[]{}\"'`".contains(c));
        if t.contains('*') || t.contains('?') || t.contains('[') {
            return t.to_string();
        }
    }
    let lower = step.to_lowercase();
    if lower.contains("rust") || lower.contains(".rs") {
        return "**/*.rs".to_string();
    }
    if lower.contains("markdown") || lower.contains(".md") {
        return "**/*.md".to_string();
    }
    if lower.contains("test") {
        return "**/*test*".to_string();
    }
    "**/*".to_string()
}

fn infer_file_write_content(step: &str, path: &str) -> String {
    if let Some(block) = extract_backtick(step) {
        return block;
    }
    if let Some(q) = extract_quoted(step) {
        return q;
    }
    format!(
        "# Auto-generated content\n\nSource step: {}\nTarget path: {}\n",
        step, path
    )
}

fn infer_project_list_action(step: &str) -> (&'static str, Option<String>) {
    let lower = step.to_lowercase();
    if lower.contains("refresh") || lower.contains("刷新") || lower.contains("重建索引") {
        return ("refresh", None);
    }
    if lower.contains("summary")
        || lower.contains("概览")
        || lower.contains("项目结构")
        || lower.contains("目录结构")
    {
        return ("summary", None);
    }
    if lower.contains("dir ") || lower.contains("目录") || lower.contains("文件夹") {
        let query = extract_quoted(step).or_else(|| guess_path(step));
        return ("dir", query);
    }
    if lower.contains("search") || lower.contains("查找") || lower.contains("搜索") || lower.contains("fuzzy") {
        let query = extract_quoted(step).or_else(|| first_keyword(step).map(str::to_string));
        return ("search", query);
    }
    if lower.contains("list") || lower.contains("列出") {
        return ("list", None);
    }
    ("summary", None)
}

fn infer_memory_category(step: &str) -> &'static str {
    let lower = step.to_lowercase();
    if lower.contains("偏好") || lower.contains("preference") {
        "preference"
    } else if lower.contains("规范") || lower.contains("约定") || lower.contains("convention") {
        "convention"
    } else if lower.contains("决策") || lower.contains("decision") {
        "decision"
    } else {
        "note"
    }
}

fn infer_todo_items(step: &str) -> Vec<serde_json::Value> {
    let mut todos = Vec::new();
    let raw = extract_backtick(step)
        .or_else(|| extract_quoted(step))
        .unwrap_or_else(|| step.to_string());
    for part in raw
        .split(['\n', ';', '；', ',', '，'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(5)
    {
        let lower = part.to_lowercase();
        let status = if lower.contains("完成") || lower.contains("done") || lower.contains("completed")
        {
            "completed"
        } else if lower.contains("进行中") || lower.contains("in progress") {
            "in_progress"
        } else {
            "pending"
        };
        let priority = if lower.contains("高优") || lower.contains("high") {
            "high"
        } else if lower.contains("低优") || lower.contains("low") {
            "low"
        } else {
            "medium"
        };
        todos.push(serde_json::json!({
            "content": part,
            "status": status,
            "priority": priority
        }));
    }
    if todos.is_empty() {
        todos.push(serde_json::json!({
            "content": step,
            "status": "pending",
            "priority": "medium"
        }));
    }
    todos
}

fn infer_json_query_params(step: &str) -> serde_json::Value {
    let lower = step.to_lowercase();
    let action = if lower.contains("validate") || lower.contains("校验") {
        "validate"
    } else if lower.contains("format") || lower.contains("格式化") {
        "format"
    } else if lower.contains("set ") || lower.contains("设置") || lower.contains("修改字段") {
        "set"
    } else {
        "get"
    };

    let json_str = extract_backtick(step)
        .or_else(|| extract_quoted(step))
        .filter(|s| s.trim_start().starts_with('{') || s.trim_start().starts_with('['))
        .unwrap_or_else(|| "{}".to_string());

    let path = if action == "get" || action == "set" {
        extract_path_hint(step).unwrap_or_else(|| "data".to_string())
    } else {
        String::new()
    };

    if action == "set" {
        serde_json::json!({
            "action": action,
            "json": json_str,
            "path": path,
            "value": "null"
        })
    } else if path.is_empty() {
        serde_json::json!({
            "action": action,
            "json": json_str
        })
    } else {
        serde_json::json!({
            "action": action,
            "json": json_str,
            "path": path
        })
    }
}

fn extract_path_hint(step: &str) -> Option<String> {
    for token in step.split_whitespace() {
        let t = token.trim_matches(|c: char| ",.;:()[]{}\"'`".contains(c));
        if t.contains('.') && !t.contains('/') && !t.starts_with("http") {
            return Some(t.to_string());
        }
    }
    None
}

fn build_schema_hint(schema: &serde_json::Value, max_bytes: usize) -> String {
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    let mut prop_hints = Vec::new();
    let props = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    for (name, prop) in props.iter().take(16) {
        let ty = prop
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let enum_vals = prop
            .get("enum")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().take(6).filter_map(|x| x.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        let default = prop.get("default").cloned().unwrap_or(serde_json::Value::Null);
        let desc = prop
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        prop_hints.push(serde_json::json!({
            "name": name,
            "type": ty,
            "required": required.iter().any(|r| r == name),
            "enum": enum_vals,
            "default": default,
            "description": safe_prefix_by_bytes(desc, 120),
        }));
    }
    let hint = serde_json::json!({
        "required": required,
        "properties": prop_hints,
    });
    let raw = serde_json::to_string(&hint).unwrap_or_else(|_| "{}".to_string());
    if raw.len() <= max_bytes {
        raw
    } else {
        safe_prefix_by_bytes(&raw, max_bytes).to_string()
    }
}

fn extract_command(step: &str) -> Option<String> {
    let lower = step.to_lowercase();
    for marker in ["`", "cmd:", "command:"] {
        if marker == "`" {
            if let Some(cmd) = extract_backtick(step) {
                return Some(cmd);
            }
        } else if let Some(idx) = lower.find(marker) {
            let cmd = step[idx + marker.len()..].trim();
            if !cmd.is_empty() {
                return Some(cmd.to_string());
            }
        }
    }

    if lower.contains("cargo test") {
        return Some("cargo test".to_string());
    }
    if lower.contains("cargo check") {
        return Some("cargo check".to_string());
    }
    if lower.contains("cargo clippy") {
        return Some("cargo clippy".to_string());
    }
    if lower.contains("git status") {
        return Some("git status".to_string());
    }
    None
}

fn extract_backtick(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut start = None;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'`' {
            if let Some(st) = start {
                if i > st + 1 {
                    let cmd = &s[st + 1..i];
                    if !cmd.trim().is_empty() {
                        return Some(cmd.trim().to_string());
                    }
                }
                start = None;
            } else {
                start = Some(i);
            }
        }
    }
    None
}

fn extract_replace_triplet(s: &str) -> Option<(String, String)> {
    let lower = s.to_lowercase();
    let replace_pos = lower.find("replace")?;
    let with_pos = lower[replace_pos..].find(" with ")? + replace_pos;
    let before = s[replace_pos + "replace".len()..with_pos].trim();
    let after = s[with_pos + " with ".len()..].trim();
    let old_s = extract_quoted(before)?;
    let new_s = extract_quoted(after)?;
    Some((old_s, new_s))
}

fn shell_safe_echo(s: &str) -> String {
    let cleaned = s.replace('\n', " ").replace('"', "\\\"");
    format!("\"{}\"", cleaned)
}

pub(crate) fn is_drift_interruption_signal(user_input: &str) -> bool {
    let lower = user_input.to_lowercase();
    let markers = [
        "跑偏",
        "不是重点",
        "先别",
        "先不要",
        "停一下",
        "stop that",
        "off track",
        "wrong focus",
        "not the point",
    ];
    markers.iter().any(|m| lower.contains(m))
}

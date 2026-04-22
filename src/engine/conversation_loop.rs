//! 统一对话循环
//!
//! 将 QueryEngine 和 StreamingEngineInner 中重复的工具调用循环合并为一处。
//! 支持流式/非流式两种输出模式，内部逻辑完全一致。
//!
//! 改进（借鉴 hermes-agent）：
//! - 前置压缩（Preflight）：循环前检查总 token，超阈值提前压缩
//! - IterationBudget：迭代预算退还机制（只读工具可退还）

use crate::engine::workflow::{Gate, StepExecutor, WorkflowEngine, WorkflowPolicy};
use crate::services::api::{ChatRequest, ChatResponse, LlmProvider, Message, ToolCall};
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use anyhow::Result;
use futures::StreamExt;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, warn};

use super::context_compressor::{
    estimate_messages_tokens, estimate_tool_schemas_tokens, ContextCompressor,
};
use super::hooks::{HookDecision, ToolHookManager};
use super::streaming::StreamEvent;

/// 只读工具列表（不消耗迭代预算，可并发执行）
const READ_ONLY_TOOLS: &[&str] = &[
    "grep",
    "glob",
    "file_read",
    "project_list",
    "memory_load",
    "skills_list",
    "skill_view",
    "web_search",
];

const DEFAULT_READ_ONLY_TOOL_CONCURRENCY: usize = 8;

/// 工具结果截断阈值（字节），超过此值会截断并写入磁盘
const TOOL_RESULT_TRUNCATE_THRESHOLD: usize = 32 * 1024; // 32 KiB
/// 工具结果磁盘缓存目录
fn tool_result_cache_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("priority-agent")
        .join("tool-results")
}

fn read_only_tool_concurrency() -> usize {
    std::env::var("PRIORITY_AGENT_READ_ONLY_TOOL_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_READ_ONLY_TOOL_CONCURRENCY)
}

fn safe_prefix_by_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn safe_suffix_by_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut start = s.len().saturating_sub(max_bytes);
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

#[derive(Clone)]
struct WorkflowRealStepExecutor {
    tool_registry: Arc<ToolRegistry>,
    llm_provider: Arc<dyn LlmProvider>,
    model: String,
    base_context: ToolContext,
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
        // 先用 LLM 根据 tool schema 生成参数（真实执行路径）
        if let Ok(params) = self.llm_build_params(step, &schema).await {
            return Self::normalize_params(params, &schema, step);
        }
        // LLM 失败时回退到 schema 驱动的最小参数
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

    fn normalize_params(
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
    // 约定：... replace 'old' with 'new' ...
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

fn is_drift_interruption_signal(user_input: &str) -> bool {
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

/// 截断工具结果，如果超过阈值则写入磁盘
fn truncate_tool_result(result: &mut ToolResult, tool_name: &str, tool_call_id: &str) {
    if result.content.len() > TOOL_RESULT_TRUNCATE_THRESHOLD {
        let cache_dir = tool_result_cache_dir();
        // 忽略 mkdir 错误（权限问题等）
        let _ = std::fs::create_dir_all(&cache_dir);

        let filename = format!(
            "{}_{}_{}.txt",
            tool_name,
            tool_call_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let file_path = cache_dir.join(&filename);

        if std::fs::write(&file_path, &result.content).is_ok() {
            let original = result.content.clone();
            let original_len = original.len();
            let target_half_bytes = 2048.min(original_len / 2);
            let first = safe_prefix_by_bytes(&original, target_half_bytes);
            let last = safe_suffix_by_bytes(&original, target_half_bytes);
            result.content = format!(
                "[Output truncated: {} bytes -> saved to {}]\n\n--- First {} bytes ---\n{}\n\n--- Last {} bytes ---\n{}",
                original_len,
                file_path.display(),
                first.len(),
                first,
                last.len(),
                last
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub tool_call: ToolCall,
    pub prompt: String,
}

/// 待审批的工具请求 + 响应通道
type PendingApproval = Option<(ToolApprovalRequest, tokio::sync::oneshot::Sender<bool>)>;

/// 工具授权通道（类似 PlanApprovalChannel）
pub struct ToolApprovalChannel {
    pending: Arc<Mutex<PendingApproval>>,
}

impl ToolApprovalChannel {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// 提交授权请求并等待响应（60 秒超时）
    pub async fn submit(&self, request: ToolApprovalRequest) -> anyhow::Result<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            *pending = Some((request, tx));
        }
        match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
            Ok(result) => result.map_err(|_| anyhow::anyhow!("Approval channel closed")),
            Err(_) => Err(anyhow::anyhow!("Tool approval timed out after 60 seconds")),
        }
    }

    /// TUI 取出待审批的请求
    pub async fn take_pending(
        &self,
    ) -> Option<(ToolApprovalRequest, tokio::sync::oneshot::Sender<bool>)> {
        let mut pending = self.pending.lock().await;
        pending.take()
    }

    /// 是否有待审批的请求
    pub async fn has_pending(&self) -> bool {
        self.pending.lock().await.is_some()
    }
}

impl Default for ToolApprovalChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// 统一对话循环
pub struct ConversationLoop {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
    model: String,
    /// 会话 ID（固定，用于追踪 checkpoint、记忆等）
    session_id: String,
    max_iterations: usize,
    agent_manager: Option<Arc<crate::agent::AgentManager>>,
    mcp_manager: Option<Arc<crate::engine::mcp::McpManager>>,
    lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    hook_manager: Option<Arc<ToolHookManager>>,
    /// 上下文压缩器
    compressor: Option<Mutex<ContextCompressor>>,
    /// 记忆管理器（预取 + 围栏注入 + 同步）
    memory_manager: Option<Arc<Mutex<crate::memory::MemoryManager>>>,
    /// 工具权限模式（由上层引擎注入）
    permission_mode: crate::permissions::PermissionMode,
    /// 是否启用 LLM 驱动的记忆提取
    llm_memory_extraction: bool,
    /// 工具授权通道（用于 MCP 等工具的交互式授权）
    approval_channel: Option<Arc<ToolApprovalChannel>>,
    /// 工具白名单（用于子 Agent 隔离；None 表示不限制）
    allowed_tools: Option<HashSet<String>>,
    /// 本轮是否已触发过 Workflow（每轮最多一次）
    workflow_triggered_this_turn: std::sync::atomic::AtomicBool,
    /// Workflow 策略（默认从环境变量读取，可覆盖）
    workflow_policy: WorkflowPolicy,
    /// 拒绝追踪器
    denial_tracker: Option<Arc<crate::security::DenialTracker>>,
    /// 安全审计日志
    audit_log: Option<Arc<crate::security::SecurityAuditLog>>,
}

/// 对话循环结果
pub struct LoopResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub iterations: usize,
    /// 流式预执行的只读工具结果（tool_index → result）
    /// execute_tools_parallel 应跳过已有结果的只读工具
    pub pre_executed_results: std::collections::HashMap<usize, ToolResult>,
}

impl ConversationLoop {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        cost_tracker: Arc<Mutex<crate::cost_tracker::CostTracker>>,
        model: String,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            cost_tracker,
            model,
            max_iterations: 10,
            agent_manager: None,
            mcp_manager: None,
            lsp_manager: None,
            worktree_manager: None,
            hook_manager: ToolHookManager::from_env().map(Arc::new),
            compressor: None,
            memory_manager: None,
            permission_mode: crate::permissions::PermissionMode::AutoLowRisk,
            llm_memory_extraction: false,
            approval_channel: None,
            allowed_tools: None,
            workflow_triggered_this_turn: std::sync::atomic::AtomicBool::new(false),
            workflow_policy: WorkflowPolicy::from_env(),
            session_id: format!("session-{}", uuid::Uuid::new_v4()),
            denial_tracker: None,
            audit_log: None,
        }
    }

    /// 启用记忆管理器（预取 + 围栏注入 + 同步）
    pub fn with_memory_manager(
        mut self,
        manager: Arc<Mutex<crate::memory::MemoryManager>>,
    ) -> Self {
        self.memory_manager = Some(manager);
        self
    }

    /// 启用上下文压缩（设置最大上下文 token 数）
    pub fn with_compression(mut self, max_context_tokens: u64) -> Self {
        self.compressor = Some(Mutex::new(
            ContextCompressor::new(max_context_tokens)
                .with_llm_provider(self.provider.clone(), &self.model),
        ));
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_agent_manager(mut self, manager: Arc<crate::agent::AgentManager>) -> Self {
        self.agent_manager = Some(manager);
        self
    }

    pub fn with_mcp_manager(mut self, manager: Arc<crate::engine::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub fn with_lsp_manager(mut self, manager: Arc<crate::engine::lsp::LspManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    pub fn with_worktree_manager(
        mut self,
        manager: Arc<crate::engine::worktree::WorktreeManager>,
    ) -> Self {
        self.worktree_manager = Some(manager);
        self
    }

    pub fn with_hook_manager(mut self, manager: Arc<ToolHookManager>) -> Self {
        self.hook_manager = Some(manager);
        self
    }

    pub fn with_permission_mode(mut self, mode: crate::permissions::PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    pub fn with_llm_memory_extraction(mut self, enabled: bool) -> Self {
        self.llm_memory_extraction = enabled;
        self
    }

    pub fn with_approval_channel(mut self, channel: Arc<ToolApprovalChannel>) -> Self {
        self.approval_channel = Some(channel);
        self
    }

    pub fn with_allowed_tools(mut self, tools: HashSet<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    pub fn with_workflow_policy(mut self, policy: WorkflowPolicy) -> Self {
        self.workflow_policy = policy;
        self
    }

    /// 创建工具执行上下文
    fn create_tool_context(&self) -> ToolContext {
        let mut ctx = ToolContext::new(".", self.session_id.clone());
        if let Some(ref manager) = self.agent_manager {
            ctx = ctx.with_agent_manager(manager.clone());
        }
        if let Some(ref mcp) = self.mcp_manager {
            ctx = ctx.with_mcp_manager(mcp.clone());
        }
        if let Some(ref lsp) = self.lsp_manager {
            ctx = ctx.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            ctx = ctx.with_worktree_manager(wt.clone());
        }
        ctx = ctx.with_llm_provider(self.provider.clone());
        ctx = ctx.with_model(&self.model);
        ctx = ctx.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        // 权限模式由上层引擎注入（默认 AutoLowRisk）
        ctx.permission_context.mode = self.permission_mode;
        ctx
    }

    /// 运行对话循环（非流式）
    pub async fn run(&self, messages: Vec<Message>) -> Result<LoopResult> {
        self.run_inner(messages, None::<&mpsc::Sender<StreamEvent>>)
            .await
    }

    /// 运行对话循环（流式）
    pub async fn run_streaming(
        &self,
        messages: Vec<Message>,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<LoopResult> {
        self.run_inner(messages, Some(tx)).await
    }

    /// 核心循环实现
    async fn run_inner(
        &self,
        mut messages: Vec<Message>,
        tx: Option<&mpsc::Sender<StreamEvent>>,
    ) -> Result<LoopResult> {
        // ── Workflow 闸门检查 ──────────────────────────
        // M1: 根据用户输入判定走 Direct 模式还是 Workflow 结构化流程
        // 每轮最多触发一次 workflow（防止递归或重复触发）
        let already_triggered = self
            .workflow_triggered_this_turn
            .swap(true, std::sync::atomic::Ordering::SeqCst);
        if !already_triggered {
            if let Some(last_user_msg) = messages
                .iter()
                .rposition(|m| matches!(m, Message::User { .. }))
                .and_then(|i| match &messages[i] {
                    Message::User { content } => Some(content.as_str()),
                    _ => None,
                })
            {
                let workflow_policy = self.workflow_policy.clone();
                let gate = Gate::new().with_policy(workflow_policy.gate.clone());
                if is_drift_interruption_signal(last_user_msg) {
                    crate::engine::workflow::metrics::record_drift_interruption();
                }
                let decision = if workflow_policy.gate.llm_classifier_enabled {
                    gate.decide_with_llm(last_user_msg, self.provider.as_ref(), &self.model)
                        .await
                } else {
                    gate.decide(last_user_msg)
                };
                if decision.is_workflow() {
                    crate::engine::workflow::metrics::record_workflow_run();
                    if let Some(ref mem_mgr) = self.memory_manager {
                        let mut mem = mem_mgr.lock().await;
                        mem.save_workflow_decision(
                            "gate",
                            last_user_msg,
                            "Workflow",
                            decision.reason(),
                        );
                    }
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::TextChunk(format!(
                                "[Workflow mode activated: {}]\n\n",
                                decision.reason()
                            )))
                            .await;
                    }
                    let workflow_executor = WorkflowRealStepExecutor {
                        tool_registry: self.tool_registry.clone(),
                        llm_provider: self.provider.clone(),
                        model: self.model.clone(),
                        base_context: self.create_tool_context(),
                    };
                    let workflow_engine = WorkflowEngine::new(self.provider.clone())
                        .with_policy(workflow_policy);
                    match workflow_engine
                        .run(last_user_msg, last_user_msg, &workflow_executor)
                        .await
                    {
                        Ok(result) => {
                            if let Some(ref mem_mgr) = self.memory_manager {
                                let mut mem = mem_mgr.lock().await;
                                mem.save_workflow_decision(
                                    "execution",
                                    last_user_msg,
                                    "Success",
                                    &format!(
                                        "workflow completed with {} steps",
                                        result.plan.steps.len()
                                    ),
                                );
                            }
                            if let Some(tx) = tx {
                                let _ = tx.send(StreamEvent::Complete).await;
                            }
                            return Ok(LoopResult {
                                content: result.final_report,
                                tool_calls: Vec::new(),
                                iterations: 0,
                                pre_executed_results: std::collections::HashMap::new(),
                            });
                        }
                        Err(e) => {
                            if let Some(ref mem_mgr) = self.memory_manager {
                                let mut mem = mem_mgr.lock().await;
                                mem.save_workflow_decision(
                                    "fallback",
                                    last_user_msg,
                                    "DirectMode",
                                    &e,
                                );
                            }
                            warn!(
                                "Workflow execution failed: {}, falling back to direct mode",
                                e
                            );
                            // Fall through to direct mode
                        }
                    }
                }
            }
        }

        let tools = self.get_tools();
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();
        let mut iterations_used = 0;

        // ── 前置压缩（Preflight）─────────────────────────
        // 进入循环前检查总 token（消息 + 工具 schema），超阈值提前压缩
        // 支持最多 3 轮连续压缩（Hermes 风格）
        if let Some(ref compressor_mutex) = self.compressor {
            let mut no_gain_passes = 0u8;
            for pass in 0..3 {
                let compressor = compressor_mutex.lock().await;
                let tool_tokens = estimate_tool_schemas_tokens(&tools);
                let msg_tokens = estimate_messages_tokens(&messages);
                if !compressor.preflight_check(&messages, msg_tokens, tool_tokens) {
                    break; // 不再需要压缩
                }
                debug!(
                    "Preflight compression pass {}/3 ({} msg + {} tool tokens)",
                    pass + 1,
                    msg_tokens,
                    tool_tokens
                );
                drop(compressor); // 释放锁
                let before_tokens = estimate_messages_tokens(&messages);
                messages = compressor_mutex
                    .lock()
                    .await
                    .compress_async(&messages)
                    .await;
                let after_tokens = estimate_messages_tokens(&messages);
                if after_tokens >= before_tokens {
                    no_gain_passes += 1;
                    if no_gain_passes >= 2 {
                        warn!(
                            "Preflight compression made no progress for 2 consecutive passes ({} -> {}). Stop retrying this turn.",
                            before_tokens, after_tokens
                        );
                        break;
                    }
                } else {
                    no_gain_passes = 0;
                }
            }
        }

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Start).await;
        }

        // ── 记忆围栏注入 ───────────────────────────────
        // 将冻结的记忆快照作为 system message 注入（XML 围栏包裹）
        if let Some(ref mem_mutex) = self.memory_manager {
            let mem = mem_mutex.lock().await;
            let snapshot = mem.get_snapshot();
            if !snapshot.is_empty() {
                // 在 system messages 末尾（用户消息之前）注入记忆
                // 找到第一个非 system 消息的位置
                let insert_pos = messages
                    .iter()
                    .position(|m| !matches!(m, Message::System { .. }))
                    .unwrap_or(messages.len());
                messages.insert(insert_pos, Message::system(&snapshot));
                debug!("Injected memory context fence at position {}", insert_pos);
            }
        }

        // ── 迭代预算 ─────────────────────────────────────
        let mut effective_iterations: usize = 0; // 消耗的"有效"迭代（扣除了退还的）

        for iteration in 0..self.max_iterations {
            debug!(
                "Conversation loop iteration {} (effective: {}/{})",
                iteration, effective_iterations, self.max_iterations
            );
            iterations_used = iteration + 1;

            // 每次迭代开始重置预取状态，确保当前轮可再次进行 prefetch
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                mem.reset_turn();
            }

            // 检查有效迭代是否耗尽
            if effective_iterations >= self.max_iterations {
                warn!(
                    "Effective iteration budget exhausted ({}/{})",
                    effective_iterations, self.max_iterations
                );
                break;
            }

            // 构建请求
            // 记忆预取：在每次 API 调用前搜索相关记忆并注入到最后的用户消息
            let mut request_messages = messages.clone();
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                // 找到最后一条用户消息
                if let Some(last_user_idx) = request_messages
                    .iter()
                    .rposition(|m| matches!(m, Message::User { .. }))
                {
                    if let Message::User { content } = &request_messages[last_user_idx] {
                        let prefetch = mem.prefetch(content);
                        if !prefetch.is_empty() {
                            // 将预取的记忆注入到用户消息中（XML 围栏包裹）
                            let enhanced = format!(
                                "{}\n<relevant-memory>\n{}\n</relevant-memory>",
                                content, prefetch
                            );
                            request_messages[last_user_idx] = Message::user(&enhanced);
                            debug!("Prefetched memory context injected into user message");
                        }
                    }
                }
            }

            let mut request = ChatRequest::new(&self.model)
                .with_messages(request_messages)
                .with_tools(tools.clone());

            // ── 响应式压缩循环（遇到 413 等上下文超限自动触发）────────────
            let mut compressed_this_turn = false;
            let mut api_result: Result<(
                String,
                Vec<ToolCall>,
                std::collections::HashMap<usize, ToolResult>,
            )> = Err(anyhow::anyhow!("initial"));
            for compress_retry in 0..3 {
                api_result = if let Some(tx) = tx {
                    self.call_api_streaming(request.clone(), tx).await
                } else {
                    self.call_api(request.clone()).await
                };

                match &api_result {
                    Ok(_) => break, // 成功，跳出重试循环
                    Err(e) => {
                        let err_str = e.to_string().to_lowercase();
                        let needs_compress = err_str.contains("payload too large")
                            || err_str.contains("413")
                            || err_str.contains("context")
                            || err_str.contains("too many tokens")
                            || err_str.contains("maximum context length");
                        if needs_compress && compress_retry < 2 {
                            warn!(
                                "API error (attempt {}/3): {}. Compressing context and retrying...",
                                compress_retry + 1,
                                e
                            );
                            if let Some(ref comp) = self.compressor {
                                let msgs_for_comp = if compress_retry == 0 {
                                    messages.clone()
                                } else {
                                    // 第二次重试，用更激进的 micro_compress
                                    let mut comp = comp.lock().await;
                                    comp.micro_compress(&messages)
                                };
                                let compressed =
                                    comp.lock().await.compress_async(&msgs_for_comp).await;
                                request = ChatRequest::new(&self.model)
                                    .with_messages(compressed)
                                    .with_tools(tools.clone());
                                compressed_this_turn = true;
                            }
                        } else {
                            break; // 不需要压缩或已达最大重试
                        }
                    }
                }
            }

            let (content, tool_calls, pre_executed) = api_result?;

            // 如果本轮发生了压缩，通知前端
            if compressed_this_turn {
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::TextChunk(
                            "\n[Context compressed due to size limits]\n".to_string(),
                        ))
                        .await;
                }
            }

            final_content = content.clone();
            final_tool_calls = tool_calls.clone();

            // 没有工具调用 → 完成
            if tool_calls.is_empty() {
                break;
            }

            // 有工具调用 → 添加助手消息到历史
            messages.push(Message::assistant_with_tools(&content, tool_calls.clone()));

            // 并行执行工具（跳过流式预执行的只读工具）
            let mut results = self
                .execute_tools_parallel(&tool_calls, tx, pre_executed)
                .await;

            // ── 迭代预算退还 ──────────────────────────────
            // 检查本轮工具调用是否全是只读的，如果是则退还迭代
            let all_read_only = tool_calls
                .iter()
                .all(|tc| READ_ONLY_TOOLS.iter().any(|&name| tc.name == name));

            if all_read_only {
                debug!("All tools read-only, refunding iteration budget");
                // 不增加 effective_iterations → 退还
            } else {
                effective_iterations += 1;
            }

            // 将工具结果添加到消息历史（截断过大的结果）
            let mut tool_results_text = String::new();
            let mut changed_files = Vec::new();
            for (tc, result) in results.iter_mut() {
                // 截断过大的工具结果，写入磁盘
                truncate_tool_result(result, &tc.name, &tc.id);
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                tool_results_text.push_str(&result_content);
                tool_results_text.push('\n');
                messages.push(Message::tool(tc.id.clone(), result_content));

                // 收集文件修改成功的路径用于自动验证
                if result.success && (tc.name == "file_edit" || tc.name == "file_write") {
                    if let Some(path) = tc.arguments["path"].as_str() {
                        changed_files.push(std::path::PathBuf::from(path));
                    }
                }
            }

            // ── 自动验证闭环 ──────────────────────────────
            if !changed_files.is_empty() {
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let verify_results =
                    super::auto_verify::verify_file_changes(&working_dir, &changed_files).await;
                let check_passed = verify_results.iter().all(|r| r.success);
                for result in verify_results {
                    let verify_text = result.to_dialog_text();
                    if !result.success {
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&verify_text);
                        messages.push(Message::system(verify_text));
                    } else {
                        // 验证通过也可作为轻量提示
                        debug!("{}", verify_text);
                    }
                }

                // ── LSP 诊断补充 ───────────────────────────
                // 如果 LSP manager 可用，获取修改文件的缓存诊断
                if let Some(ref lsp_mgr) = self.lsp_manager {
                    let mut lsp_issues = Vec::new();
                    for path in &changed_files {
                        let uri = super::lsp::path_to_uri(path);
                        for name in lsp_mgr.server_names() {
                            if let Some(client) = lsp_mgr.get_client(&name) {
                                let diagnostics = client.get_diagnostics(&uri).await;
                                for d in diagnostics {
                                    let sev = match d.severity {
                                        Some(1) => "error",
                                        Some(2) => "warning",
                                        Some(3) => "info",
                                        Some(4) => "hint",
                                        _ => "diagnostic",
                                    };
                                    lsp_issues.push(format!(
                                        "  [{}] {}:{}: {}",
                                        sev,
                                        path.display(),
                                        d.range.start.line + 1,
                                        d.message.replace('\n', " ")
                                    ));
                                }
                            }
                        }
                    }
                    if !lsp_issues.is_empty() {
                        let lsp_text = format!(
                            "[LSP diagnostics for modified files]:\n{}",
                            lsp_issues.join("\n")
                        );
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&lsp_text);
                        messages.push(Message::system(lsp_text));
                    }
                }

                // ── 自动测试闭环 ──────────────────────────────
                let test_results =
                    super::auto_verify::run_tests(&working_dir, &changed_files, check_passed).await;
                let tests_passed = test_results.iter().all(|r| r.success);
                for result in test_results {
                    let test_text = result.to_dialog_text();
                    if !result.success {
                        tool_results_text.push('\n');
                        tool_results_text.push_str(&test_text);
                        messages.push(Message::system(test_text));
                    } else {
                        debug!("{}", test_text);
                    }
                }

                // ── 代码自审查 ────────────────────────────────
                let review_result =
                    super::code_review::review_changed_files(&working_dir, &changed_files);
                if !review_result.success {
                    let review_text = review_result.to_dialog_text();
                    tool_results_text.push('\n');
                    tool_results_text.push_str(&review_text);
                    messages.push(Message::system(review_text));
                }

                // ── 编程质量可观测性 ───────────────────────
                let verify_passed = check_passed && tests_passed && review_result.success;
                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_coding_round(verify_passed);
                }
            }

            // ── 记忆同步 ──────────────────────────────────
            // 从本轮对话中提取学习内容
            if let Some(ref mem_mutex) = self.memory_manager {
                let mut mem = mem_mutex.lock().await;
                // sync_turn 用最后一条用户消息 + 助手/工具结果来提取学习
                let user_msg = messages
                    .iter()
                    .rposition(|m| matches!(m, Message::User { .. }))
                    .and_then(|i| match &messages[i] {
                        Message::User { content } => Some(content.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                if !user_msg.is_empty() {
                    let assistant_text = format!("{} {}", final_content, tool_results_text);
                    if self.llm_memory_extraction {
                        // 先检查是否应触发 LLM 提取（throttle + mutual exclusion）
                        if mem.should_extract_with_llm() {
                            let provider: Option<&dyn LlmProvider> = Some(self.provider.as_ref());
                            mem.sync_turn_llm(user_msg, &assistant_text, provider, &self.model)
                                .await;
                            mem.mark_main_agent_wrote();
                        }
                    } else {
                        mem.sync_turn(user_msg, &assistant_text);
                        mem.mark_main_agent_wrote();
                    }
                }
                // 每轮结束，增加轮数计数
                mem.increment_turn();
            }
        }

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Complete).await;
        }

        Ok(LoopResult {
            content: final_content,
            tool_calls: final_tool_calls,
            iterations: iterations_used,
            pre_executed_results: std::collections::HashMap::new(),
        })
    }

    /// 非流式 API 调用
    async fn call_api(
        &self,
        request: ChatRequest,
    ) -> Result<(
        String,
        Vec<ToolCall>,
        std::collections::HashMap<usize, ToolResult>,
    )> {
        let response = self.provider.chat(request).await?;
        self.record_cost(&response).await;

        let content = response.content.clone();
        let tool_calls = response.tool_calls.unwrap_or_default();

        Ok((content, tool_calls, std::collections::HashMap::new()))
    }

    /// 流式 API 调用
    async fn call_api_streaming(
        &self,
        request: ChatRequest,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<(
        String,
        Vec<ToolCall>,
        std::collections::HashMap<usize, ToolResult>,
    )> {
        // 保存 fallback 需要的数据
        let fallback_messages = request.messages.clone();
        let fallback_tools = request.tools.clone();

        match self.provider.chat_stream(request).await {
            Ok(mut stream) => {
                let mut full_content = String::new();
                let mut collected_tool_calls: Vec<ToolCall> = Vec::new();
                let mut raw_args_accum: Vec<String> = Vec::new();

                // ── Extended Thinking 信号 ─────────────────────────────
                // Kimi K2.5 等支持 extended thinking 的模型会生成内部推理 tokens
                // 这些 tokens 不显示在 content 中，仅在 usage 中可见
                // 我们发送 ThinkingStart/Complete 信号供 UI 显示 thinking 状态
                // 注意：实际的 thinking 内容对客户端不可见，这是统计意义上的信号
                let _ = tx.send(StreamEvent::ThinkingStart).await;

                // ── 流式只读工具并行执行 ─────────────────────────────────
                // 当只读工具的参数开始到达时，在后台并行执行
                // key: tool index, value: join_handle
                let mut read_only_tasks: std::collections::HashMap<
                    usize,
                    tokio::task::JoinHandle<ToolResult>,
                > = std::collections::HashMap::new();
                let read_only_concurrency = read_only_tool_concurrency();
                let tool_registry = self.tool_registry.clone();
                let tool_context = self.create_tool_context();
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(chunk) => {
                            if let Some(choice) = chunk.choices.first() {
                                // 处理文本内容
                                if let Some(content) = &choice.delta.content {
                                    if !content.is_empty() {
                                        full_content.push_str(content);
                                        let _ =
                                            tx.send(StreamEvent::TextChunk(content.clone())).await;
                                    }
                                }

                                // 处理工具调用增量
                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                    for tc_delta in tool_calls {
                                        let idx = tc_delta.index as usize;
                                        while collected_tool_calls.len() <= idx {
                                            collected_tool_calls.push(ToolCall {
                                                id: String::new(),
                                                name: String::new(),
                                                arguments: serde_json::Value::Null,
                                            });
                                            raw_args_accum.push(String::new());
                                        }

                                        // 提前提取工具名称（避免在后续 borrow 中冲突）
                                        let mut tool_name_for_spawn: Option<String> = None;
                                        let mut tool_id_for_spawn: Option<String> = None;
                                        let mut args_for_spawn: Option<String> = None;

                                        let tc = &mut collected_tool_calls[idx];
                                        if let Some(id) = &tc_delta.id {
                                            tc.id = id.clone();
                                            let _ = tx
                                                .send(StreamEvent::ToolCallStart {
                                                    id: id.clone(),
                                                    name: tc.name.clone(),
                                                })
                                                .await;
                                        }
                                        if let Some(function) = &tc_delta.function {
                                            if let Some(name) = &function.name {
                                                tc.name = name.clone();
                                            }
                                            if let Some(args) = &function.arguments {
                                                raw_args_accum[idx].push_str(args);

                                                // 提取所有需要的数据，在 mutable borrow 释放后使用
                                                tool_name_for_spawn = Some(tc.name.clone());
                                                tool_id_for_spawn = Some(tc.id.clone());
                                                args_for_spawn = Some(raw_args_accum[idx].clone());

                                                let _ = tx
                                                    .send(StreamEvent::ToolCallArgs {
                                                        id: tc.id.clone(),
                                                        args_delta: args.clone(),
                                                    })
                                                    .await;
                                            }
                                        }

                                        // ── 触发只读工具后台执行 ─────────────────
                                        // 在收到 args_delta 后，工具名/id/参数都已齐全，此时启动后台执行
                                        if let (Some(tool_name), Some(tid), Some(current_args)) =
                                            (tool_name_for_spawn, tool_id_for_spawn, args_for_spawn)
                                        {
                                            if !tool_name.is_empty()
                                                && Self::is_read_only(&tool_name)
                                                && !read_only_tasks.contains_key(&idx)
                                                && read_only_tasks.len() < read_only_concurrency
                                            {
                                                let registry = tool_registry.clone();
                                                let context = tool_context.clone();
                                                let ct = cost_tracker.clone();
                                                let hooks = hook_manager.clone();
                                                let tid2 = tid.clone();
                                                let tool_n = tool_name.clone();
                                                let tool_n2 = tool_name.clone();

                                                read_only_tasks.insert(
                                                    idx,
                                                    tokio::spawn(async move {
                                                        let started_at =
                                                            std::time::Instant::now();
                                                        let pre_decision = if let Some(ref h)
                                                            = hooks
                                                        {
                                                            let t = ToolCall {
                                                                id: tid.clone(),
                                                                name: tool_n.clone(),
                                                                arguments:
                                                                    serde_json::from_str(
                                                                        &current_args,
                                                                    )
                                                                    .unwrap_or(serde_json::Value::Null),
                                                            };
                                                            h.run_pre_tool(&t, &context).await
                                                        } else {
                                                            HookDecision {
                                                                allow: true,
                                                                reason: None,
                                                            }
                                                        };

                                                        let ctx_clone = context.clone();
                                                        let mut result = if !pre_decision.allow {
                                                            ToolResult::error(
                                                                pre_decision.reason.unwrap_or_else(
                                                                    || format!(
                                                                        "blocked by pre-tool hook: {}",
                                                                        tool_n
                                                                    ),
                                                                ),
                                                            )
                                                        } else if let Some(tool) =
                                                            registry.get(&tool_n)
                                                        {
                                                            let parsed_args =
                                                                serde_json::from_str(
                                                                    &current_args,
                                                                )
                                                                .unwrap_or(serde_json::Value::Null);
                                                            tool.execute(parsed_args, context)
                                                                .await
                                                        } else {
                                                            ToolResult::error(format!(
                                                                "Tool '{}' not found",
                                                                tool_n
                                                            ))
                                                        };

                                                        let duration_ms =
                                                            started_at.elapsed().as_millis()
                                                                as u64;
                                                        if result.duration_ms.is_none() {
                                                            result.duration_ms =
                                                                Some(duration_ms);
                                                        }
                                                        if let Some(ref h) = hooks {
                                                            let tc_for_hook = ToolCall {
                                                                id: tid2.clone(),
                                                                name: tool_n2.clone(),
                                                                arguments:
                                                                    serde_json::from_str(
                                                                        &current_args,
                                                                    )
                                                                    .unwrap_or(serde_json::Value::Null),
                                                            };
                                                            h.run_post_tool(&tc_for_hook, &result, &ctx_clone)
                                                                .await;
                                                        }
                                                        {
                                                            let mut tracker = ct.lock().await;
                                                            tracker.record_tool_execution(
                                                                &tool_n,
                                                                result.success,
                                                                duration_ms,
                                                                result.success.then_some(
                                                                    &result.content,
                                                                ),
                                                            );
                                                        }
                                                        result
                                                    }),
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            // 检测输出截断（FinishReason::Length）
                            let truncated = chunk.choices.iter().any(|c| {
                                c.finish_reason.as_ref().is_some_and(|fr| {
                                    // Length 表示达到 max_tokens 限制
                                    format!("{:?}", fr).contains("Length")
                                })
                            });
                            if truncated {
                                let _ = tx.send(StreamEvent::OutputTruncated).await;
                            }
                            if chunk.choices.iter().any(|c| c.finish_reason.is_some()) {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {}", e);
                            let _ = tx
                                .send(StreamEvent::Error(format!("Stream error: {}", e)))
                                .await;
                            break;
                        }
                    }
                }

                // ── Extended Thinking 完成信号 ────────────────────────
                let _ = tx.send(StreamEvent::ThinkingComplete).await;

                // 解析累积的工具调用参数
                for (i, tc) in collected_tool_calls.iter_mut().enumerate() {
                    if i < raw_args_accum.len() && !raw_args_accum[i].is_empty() {
                        tc.arguments =
                            serde_json::from_str(&raw_args_accum[i]).unwrap_or_else(|e| {
                                warn!("Failed to parse tool args: {}", e);
                                serde_json::Value::Null
                            });
                        let _ = tx
                            .send(StreamEvent::ToolCallComplete { id: tc.id.clone() })
                            .await;
                    }
                }

                // ── 等待并收集后台只读工具结果 ─────────────────────────
                // 收集预执行结果，供 execute_tools_parallel 跳过已执行的只读工具
                let mut pre_executed: std::collections::HashMap<usize, ToolResult> =
                    std::collections::HashMap::new();
                for (idx, handle) in read_only_tasks {
                    if let Ok(result) = handle.await {
                        debug!(
                            "Read-only tool at index {} pre-executed with result: {}",
                            idx,
                            if result.success { "OK" } else { "ERROR" }
                        );
                        pre_executed.insert(idx, result);
                    }
                }

                Ok((full_content, collected_tool_calls, pre_executed))
            }
            Err(e) => {
                // 流式 API 失败，回退到非流式
                warn!("Streaming failed, falling back to non-streaming: {}", e);
                let response = self
                    .provider
                    .chat(
                        ChatRequest::new(&self.model)
                            .with_messages(fallback_messages)
                            .with_tools(fallback_tools.unwrap_or_default()),
                    )
                    .await?;
                self.record_cost(&response).await;

                let content = response.content.clone();
                if !content.is_empty() {
                    let _ = tx.send(StreamEvent::TextChunk(content.clone())).await;
                }
                let tool_calls = response.tool_calls.unwrap_or_default();
                Ok((content, tool_calls, std::collections::HashMap::new()))
            }
        }
    }

    /// 记录 API 调用成本
    async fn record_cost(&self, response: &ChatResponse) {
        if let Some(ref usage) = response.usage {
            let mut tracker = self.cost_tracker.lock().await;
            tracker.record_api_call(
                &self.model,
                usage.prompt_tokens as u64,
                usage.completion_tokens as u64,
            );
        }
    }

    /// 获取工具定义列表
    fn get_tools(&self) -> Vec<crate::services::api::Tool> {
        self.tool_registry
            .iter_tools()
            .filter(|t| {
                if let Some(ref allowed) = self.allowed_tools {
                    allowed.contains(t.name())
                } else {
                    true
                }
            })
            .map(|t| crate::services::api::Tool {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }

    /// 检查工具是否为只读（可并发执行）
    fn is_read_only(tool_name: &str) -> bool {
        READ_ONLY_TOOLS.contains(&tool_name)
    }

    /// 并行执行工具调用
    async fn execute_tools_parallel(
        &self,
        tool_calls: &[ToolCall],
        tx: Option<&mpsc::Sender<StreamEvent>>,
        pre_executed: std::collections::HashMap<usize, ToolResult>,
    ) -> Vec<(ToolCall, ToolResult)> {
        let mut read_only_jobs = Vec::new();
        let mut read_write_calls = Vec::new();
        let mut denied_results = Vec::new();
        let mut results: Vec<(ToolCall, ToolResult)> = Vec::new();

        for (i, tc) in tool_calls.iter().enumerate() {
            if tc.name.is_empty() {
                continue;
            }
            if let Some(ref allowed) = self.allowed_tools {
                if !allowed.contains(&tc.name) {
                    denied_results.push((
                        tc.clone(),
                        ToolResult::error(format!(
                            "Tool '{}' is not allowed in this agent context",
                            tc.name
                        )),
                    ));
                    continue;
                }
            }

            // 如果该工具已在流式期间预执行，直接使用预执行结果
            if let Some(pre_result) = pre_executed.get(&i) {
                debug!(
                    "Skipping pre-executed read-only tool at index {}: {}",
                    i, tc.name
                );
                results.push((tc.clone(), pre_result.clone()));
                if let Some(tx) = tx {
                    let result_content = format!(
                        "Result: {}\n{}",
                        if pre_result.success { "OK" } else { "ERROR" },
                        pre_result.content
                    );
                    let _ = tx
                        .send(StreamEvent::ToolExecutionComplete {
                            id: tc.id.clone(),
                            result: result_content,
                        })
                        .await;
                }
                continue;
            }

            if Self::is_read_only(&tc.name) {
                if let Some(tx) = tx {
                    let _ = tx
                        .send(StreamEvent::ToolExecutionStart {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                        })
                        .await;
                }
                let registry = self.tool_registry.clone();
                let context = self.create_tool_context();
                let tc_clone = tc.clone();
                let tool_name = tc.name.clone();
                let cost_tracker = self.cost_tracker.clone();
                let hook_manager = self.hook_manager.clone();
                read_only_jobs.push(async move {
                    let started_at = std::time::Instant::now();
                    let pre_decision = if let Some(ref hooks) = hook_manager {
                        hooks.run_pre_tool(&tc_clone, &context).await
                    } else {
                        HookDecision {
                            allow: true,
                            reason: None,
                        }
                    };

                    let mut result =
                        if !pre_decision.allow {
                            ToolResult::error(pre_decision.reason.unwrap_or_else(|| {
                                format!("blocked by pre-tool hook: {}", tool_name)
                            }))
                        } else if let Some(tool) = registry.get(&tool_name) {
                            tool.execute(tc_clone.arguments.clone(), context.clone())
                                .await
                        } else {
                            ToolResult::error(format!("Tool '{}' not found", tool_name))
                        };
                    let duration_ms = started_at.elapsed().as_millis() as u64;
                    if result.duration_ms.is_none() {
                        result.duration_ms = Some(duration_ms);
                    }

                    if let Some(ref hooks) = hook_manager {
                        hooks.run_post_tool(&tc_clone, &result, &context).await;
                    };
                    {
                        let mut tracker = cost_tracker.lock().await;
                        tracker.record_tool_execution(
                            &tool_name,
                            result.success,
                            duration_ms,
                            result.error.as_deref(),
                        );
                    }
                    (tc_clone, result)
                });
            } else {
                read_write_calls.push(tc.clone());
            }
        }

        // 添加工具拒绝结果
        results.append(&mut denied_results);

        // 并发执行只读工具（带上限）
        let concurrency = read_only_tool_concurrency();
        let mut readonly_stream =
            futures::stream::iter(read_only_jobs).buffer_unordered(concurrency);

        while let Some((tc, result)) = readonly_stream.next().await {
            if let Some(tx) = tx {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tc.id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            results.push((tc, result));
        }

        // 串行执行读写工具
        for tc in read_write_calls {
            let tool_id = tc.id.clone();
            let tool_name = tc.name.clone();
            if let Some(ref allowed) = self.allowed_tools {
                if !allowed.contains(&tool_name) {
                    results.push((
                        tc,
                        ToolResult::error(format!(
                            "Tool '{}' is not allowed in this agent context",
                            tool_name
                        )),
                    ));
                    continue;
                }
            }

            if let Some(tx) = tx {
                let _ = tx
                    .send(StreamEvent::ToolExecutionStart {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                    })
                    .await;
            }

            let (result, hook_context) = if let Some(tool) = self.tool_registry.get(&tool_name) {
                let context = self.create_tool_context();
                let pre_decision = if let Some(ref hooks) = self.hook_manager {
                    hooks.run_pre_tool(&tc, &context).await
                } else {
                    HookDecision {
                        allow: true,
                        reason: None,
                    }
                };

                let started_at = std::time::Instant::now();
                let mut result = if !pre_decision.allow {
                    ToolResult::error(
                        pre_decision
                            .reason
                            .unwrap_or_else(|| format!("blocked by pre-tool hook: {}", tool_name)),
                    )
                } else if context
                    .permission_context
                    .requires_confirmation(&tool_name, &tc.arguments)
                {
                    // 交互式授权（适用于所有需要确认的工具）
                    let mut approved = false;
                    if let (Some(ref channel), Some(tx)) = (&self.approval_channel, tx) {
                        let prompt = if tool_name == "mcp_tool" {
                            let server = tc.arguments["server_name"].as_str().unwrap_or("");
                            let t = tc.arguments["tool_name"].as_str().unwrap_or("");
                            format!(
                                "MCP tool '{}' on server '{}' requires approval. Allow?",
                                t, server
                            )
                        } else {
                            format!("Tool '{}' requires approval. Allow?", tool_name)
                        };
                        let _ = tx
                            .send(StreamEvent::PermissionRequest {
                                id: tool_id.clone(),
                                tool_name: tool_name.clone(),
                                arguments: tc.arguments.clone(),
                                prompt: prompt.clone(),
                            })
                            .await;
                        let request = ToolApprovalRequest {
                            tool_call: tc.clone(),
                            prompt,
                        };
                        match channel.submit(request).await {
                            Ok(is_approved) => approved = is_approved,
                            Err(e) => {
                                warn!("Tool approval error: {}", e);
                            }
                        }
                    }
                    if approved {
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamEvent::ToolExecutionProgress {
                                    id: tool_id.clone(),
                                    progress: format!("Executing {}...", tool_name),
                                })
                                .await;
                        }
                        tool.execute(tc.arguments.clone(), context.clone()).await
                    } else {
                        ToolResult::error(format!(
                            "Permission denied: '{}' requires user confirmation.",
                            tool_name
                        ))
                    }
                } else {
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamEvent::ToolExecutionProgress {
                                id: tool_id.clone(),
                                progress: format!("Executing {}...", tool_name),
                            })
                            .await;
                    }
                    tool.execute(tc.arguments.clone(), context.clone()).await
                };
                let duration_ms = started_at.elapsed().as_millis() as u64;
                if result.duration_ms.is_none() {
                    result.duration_ms = Some(duration_ms);
                }

                // ── Security Audit & Denial Tracking ──────────────────────
                let params_summary = if let Some(tool) = self.tool_registry.get(&tool_name) {
                    tool.to_classifier_input(&tc.arguments)
                } else {
                    tool_name.clone()
                };

                if let Some(ref log) = self.audit_log {
                    let decision = if result.success {
                        "EXECUTED"
                    } else if result.error.as_deref().unwrap_or("").contains("Permission denied")
                    {
                        "DENIED"
                    } else {
                        "FAILED"
                    };
                    log.log_execution(&tool_name, &params_summary, result.success, decision)
                        .await;
                }

                if let Some(ref tracker) = self.denial_tracker {
                    if result.success {
                        tracker.record_success().await;
                    } else if result.error.as_deref().unwrap_or("").contains("Permission denied")
                        || result.error.as_deref().unwrap_or("").contains("Dangerous command")
                    {
                        tracker
                            .record_denial(
                                &tool_name,
                                &params_summary,
                                result.error.as_deref().unwrap_or("security block"),
                            )
                            .await;
                    }
                }
                // ─────────────────────────────────────────────────────────

                {
                    let mut tracker = self.cost_tracker.lock().await;
                    tracker.record_tool_execution(
                        &tool_name,
                        result.success,
                        duration_ms,
                        result.error.as_deref(),
                    );
                }

                (result, Some(context))
            } else {
                (
                    ToolResult::error(format!("Tool '{}' not found", tool_name)),
                    None,
                )
            };

            if let (Some(hooks), Some(context)) = (&self.hook_manager, &hook_context) {
                hooks.run_post_tool(&tc, &result, context).await;
            }

            if let Some(tx) = tx {
                let result_content = format!(
                    "Result: {}\n{}",
                    if result.success { "OK" } else { "ERROR" },
                    result.content
                );
                let _ = tx
                    .send(StreamEvent::ToolExecutionComplete {
                        id: tool_id.clone(),
                        result: result_content,
                    })
                    .await;
            }
            results.push((tc, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::{ChatResponse, ToolCall, Usage};
    use crate::test_utils::env_guard::EnvVarGuard;
    use crate::tools::{FileReadTool, FileWriteTool};
    use async_openai::types::ChatCompletionResponseStream;
    use serde::Deserialize;
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    #[test]
    fn test_truncate_tool_result_handles_utf8_boundaries() {
        let mut result = ToolResult::success("中".repeat(20_000));
        truncate_tool_result(&mut result, "grep", "call_utf8");
        assert!(result.content.contains("Output truncated"));
    }

    #[test]
    fn test_truncate_tool_result_keeps_small_output_unchanged() {
        let original = "short output".to_string();
        let mut result = ToolResult::success(original.clone());
        truncate_tool_result(&mut result, "grep", "call_small");
        assert_eq!(result.content, original);
    }

    #[test]
    fn test_truncate_tool_result_includes_head_and_tail_markers() {
        let mut result = ToolResult::success(format!(
            "{}\n{}\n{}",
            "A".repeat(40_000),
            "中".repeat(8_000),
            "Z".repeat(40_000)
        ));
        truncate_tool_result(&mut result, "grep", "call_markers");
        assert!(result.content.contains("--- First"));
        assert!(result.content.contains("--- Last"));
        assert!(result.content.contains("Output truncated"));
    }

    #[test]
    fn test_normalize_params_fills_missing_required_fields() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "运行 cargo test 验证修复",
            Some("bash".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "timeout": { "type": "integer" }
            },
            "required": ["command", "timeout"]
        });

        let out = WorkflowRealStepExecutor::normalize_params(serde_json::json!({}), &schema, &step)
            .expect("normalize should succeed");
        assert_eq!(out["command"], "cargo test");
        assert!(out["timeout"].is_number());
    }

    #[test]
    fn test_normalize_params_coerces_required_field_types() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "在 src/main.rs 中搜索 TODO",
            Some("grep".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "path": { "type": "string" },
                "limit": { "type": "integer" },
                "recursive": { "type": "boolean" }
            },
            "required": ["pattern", "path", "limit", "recursive"]
        });

        let out = WorkflowRealStepExecutor::normalize_params(
            serde_json::json!({
                "pattern": 123,
                "path": true,
                "limit": "20",
                "recursive": "yes"
            }),
            &schema,
            &step,
        )
        .expect("normalize should succeed");

        assert_eq!(out["pattern"], "123");
        assert_eq!(out["path"], "true");
        assert_eq!(out["limit"], 20);
        assert_eq!(out["recursive"], true);
    }

    #[test]
    fn test_normalize_params_rejects_non_object_payload() {
        let step = crate::engine::plan_mode::PlanStep::new(
            "读取 README.md",
            Some("file_read".to_string()),
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        });
        let err = WorkflowRealStepExecutor::normalize_params(
            serde_json::json!(["not", "object"]),
            &schema,
            &step,
        )
        .expect_err("non-object params should be rejected");
        assert!(err.contains("JSON object"));
    }

    struct MockLlmProvider {
        responses: StdMutex<VecDeque<ChatResponse>>,
    }

    fn workflow_test_executor() -> WorkflowRealStepExecutor {
        WorkflowRealStepExecutor {
            tool_registry: Arc::new(ToolRegistry::new()),
            llm_provider: Arc::new(MockLlmProvider {
                responses: StdMutex::new(VecDeque::new()),
            }),
            model: "mock-model".to_string(),
            base_context: ToolContext::new(".", "workflow-test"),
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let mut guard = self.responses.lock().unwrap();
            guard
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("no mock response left"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    #[tokio::test]
    async fn test_coding_quality_tracks_fail_then_repair_cycle() {
        let mut env = EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_REVIEW", "1");
        let tmp = tempdir().expect("create temp dir");
        let target_file = tmp.path().join("sample.rs");
        let target_path = target_file.to_string_lossy().to_string();

        let failing_code = "fn main() { let x = Some(1).unwrap(); let _ = x; }";
        let fixed_code = "fn main() { let x = Some(1); if let Some(v) = x { let _ = v; } }";

        let responses = VecDeque::from(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": failing_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                    reasoning_tokens: None,
                }),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_2".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_file.to_string_lossy(),
                        "content": fixed_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                    reasoning_tokens: None,
                }),
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileWriteTool);
        let registry = Arc::new(registry);
        let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_engine = ConversationLoop::new(
            provider,
            registry,
            tracker.clone(),
            "mock-model".to_string(),
        )
        .with_permission_mode(crate::permissions::PermissionMode::AutoAll)
        .with_max_iterations(4)
        .with_workflow_policy(WorkflowPolicy {
            gate: crate::engine::workflow::GatePolicy {
                workflow_enabled: false,
                llm_classifier_enabled: false,
            },
            ..WorkflowPolicy::default()
        });

        let run1 = loop_engine
            .run(vec![
                Message::system("sys"),
                Message::user("write failing code"),
            ])
            .await;
        assert!(run1.is_ok(), "first run failed: {:?}", run1.err());

        {
            let t = tracker.lock().await;
            assert_eq!(t.coding_quality.file_change_rounds, 1);
            assert_eq!(t.coding_quality.verify_failures, 1);
            assert_eq!(t.coding_quality.repair_cycles, 0);
        }

        let run2 = loop_engine
            .run(vec![Message::system("sys"), Message::user("fix the code")])
            .await;
        assert!(run2.is_ok(), "second run failed: {:?}", run2.err());

        {
            let t = tracker.lock().await;
            assert_eq!(t.coding_quality.file_change_rounds, 2);
            assert_eq!(t.coding_quality.verify_failures, 1);
            assert_eq!(t.coding_quality.repair_cycles, 1);
            assert_eq!(t.coding_quality.first_pass_successes, 0);
        }
    }

    #[tokio::test]
    async fn test_coding_quality_tracks_first_pass_success() {
        let mut env = EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_REVIEW", "1");
        let tmp = tempdir().expect("create temp dir");
        let target_file = tmp.path().join("sample_ok.rs");
        let target_path = target_file.to_string_lossy().to_string();

        let safe_code = "fn main() { let x = Some(1); if let Some(v) = x { let _ = v; } }";
        let responses = VecDeque::from(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_ok_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": target_path,
                        "content": safe_code
                    }),
                }]),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    reasoning_tokens: None,
                }),
            },
            ChatResponse {
                content: "done".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 3,
                    total_tokens: 8,
                    reasoning_tokens: None,
                }),
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let mut registry = ToolRegistry::new();
        registry.register(FileWriteTool);
        let registry = Arc::new(registry);
        let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_engine = ConversationLoop::new(
            provider,
            registry,
            tracker.clone(),
            "mock-model".to_string(),
        )
        .with_permission_mode(crate::permissions::PermissionMode::AutoAll)
        .with_max_iterations(3)
        .with_workflow_policy(WorkflowPolicy {
            gate: crate::engine::workflow::GatePolicy {
                workflow_enabled: false,
                llm_classifier_enabled: false,
            },
            ..WorkflowPolicy::default()
        });

        let run = loop_engine
            .run(vec![
                Message::system("sys"),
                Message::user("write safe code"),
            ])
            .await;
        assert!(run.is_ok(), "run failed: {:?}", run.err());

        {
            let t = tracker.lock().await;
            assert_eq!(t.coding_quality.file_change_rounds, 1);
            assert_eq!(t.coding_quality.first_pass_successes, 1);
            assert_eq!(t.coding_quality.verify_failures, 0);
            assert_eq!(t.coding_quality.repair_cycles, 0);
        }
    }

    // ── Workflow Gate 路由集成测试 ──────────────────────────

    #[tokio::test]
    async fn test_conversation_loop_workflow_routing() {
        // Mock provider: 4 轮 questioning 回答
        let responses = VecDeque::from(vec![
            ChatResponse {
                content: "实现用户认证系统".into(),
                tool_calls: None,
                usage: None,
            },
            ChatResponse {
                content: "需要数据库支持和密码哈希库".into(),
                tool_calls: None,
                usage: None,
            },
            ChatResponse {
                content: "密码泄露和 SQL 注入风险".into(),
                tool_calls: None,
                usage: None,
            },
            ChatResponse {
                content: "1. 设计数据库表结构\n2. 实现登录接口\n3. 实现注册接口\n4. 编写测试验证".into(),
                tool_calls: None,
                usage: None,
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let registry = Arc::new(ToolRegistry::new());
        let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_engine = ConversationLoop::new(
            provider,
            registry,
            tracker,
            "mock-model".to_string(),
        );

        // 高风险消息 → Workflow 模式
        let result = loop_engine
            .run(vec![Message::user("重构整个用户认证系统")])
            .await;
        assert!(
            result.is_ok(),
            "Workflow routing failed: {:?}",
            result.err()
        );
        let loop_result = result.unwrap();

        assert!(
            loop_result.content.contains("Workflow 执行报告"),
            "Expected Workflow report, got: {}",
            loop_result.content
        );
        assert!(
            loop_result.content.contains("问题本质"),
            "Expected problem statement in report"
        );
        assert!(loop_result.tool_calls.is_empty());
        assert_eq!(loop_result.iterations, 0);
    }

    #[tokio::test]
    async fn test_conversation_loop_direct_routing() {
        let responses = VecDeque::from(vec![ChatResponse {
            content: "Sure, let me help with that typo.".into(),
            tool_calls: None,
            usage: None,
        }]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let registry = Arc::new(ToolRegistry::new());
        let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_engine = ConversationLoop::new(
            provider,
            registry,
            tracker,
            "mock-model".to_string(),
        );

        // 低风险消息 → Direct 模式
        let result = loop_engine
            .run(vec![Message::user("修复一个 typo")])
            .await;
        assert!(result.is_ok(), "Direct routing failed: {:?}", result.err());
        let loop_result = result.unwrap();

        // 应走正常 Direct 链路，返回 LLM 回复而非 Workflow 报告
        assert!(
            !loop_result.content.contains("Workflow 执行报告"),
            "Expected direct response, got Workflow report: {}",
            loop_result.content
        );
        assert!(
            loop_result.content.contains("Sure, let me help"),
            "Expected mock LLM response, got: {}",
            loop_result.content
        );
    }

    #[tokio::test]
    async fn test_conversation_loop_workflow_trigger_limit() {
        // Mock 响应：第一次 workflow 消耗 1 个（thinking 在第 1 个问题后收敛），
        // 第二次 direct 消耗第 2 个
        let responses = VecDeque::from(vec![
            ChatResponse {
                content: "实现用户认证系统".into(),
                tool_calls: None,
                usage: None,
            },
            ChatResponse {
                content: "Direct mode response after workflow limit.".into(),
                tool_calls: None,
                usage: None,
            },
        ]);

        let provider = Arc::new(MockLlmProvider {
            responses: StdMutex::new(responses),
        });
        let registry = Arc::new(ToolRegistry::new());
        let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

        let loop_engine = ConversationLoop::new(
            provider,
            registry,
            tracker,
            "mock-model".to_string(),
        );

        // 第一次调用：高风险消息 → Workflow 模式
        let result1 = loop_engine
            .run(vec![Message::user("重构整个用户认证系统")])
            .await
            .unwrap();
        assert!(
            result1.content.contains("Workflow 执行报告"),
            "First call should trigger workflow"
        );

        // 第二次调用：同样高风险消息，但应走 Direct（每轮最多一次 workflow）
        let result2 = loop_engine
            .run(vec![Message::user("重构整个用户认证系统")])
            .await
            .unwrap();
        assert!(
            !result2.content.contains("Workflow 执行报告"),
            "Second call should not trigger workflow, got: {}",
            result2.content
        );
        assert!(
            result2.content.contains("Direct mode response"),
            "Expected direct response, got: {}",
            result2.content
        );
    }

    struct ReplayLlmProvider;

    #[async_trait::async_trait]
    impl LlmProvider for ReplayLlmProvider {
        async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
            let last_user = request
                .messages
                .iter()
                .rev()
                .find_map(|m| match m {
                    Message::User { content } => Some(content.as_str()),
                    _ => None,
                })
                .unwrap_or("");

            let content = if last_user.contains("工具参数生成器") {
                if last_user.contains("file_read") {
                    r#"{"path":"README.md"}"#.to_string()
                } else {
                    "{}".to_string()
                }
            } else if last_user.contains("最终执行方案") || last_user.contains("列出具体步骤") {
                "1. 读取 README.md 理解当前行为\n2. 读取 workflow-spec.md 对齐目标\n3. 读取 PLAN.md 核对当前阶段"
                    .to_string()
            } else if last_user.contains("最核心目标") {
                "核心目标是先定位主线 blocker，再给出可执行改进路径。".to_string()
            } else if last_user.contains("前提条件") {
                "需要先确认现有实现状态、关键文件边界、可回退路径。".to_string()
            } else if last_user.contains("最大风险") {
                "最大风险是跑偏到细枝末节，或者执行顺序错误导致返工。".to_string()
            } else {
                "这是一个可执行的分析结论。".to_string()
            };

            Ok(ChatResponse {
                content,
                tool_calls: None,
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream unsupported in replay provider"))
        }

        fn base_url(&self) -> &str {
            "mock://replay"
        }

        fn default_model(&self) -> &str {
            "replay-model"
        }
    }

    fn parse_metric_percent(report: &str, key: &str) -> Option<f64> {
        let needle = format!("{}: ", key);
        let idx = report.find(&needle)?;
        let tail = &report[idx + needle.len()..];
        let line = tail.lines().next()?.trim();
        let value = line.trim_end_matches('%').trim();
        value.parse::<f64>().ok()
    }

    #[tokio::test]
    async fn test_workflow_real_devflow_round2_acceptance() {
        let tasks = vec![
            "重构 slash_handler 中 session/redo/retry 命令稳定性并避免跑偏",
            "改进 workflow gate 让复杂任务优先走结构化流程",
            "完善 workflow metrics 并输出北极星指标",
            "清理 workflow 执行链里的参数生成失败路径",
            "增强 Socratic 主动提问，避免细节纠结",
            "优化 planner 的依赖推断与重算权重逻辑",
            "把关键 workflow 决策写入记忆系统供后续复用",
            "做一轮真实开发流验收并输出结果",
        ];

        let mut workflow_reports = 0usize;
        let mut mainline_hits = 0usize;
        let mut avg_coverage_acc = 0.0f64;
        let mut avg_rework_acc = 0.0f64;

        for task in &tasks {
            let provider = Arc::new(ReplayLlmProvider);
            let mut registry = ToolRegistry::new();
            registry.register(FileReadTool);
            let registry = Arc::new(registry);
            let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

            let loop_engine = ConversationLoop::new(
                provider,
                registry,
                tracker,
                "replay-model".to_string(),
            )
            .with_permission_mode(crate::permissions::PermissionMode::AutoAll)
            .with_max_iterations(4);

            let result = loop_engine
                .run(vec![Message::user(*task)])
                .await
                .expect("conversation loop should run");

            if result.content.contains("Workflow 执行报告") {
                workflow_reports += 1;
            }
            if result.content.contains("Mainline Hit: yes") {
                mainline_hits += 1;
            }
            avg_coverage_acc +=
                parse_metric_percent(&result.content, "First Plan Coverage").unwrap_or(0.0);
            avg_rework_acc += parse_metric_percent(&result.content, "Rework Rate").unwrap_or(0.0);
        }

        let n = tasks.len() as f64;
        let workflow_rate = workflow_reports as f64 / n * 100.0;
        let mainline_hit_rate = mainline_hits as f64 / n * 100.0;
        let avg_coverage = avg_coverage_acc / n;
        let avg_rework = avg_rework_acc / n;

        let report = format!(
            "# Workflow 真实开发流验收（Round 2）\n\n- 任务数: {}\n- Workflow 触发率: {:.1}%\n- Mainline Hit Rate: {:.1}%\n- Avg First Plan Coverage: {:.1}%\n- Avg Rework Rate: {:.1}%\n\n## 结论\n- Gate + Workflow + 真实工具执行链路可运行\n- 指标可回收，可用于下一轮优化\n",
            tasks.len(),
            workflow_rate,
            mainline_hit_rate,
            avg_coverage,
            avg_rework
        );

        let _ = std::fs::create_dir_all("docs/workflow");
        std::fs::write("docs/workflow/real-devflow-round2-report.md", report)
            .expect("write round2 report");

        assert!(
            workflow_rate >= 80.0,
            "workflow trigger rate too low: {:.1}%",
            workflow_rate
        );
        assert!(
            mainline_hit_rate >= 60.0,
            "mainline hit rate too low: {:.1}%",
            mainline_hit_rate
        );
    }

    #[test]
    fn test_tool_specific_params_file_write_glob_project_list() {
        let executor = workflow_test_executor();

        let write_step = crate::engine::plan_mode::PlanStep::new(
            "写入 docs/workflow/notes.md 内容 `hello world`",
            Some("file_write".to_string()),
        );
        let write = executor
            .tool_specific_params(&write_step)
            .expect("file_write params");
        assert_eq!(write["path"], "docs/workflow/notes.md");
        assert_eq!(write["content"], "hello world");

        let glob_step = crate::engine::plan_mode::PlanStep::new(
            "搜索 `src/**/*.rs` 文件",
            Some("glob".to_string()),
        );
        let glob = executor.tool_specific_params(&glob_step).expect("glob params");
        assert_eq!(glob["pattern"], "src/**/*.rs");

        let project_step = crate::engine::plan_mode::PlanStep::new(
            "在项目里搜索 \"workflow\" 相关文件",
            Some("project_list".to_string()),
        );
        let project = executor
            .tool_specific_params(&project_step)
            .expect("project_list params");
        assert_eq!(project["action"], "search");
        assert_eq!(project["query"], "workflow");
    }

    #[test]
    fn test_tool_specific_params_memory_todo_json_query() {
        let executor = workflow_test_executor();

        let memory_step = crate::engine::plan_mode::PlanStep::new(
            "保存团队约定：`代码提交前必须跑 workflow-production-gates.sh`",
            Some("memory_save".to_string()),
        );
        let memory = executor
            .tool_specific_params(&memory_step)
            .expect("memory_save params");
        assert_eq!(memory["category"], "convention");
        assert!(memory["content"]
            .as_str()
            .unwrap_or_default()
            .contains("workflow-production-gates.sh"));

        let todo_step = crate::engine::plan_mode::PlanStep::new(
            "todo: `修复 gate 样本; 完成 param replay`",
            Some("todo_write".to_string()),
        );
        let todo = executor
            .tool_specific_params(&todo_step)
            .expect("todo_write params");
        assert!(todo["todos"].is_array());
        assert!(!todo["todos"].as_array().unwrap_or(&Vec::new()).is_empty());

        let json_step = crate::engine::plan_mode::PlanStep::new(
            "校验 JSON: `{\"name\":\"alice\"}`",
            Some("json_query".to_string()),
        );
        let jq = executor
            .tool_specific_params(&json_step)
            .expect("json_query params");
        assert_eq!(jq["action"], "validate");
        assert!(jq["json"].as_str().unwrap_or_default().contains("alice"));
    }

    #[derive(Debug, Deserialize)]
    struct ParamReplaySample {
        tool: String,
        description: String,
        required_fields: Vec<String>,
        expected_action: Option<String>,
        expected_pattern_contains: Option<String>,
        expected_path_contains: Option<String>,
    }

    #[test]
    fn test_param_planner_replay_samples() {
        let raw = include_str!("../../docs/workflow/param-replay-samples.json");
        let samples: Vec<ParamReplaySample> =
            serde_json::from_str(raw).expect("valid param-replay-samples.json");
        assert!(!samples.is_empty(), "param replay samples should not be empty");

        let executor = workflow_test_executor();
        for sample in &samples {
            let step = crate::engine::plan_mode::PlanStep::new(
                sample.description.clone(),
                Some(sample.tool.clone()),
            );
            let params = executor
                .tool_specific_params(&step)
                .unwrap_or_else(|e| panic!("planner failed for tool '{}': {}", sample.tool, e));

            for field in &sample.required_fields {
                assert!(
                    params.get(field).is_some(),
                    "missing required field '{}' for tool '{}'",
                    field,
                    sample.tool
                );
            }
            if let Some(action) = &sample.expected_action {
                assert_eq!(
                    params["action"].as_str(),
                    Some(action.as_str()),
                    "unexpected action for {:?}",
                    sample
                );
            }
            if let Some(pattern) = &sample.expected_pattern_contains {
                let got = params["pattern"].as_str().unwrap_or_default();
                assert!(
                    got.contains(pattern),
                    "pattern '{}' does not contain expected '{}'",
                    got,
                    pattern
                );
            }
            if let Some(path) = &sample.expected_path_contains {
                let got = params["path"].as_str().unwrap_or_default();
                assert!(
                    got.contains(path),
                    "path '{}' does not contain expected '{}'",
                    got,
                    path
                );
            }
        }
    }

    #[derive(Debug, Deserialize)]
    struct Round3Task {
        #[allow(dead_code)]
        commit: String,
        #[allow(dead_code)]
        date: String,
        #[allow(dead_code)]
        subject: String,
        task_description: String,
    }

    #[tokio::test]
    async fn test_workflow_real_devflow_round3_acceptance() {
        let tasks_raw = include_str!("../../docs/workflow/real-devflow-round3-tasks.json");
        let tasks: Vec<Round3Task> =
            serde_json::from_str(tasks_raw).expect("valid round3 tasks json");
        assert!(
            tasks.len() >= 8,
            "round3 tasks should contain at least 8 items"
        );

        let mut workflow_reports = 0usize;
        let mut mainline_hits = 0usize;
        let mut avg_coverage_acc = 0.0f64;
        let mut avg_rework_acc = 0.0f64;

        for task in &tasks {
            let provider = Arc::new(ReplayLlmProvider);
            let mut registry = ToolRegistry::new();
            registry.register(FileReadTool);
            let registry = Arc::new(registry);
            let tracker = Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new()));

            let loop_engine = ConversationLoop::new(
                provider,
                registry,
                tracker,
                "replay-model".to_string(),
            )
            .with_permission_mode(crate::permissions::PermissionMode::AutoAll)
            .with_max_iterations(4);

            let result = loop_engine
                .run(vec![Message::user(task.task_description.clone())])
                .await
                .expect("conversation loop should run");

            if result.content.contains("Workflow 执行报告") {
                workflow_reports += 1;
            }
            if result.content.contains("Mainline Hit: yes") {
                mainline_hits += 1;
            }
            avg_coverage_acc +=
                parse_metric_percent(&result.content, "First Plan Coverage").unwrap_or(0.0);
            avg_rework_acc += parse_metric_percent(&result.content, "Rework Rate").unwrap_or(0.0);
        }

        let n = tasks.len() as f64;
        let workflow_rate = workflow_reports as f64 / n * 100.0;
        let mainline_hit_rate = mainline_hits as f64 / n * 100.0;
        let avg_coverage = avg_coverage_acc / n;
        let avg_rework = avg_rework_acc / n;

        let report = format!(
            "# Workflow 真实开发流验收（Round 3 - 本周提交回放）\n\n- 样本任务数: {}\n- Workflow 触发率: {:.1}%\n- Mainline Hit Rate: {:.1}%\n- Avg First Plan Coverage: {:.1}%\n- Avg Rework Rate: {:.1}%\n\n## 结论\n- 使用本周真实提交提炼任务进行回放验收\n- Gate + Workflow + 真实工具执行链路可运行\n- 结果可用于下一轮优化决策\n",
            tasks.len(),
            workflow_rate,
            mainline_hit_rate,
            avg_coverage,
            avg_rework
        );

        let _ = std::fs::create_dir_all("docs/workflow");
        std::fs::write("docs/workflow/real-devflow-round3-report.md", report)
            .expect("write round3 report");

        assert!(
            workflow_rate >= 80.0,
            "workflow trigger rate too low: {:.1}%",
            workflow_rate
        );
        assert!(
            mainline_hit_rate >= 60.0,
            "mainline hit rate too low: {:.1}%",
            mainline_hit_rate
        );
    }
}

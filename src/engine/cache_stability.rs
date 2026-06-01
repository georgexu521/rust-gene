use crate::engine::context_compressor::estimate_tokens;
use crate::engine::prompt_context::stable_fingerprint;
use crate::services::api::{Message, Tool as ProviderTool};
use crate::tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSchemaCacheEntry {
    pub name: String,
    pub description: String,
    pub strict_schema: bool,
    pub parameters_canonical: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSchemaCacheManifest {
    pub tool_count: usize,
    pub estimated_tokens: u64,
    pub fingerprint: String,
    pub entries: Vec<ToolSchemaCacheEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PromptCacheUsage {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub cache_miss_tokens: u64,
    pub hit_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheDiagnosticShape {
    pub prefix_fingerprint: String,
    pub system_fingerprint: String,
    pub tool_schema_fingerprint: String,
    pub few_shots_fingerprint: String,
    pub dynamic_tail_fingerprint: String,
    pub tool_count: usize,
    pub tool_names: Vec<String>,
    pub message_count: usize,
    pub dynamic_zone_messages: usize,
    pub dynamic_zones_before_last_user: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheMissReason {
    NoMiss,
    ColdStart,
    SystemPromptChanged,
    ToolListChanged,
    ToolSchemaOrOrderChanged,
    MemoryOrSkillChanged,
    DynamicZoneMoved,
    DynamicTailChanged,
    Unknown,
}

impl CacheMissReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoMiss => "no-miss",
            Self::ColdStart => "cold-start",
            Self::SystemPromptChanged => "system-prompt-changed",
            Self::ToolListChanged => "tool-list-changed",
            Self::ToolSchemaOrOrderChanged => "tool-schema-or-order-changed",
            Self::MemoryOrSkillChanged => "memory-or-skill-changed",
            Self::DynamicZoneMoved => "dynamic-zone-moved",
            Self::DynamicTailChanged => "dynamic-tail-changed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheMissInference {
    pub reason: CacheMissReason,
    pub detail: String,
}

pub fn canonicalize_provider_tools(tools: &[ProviderTool]) -> Vec<ProviderTool> {
    let mut tools = tools.to_vec();
    tools.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.description.cmp(&right.description))
    });
    tools
}

pub fn provider_tool_schema_manifest(tools: &[ProviderTool]) -> ToolSchemaCacheManifest {
    let entries = canonicalize_provider_tools(tools)
        .iter()
        .map(provider_tool_entry)
        .collect::<Vec<_>>();
    manifest_from_entries(entries)
}

pub fn registry_tool_schema_manifest(registry: &ToolRegistry) -> ToolSchemaCacheManifest {
    let mut entries = registry
        .iter_tools()
        .map(|tool| ToolSchemaCacheEntry {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            strict_schema: tool.strict_schema(),
            parameters_canonical: canonical_json_text(&tool.parameters()),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.description.cmp(&right.description))
    });
    manifest_from_entries(entries)
}

pub fn prompt_cache_usage(prompt_tokens: u64, cached_tokens: Option<u64>) -> PromptCacheUsage {
    let cached_tokens = cached_tokens.unwrap_or(0).min(prompt_tokens);
    let cache_miss_tokens = prompt_tokens.saturating_sub(cached_tokens);
    let hit_ratio = if prompt_tokens == 0 {
        0.0
    } else {
        cached_tokens as f64 / prompt_tokens as f64
    };
    PromptCacheUsage {
        prompt_tokens,
        cached_tokens,
        cache_miss_tokens,
        hit_ratio,
    }
}

pub fn canonical_json_text(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_default(),
        Value::Array(items) => {
            let items = items
                .iter()
                .map(canonical_json_text)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{items}]")
        }
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let items = keys
                .into_iter()
                .map(|key| {
                    let key_json = serde_json::to_string(key).unwrap_or_default();
                    let value_json = canonical_json_text(&map[key]);
                    format!("{key_json}:{value_json}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{items}}}")
        }
    }
}

pub fn request_cache_diagnostic_shape(
    messages: &[Message],
    tools: &[ProviderTool],
) -> CacheDiagnosticShape {
    let canonical_tools = canonicalize_provider_tools(tools);
    let tool_manifest = provider_tool_schema_manifest(&canonical_tools);
    let tool_names = tool_manifest
        .entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect::<Vec<_>>();
    let system_prefix = stable_system_prefix(messages);
    let few_shots = few_shot_material(messages);
    let dynamic_tail = dynamic_tail_material(messages);
    let dynamic_zone_messages = messages
        .iter()
        .filter(|message| {
            matches!(message, Message::System { content } if is_dynamic_context_system_message(content))
        })
        .count();
    let last_user_index = messages
        .iter()
        .rposition(|message| matches!(message, Message::User { .. }));
    let dynamic_zones_before_last_user = messages
        .iter()
        .enumerate()
        .filter(|(index, message)| {
            matches!(message, Message::System { content } if is_dynamic_context_system_message(content))
                && last_user_index.map(|last_user| *index < last_user).unwrap_or(false)
        })
        .count();

    let prefix_fingerprint = stable_fingerprint(&format!(
        "system={}\ntools={}\nfew_shots={}",
        system_prefix, tool_manifest.fingerprint, few_shots
    ));

    CacheDiagnosticShape {
        prefix_fingerprint,
        system_fingerprint: stable_fingerprint(&system_prefix),
        tool_schema_fingerprint: tool_manifest.fingerprint,
        few_shots_fingerprint: stable_fingerprint(&few_shots),
        dynamic_tail_fingerprint: stable_fingerprint(&dynamic_tail),
        tool_count: tool_manifest.tool_count,
        tool_names,
        message_count: messages.len(),
        dynamic_zone_messages,
        dynamic_zones_before_last_user,
    }
}

/// Returns a list of prefix-change reasons, mirroring Reasonix's
/// `prefixChangeReasons`. Empty list means the prefix shape is stable
/// (likely cache hit). Each string is one of: "system", "tools",
/// "few_shots", "dynamic_tail", "log_rewrite".
pub fn prefix_change_reasons(
    previous: Option<&CacheDiagnosticShape>,
    current: &CacheDiagnosticShape,
) -> Vec<String> {
    let Some(previous) = previous else {
        return vec!["cold_start".to_string()];
    };
    let mut reasons = Vec::new();
    if previous.system_fingerprint != current.system_fingerprint {
        reasons.push("system".to_string());
    }
    if previous.tool_schema_fingerprint != current.tool_schema_fingerprint {
        reasons.push("tools".to_string());
    }
    if previous.few_shots_fingerprint != current.few_shots_fingerprint {
        reasons.push("few_shots".to_string());
    }
    if previous.dynamic_tail_fingerprint != current.dynamic_tail_fingerprint {
        reasons.push("dynamic_tail".to_string());
    }
    if previous.message_count != current.message_count {
        reasons.push("message_count".to_string());
    }
    reasons
}

pub fn infer_cache_miss_reason(
    previous: Option<&CacheDiagnosticShape>,
    current: &CacheDiagnosticShape,
    usage: PromptCacheUsage,
) -> CacheMissInference {
    if usage.cache_miss_tokens == 0 {
        return CacheMissInference {
            reason: CacheMissReason::NoMiss,
            detail: "provider reported no prompt-side cache miss tokens".to_string(),
        };
    }
    let Some(previous) = previous else {
        return CacheMissInference {
            reason: CacheMissReason::ColdStart,
            detail: "no previous cache diagnostic exists for this session".to_string(),
        };
    };
    if previous.system_fingerprint != current.system_fingerprint {
        return CacheMissInference {
            reason: CacheMissReason::SystemPromptChanged,
            detail: format!(
                "system {} -> {}",
                short_hash(&previous.system_fingerprint),
                short_hash(&current.system_fingerprint)
            ),
        };
    }
    if previous.few_shots_fingerprint != current.few_shots_fingerprint {
        return CacheMissInference {
            reason: CacheMissReason::MemoryOrSkillChanged,
            detail: format!(
                "few-shot/memory {} -> {}",
                short_hash(&previous.few_shots_fingerprint),
                short_hash(&current.few_shots_fingerprint)
            ),
        };
    }
    if previous.tool_schema_fingerprint != current.tool_schema_fingerprint {
        let added = current
            .tool_names
            .iter()
            .filter(|name| !previous.tool_names.contains(name))
            .cloned()
            .collect::<Vec<_>>();
        let removed = previous
            .tool_names
            .iter()
            .filter(|name| !current.tool_names.contains(name))
            .cloned()
            .collect::<Vec<_>>();
        if !added.is_empty() || !removed.is_empty() || previous.tool_count != current.tool_count {
            let mut parts = Vec::new();
            if !added.is_empty() {
                parts.push(format!("added {}", added.join(",")));
            }
            if !removed.is_empty() {
                parts.push(format!("removed {}", removed.join(",")));
            }
            if parts.is_empty() {
                parts.push(format!(
                    "tool count {} -> {}",
                    previous.tool_count, current.tool_count
                ));
            }
            return CacheMissInference {
                reason: CacheMissReason::ToolListChanged,
                detail: parts.join("; "),
            };
        }
        return CacheMissInference {
            reason: CacheMissReason::ToolSchemaOrOrderChanged,
            detail: format!(
                "tool schema {} -> {}",
                short_hash(&previous.tool_schema_fingerprint),
                short_hash(&current.tool_schema_fingerprint)
            ),
        };
    }
    if previous.dynamic_zones_before_last_user != current.dynamic_zones_before_last_user {
        return CacheMissInference {
            reason: CacheMissReason::DynamicZoneMoved,
            detail: format!(
                "dynamic zones before last user {} -> {}",
                previous.dynamic_zones_before_last_user, current.dynamic_zones_before_last_user
            ),
        };
    }
    if previous.dynamic_tail_fingerprint != current.dynamic_tail_fingerprint {
        return CacheMissInference {
            reason: CacheMissReason::DynamicTailChanged,
            detail: format!(
                "dynamic tail {} -> {}",
                short_hash(&previous.dynamic_tail_fingerprint),
                short_hash(&current.dynamic_tail_fingerprint)
            ),
        };
    }
    CacheMissInference {
        reason: CacheMissReason::Unknown,
        detail: "prefix hashes matched; miss likely came from provider TTL/state or bytes outside the stable prefix".to_string(),
    }
}

pub fn is_dynamic_context_system_message(content: &str) -> bool {
    let trimmed = content.trim_start();
    [
        "<task-state>",
        "<task_state>",
        "<task-contract>",
        "<context-pack>",
        "<relevant_material>",
        "<recent_observation>",
        "<self-evolution-guidance>",
        "<context_zones",
        "<retrieval-context",
        "MVA profile:",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

fn provider_tool_entry(tool: &ProviderTool) -> ToolSchemaCacheEntry {
    ToolSchemaCacheEntry {
        name: tool.name.clone(),
        description: tool.description.clone(),
        strict_schema: tool.strict_schema,
        parameters_canonical: canonical_json_text(&tool.parameters),
    }
}

fn manifest_from_entries(entries: Vec<ToolSchemaCacheEntry>) -> ToolSchemaCacheManifest {
    let estimated_tokens = entries
        .iter()
        .map(|entry| {
            estimate_tokens(&entry.name)
                + estimate_tokens(&entry.description)
                + estimate_tokens(&entry.parameters_canonical)
                + 10
        })
        .sum();
    let mut manifest_text = String::new();
    for entry in &entries {
        manifest_text.push_str(&entry.name);
        manifest_text.push('\n');
        manifest_text.push_str(&entry.description);
        manifest_text.push('\n');
        manifest_text.push_str(if entry.strict_schema {
            "strict=true"
        } else {
            "strict=false"
        });
        manifest_text.push('\n');
        manifest_text.push_str(&entry.parameters_canonical);
        manifest_text.push('\n');
    }
    ToolSchemaCacheManifest {
        tool_count: entries.len(),
        estimated_tokens,
        fingerprint: stable_fingerprint(&manifest_text),
        entries,
    }
}

fn stable_system_prefix(messages: &[Message]) -> String {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::System { content } if !is_dynamic_context_system_message(content) => {
                Some(content.trim())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn few_shot_material(messages: &[Message]) -> String {
    let Some(first_user) = messages
        .iter()
        .position(|message| matches!(message, Message::User { .. }))
    else {
        return String::new();
    };
    messages[..first_user]
        .iter()
        .filter_map(|message| match message {
            Message::User { content } => Some(format!("user:{content}")),
            Message::Assistant {
                content,
                tool_calls,
            } => Some(format!(
                "assistant:{}\ntool_calls:{}",
                content,
                tool_calls
                    .as_ref()
                    .and_then(|calls| serde_json::to_string(calls).ok())
                    .unwrap_or_default()
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn dynamic_tail_material(messages: &[Message]) -> String {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::System { content } if is_dynamic_context_system_message(content) => {
                Some(format!("system:{content}"))
            }
            Message::User { content } => Some(format!("user:{content}")),
            Message::Assistant {
                content,
                tool_calls,
            } => Some(format!(
                "assistant:{}\ntool_calls:{}",
                content,
                tool_calls
                    .as_ref()
                    .and_then(|calls| serde_json::to_string(calls).ok())
                    .unwrap_or_default()
            )),
            Message::Tool {
                tool_call_id,
                content,
            } => Some(format!("tool:{tool_call_id}:{content}")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_tool(name: &str, parameters: Value) -> ProviderTool {
        ProviderTool {
            name: name.to_string(),
            description: format!("{name} description"),
            parameters,
            strict_schema: false,
        }
    }

    #[test]
    fn provider_tool_schema_fingerprint_is_order_independent() {
        let alpha = provider_tool(
            "alpha",
            serde_json::json!({"type": "object", "properties": {"b": {}, "a": {}}}),
        );
        let beta = provider_tool(
            "beta",
            serde_json::json!({"required": ["x"], "type": "object"}),
        );

        let left = provider_tool_schema_manifest(&[beta.clone(), alpha.clone()]);
        let right = provider_tool_schema_manifest(&[alpha, beta]);

        assert_eq!(left.fingerprint, right.fingerprint);
        assert_eq!(
            left.entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
    }

    #[test]
    fn canonical_json_sorts_object_keys_recursively() {
        let value = serde_json::json!({
            "z": 1,
            "a": {"d": true, "b": false},
            "m": [ {"y": 2, "x": 1} ]
        });
        assert_eq!(
            canonical_json_text(&value),
            r#"{"a":{"b":false,"d":true},"m":[{"x":1,"y":2}],"z":1}"#
        );
    }

    #[test]
    fn prompt_cache_usage_clamps_cached_tokens_to_prompt_tokens() {
        let usage = prompt_cache_usage(100, Some(120));
        assert_eq!(usage.cached_tokens, 100);
        assert_eq!(usage.cache_miss_tokens, 0);
        assert_eq!(usage.hit_ratio, 1.0);

        let usage = prompt_cache_usage(100, Some(75));
        assert_eq!(usage.cache_miss_tokens, 25);
        assert!((usage.hit_ratio - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn request_cache_shape_ignores_tool_order_and_tracks_dynamic_tail() {
        let alpha = provider_tool(
            "alpha",
            serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        );
        let beta = provider_tool("beta", serde_json::json!({"type": "object"}));
        let messages = vec![
            Message::system("stable system"),
            Message::system("<context_zones>\n<task-state>fresh</task-state>\n</context_zones>"),
            Message::user("change the page"),
        ];

        let left = request_cache_diagnostic_shape(&messages, &[beta.clone(), alpha.clone()]);
        let right = request_cache_diagnostic_shape(&messages, &[alpha, beta]);

        assert_eq!(left.tool_schema_fingerprint, right.tool_schema_fingerprint);
        assert_eq!(left.tool_names, vec!["alpha", "beta"]);
        assert_eq!(left.dynamic_zone_messages, 1);
        assert_eq!(left.dynamic_zones_before_last_user, 1);
        assert!(!left.dynamic_tail_fingerprint.is_empty());
    }

    #[test]
    fn dynamic_context_changes_do_not_bust_stable_prefix_fingerprint() {
        let alpha = provider_tool("alpha", serde_json::json!({"type": "object"}));
        let first = vec![
            Message::system("stable system"),
            Message::system("<context_zones>\n<task-state>one</task-state>\n</context_zones>"),
            Message::user("change the page"),
        ];
        let second = vec![
            Message::system("stable system"),
            Message::system("<context_zones>\n<task-state>two</task-state>\n</context_zones>"),
            Message::user("change the page"),
        ];

        let left = request_cache_diagnostic_shape(&first, std::slice::from_ref(&alpha));
        let right = request_cache_diagnostic_shape(&second, std::slice::from_ref(&alpha));

        assert_eq!(left.prefix_fingerprint, right.prefix_fingerprint);
        assert_ne!(
            left.dynamic_tail_fingerprint,
            right.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn cache_miss_inference_identifies_tool_and_tail_changes() {
        let alpha = provider_tool("alpha", serde_json::json!({"type": "object"}));
        let beta = provider_tool("beta", serde_json::json!({"type": "object"}));
        let base_messages = vec![Message::system("stable system"), Message::user("one")];
        let tail_messages = vec![Message::system("stable system"), Message::user("two")];
        let usage = prompt_cache_usage(100, Some(20));

        let previous = request_cache_diagnostic_shape(&base_messages, std::slice::from_ref(&alpha));
        let tool_changed = request_cache_diagnostic_shape(&base_messages, &[alpha.clone(), beta]);
        let inferred = infer_cache_miss_reason(Some(&previous), &tool_changed, usage);
        assert_eq!(inferred.reason, CacheMissReason::ToolListChanged);

        let tail_changed =
            request_cache_diagnostic_shape(&tail_messages, std::slice::from_ref(&alpha));
        let inferred = infer_cache_miss_reason(Some(&previous), &tail_changed, usage);
        assert_eq!(inferred.reason, CacheMissReason::DynamicTailChanged);
    }
}

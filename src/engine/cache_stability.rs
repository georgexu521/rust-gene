//! Prompt-cache stability accounting.
//!
//! This module fingerprints stable-prefix inputs and estimates cacheable token
//! regions so runtime diagnostics can explain cache misses and compaction
//! decisions.

use crate::engine::context_compressor::{
    estimate_tokens, estimate_tokens_for_profile, TokenEstimateProfile,
};
use crate::engine::dynamic_context;
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
    pub tool_schema_tokens: u64,
    pub tool_names: Vec<String>,
    pub message_count: usize,
    pub dynamic_zone_messages: usize,
    pub dynamic_zones_before_last_user: usize,
    pub request_phase: Option<String>,
    pub effective_output_cap: Option<u32>,
    pub tool_round_count: Option<u64>,
    pub compaction_decision: Option<String>,
    /// Dynamic zone counts by tier (Phase C/D: opencode alignment).
    /// stable-prefix: count of zones eligible for stable prefix inclusion.
    /// last-user: count of zones injected after the last user message.
    /// repair-only: count of zones injected only during repair turns.
    #[serde(default)]
    pub dynamic_zones_stable_prefix: usize,
    #[serde(default)]
    pub dynamic_zones_last_user: usize,
    #[serde(default)]
    pub dynamic_zones_repair_only: usize,
}

/// Dynamic context zone tier classification.
/// Phase C: maps each injected system-message zone to one of three
/// stability tiers so the runtime can record zone composition in traces
/// and usage ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicZoneTier {
    /// Stable enough to live in the prefix (task contract, context pack).
    StablePrefix,
    /// Changes per-turn but sits after the last user message.
    LastUserDynamic,
    /// Only injected during repair / failure recovery turns.
    RepairOnly,
}

impl DynamicZoneTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::StablePrefix => "stable-prefix",
            Self::LastUserDynamic => "last-user",
            Self::RepairOnly => "repair-only",
        }
    }
}

/// Classify a dynamic context system message into one of three tiers.
pub fn classify_dynamic_zone(content: &str) -> DynamicZoneTier {
    let trimmed = content.trim_start();
    // Stable enough for the prefix (rarely changes during a task)
    if trimmed.starts_with("<task-contract>")
        || trimmed.starts_with("<context-pack>")
        || content.contains("<task-contract>")
        || content.contains("<context-pack>")
    {
        return DynamicZoneTier::StablePrefix;
    }
    // Only present during repair/failure turns
    if trimmed.starts_with("<recent_observation>")
        || trimmed.starts_with("<focused-repair>")
        || trimmed.starts_with("<self-evolution-guidance>")
        || trimmed.starts_with("MVA profile:")
        || content.contains("<recent_observation>")
        || content.contains("<focused-repair>")
        || content.contains("<self-evolution-guidance>")
    {
        return DynamicZoneTier::RepairOnly;
    }
    // Default: per-turn dynamic, sits with the user message
    DynamicZoneTier::LastUserDynamic
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
        .filter(|message| message_contains_dynamic_context(message))
        .count();
    let last_user_index = messages
        .iter()
        .rposition(|message| matches!(message, Message::User { .. }));
    let dynamic_zones_before_last_user = messages
        .iter()
        .enumerate()
        .filter(|(index, message)| {
            message_contains_dynamic_context(message)
                && last_user_index
                    .map(|last_user| *index < last_user)
                    .unwrap_or(false)
        })
        .count();

    // Count dynamic zones by tier for trace / usage ledger.
    let mut dynamic_zones_stable_prefix: usize = 0;
    let mut dynamic_zones_last_user: usize = 0;
    let mut dynamic_zones_repair_only: usize = 0;
    for message in messages {
        if let Some(content) = message_dynamic_context_content(message) {
            match classify_dynamic_zone(content) {
                DynamicZoneTier::StablePrefix => dynamic_zones_stable_prefix += 1,
                DynamicZoneTier::LastUserDynamic => dynamic_zones_last_user += 1,
                DynamicZoneTier::RepairOnly => dynamic_zones_repair_only += 1,
            }
        }
    }

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
        tool_schema_tokens: tool_manifest.estimated_tokens,
        tool_names,
        message_count: messages.len(),
        dynamic_zone_messages,
        dynamic_zones_before_last_user,
        dynamic_zones_stable_prefix,
        dynamic_zones_last_user,
        dynamic_zones_repair_only,
        request_phase: None,
        effective_output_cap: None,
        tool_round_count: None,
        compaction_decision: None,
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
                "few-shot/memory/skill {} -> {} (dynamic zones: prev {} {}, now {} {})",
                short_hash(&previous.few_shots_fingerprint),
                short_hash(&current.few_shots_fingerprint),
                previous.dynamic_zone_messages,
                if previous.dynamic_zone_messages == 1 {
                    "zone"
                } else {
                    "zones"
                },
                current.dynamic_zone_messages,
                if current.dynamic_zone_messages == 1 {
                    "zone"
                } else {
                    "zones"
                },
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
    dynamic_context::is_dynamic_context_system_message(content)
}

fn message_contains_dynamic_context(message: &Message) -> bool {
    message_dynamic_context_content(message).is_some()
}

fn message_dynamic_context_content(message: &Message) -> Option<&str> {
    match message {
        Message::System { content } if is_dynamic_context_system_message(content) => {
            Some(content.as_str())
        }
        Message::User { content } if user_message_contains_dynamic_context(content) => {
            Some(content.as_str())
        }
        _ => None,
    }
}

fn user_message_contains_dynamic_context(content: &str) -> bool {
    dynamic_context::user_message_contains_dynamic_context(content)
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
                + estimate_tokens_for_profile(
                    &entry.parameters_canonical,
                    TokenEstimateProfile::JsonToolSchema,
                )
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

    #[test]
    fn cache_stability_matrix_dynamic_zones_excluded_from_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let messages = vec![
            Message::system("stable system prefix"),
            Message::system("<task-state>turn-1-state</task-state>"),
            Message::system("<recent_observation>obs-1</recent_observation>"),
            Message::user("fix the bug"),
        ];

        let shape = request_cache_diagnostic_shape(&messages, std::slice::from_ref(&tool));

        assert_eq!(shape.dynamic_zone_messages, 2);
        assert!(!shape.prefix_fingerprint.is_empty());
        assert!(shape.prefix_fingerprint.len() == 12);
    }

    #[test]
    fn cache_diagnostic_counts_dynamic_context_prepended_to_user_message() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let messages = vec![
            Message::system("stable system prefix"),
            Message::user(
                "<task-state>turn-1-state</task-state>\n\
                 <recent_observation>validation failed</recent_observation>\n\
                 fix the bug",
            ),
        ];

        let shape = request_cache_diagnostic_shape(&messages, std::slice::from_ref(&tool));

        assert_eq!(shape.dynamic_zone_messages, 1);
        assert_eq!(shape.dynamic_zones_before_last_user, 0);
        assert_eq!(shape.dynamic_zones_repair_only, 1);
    }

    #[test]
    fn stable_prefix_unchanged_by_dynamic_context_changes() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let base_messages = vec![
            Message::system("stable system"),
            Message::system("<task-state>state-v1</task-state>"),
            Message::user("do something"),
        ];
        let with_update = vec![
            Message::system("stable system"),
            Message::system("<task-state>state-v2</task-state>"),
            Message::user("do something"),
        ];

        let base = request_cache_diagnostic_shape(&base_messages, std::slice::from_ref(&tool));
        let updated = request_cache_diagnostic_shape(&with_update, std::slice::from_ref(&tool));

        assert_eq!(base.prefix_fingerprint, updated.prefix_fingerprint);
        assert_ne!(
            base.dynamic_tail_fingerprint,
            updated.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn stable_prefix_unchanged_by_user_message_count() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let single_turn = vec![
            Message::system("stable system"),
            Message::user("first request"),
        ];
        let multi_turn = vec![
            Message::system("stable system"),
            Message::user("first request"),
            Message::assistant("done"),
            Message::user("second request"),
        ];

        let first = request_cache_diagnostic_shape(&single_turn, std::slice::from_ref(&tool));
        let second = request_cache_diagnostic_shape(&multi_turn, std::slice::from_ref(&tool));

        assert_eq!(first.prefix_fingerprint, second.prefix_fingerprint);
        assert_ne!(
            first.dynamic_tail_fingerprint,
            second.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn stable_prefix_unchanged_by_dynamic_memory_context() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let base_messages = vec![
            Message::system("stable system"),
            Message::system("<context-pack>memory-records=0</context-pack>"),
            Message::user("remember this"),
        ];
        let with_record = vec![
            Message::system("stable system"),
            Message::system(
                "<context-pack>memory-records=1: user prefers dark mode</context-pack>",
            ),
            Message::user("remember this"),
        ];

        let base = request_cache_diagnostic_shape(&base_messages, std::slice::from_ref(&tool));
        let record = request_cache_diagnostic_shape(&with_record, std::slice::from_ref(&tool));

        assert_eq!(base.prefix_fingerprint, record.prefix_fingerprint);
    }

    #[test]
    fn stable_prefix_unchanged_by_dynamic_retrieval_context() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let base_messages = vec![
            Message::system("stable system"),
            Message::system("<retrieval-context>items=0</retrieval-context>"),
            Message::user("search for something"),
        ];
        let with_results = vec![
            Message::system("stable system"),
            Message::system("<retrieval-context>items=3: found relevant docs</retrieval-context>"),
            Message::user("search for something"),
        ];

        let base = request_cache_diagnostic_shape(&base_messages, std::slice::from_ref(&tool));
        let results = request_cache_diagnostic_shape(&with_results, std::slice::from_ref(&tool));

        assert_eq!(base.prefix_fingerprint, results.prefix_fingerprint);
    }

    #[test]
    fn prefix_change_reasons_detects_only_real_changes() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let messages_a = vec![Message::system("system v1"), Message::user("request")];
        let messages_b = vec![Message::system("system v2"), Message::user("request")];

        let shape_a = request_cache_diagnostic_shape(&messages_a, std::slice::from_ref(&tool));
        let shape_b = request_cache_diagnostic_shape(&messages_b, std::slice::from_ref(&tool));

        let reasons = prefix_change_reasons(Some(&shape_a), &shape_b);
        assert!(reasons.contains(&"system".to_string()));
        assert!(!reasons.contains(&"tools".to_string()));

        let no_change = prefix_change_reasons(Some(&shape_a), &shape_a);
        assert!(no_change.is_empty());
    }

    #[test]
    fn stable_prefix_stable_across_equivalent_tool_sets() {
        let tool_a = provider_tool("alpha", serde_json::json!({"type": "object"}));
        let tool_b = provider_tool("beta", serde_json::json!({"type": "object"}));
        let messages = vec![Message::system("system"), Message::user("go")];

        let ab = request_cache_diagnostic_shape(&messages, &[tool_a.clone(), tool_b.clone()]);
        let ba = request_cache_diagnostic_shape(&messages, &[tool_b, tool_a]);

        assert_eq!(ab.prefix_fingerprint, ba.prefix_fingerprint);
        assert_eq!(ab.tool_schema_fingerprint, ba.tool_schema_fingerprint);
        assert_eq!(ab.tool_names, ba.tool_names);
    }

    #[test]
    fn stable_prefix_changes_when_system_prompt_changes() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let v1 = vec![Message::system("instructions v1"), Message::user("go")];
        let v2 = vec![Message::system("instructions v2"), Message::user("go")];

        let shape_v1 = request_cache_diagnostic_shape(&v1, std::slice::from_ref(&tool));
        let shape_v2 = request_cache_diagnostic_shape(&v2, std::slice::from_ref(&tool));

        assert_ne!(shape_v1.prefix_fingerprint, shape_v2.prefix_fingerprint);
        assert_ne!(shape_v1.system_fingerprint, shape_v2.system_fingerprint);
    }

    #[test]
    fn stable_prefix_changes_when_tool_added_or_removed() {
        let tool_a = provider_tool("alpha", serde_json::json!({"type": "object"}));
        let tool_b = provider_tool("beta", serde_json::json!({"type": "object"}));
        let messages = vec![Message::system("system"), Message::user("go")];

        let one_tool = request_cache_diagnostic_shape(&messages, std::slice::from_ref(&tool_a));
        let two_tools = request_cache_diagnostic_shape(&messages, &[tool_a, tool_b]);

        assert_ne!(one_tool.prefix_fingerprint, two_tools.prefix_fingerprint);
        assert_ne!(
            one_tool.tool_schema_fingerprint,
            two_tools.tool_schema_fingerprint
        );
    }

    // ---- Phase 2 (Reasonix alignment): scenario-specific stable-prefix tests ----

    #[test]
    fn tool_failure_notice_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let without_failure = vec![
            Message::system("stable system"),
            Message::system("<recent_observation>\nno failures\n</recent_observation>"),
            Message::user("re-run tests"),
        ];
        let with_failure = vec![
            Message::system("stable system"),
            Message::system("<recent_observation>\ntool failure: cargo test exited code 1\n</recent_observation>"),
            Message::user("re-run tests"),
        ];

        let without = request_cache_diagnostic_shape(&without_failure, std::slice::from_ref(&tool));
        let with = request_cache_diagnostic_shape(&with_failure, std::slice::from_ref(&tool));

        assert_eq!(without.prefix_fingerprint, with.prefix_fingerprint);
        assert_ne!(
            without.dynamic_tail_fingerprint,
            with.dynamic_tail_fingerprint
        );
        assert_eq!(with.dynamic_zone_messages, 1);
    }

    #[test]
    fn repair_hint_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let base = vec![
            Message::system("stable system"),
            Message::system(
                "<recent_observation>\nrepair hint: check file encoding\n</recent_observation>",
            ),
            Message::user("fix the import"),
        ];
        let without_hint = vec![
            Message::system("stable system"),
            Message::user("fix the import"),
        ];

        let shape_base = request_cache_diagnostic_shape(&base, std::slice::from_ref(&tool));
        let shape_no_hint =
            request_cache_diagnostic_shape(&without_hint, std::slice::from_ref(&tool));

        assert_eq!(
            shape_base.prefix_fingerprint,
            shape_no_hint.prefix_fingerprint
        );
    }

    #[test]
    fn provider_slow_warning_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let normal = vec![
            Message::system("stable system"),
            Message::system("<recent_observation>\nprovider latency normal\n</recent_observation>"),
            Message::user("next step"),
        ];
        let with_warning = vec![
            Message::system("stable system"),
            Message::system(
                "<recent_observation>\nprovider slow warning: 45s elapsed\n</recent_observation>",
            ),
            Message::user("next step"),
        ];

        let shape_normal = request_cache_diagnostic_shape(&normal, std::slice::from_ref(&tool));
        let shape_warning =
            request_cache_diagnostic_shape(&with_warning, std::slice::from_ref(&tool));

        assert_eq!(
            shape_normal.prefix_fingerprint,
            shape_warning.prefix_fingerprint
        );
        assert_ne!(
            shape_normal.dynamic_tail_fingerprint,
            shape_warning.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn background_job_notice_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let before = vec![
            Message::system("stable system"),
            Message::system(
                "<recent_observation>\nbackground job: cargo build started\n</recent_observation>",
            ),
            Message::user("check progress"),
        ];
        let after = vec![
            Message::system("stable system"),
            Message::system("<recent_observation>\nbackground job: cargo build completed\n</recent_observation>"),
            Message::user("check progress"),
        ];

        let shape_before = request_cache_diagnostic_shape(&before, std::slice::from_ref(&tool));
        let shape_after = request_cache_diagnostic_shape(&after, std::slice::from_ref(&tool));

        assert_eq!(
            shape_before.prefix_fingerprint,
            shape_after.prefix_fingerprint
        );
        assert_ne!(
            shape_before.dynamic_tail_fingerprint,
            shape_after.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn closeout_repair_feedback_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let without_feedback = vec![
            Message::system("stable system"),
            Message::system("<recent_observation>\ncloseout: passed\n</recent_observation>"),
            Message::user("done"),
        ];
        let with_feedback = vec![
            Message::system("stable system"),
            Message::system("<recent_observation>\ncloseout: failed, suggest running clippy\n</recent_observation>"),
            Message::user("done"),
        ];

        let shape_clean =
            request_cache_diagnostic_shape(&without_feedback, std::slice::from_ref(&tool));
        let shape_feedback =
            request_cache_diagnostic_shape(&with_feedback, std::slice::from_ref(&tool));

        assert_eq!(
            shape_clean.prefix_fingerprint,
            shape_feedback.prefix_fingerprint
        );
        assert_ne!(
            shape_clean.dynamic_tail_fingerprint,
            shape_feedback.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn unexpected_system_message_mid_session_changes_prefix_fingerprint() {
        // An untagged system message that is NOT recognized as a dynamic zone
        // becomes part of the stable prefix and therefore busts the cache.
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let before = vec![
            Message::system("stable system"),
            Message::user("do something"),
        ];
        let after = vec![
            Message::system("stable system"),
            Message::system("unexpected mid-session advisory message without zone tags"),
            Message::user("do something"),
        ];

        let shape_before = request_cache_diagnostic_shape(&before, std::slice::from_ref(&tool));
        let shape_after = request_cache_diagnostic_shape(&after, std::slice::from_ref(&tool));

        // The unexpected system message IS part of the prefix → fingerprint differs.
        assert_ne!(
            shape_before.prefix_fingerprint,
            shape_after.prefix_fingerprint
        );
        // It should NOT be counted as a dynamic zone message.
        assert_eq!(shape_after.dynamic_zone_messages, 0);
        // Verify it was classified as stable system material.
        assert_ne!(
            shape_before.system_fingerprint,
            shape_after.system_fingerprint
        );
    }

    #[test]
    fn cache_shape_end_to_end_baseline_snapshot() {
        // Baseline regression test: a representative full prompt shape
        // (system + tools + dynamic zones + user message) produces a stable
        // non-empty fingerprint.
        let tool_a = provider_tool(
            "file_read",
            serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        );
        let tool_b = provider_tool(
            "bash",
            serde_json::json!({"type": "object", "properties": {"command": {"type": "string"}}}),
        );
        let tool_c = provider_tool("glob", serde_json::json!({"type": "object"}));

        let messages = vec![
            Message::system("You are a coding assistant."),
            Message::system("<task-state>\nfix bug #42 in src/parser.rs\n</task-state>"),
            Message::system("<context-pack>\nproject: rust-agent, language: Rust\n</context-pack>"),
            Message::system(
                "<relevant_material>\nparser.rs:239 handles edge cases\n</relevant_material>",
            ),
            Message::system(
                "<recent_observation>\nprevious edit failed at line 240\n</recent_observation>",
            ),
            Message::user("apply the fix"),
        ];

        let shape = request_cache_diagnostic_shape(
            &messages,
            &[tool_a.clone(), tool_b.clone(), tool_c.clone()],
        );

        assert!(!shape.prefix_fingerprint.is_empty());
        assert!(!shape.system_fingerprint.is_empty());
        assert!(!shape.tool_schema_fingerprint.is_empty());
        assert_eq!(shape.tool_count, 3);
        assert_eq!(shape.tool_names, vec!["bash", "file_read", "glob"]);
        assert_eq!(shape.dynamic_zone_messages, 4);
        assert_eq!(shape.message_count, 6);

        // Re-compute with same inputs — fingerprint must be identical.
        let shape_again = request_cache_diagnostic_shape(&messages, &[tool_a, tool_b, tool_c]);
        assert_eq!(shape.prefix_fingerprint, shape_again.prefix_fingerprint);
        assert_eq!(
            shape.tool_schema_fingerprint,
            shape_again.tool_schema_fingerprint
        );
        assert_eq!(
            shape.dynamic_tail_fingerprint,
            shape_again.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn mid_session_memory_save_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let before_save = vec![
            Message::system("stable system"),
            Message::system("<context-pack>\nmemory-records=2\n</context-pack>"),
            Message::user("save preference"),
        ];
        let after_save = vec![
            Message::system("stable system"),
            Message::system(
                "<context-pack>\nmemory-records=3: user prefers tabs over spaces\n</context-pack>",
            ),
            Message::user("save preference"),
        ];

        let shape_before =
            request_cache_diagnostic_shape(&before_save, std::slice::from_ref(&tool));
        let shape_after = request_cache_diagnostic_shape(&after_save, std::slice::from_ref(&tool));

        assert_eq!(
            shape_before.prefix_fingerprint,
            shape_after.prefix_fingerprint
        );
        assert_ne!(
            shape_before.dynamic_tail_fingerprint,
            shape_after.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn plan_mode_toggle_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let normal_mode = vec![
            Message::system("stable system"),
            Message::system(
                "<task-state>\nmode: standard, task: refactor auth module\n</task-state>",
            ),
            Message::user("refactor"),
        ];
        let plan_mode = vec![
            Message::system("stable system"),
            Message::system(
                "<task-state>\nmode: plan-only, task: refactor auth module\n</task-state>",
            ),
            Message::user("refactor"),
        ];

        let shape_normal =
            request_cache_diagnostic_shape(&normal_mode, std::slice::from_ref(&tool));
        let shape_plan = request_cache_diagnostic_shape(&plan_mode, std::slice::from_ref(&tool));

        assert_eq!(
            shape_normal.prefix_fingerprint,
            shape_plan.prefix_fingerprint
        );
        assert_ne!(
            shape_normal.dynamic_tail_fingerprint,
            shape_plan.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn task_focus_change_does_not_bust_stable_prefix() {
        let tool = provider_tool("bash", serde_json::json!({"type": "object"}));
        let focus_a = vec![
            Message::system("stable system"),
            Message::system("<task-state>\nfocus: implement login endpoint\n</task-state>"),
            Message::user("next"),
        ];
        let focus_b = vec![
            Message::system("stable system"),
            Message::system("<task-state>\nfocus: implement logout endpoint\n</task-state>"),
            Message::user("next"),
        ];

        let shape_a = request_cache_diagnostic_shape(&focus_a, std::slice::from_ref(&tool));
        let shape_b = request_cache_diagnostic_shape(&focus_b, std::slice::from_ref(&tool));

        assert_eq!(shape_a.prefix_fingerprint, shape_b.prefix_fingerprint);
        assert_ne!(
            shape_a.dynamic_tail_fingerprint,
            shape_b.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn cache_shape_fingerprint_is_deterministic() {
        // Prove that for the same inputs, all fingerprints are identical
        // across multiple calls (no randomness, no timestamp influence).
        let tool = provider_tool("file_read", serde_json::json!({"type": "object"}));
        let messages = vec![
            Message::system("You are a coding agent."),
            Message::system("<task-state>\ntask: add unit tests\n</task-state>"),
            Message::user("add tests"),
        ];

        let mut shapes = Vec::new();
        for _ in 0..5 {
            shapes.push(request_cache_diagnostic_shape(
                &messages,
                std::slice::from_ref(&tool),
            ));
        }

        let first = &shapes[0];
        for shape in &shapes[1..] {
            assert_eq!(first.prefix_fingerprint, shape.prefix_fingerprint);
            assert_eq!(first.system_fingerprint, shape.system_fingerprint);
            assert_eq!(first.tool_schema_fingerprint, shape.tool_schema_fingerprint);
            assert_eq!(first.few_shots_fingerprint, shape.few_shots_fingerprint);
            assert_eq!(
                first.dynamic_tail_fingerprint,
                shape.dynamic_tail_fingerprint
            );
        }
    }
}

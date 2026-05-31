use crate::engine::context_compressor::estimate_tokens;
use crate::engine::prompt_context::stable_fingerprint;
use crate::services::api::Tool as ProviderTool;
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
}

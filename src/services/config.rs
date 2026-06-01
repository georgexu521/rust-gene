//! 配置管理
//!
//! 管理应用配置，支持配置文件和环境变量

use anyhow::Result;
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

type ConfigCallback = dyn Fn(&AppConfig) + Send + Sync;

/// 配置钩子 - 配置加载后调用的回调
pub struct ConfigHook {
    /// 钩子名称
    pub name: String,
    /// 配置加载后回调（接收配置快照）
    pub callback: Option<Box<ConfigCallback>>,
}

impl std::fmt::Debug for ConfigHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigHook")
            .field("name", &self.name)
            .finish()
    }
}

impl Clone for ConfigHook {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            callback: None, // Cannot clone closures
        }
    }
}

impl ConfigHook {
    /// 创建 ConfigHook
    pub fn new(
        name: impl Into<String>,
        callback: impl Fn(&AppConfig) + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            callback: Some(Box::new(callback)),
        }
    }

    /// 执行钩子回调
    pub fn execute(&self, config: &AppConfig) {
        if let Some(ref cb) = self.callback {
            cb(config);
        }
    }
}

/// 配置加载器 - 支持加载后钩子回调
pub struct ConfigLoader {
    hooks: Vec<ConfigHook>,
}

impl ConfigLoader {
    /// 创建新的配置加载器
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// 注册配置加载后钩子
    pub fn register_hook(&mut self, hook: ConfigHook) {
        self.hooks.push(hook);
    }

    /// 加载配置并触发所有钩子
    pub fn load(&self) -> Result<AppConfig, ConfigError> {
        let config = AppConfig::load()?;

        // 触发所有钩子
        for hook in &self.hooks {
            hook.execute(&config);
        }

        Ok(config)
    }

    /// 从环境变量创建并加载配置
    pub fn load_from_env() -> Result<AppConfig, ConfigError> {
        static LOADER: std::sync::OnceLock<ConfigLoader> = std::sync::OnceLock::new();
        let loader = LOADER.get_or_init(ConfigLoader::new);
        loader.load()
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// 应用配置
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AppConfig {
    /// API 配置
    pub api: ApiConfig,
    /// UI 配置
    pub ui: UiConfig,
    /// 存储配置
    pub storage: StorageConfig,
    /// 功能开关
    pub features: FeatureFlags,
    /// 引擎配置
    #[serde(default)]
    pub engine: EngineConfig,
    /// Memory provider configuration
    #[serde(default)]
    pub memory: MemoryConfig,
}

impl AppConfig {
    /// 从默认位置加载配置
    pub fn load() -> Result<Self, ConfigError> {
        let config_dir = dirs::config_dir()
            .map(|d| d.join("priority-agent"))
            .unwrap_or_else(|| PathBuf::from(".priority-agent"));

        let config_path = config_dir.join("config.toml");

        let builder = Config::builder()
            // 1. 默认配置 (provider-agnostic, 实际值由 env vars 决定)
            .set_default("api.model", "")?
            .set_default("api.base_url", "")?
            .set_default("ui.theme", "dark")?
            .set_default("storage.persistence_enabled", true)?
            .set_default("features.tui_enabled", true)?
            .set_default("features.agent_enabled", true)?
            .set_default("features.llm_memory_extraction", true)?
            .set_default("features.plugin_trust_mode", "warn")?
            .set_default("engine.max_iterations", 50)?
            .set_default("memory.external_provider.enabled", false)?
            .set_default("memory.external_provider.provider_type", "none")?
            .set_default("memory.external_provider.name", "external-memory")?
            .set_default("memory.external_provider.prompt_block", true)?
            .set_default("memory.external_provider.prefetch", true)?
            .set_default("memory.external_provider.search", true)?
            .set_default("memory.external_provider.queue_prefetch", false)?
            .set_default("memory.external_provider.sync_turn", false)?
            .set_default("memory.external_provider.session_end", false)?
            .set_default("memory.external_provider.pre_compress", false)?
            .set_default("memory.external_provider.write_mirror", false)?
            .set_default("memory.external_provider.tools", false)?
            // 2. 配置文件
            .add_source(File::from(config_path).required(false))
            // 3. 环境变量（前缀 PRIORITY_AGENT）
            .add_source(
                Environment::with_prefix("PRIORITY_AGENT")
                    .separator("_")
                    .try_parsing(true),
            );

        let config = builder.build()?;
        config.try_deserialize()
    }

    /// 保存配置到文件
    pub fn save(&self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .map(|d| d.join("priority-agent"))
            .unwrap_or_else(|| PathBuf::from(".priority-agent"));

        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.toml");

        let toml = toml::to_string_pretty(self)?;
        std::fs::write(config_path, toml)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigScopePaths {
    pub user_config: PathBuf,
    pub project_config: PathBuf,
    pub legacy_config_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigKeySpec {
    pub key: &'static str,
    pub value_type: &'static str,
    pub mutable: bool,
    pub secret: bool,
    pub description: &'static str,
}

pub const CONFIG_KEY_SPECS: &[ConfigKeySpec] = &[
    ConfigKeySpec {
        key: "api.base_url",
        value_type: "string",
        mutable: true,
        secret: false,
        description: "LLM provider base URL",
    },
    ConfigKeySpec {
        key: "api.model",
        value_type: "string",
        mutable: true,
        secret: false,
        description: "Default LLM model",
    },
    ConfigKeySpec {
        key: "api.temperature",
        value_type: "float",
        mutable: true,
        secret: false,
        description: "Sampling temperature",
    },
    ConfigKeySpec {
        key: "api.max_tokens",
        value_type: "integer|none",
        mutable: true,
        secret: false,
        description: "Optional max tokens",
    },
    ConfigKeySpec {
        key: "ui.theme",
        value_type: "string",
        mutable: true,
        secret: false,
        description: "UI theme",
    },
    ConfigKeySpec {
        key: "ui.show_token_usage",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Show token usage in UI",
    },
    ConfigKeySpec {
        key: "ui.compact_mode",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Compact UI layout",
    },
    ConfigKeySpec {
        key: "storage.persistence_enabled",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Persist sessions and state",
    },
    ConfigKeySpec {
        key: "storage.auto_save_interval_secs",
        value_type: "integer",
        mutable: true,
        secret: false,
        description: "Auto-save interval",
    },
    ConfigKeySpec {
        key: "features.mcp_enabled",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Enable MCP features",
    },
    ConfigKeySpec {
        key: "features.skills_enabled",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Enable skills",
    },
    ConfigKeySpec {
        key: "features.web_search",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Enable web search",
    },
    ConfigKeySpec {
        key: "features.plugin_trust_mode",
        value_type: "strict|warn|off",
        mutable: true,
        secret: false,
        description: "Plugin signature trust policy",
    },
    ConfigKeySpec {
        key: "engine.max_iterations",
        value_type: "integer",
        mutable: true,
        secret: false,
        description: "Maximum tool loop iterations",
    },
    ConfigKeySpec {
        key: "memory.external_provider.enabled",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Enable one read-only external memory provider",
    },
    ConfigKeySpec {
        key: "memory.external_provider.provider_type",
        value_type: "none|no_network_jsonl",
        mutable: true,
        secret: false,
        description: "External memory provider adapter type",
    },
    ConfigKeySpec {
        key: "memory.external_provider.name",
        value_type: "string",
        mutable: true,
        secret: false,
        description: "External memory provider display name",
    },
    ConfigKeySpec {
        key: "memory.external_provider.records_path",
        value_type: "path|none",
        mutable: true,
        secret: false,
        description: "Local JSONL records file for no-network external memory",
    },
];

pub fn config_scope_paths(working_dir: &Path) -> ConfigScopePaths {
    let user_config = dirs::config_dir()
        .map(|d| d.join("priority-agent").join("config.toml"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent").join("config.toml"));
    let project_config = working_dir.join(".priority-agent").join("config.toml");
    let legacy_config_dir = dirs::home_dir()
        .map(|d| d.join(".priority-agent"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent"));

    ConfigScopePaths {
        user_config,
        project_config,
        legacy_config_dir,
    }
}

pub fn config_schema_json() -> Value {
    json!({
        "version": 1,
        "keys": CONFIG_KEY_SPECS,
        "scopes": ["user", "project", "legacy"],
        "env_prefix": "PRIORITY_AGENT"
    })
}

pub fn format_config_summary(config: &AppConfig) -> String {
    format!(
        "Config:\n  api.base_url = {}\n  api.model = {}\n  api.temperature = {}\n  api.max_tokens = {}\n  ui.theme = {}\n  ui.show_token_usage = {}\n  ui.compact_mode = {}\n  storage.persistence_enabled = {}\n  storage.auto_save_interval_secs = {}\n  features.mcp_enabled = {}\n  features.skills_enabled = {}\n  features.web_search = {}\n  features.plugin_trust_mode = {}\n  engine.max_iterations = {}\n  memory.external_provider.enabled = {}\n  memory.external_provider.provider_type = {}\n  memory.external_provider.name = {}\n  memory.external_provider.records_path = {}",
        config.api.base_url,
        config.api.model,
        config.api.temperature,
        config.api.max_tokens.map(|v| v.to_string()).unwrap_or_else(|| "none".to_string()),
        config.ui.theme,
        config.ui.show_token_usage,
        config.ui.compact_mode,
        config.storage.persistence_enabled,
        config.storage.auto_save_interval_secs,
        config.features.mcp_enabled,
        config.features.skills_enabled,
        config.features.web_search,
        config.features.plugin_trust_mode,
        config.engine.max_iterations,
        config.memory.external_provider.enabled,
        config.memory.external_provider.provider_type,
        config.memory.external_provider.name,
        config
            .memory
            .external_provider
            .records_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string()),
    )
}

pub fn get_config_value(config: &AppConfig, key: &str) -> Option<String> {
    match key {
        "api.base_url" => Some(config.api.base_url.clone()),
        "api.model" => Some(config.api.model.clone()),
        "api.temperature" => Some(config.api.temperature.to_string()),
        "api.max_tokens" => Some(
            config
                .api
                .max_tokens
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ),
        "ui.theme" => Some(config.ui.theme.clone()),
        "ui.show_token_usage" => Some(config.ui.show_token_usage.to_string()),
        "ui.compact_mode" => Some(config.ui.compact_mode.to_string()),
        "storage.persistence_enabled" => Some(config.storage.persistence_enabled.to_string()),
        "storage.auto_save_interval_secs" => {
            Some(config.storage.auto_save_interval_secs.to_string())
        }
        "features.mcp_enabled" => Some(config.features.mcp_enabled.to_string()),
        "features.skills_enabled" => Some(config.features.skills_enabled.to_string()),
        "features.web_search" => Some(config.features.web_search.to_string()),
        "features.plugin_trust_mode" => Some(config.features.plugin_trust_mode.clone()),
        "engine.max_iterations" => Some(config.engine.max_iterations.to_string()),
        "memory.external_provider.enabled" => {
            Some(config.memory.external_provider.enabled.to_string())
        }
        "memory.external_provider.provider_type" => {
            Some(config.memory.external_provider.provider_type.clone())
        }
        "memory.external_provider.name" => Some(config.memory.external_provider.name.clone()),
        "memory.external_provider.records_path" => Some(
            config
                .memory
                .external_provider
                .records_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string()),
        ),
        _ => None,
    }
}

pub fn set_config_value(config: &mut AppConfig, key: &str, value: &str) -> Result<(), String> {
    match key {
        "api.base_url" => config.api.base_url = value.to_string(),
        "api.model" => config.api.model = value.to_string(),
        "api.temperature" => {
            config.api.temperature = value
                .parse::<f32>()
                .map_err(|_| format!("Invalid float for {}: {}", key, value))?;
        }
        "api.max_tokens" => {
            if value.eq_ignore_ascii_case("none") {
                config.api.max_tokens = None;
            } else {
                config.api.max_tokens = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| format!("Invalid integer for {}: {}", key, value))?,
                );
            }
        }
        "ui.theme" => config.ui.theme = value.to_string(),
        "ui.show_token_usage" => config.ui.show_token_usage = parse_bool(value)?,
        "ui.compact_mode" => config.ui.compact_mode = parse_bool(value)?,
        "storage.persistence_enabled" => config.storage.persistence_enabled = parse_bool(value)?,
        "storage.auto_save_interval_secs" => {
            config.storage.auto_save_interval_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "features.mcp_enabled" => config.features.mcp_enabled = parse_bool(value)?,
        "features.skills_enabled" => config.features.skills_enabled = parse_bool(value)?,
        "features.web_search" => config.features.web_search = parse_bool(value)?,
        "features.plugin_trust_mode" => {
            let mode = crate::plugins::trust::TrustMode::parse_lossy(value);
            config.features.plugin_trust_mode = mode.as_str().to_string();
        }
        "engine.max_iterations" => {
            config.engine.max_iterations = value
                .parse::<usize>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "memory.external_provider.enabled" => {
            config.memory.external_provider.enabled = parse_bool(value)?;
        }
        "memory.external_provider.provider_type" => {
            config.memory.external_provider.provider_type = value.to_string();
        }
        "memory.external_provider.name" => {
            config.memory.external_provider.name = value.to_string();
        }
        "memory.external_provider.records_path" => {
            config.memory.external_provider.records_path = if value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(PathBuf::from(value))
            };
        }
        _ => return Err(format!("Unknown config key: {}", key)),
    }
    Ok(())
}

pub fn validate_config(config: &AppConfig) -> Vec<String> {
    let mut issues = Vec::new();

    if !(0.0..=2.0).contains(&config.api.temperature) {
        issues.push("api.temperature should be between 0.0 and 2.0".to_string());
    }
    if config.storage.auto_save_interval_secs == 0 {
        issues.push("storage.auto_save_interval_secs must be greater than 0".to_string());
    }
    if config.engine.max_iterations == 0 {
        issues.push("engine.max_iterations must be greater than 0".to_string());
    }
    issues.extend(validate_external_memory_provider_config(
        &config.memory.external_provider,
    ));
    let trust_mode =
        crate::plugins::trust::TrustMode::parse_lossy(&config.features.plugin_trust_mode);
    if trust_mode.as_str() != config.features.plugin_trust_mode {
        issues.push(format!(
            "features.plugin_trust_mode '{}' will be normalized to '{}'",
            config.features.plugin_trust_mode,
            trust_mode.as_str()
        ));
    }

    issues
}

fn validate_external_memory_provider_config(config: &ExternalMemoryProviderConfig) -> Vec<String> {
    let mut issues = Vec::new();
    if !config.enabled {
        return issues;
    }
    if config.name.trim().is_empty() {
        issues.push("memory.external_provider.name cannot be empty".to_string());
    }
    if config.write_mirror {
        issues.push("memory.external_provider.write_mirror must remain false".to_string());
    }
    if config.tools {
        issues.push("memory.external_provider.tools must remain false".to_string());
    }
    match config.provider_type.as_str() {
        "no_network_jsonl" => {
            if config.records_path.is_none() {
                issues.push(
                    "memory.external_provider.records_path is required for no_network_jsonl"
                        .to_string(),
                );
            }
        }
        "none" => {
            issues.push(
                "memory.external_provider.provider_type cannot be none when enabled".to_string(),
            );
        }
        other => issues.push(format!(
            "memory.external_provider.provider_type '{}' is unsupported",
            other
        )),
    }
    issues
}

pub fn redacted_config_json(config: &AppConfig) -> Value {
    let mut value = serde_json::to_value(config).unwrap_or_else(|_| json!({}));
    redact_secrets(&mut value);
    value
}

pub fn redacted_config_export(config: &AppConfig, working_dir: &Path) -> Value {
    let paths = config_scope_paths(working_dir);
    json!({
        "schema_version": 1,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "config": redacted_config_json(config),
        "schema": config_schema_json(),
        "paths": paths,
        "validation": validate_config(config),
    })
}

pub fn format_config_schema_text() -> String {
    let mut lines = vec!["Config schema v1:".to_string()];
    for spec in CONFIG_KEY_SPECS {
        lines.push(format!(
            "- {} ({}) mutable={} secret={} - {}",
            spec.key, spec.value_type, spec.mutable, spec.secret, spec.description
        ));
    }
    lines.join("\n")
}

fn redact_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let lower = key.to_ascii_lowercase();
                if lower.contains("api_key")
                    || lower == "key"
                    || lower.contains("token")
                    || lower.contains("secret")
                {
                    *child = Value::String("[redacted]".to_string());
                } else {
                    redact_secrets(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_secrets(item);
            }
        }
        _ => {}
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Ok(true),
        "false" | "0" | "off" | "no" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

/// API 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    /// API Key
    pub api_key: Option<String>,
    /// 基础 URL
    pub base_url: String,
    /// 默认模型
    pub model: String,
    /// 温度参数
    pub temperature: f32,
    /// 最大 token 数
    pub max_tokens: Option<u32>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: String::new(),
            model: String::new(),
            temperature: 0.6,
            max_tokens: None,
        }
    }
}

/// UI 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    pub theme: String,
    pub show_token_usage: bool,
    pub compact_mode: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            show_token_usage: true,
            compact_mode: false,
        }
    }
}

/// 存储配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub data_dir: Option<PathBuf>,
    pub persistence_enabled: bool,
    pub auto_save_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct MemoryConfig {
    #[serde(default)]
    pub external_provider: ExternalMemoryProviderConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalMemoryProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_external_memory_provider_type")]
    pub provider_type: String,
    #[serde(default = "default_external_memory_provider_name")]
    pub name: String,
    #[serde(default)]
    pub records_path: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub prompt_block: bool,
    #[serde(default = "default_true")]
    pub prefetch: bool,
    #[serde(default = "default_true")]
    pub search: bool,
    #[serde(default)]
    pub queue_prefetch: bool,
    #[serde(default)]
    pub sync_turn: bool,
    #[serde(default)]
    pub session_end: bool,
    #[serde(default)]
    pub pre_compress: bool,
    #[serde(default)]
    pub write_mirror: bool,
    #[serde(default)]
    pub tools: bool,
}

fn default_external_memory_provider_type() -> String {
    "none".to_string()
}

fn default_external_memory_provider_name() -> String {
    "external-memory".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for ExternalMemoryProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_type: default_external_memory_provider_type(),
            name: default_external_memory_provider_name(),
            records_path: None,
            prompt_block: true,
            prefetch: true,
            search: true,
            queue_prefetch: false,
            sync_turn: false,
            session_end: false,
            pre_compress: false,
            write_mirror: false,
            tools: false,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            persistence_enabled: true,
            auto_save_interval_secs: 300,
        }
    }
}

/// 功能开关
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureFlags {
    pub tui_enabled: bool,
    pub agent_enabled: bool,
    pub mcp_enabled: bool,
    pub skills_enabled: bool,
    pub web_search: bool,
    /// 启用 LLM 驱动的记忆提取
    #[serde(default)]
    pub llm_memory_extraction: bool,
    /// 插件信任模式: strict | warn | off
    #[serde(default = "default_plugin_trust_mode")]
    pub plugin_trust_mode: String,
}

fn default_plugin_trust_mode() -> String {
    "warn".to_string()
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            tui_enabled: true,
            agent_enabled: true,
            mcp_enabled: false,
            skills_enabled: true,
            web_search: true,
            llm_memory_extraction: true,
            plugin_trust_mode: "warn".to_string(),
        }
    }
}

/// 引擎配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineConfig {
    /// 工具调用循环的最大迭代次数
    pub max_iterations: usize,
    /// MCP 服务器配置
    #[serde(default)]
    pub mcp_servers: Vec<crate::engine::mcp::McpServerConfig>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            mcp_servers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.api.model, "");
        assert_eq!(config.api.base_url, "");
        assert!(config.storage.persistence_enabled);
        assert_eq!(config.engine.max_iterations, 50);
    }

    #[test]
    fn config_schema_exposes_release_keys() {
        let schema = config_schema_json();
        let keys = schema["keys"].as_array().expect("keys array");

        assert!(keys.iter().any(|item| item["key"] == "api.model"));
        assert!(keys
            .iter()
            .any(|item| item["key"] == "features.plugin_trust_mode"));
        assert!(keys
            .iter()
            .any(|item| item["key"] == "engine.max_iterations"));
        assert!(keys
            .iter()
            .any(|item| item["key"] == "memory.external_provider.enabled"));
        assert!(keys
            .iter()
            .any(|item| item["key"] == "memory.external_provider.records_path"));
    }

    #[test]
    fn config_get_set_covers_release_keys() {
        let mut config = AppConfig::default();

        set_config_value(&mut config, "features.plugin_trust_mode", "strict").unwrap();
        set_config_value(&mut config, "engine.max_iterations", "7").unwrap();
        set_config_value(&mut config, "memory.external_provider.enabled", "true").unwrap();
        set_config_value(
            &mut config,
            "memory.external_provider.provider_type",
            "no_network_jsonl",
        )
        .unwrap();
        set_config_value(
            &mut config,
            "memory.external_provider.records_path",
            "/tmp/mem.jsonl",
        )
        .unwrap();

        assert_eq!(
            get_config_value(&config, "features.plugin_trust_mode").as_deref(),
            Some("strict")
        );
        assert_eq!(
            get_config_value(&config, "engine.max_iterations").as_deref(),
            Some("7")
        );
        assert_eq!(
            get_config_value(&config, "memory.external_provider.enabled").as_deref(),
            Some("true")
        );
        assert_eq!(
            get_config_value(&config, "memory.external_provider.provider_type").as_deref(),
            Some("no_network_jsonl")
        );
        assert_eq!(
            get_config_value(&config, "memory.external_provider.records_path").as_deref(),
            Some("/tmp/mem.jsonl")
        );
    }

    #[test]
    fn config_validation_reports_invalid_release_values() {
        let mut config = AppConfig::default();
        config.storage.auto_save_interval_secs = 0;
        config.engine.max_iterations = 0;
        config.features.plugin_trust_mode = "invalid".to_string();
        config.memory.external_provider.enabled = true;
        config.memory.external_provider.provider_type = "no_network_jsonl".to_string();
        config.memory.external_provider.write_mirror = true;

        let issues = validate_config(&config);

        assert!(issues
            .iter()
            .any(|issue| issue.contains("storage.auto_save_interval_secs")));
        assert!(issues
            .iter()
            .any(|issue| issue.contains("engine.max_iterations")));
        assert!(issues
            .iter()
            .any(|issue| issue.contains("features.plugin_trust_mode")));
        assert!(issues
            .iter()
            .any(|issue| issue.contains("memory.external_provider.records_path")));
        assert!(issues
            .iter()
            .any(|issue| issue.contains("memory.external_provider.write_mirror")));
    }

    #[test]
    fn redacted_config_export_hides_api_key() {
        let mut config = AppConfig::default();
        config.api.api_key = Some("secret-key".to_string());

        let export = redacted_config_export(&config, Path::new("/tmp/project"));
        let text = serde_json::to_string(&export).unwrap();

        assert!(text.contains("[redacted]"));
        assert!(!text.contains("secret-key"));
        assert!(text.contains("schema_version"));
    }
}

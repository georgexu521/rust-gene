//! 配置管理
//!
//! 管理应用配置，支持配置文件和环境变量

use anyhow::Result;
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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
    #[serde(default)]
    pub features: FeatureFlags,
    /// 引擎配置
    #[serde(default)]
    pub engine: EngineConfig,
    /// Memory provider configuration
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Tool-output paging and retention policy
    #[serde(default)]
    pub tool_output: ToolOutputConfig,
    /// Hooks configuration
    #[serde(default)]
    pub hooks: HooksConfig,
    /// LSP (Language Server Protocol) configuration
    #[serde(default)]
    pub lsp: LspConfig,
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
            .set_default("api.providers_config_path", "")?
            .set_default("ui.theme", "dark")?
            .set_default("storage.persistence_enabled", true)?
            .set_default("features.llm_memory_extraction", true)?
            .set_default("features.plugin_trust_mode", "warn")?
            .set_default("engine.max_iterations", 50)?
            .set_default("engine.turn_timeout_secs", 1800)?
            .set_default("engine.session_end_memory_flush_timeout_secs", 5)?
            .set_default("engine.llm_request_timeout_secs", 120)?
            .set_default("engine.stream_idle_timeout_secs", 120)?
            .set_default("engine.runtime_profile", "standard")?
            .set_default("engine.closeout_visibility", "concise")?
            .set_default("engine.self_correction_enabled", true)?
            .set_default("engine.approval_timeout_secs", 300)?
            .set_default("engine.closeout_background_timeout_secs", 5)?
            .set_default("engine.patch_synthesis_enabled", true)?
            .set_default("engine.deterministic_patch_synthesis_enabled", true)?
            .set_default("engine.streaming_tool_execution", "off")?
            .set_default("engine.auto_memory_write", "review_only")?
            .set_default("engine.memory_dialectic_depth", 0)?
            .set_default("engine.tool_dispatch_serial", false)?
            .set_default("engine.read_only_tool_concurrency", 8)?
            .set_default("engine.tool_profile", "standard")?
            .set_default("engine.workflow_contract", "auto")?
            .set_default("engine.task_guidance_enabled", false)?
            .set_default("memory.external_provider.enabled", false)?
            .set_default("memory.external_provider.mode", "off")?
            .set_default("memory.external_provider.provider_type", "none")?
            .set_default("tool_output.max_bytes", 32 * 1024)?
            .set_default("tool_output.max_lines", 500)?
            .set_default("tool_output.preview_direction", "tail")?
            .set_default("tool_output.retention_days", 7)?
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
        key: "api.providers_config_path",
        value_type: "string|none",
        mutable: true,
        secret: false,
        description: "Path to custom providers.toml manifest",
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
        key: "storage.persistence_enabled",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Persist sessions and state",
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
        key: "features.goals",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Enable Codex-style durable goal mode",
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
        key: "memory.external_provider.mode",
        value_type: "off|context|tools|hybrid",
        mutable: true,
        secret: false,
        description: "External memory provider mode",
    },
    ConfigKeySpec {
        key: "memory.external_provider.provider_type",
        value_type: "none|no_network_jsonl",
        mutable: true,
        secret: false,
        description: "External memory provider adapter type",
    },
    ConfigKeySpec {
        key: "memory.external_provider.records_path",
        value_type: "path|none",
        mutable: true,
        secret: false,
        description: "Local JSONL records file for no-network external memory",
    },
    ConfigKeySpec {
        key: "tool_output.max_bytes",
        value_type: "integer",
        mutable: true,
        secret: false,
        description: "Maximum inline tool-output bytes before paging to store",
    },
    ConfigKeySpec {
        key: "tool_output.max_lines",
        value_type: "integer",
        mutable: true,
        secret: false,
        description: "Maximum preview lines shown for stored tool output",
    },
    ConfigKeySpec {
        key: "tool_output.preview_direction",
        value_type: "head|tail|head_tail",
        mutable: true,
        secret: false,
        description: "Preview slice direction for stored tool output",
    },
    ConfigKeySpec {
        key: "tool_output.retention_days",
        value_type: "integer",
        mutable: true,
        secret: false,
        description: "Tool-output retention window in days",
    },
    ConfigKeySpec {
        key: "lsp.enabled",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Enable optional LSP diagnostics",
    },
    ConfigKeySpec {
        key: "lsp.auto_detect",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Auto-detect language servers from project files",
    },
    ConfigKeySpec {
        key: "lsp.disable_downloads",
        value_type: "bool",
        mutable: true,
        secret: false,
        description: "Prevent automatic LSP server downloads",
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
        "Config:\n  api.base_url = {}\n  api.model = {}\n  api.temperature = {}\n  api.max_tokens = {}\n  ui.theme = {}\n  ui.show_token_usage = {}\n  storage.persistence_enabled = {}\n  features.mcp_enabled = {}\n  features.skills_enabled = {}\n  features.web_search = {}\n      features.plugin_trust_mode = {}\n  features.goals = {}\n  engine.max_iterations = {}\n  memory.external_provider.enabled = {}\n  memory.external_provider.mode = {}\n  memory.external_provider.provider_type = {}\n  memory.external_provider.records_path = {}\n  tool_output.max_bytes = {}\n  tool_output.max_lines = {}\n  tool_output.preview_direction = {}\n  tool_output.retention_days = {}\n  lsp.enabled = {}\n  lsp.auto_detect = {}\n  lsp.disable_downloads = {}",
        config.api.base_url,
        config.api.model,
        config.api.temperature,
        config
            .api
            .max_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string()),
        config.ui.theme,
        config.ui.show_token_usage,
        config.storage.persistence_enabled,
        config.features.mcp_enabled,
        config.features.skills_enabled,
        config.features.web_search,
        config.features.plugin_trust_mode,
        config.features.goals,
        config.engine.max_iterations,
        config.memory.external_provider.enabled,
        config.memory.external_provider.effective_mode(),
        config.memory.external_provider.provider_type,
        config
            .memory
            .external_provider
            .records_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string()),
        config.tool_output.max_bytes,
        config.tool_output.max_lines,
        config.tool_output.preview_direction,
        config.tool_output.retention_days,
        config.lsp.enabled,
        config.lsp.auto_detect,
        config.lsp.disable_downloads,
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
        "storage.persistence_enabled" => Some(config.storage.persistence_enabled.to_string()),
        "features.mcp_enabled" => Some(config.features.mcp_enabled.to_string()),
        "features.skills_enabled" => Some(config.features.skills_enabled.to_string()),
        "features.web_search" => Some(config.features.web_search.to_string()),
        "features.plugin_trust_mode" => Some(config.features.plugin_trust_mode.clone()),
        "features.goals" => Some(config.features.goals.to_string()),
        "engine.max_iterations" => Some(config.engine.max_iterations.to_string()),
        "engine.turn_timeout_secs" => Some(config.engine.turn_timeout_secs.to_string()),
        "engine.session_end_memory_flush_timeout_secs" => Some(
            config
                .engine
                .session_end_memory_flush_timeout_secs
                .to_string(),
        ),
        "engine.llm_request_timeout_secs" => {
            Some(config.engine.llm_request_timeout_secs.to_string())
        }
        "engine.stream_idle_timeout_secs" => {
            Some(config.engine.stream_idle_timeout_secs.to_string())
        }
        "engine.fallback_model" => config.engine.fallback_model.clone(),
        "engine.runtime_profile" => Some(config.engine.runtime_profile.clone()),
        "engine.closeout_visibility" => Some(config.engine.closeout_visibility.clone()),
        "memory.external_provider.enabled" => {
            Some(config.memory.external_provider.enabled.to_string())
        }
        "memory.external_provider.mode" => Some(config.memory.external_provider.effective_mode()),
        "memory.external_provider.provider_type" => {
            Some(config.memory.external_provider.provider_type.clone())
        }
        "memory.external_provider.records_path" => Some(
            config
                .memory
                .external_provider
                .records_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string()),
        ),
        "tool_output.max_bytes" => Some(config.tool_output.max_bytes.to_string()),
        "tool_output.max_lines" => Some(config.tool_output.max_lines.to_string()),
        "tool_output.preview_direction" => Some(config.tool_output.preview_direction.clone()),
        "tool_output.retention_days" => Some(config.tool_output.retention_days.to_string()),
        "lsp.enabled" => Some(config.lsp.enabled.to_string()),
        "lsp.auto_detect" => Some(config.lsp.auto_detect.to_string()),
        "lsp.disable_downloads" => Some(config.lsp.disable_downloads.to_string()),
        "engine.approval_timeout_secs" => Some(config.engine.approval_timeout_secs.to_string()),
        "engine.closeout_background_timeout_secs" => {
            Some(config.engine.closeout_background_timeout_secs.to_string())
        }
        "engine.patch_synthesis_enabled" => Some(config.engine.patch_synthesis_enabled.to_string()),
        "engine.deterministic_patch_synthesis_enabled" => Some(
            config
                .engine
                .deterministic_patch_synthesis_enabled
                .to_string(),
        ),
        "engine.streaming_tool_execution" => Some(config.engine.streaming_tool_execution.clone()),
        "engine.auto_memory_write" => Some(config.engine.auto_memory_write.clone()),
        "engine.memory_dialectic_depth" => Some(config.engine.memory_dialectic_depth.to_string()),
        "engine.required_validation_timeout_secs" => Some(
            config
                .engine
                .required_validation_timeout_secs
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ),
        "engine.tool_dispatch_serial" => Some(config.engine.tool_dispatch_serial.to_string()),
        "engine.read_only_tool_concurrency" => {
            Some(config.engine.read_only_tool_concurrency.to_string())
        }
        "engine.tool_profile" => Some(config.engine.tool_profile.clone()),
        "engine.workflow_contract" => Some(config.engine.workflow_contract.clone()),
        "engine.task_guidance_enabled" => Some(config.engine.task_guidance_enabled.to_string()),
        "engine.self_correction_enabled" => Some(config.engine.self_correction_enabled.to_string()),
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
        "storage.persistence_enabled" => config.storage.persistence_enabled = parse_bool(value)?,
        "features.mcp_enabled" => config.features.mcp_enabled = parse_bool(value)?,
        "features.skills_enabled" => config.features.skills_enabled = parse_bool(value)?,
        "features.web_search" => config.features.web_search = parse_bool(value)?,
        "features.plugin_trust_mode" => {
            let mode = crate::plugins::trust::TrustMode::parse_lossy(value);
            config.features.plugin_trust_mode = mode.as_str().to_string();
        }
        "features.goals" => config.features.goals = parse_bool(value)?,
        "engine.max_iterations" => {
            config.engine.max_iterations = value
                .parse::<usize>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.turn_timeout_secs" => {
            config.engine.turn_timeout_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.session_end_memory_flush_timeout_secs" => {
            config.engine.session_end_memory_flush_timeout_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.llm_request_timeout_secs" => {
            config.engine.llm_request_timeout_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.stream_idle_timeout_secs" => {
            config.engine.stream_idle_timeout_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.fallback_model" => {
            config.engine.fallback_model = if value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(value.to_string())
            };
        }
        "engine.runtime_profile" => {
            config.engine.runtime_profile = value.to_string();
        }
        "engine.closeout_visibility" => {
            config.engine.closeout_visibility = value.to_string();
        }
        "memory.external_provider.enabled" => {
            config.memory.external_provider.enabled = parse_bool(value)?;
        }
        "memory.external_provider.mode" => {
            let mode = value.trim().to_ascii_lowercase();
            config.memory.external_provider.enabled = mode != "off";
            config.memory.external_provider.mode = mode;
        }
        "memory.external_provider.provider_type" => {
            config.memory.external_provider.provider_type = value.to_string();
        }
        "memory.external_provider.records_path" => {
            config.memory.external_provider.records_path = if value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(PathBuf::from(value))
            };
        }
        "tool_output.max_bytes" => {
            config.tool_output.max_bytes = value
                .parse::<usize>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "tool_output.max_lines" => {
            config.tool_output.max_lines = value
                .parse::<usize>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "tool_output.preview_direction" => {
            config.tool_output.preview_direction = normalize_tool_output_preview_direction(value)?;
        }
        "tool_output.retention_days" => {
            config.tool_output.retention_days = value
                .parse::<u32>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "lsp.enabled" => config.lsp.enabled = parse_bool(value)?,
        "lsp.auto_detect" => config.lsp.auto_detect = parse_bool(value)?,
        "lsp.disable_downloads" => config.lsp.disable_downloads = parse_bool(value)?,
        "engine.approval_timeout_secs" => {
            config.engine.approval_timeout_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.closeout_background_timeout_secs" => {
            config.engine.closeout_background_timeout_secs = value
                .parse::<u64>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.patch_synthesis_enabled" => {
            config.engine.patch_synthesis_enabled = parse_bool(value)?
        }
        "engine.deterministic_patch_synthesis_enabled" => {
            config.engine.deterministic_patch_synthesis_enabled = parse_bool(value)?
        }
        "engine.streaming_tool_execution" => {
            config.engine.streaming_tool_execution = value.to_string();
        }
        "engine.auto_memory_write" => {
            config.engine.auto_memory_write = value.to_string();
        }
        "engine.memory_dialectic_depth" => {
            config.engine.memory_dialectic_depth = value
                .parse::<usize>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.required_validation_timeout_secs" => {
            config.engine.required_validation_timeout_secs = if value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("Invalid integer for {}: {}", key, value))?,
                )
            };
        }
        "engine.tool_dispatch_serial" => config.engine.tool_dispatch_serial = parse_bool(value)?,
        "engine.read_only_tool_concurrency" => {
            config.engine.read_only_tool_concurrency = value
                .parse::<usize>()
                .map_err(|_| format!("Invalid integer for {}: {}", key, value))?;
        }
        "engine.tool_profile" => {
            config.engine.tool_profile = value.to_string();
        }
        "engine.workflow_contract" => {
            config.engine.workflow_contract = value.to_string();
        }
        "engine.task_guidance_enabled" => config.engine.task_guidance_enabled = parse_bool(value)?,
        "engine.self_correction_enabled" => {
            config.engine.self_correction_enabled = parse_bool(value)?
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
    if config.engine.max_iterations == 0 {
        issues.push("engine.max_iterations must be greater than 0".to_string());
    }
    issues.extend(validate_external_memory_provider_config(
        &config.memory.external_provider,
    ));
    issues.extend(validate_tool_output_config(&config.tool_output));
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

fn validate_tool_output_config(config: &ToolOutputConfig) -> Vec<String> {
    let mut issues = Vec::new();
    if config.max_bytes == 0 {
        issues.push("tool_output.max_bytes must be greater than 0".to_string());
    }
    if config.max_lines == 0 {
        issues.push("tool_output.max_lines must be greater than 0".to_string());
    }
    if config.retention_days == 0 {
        issues.push("tool_output.retention_days must be greater than 0".to_string());
    }
    if normalize_tool_output_preview_direction(&config.preview_direction).is_err() {
        issues.push(format!(
            "tool_output.preview_direction '{}' is unsupported",
            config.preview_direction
        ));
    }
    issues
}

fn validate_external_memory_provider_config(config: &ExternalMemoryProviderConfig) -> Vec<String> {
    let mut issues = Vec::new();
    let mode = config.effective_mode();
    match mode.as_str() {
        "off" | "context" | "tools" | "hybrid" => {}
        other => issues.push(format!(
            "memory.external_provider.mode '{}' is unsupported",
            other
        )),
    }
    if mode == "tools" || mode == "hybrid" {
        issues.push(
            "memory.external_provider.mode tools/hybrid is reserved; external provider tool schemas are disabled by current policy".to_string(),
        );
    }
    if mode == "off" {
        return issues;
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
    /// Selected provider name (persisted across sessions).
    #[serde(default)]
    pub provider_name: Option<String>,
    /// Optional path to a custom providers.toml manifest.
    #[serde(default)]
    pub providers_config_path: Option<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: String::new(),
            model: String::new(),
            temperature: 0.6,
            max_tokens: None,
            provider_name: None,
            providers_config_path: None,
        }
    }
}

/// UI 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    pub theme: String,
    #[serde(default = "default_show_token_usage")]
    pub show_token_usage: bool,
    #[serde(default)]
    pub pinned_sessions: Vec<String>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            show_token_usage: default_show_token_usage(),
            pinned_sessions: Vec::new(),
        }
    }
}

fn default_show_token_usage() -> bool {
    true
}

/// 存储配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub data_dir: Option<PathBuf>,
    pub persistence_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct MemoryConfig {
    #[serde(default)]
    pub external_provider: ExternalMemoryProviderConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolOutputConfig {
    pub max_bytes: usize,
    pub max_lines: usize,
    pub preview_direction: String,
    pub retention_days: u32,
}

impl Default for ToolOutputConfig {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024,
            max_lines: 500,
            preview_direction: "tail".to_string(),
            retention_days: 7,
        }
    }
}

fn normalize_tool_output_preview_direction(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "head" => Ok("head".to_string()),
        "tail" => Ok("tail".to_string()),
        "head_tail" | "headtail" => Ok("head_tail".to_string()),
        _ => Err(format!(
            "Invalid preview direction for tool_output.preview_direction: {}",
            value
        )),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalMemoryProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_external_memory_provider_mode")]
    pub mode: String,
    #[serde(default = "default_external_memory_provider_type")]
    pub provider_type: String,
    #[serde(default)]
    pub records_path: Option<PathBuf>,
}

fn default_external_memory_provider_mode() -> String {
    "off".to_string()
}

fn default_external_memory_provider_type() -> String {
    "none".to_string()
}

impl ExternalMemoryProviderConfig {
    pub fn effective_mode(&self) -> String {
        let mode = self.mode.trim().to_ascii_lowercase();
        if mode != "off" {
            mode
        } else if self.enabled {
            "context".to_string()
        } else {
            "off".to_string()
        }
    }
}

impl Default for ExternalMemoryProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_external_memory_provider_mode(),
            provider_type: default_external_memory_provider_type(),
            records_path: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct HooksConfig {
    /// Pre-tool hook command
    #[serde(default)]
    pub pre_tool: Option<String>,
    /// Post-tool hook command
    #[serde(default)]
    pub post_tool: Option<String>,
    /// Permission request hook command
    #[serde(default)]
    pub permission_request: Option<String>,
    /// Permission resolved hook command
    #[serde(default)]
    pub permission_resolved: Option<String>,
    /// Hook timeout in milliseconds
    #[serde(default = "default_hook_timeout_ms")]
    pub timeout_ms: u64,
    /// Whether to fail closed (deny) on hook error
    #[serde(default)]
    pub fail_closed: bool,
}

fn default_hook_timeout_ms() -> u64 {
    5000
}

/// LSP (Language Server Protocol) configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LspConfig {
    /// Master switch. When false, no LSP servers are started.
    #[serde(default)]
    pub enabled: bool,
    /// Auto-detect language servers from project files (Cargo.toml, etc.).
    #[serde(default = "default_true")]
    pub auto_detect: bool,
    /// Prevent automatic download/install of LSP server binaries.
    #[serde(default = "default_true")]
    pub disable_downloads: bool,
    /// Per-server overrides. Key is the server name (e.g. "rust-analyzer").
    #[serde(default)]
    pub servers: std::collections::HashMap<String, LspServerConfigEntry>,
}

/// Per-server LSP configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LspServerConfigEntry {
    /// Override the server binary command.
    #[serde(default)]
    pub command: Option<String>,
    /// Additional arguments for the server binary.
    #[serde(default)]
    pub args: Vec<String>,
    /// File extensions this server handles (e.g. ["rs"]).
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Extra environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Disable this specific server even when auto-detected.
    #[serde(default)]
    pub disabled: bool,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_detect: true,
            disable_downloads: true,
            servers: std::collections::HashMap::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            persistence_enabled: true,
        }
    }
}

/// 功能开关
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureFlags {
    #[serde(default)]
    pub mcp_enabled: bool,
    #[serde(default = "default_true")]
    pub skills_enabled: bool,
    #[serde(default = "default_true")]
    pub web_search: bool,
    /// 启用 LLM 驱动的记忆提取
    #[serde(default = "default_true")]
    pub llm_memory_extraction: bool,
    /// 插件信任模式: strict | warn | off
    #[serde(default = "default_plugin_trust_mode")]
    pub plugin_trust_mode: String,
    /// 启用 Codex-style goal mode（持久化目标，跨 turn 自动推进）
    #[serde(default)]
    pub goals: bool,
}

fn default_plugin_trust_mode() -> String {
    "warn".to_string()
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            mcp_enabled: false,
            skills_enabled: true,
            web_search: true,
            llm_memory_extraction: true,
            plugin_trust_mode: "warn".to_string(),
            goals: false,
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
    /// Turn 执行超时（秒）
    #[serde(default = "default_turn_timeout")]
    pub turn_timeout_secs: u64,
    /// Session-end memory flush 超时（秒）
    #[serde(default = "default_session_end_memory_flush_timeout")]
    pub session_end_memory_flush_timeout_secs: u64,
    /// LLM 请求超时（秒）
    #[serde(default = "default_llm_request_timeout")]
    pub llm_request_timeout_secs: u64,
    /// Stream idle 超时（秒）
    #[serde(default = "default_stream_idle_timeout")]
    pub stream_idle_timeout_secs: u64,
    /// Fallback 模型名称
    #[serde(default)]
    pub fallback_model: Option<String>,
    /// 运行时 profile（light / standard / full）
    #[serde(default = "default_runtime_profile")]
    pub runtime_profile: String,
    /// Closeout 可见性（hidden / concise / full）
    #[serde(default = "default_closeout_visibility")]
    pub closeout_visibility: String,
    /// 启用自我修正（用户中断时替换最后一条 assistant 消息）
    #[serde(default = "default_self_correction")]
    pub self_correction_enabled: bool,
    /// 审批超时（秒）
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout_secs: u64,
    /// Closeout 后台阶段超时（秒）
    #[serde(default = "default_closeout_background_timeout")]
    pub closeout_background_timeout_secs: u64,
    /// 启用 patch synthesis
    #[serde(default = "default_true")]
    pub patch_synthesis_enabled: bool,
    /// 启用 deterministic patch synthesis
    #[serde(default = "default_true")]
    pub deterministic_patch_synthesis_enabled: bool,
    /// Streaming tool execution mode（"off" | "shadow"）
    #[serde(default = "default_streaming_tool_execution")]
    pub streaming_tool_execution: String,
    /// Auto memory write policy（"review_only" | "narrow" | "legacy"）
    #[serde(default = "default_auto_memory_write")]
    pub auto_memory_write: String,
    /// Memory dialectic depth（0 = off）
    #[serde(default)]
    pub memory_dialectic_depth: usize,
    /// Required validation timeout（秒），None = 不限
    #[serde(default)]
    pub required_validation_timeout_secs: Option<u64>,
    /// 强制串行工具分发
    #[serde(default)]
    pub tool_dispatch_serial: bool,
    /// 只读工具并发数
    #[serde(default = "default_read_only_tool_concurrency")]
    pub read_only_tool_concurrency: usize,
    /// Tool profile（"standard" | "full" | "all" | "experimental"）
    #[serde(default = "default_tool_profile")]
    pub tool_profile: String,
    /// Workflow contract（"off" | "auto" | "force"）
    #[serde(default = "default_workflow_contract")]
    pub workflow_contract: String,
    /// 启用 task guidance 注入
    #[serde(default)]
    pub task_guidance_enabled: bool,
    /// 启用 route-scoped tools（默认 true）
    #[serde(default = "default_true")]
    pub route_scoped_tools_enabled: bool,
    /// 启用 debug tool exposure（默认 false）
    #[serde(default)]
    pub debug_tool_exposure: bool,
}

fn default_turn_timeout() -> u64 {
    1800
}

fn default_session_end_memory_flush_timeout() -> u64 {
    5
}

fn default_llm_request_timeout() -> u64 {
    120
}

fn default_stream_idle_timeout() -> u64 {
    120
}

fn default_runtime_profile() -> String {
    "standard".to_string()
}

fn default_closeout_visibility() -> String {
    "concise".to_string()
}

fn default_self_correction() -> bool {
    true
}

fn default_approval_timeout() -> u64 {
    300
}

fn default_closeout_background_timeout() -> u64 {
    5
}

fn default_streaming_tool_execution() -> String {
    "off".to_string()
}

fn default_auto_memory_write() -> String {
    "review_only".to_string()
}

fn default_read_only_tool_concurrency() -> usize {
    8
}

fn default_tool_profile() -> String {
    "standard".to_string()
}

fn default_workflow_contract() -> String {
    "auto".to_string()
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            mcp_servers: Vec::new(),
            turn_timeout_secs: default_turn_timeout(),
            session_end_memory_flush_timeout_secs: default_session_end_memory_flush_timeout(),
            llm_request_timeout_secs: default_llm_request_timeout(),
            stream_idle_timeout_secs: default_stream_idle_timeout(),
            fallback_model: None,
            runtime_profile: default_runtime_profile(),
            closeout_visibility: default_closeout_visibility(),
            self_correction_enabled: default_self_correction(),
            approval_timeout_secs: default_approval_timeout(),
            closeout_background_timeout_secs: default_closeout_background_timeout(),
            patch_synthesis_enabled: true,
            deterministic_patch_synthesis_enabled: true,
            streaming_tool_execution: default_streaming_tool_execution(),
            auto_memory_write: default_auto_memory_write(),
            memory_dialectic_depth: 0,
            required_validation_timeout_secs: None,
            tool_dispatch_serial: false,
            read_only_tool_concurrency: default_read_only_tool_concurrency(),
            tool_profile: default_tool_profile(),
            workflow_contract: default_workflow_contract(),
            task_guidance_enabled: false,
            route_scoped_tools_enabled: true,
            debug_tool_exposure: false,
        }
    }
}

mod runtime;
pub use runtime::{init_runtime_config, reload_runtime_config, runtime_config};

#[cfg(test)]
mod tests {
    use super::*;

    mod test_env;
    use test_env::EnvOverride;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.api.model, "");
        assert_eq!(config.api.base_url, "");
        assert!(config.storage.persistence_enabled);
        assert_eq!(config.engine.max_iterations, 50);
        assert!(config.ui.pinned_sessions.is_empty());
    }

    #[test]
    fn ui_config_round_trips_pinned_sessions_and_defaults_legacy_configs() {
        let ui: UiConfig = toml::from_str(
            r#"
theme = "graphite"
show_token_usage = true
pinned_sessions = ["sess_a", "sess_b"]
"#,
        )
        .unwrap();

        assert_eq!(ui.pinned_sessions, vec!["sess_a", "sess_b"]);

        let legacy: UiConfig = toml::from_str(
            r#"
theme = "graphite"
show_token_usage = true
"#,
        )
        .unwrap();

        assert!(legacy.pinned_sessions.is_empty());
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
            .any(|item| item["key"] == "memory.external_provider.mode"));
        assert!(keys
            .iter()
            .any(|item| item["key"] == "memory.external_provider.records_path"));
        assert!(keys
            .iter()
            .any(|item| item["key"] == "tool_output.max_bytes"));
        assert!(keys
            .iter()
            .any(|item| item["key"] == "tool_output.preview_direction"));
    }

    #[test]
    fn feature_flags_deserialize_legacy_partial_config_with_defaults() {
        let flags: FeatureFlags = toml::from_str("plugin_trust_mode = \"warn\"").unwrap();

        assert!(!flags.mcp_enabled);
        assert!(flags.skills_enabled);
        assert!(flags.web_search);
        assert!(flags.llm_memory_extraction);
        assert_eq!(flags.plugin_trust_mode, "warn");
        assert!(!flags.goals);
    }

    #[test]
    fn config_get_set_covers_release_keys() {
        let mut config = AppConfig::default();

        set_config_value(&mut config, "features.plugin_trust_mode", "strict").unwrap();
        set_config_value(&mut config, "engine.max_iterations", "7").unwrap();
        set_config_value(&mut config, "memory.external_provider.enabled", "true").unwrap();
        set_config_value(&mut config, "memory.external_provider.mode", "context").unwrap();
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
        set_config_value(&mut config, "tool_output.max_bytes", "4096").unwrap();
        set_config_value(&mut config, "tool_output.max_lines", "80").unwrap();
        set_config_value(&mut config, "tool_output.preview_direction", "head-tail").unwrap();
        set_config_value(&mut config, "tool_output.retention_days", "14").unwrap();

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
            get_config_value(&config, "memory.external_provider.mode").as_deref(),
            Some("context")
        );
        assert_eq!(
            get_config_value(&config, "memory.external_provider.provider_type").as_deref(),
            Some("no_network_jsonl")
        );
        assert_eq!(
            get_config_value(&config, "memory.external_provider.records_path").as_deref(),
            Some("/tmp/mem.jsonl")
        );
        assert_eq!(
            get_config_value(&config, "tool_output.max_bytes").as_deref(),
            Some("4096")
        );
        assert_eq!(
            get_config_value(&config, "tool_output.max_lines").as_deref(),
            Some("80")
        );
        assert_eq!(
            get_config_value(&config, "tool_output.preview_direction").as_deref(),
            Some("head_tail")
        );
        assert_eq!(
            get_config_value(&config, "tool_output.retention_days").as_deref(),
            Some("14")
        );

        set_config_value(&mut config, "memory.external_provider.mode", "off").unwrap();
        assert_eq!(
            get_config_value(&config, "memory.external_provider.mode").as_deref(),
            Some("off")
        );
        assert_eq!(
            get_config_value(&config, "memory.external_provider.enabled").as_deref(),
            Some("false")
        );
    }

    #[test]
    fn config_validation_reports_invalid_release_values() {
        let mut config = AppConfig::default();
        config.engine.max_iterations = 0;
        config.features.plugin_trust_mode = "invalid".to_string();
        config.memory.external_provider.enabled = true;
        config.memory.external_provider.provider_type = "no_network_jsonl".to_string();

        let issues = validate_config(&config);

        assert!(issues
            .iter()
            .any(|issue| issue.contains("engine.max_iterations")));
        assert!(issues
            .iter()
            .any(|issue| issue.contains("features.plugin_trust_mode")));
        assert!(issues
            .iter()
            .any(|issue| issue.contains("memory.external_provider.records_path")));
    }

    #[test]
    fn config_validation_reports_reserved_external_memory_tool_modes() {
        let mut config = AppConfig::default();
        config.memory.external_provider.mode = "hybrid".to_string();
        config.memory.external_provider.provider_type = "no_network_jsonl".to_string();
        config.memory.external_provider.records_path = Some(PathBuf::from("/tmp/mem.jsonl"));

        let issues = validate_config(&config);

        assert!(issues.iter().any(|issue| issue.contains("tools/hybrid")));
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

    #[test]
    fn config_accessor_turn_timeout_clamps_range() {
        let _env = EnvOverride::unset("PRIORITY_AGENT_TURN_TIMEOUT_SECS");
        let mut config = AppConfig::default();
        config.engine.turn_timeout_secs = 30;
        assert_eq!(config.turn_timeout().as_secs(), 60);

        config.engine.turn_timeout_secs = 8000;
        assert_eq!(config.turn_timeout().as_secs(), 7200);

        config.engine.turn_timeout_secs = 300;
        assert_eq!(config.turn_timeout().as_secs(), 300);
    }

    #[test]
    fn config_accessor_turn_timeout_honors_legacy_env_override() {
        let _env = EnvOverride::set("PRIORITY_AGENT_TURN_TIMEOUT_SECS", "120");
        let mut config = AppConfig::default();
        config.engine.turn_timeout_secs = 300;

        assert_eq!(config.turn_timeout().as_secs(), 120);
    }

    #[test]
    fn config_accessor_session_end_flush_timeout_clamps_range() {
        let _env = EnvOverride::unset("PRIORITY_AGENT_SESSION_END_MEMORY_FLUSH_TIMEOUT_SECS");
        let mut config = AppConfig::default();
        config.engine.session_end_memory_flush_timeout_secs = 0;
        assert_eq!(config.session_end_memory_flush_timeout().as_secs(), 1);

        config.engine.session_end_memory_flush_timeout_secs = 100;
        assert_eq!(config.session_end_memory_flush_timeout().as_secs(), 60);
    }

    #[test]
    fn config_accessor_session_end_flush_timeout_honors_legacy_env_override() {
        let _env = EnvOverride::set("PRIORITY_AGENT_SESSION_END_MEMORY_FLUSH_TIMEOUT_SECS", "10");
        let mut config = AppConfig::default();
        config.engine.session_end_memory_flush_timeout_secs = 5;

        assert_eq!(config.session_end_memory_flush_timeout().as_secs(), 10);
    }
}

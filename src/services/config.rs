//! 配置管理
//!
//! 管理应用配置，支持配置文件和环境变量

use anyhow::Result;
use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            .set_default("engine.max_iterations", 10)?
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
            theme: "dark".to_string(),
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
            max_iterations: 10,
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
    }
}

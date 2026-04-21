//! LLM Provider 注册表
//!
//! 支持动态注册和选择多个 LLM Provider

use crate::services::api::LlmProvider;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// Provider 配置
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Provider 名称
    pub name: String,
    /// Provider 类型
    pub provider_type: ProviderType,
    /// API Key
    pub api_key: String,
    /// Base URL
    pub base_url: Option<String>,
    /// 默认模型
    pub default_model: String,
    /// 是否启用
    pub enabled: bool,
}

/// Provider 类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderType {
    /// Kimi / Moonshot
    Kimi,
    /// OpenAI 兼容
    OpenAI,
    /// OpenAI 兼容（通用）
    OpenAICompat,
    /// Minimax
    Minimax,
    /// Anthropic
    Anthropic,
    /// Google Gemini
    Google,
    /// Azure OpenAI
    Azure,
    /// 自定义（通用兼容）
    Custom,
}

impl ProviderType {
    /// 从字符串解析 Provider 类型
    pub fn parse_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "kimi" | "moonshot" => ProviderType::Kimi,
            "openai" => ProviderType::OpenAI,
            "openai_compat" | "openai-compatible" => ProviderType::OpenAICompat,
            "minimax" => ProviderType::Minimax,
            "anthropic" => ProviderType::Anthropic,
            "google" | "gemini" => ProviderType::Google,
            "azure" | "azure_openai" => ProviderType::Azure,
            _ => ProviderType::Custom,
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse_lossy(s))
    }
}

/// Provider 注册表
pub struct ProviderRegistry {
    /// 已注册的 providers（name -> ProviderInstance）
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    /// Provider 配置（name -> config）
    configs: HashMap<String, ProviderConfig>,
    /// 当前选中的 provider 名称
    selected: Option<String>,
}

impl ProviderRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            configs: HashMap::new(),
            selected: None,
        }
    }

    /// 从环境变量初始化注册表
    pub fn from_env() -> Self {
        let mut registry = Self::new();

        // 加载默认的 Kimi Provider（如果配置了）
        if let Ok(api_key) = std::env::var("MOONSHOT_API_KEY") {
            let base_url = std::env::var("MOONSHOT_BASE_URL")
                .ok()
                .unwrap_or_else(|| "https://api.moonshot.cn/v1".to_string());
            let model = std::env::var("MOONSHOT_MODEL")
                .ok()
                .unwrap_or_else(|| "kimi-k2.5".to_string());

            let config = ProviderConfig {
                name: "kimi".to_string(),
                provider_type: ProviderType::Kimi,
                api_key: api_key.clone(),
                base_url: Some(base_url.clone()),
                default_model: model.clone(),
                enabled: true,
            };

            // 创建 Kimi provider
            let kimi_config = crate::services::api::kimi::KimiConfig {
                api_key,
                base_url,
                default_model: model,
                thinking_enabled: std::env::var("PRIORITY_AGENT_THINKING")
                    .map(|v| v != "0")
                    .unwrap_or(true),
                thinking_budget: std::env::var("PRIORITY_AGENT_THINKING_BUDGET")
                    .ok()
                    .and_then(|v| v.parse().ok()),
            };
            let provider = crate::services::api::kimi::KimiClient::new(kimi_config);
            registry.register("kimi".to_string(), Arc::new(provider), config);
            registry.select("kimi".to_string());
        }

        // 加载默认的 OpenAI Provider（如果配置了）
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            let base_url = std::env::var("OPENAI_BASE_URL")
                .ok()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let model = std::env::var("OPENAI_MODEL")
                .ok()
                .unwrap_or_else(|| "gpt-4o".to_string());

            let config = ProviderConfig {
                name: "openai".to_string(),
                provider_type: ProviderType::OpenAI,
                api_key: api_key.clone(),
                base_url: Some(base_url.clone()),
                default_model: model.clone(),
                enabled: true,
            };

            // 创建 OpenAI provider
            let provider = crate::services::api::openai::OpenAiClient::new(
                &api_key,
                Some(&base_url),
                Some(&model),
            );
            registry.register("openai".to_string(), Arc::new(provider), config);
            if registry.selected().is_none() {
                registry.select("openai".to_string());
            }
        }

        // 支持 PRIORITY_AGENT_PROVIDER_<NAME> 环境变量配置额外 Provider
        for (key, value) in std::env::vars() {
            if key.starts_with("PRIORITY_AGENT_PROVIDER_") {
                let name = key
                    .strip_prefix("PRIORITY_AGENT_PROVIDER_")
                    .unwrap()
                    .to_lowercase();
                if !name.is_empty() && !value.is_empty() {
                    // 不记录原始 value，避免 API key 泄露到日志
                    info!("Configuring extra provider: {}", name);
                    if let Some(config) = parse_extra_provider_env(&name, &value) {
                        if let Some(provider) = Self::create_provider(&config) {
                            registry.register(name, provider, config);
                        }
                    } else {
                        warn!("Invalid provider env format for '{}'", name);
                    }
                }
            }
        }

        registry
    }

    /// 根据配置创建 Provider
    fn create_provider(config: &ProviderConfig) -> Option<Arc<dyn LlmProvider>> {
        match config.provider_type {
            ProviderType::Kimi => {
                let kimi_config = crate::services::api::kimi::KimiConfig {
                    api_key: config.api_key.clone(),
                    base_url: config
                        .base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.moonshot.cn/v1".to_string()),
                    default_model: config.default_model.clone(),
                    thinking_enabled: true,
                    thinking_budget: None,
                };
                Some(
                    Arc::new(crate::services::api::kimi::KimiClient::new(kimi_config))
                        as Arc<dyn LlmProvider>,
                )
            }
            ProviderType::OpenAI | ProviderType::OpenAICompat => {
                // OpenAICompat 和 OpenAI 使用相同的 Client
                Some(Arc::new(crate::services::api::openai::OpenAiClient::new(
                    &config.api_key,
                    config.base_url.as_deref(),
                    Some(&config.default_model),
                )) as Arc<dyn LlmProvider>)
            }
            ProviderType::Minimax => {
                // Minimax 也使用 OpenAI 兼容方式
                Some(Arc::new(crate::services::api::minimax::MiniMaxClient::new(
                    &config.api_key,
                    Some(
                        config
                            .base_url
                            .as_deref()
                            .unwrap_or("https://api.minimax.chat/v1"),
                    ),
                    Some(&config.default_model),
                )) as Arc<dyn LlmProvider>)
            }
            _ => {
                warn!("Unsupported provider type: {:?}", config.provider_type);
                None
            }
        }
    }

    /// 注册 Provider
    pub fn register(
        &mut self,
        name: String,
        provider: Arc<dyn LlmProvider>,
        config: ProviderConfig,
    ) {
        info!(
            "Registering provider: {} ({:?})",
            name, config.provider_type
        );
        self.providers.insert(name.clone(), provider);
        self.configs.insert(name, config);
    }

    /// 选择 Provider
    pub fn select(&mut self, name: String) {
        if self.providers.contains_key(&name) {
            info!("Selected provider: {}", name);
            self.selected = Some(name);
        } else {
            warn!("Provider not found: {}", name);
        }
    }

    /// 获取当前选中的 Provider
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// 获取当前选中的 Provider 实例
    pub fn get_selected_provider(&self) -> Option<Arc<dyn LlmProvider>> {
        self.selected
            .as_ref()
            .and_then(|name| self.providers.get(name).cloned())
    }

    /// 获取 Provider
    pub fn get(&self, name: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.get(name).cloned()
    }

    /// 获取所有 Provider 名称
    pub fn list(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// 获取所有启用的 Provider 配置
    pub fn list_configs(&self) -> Vec<ProviderConfig> {
        self.configs
            .values()
            .filter(|c| c.enabled)
            .cloned()
            .collect()
    }

    /// 获取配置
    pub fn get_config(&self, name: &str) -> Option<&ProviderConfig> {
        self.configs.get(name)
    }

    /// 检查是否有 Provider 可用
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// 获取 Provider 数量
    pub fn len(&self) -> usize {
        self.providers.len()
    }
}

fn parse_extra_provider_env(name: &str, value: &str) -> Option<ProviderConfig> {
    // 首选 JSON 格式，避免分隔符歧义:
    // {"type":"openai","api_key":"...","base_url":"https://...","model":"gpt-4o"}
    if value.trim_start().starts_with('{') {
        let json = serde_json::from_str::<serde_json::Value>(value).ok()?;
        let provider_type = ProviderType::parse_lossy(json.get("type")?.as_str()?);
        let api_key = json.get("api_key")?.as_str()?.to_string();
        let base_url = json
            .get("base_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let default_model = json
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("gpt-4o")
            .to_string();
        return Some(ProviderConfig {
            name: name.to_string(),
            provider_type,
            api_key,
            base_url,
            default_model,
            enabled: true,
        });
    }

    // 兼容旧格式: TYPE:API_KEY[:BASE_URL[:MODEL]]
    // 为了解决 URL 的 ":" 问题，当第三段以 http(s):// 开头时:
    // - 可用 "|" 显式分隔 model: TYPE:KEY:https://...|gpt-4o
    // - 不带 "|" 时视为仅提供 BASE_URL
    let mut parts = value.splitn(3, ':');
    let p_type = parts.next()?;
    let p_key = parts.next()?;
    let rest = parts.next();
    if p_type.is_empty() || p_key.is_empty() {
        return None;
    }

    let provider_type = ProviderType::parse_lossy(p_type);
    let api_key = p_key.to_string();
    let (base_url, default_model) = match rest {
        None => (None, "gpt-4o".to_string()),
        Some(rem) if rem.starts_with("http://") || rem.starts_with("https://") => {
            if let Some((url, model)) = rem.rsplit_once('|') {
                (Some(url.to_string()), model.to_string())
            } else {
                (Some(rem.to_string()), "gpt-4o".to_string())
            }
        }
        Some(rem) => {
            if let Some((url, model)) = rem.split_once(':') {
                (Some(url.to_string()), model.to_string())
            } else {
                (Some(rem.to_string()), "gpt-4o".to_string())
            }
        }
    };

    Some(ProviderConfig {
        name: name.to_string(),
        provider_type,
        api_key,
        base_url,
        default_model,
        enabled: true,
    })
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局 Provider 注册表（需要使用 Cell::new 初始化）
pub struct GlobalProviderRegistry;

impl GlobalProviderRegistry {
    /// 获取或创建全局注册表
    pub fn get() -> &'static tokio::sync::RwLock<ProviderRegistry> {
        static REGISTRY: std::sync::OnceLock<tokio::sync::RwLock<ProviderRegistry>> =
            std::sync::OnceLock::new();
        REGISTRY.get_or_init(|| tokio::sync::RwLock::new(ProviderRegistry::from_env()))
    }
}

/// 初始化全局注册表（同步版本）
pub fn init_global_registry_sync() {
    let _ = GlobalProviderRegistry::get();
}

/// 获取全局注册表
pub async fn get_registry() -> Option<tokio::sync::RwLockReadGuard<'static, ProviderRegistry>> {
    let guard = GlobalProviderRegistry::get().read().await;
    Some(guard)
}

/// 获取当前选中的 Provider
pub async fn get_selected_provider() -> Option<Arc<dyn LlmProvider>> {
    let guard = GlobalProviderRegistry::get().read().await;
    guard.get_selected_provider()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_from_str() {
        assert_eq!(ProviderType::parse_lossy("kimi"), ProviderType::Kimi);
        assert_eq!(ProviderType::parse_lossy("moonshot"), ProviderType::Kimi);
        assert_eq!(ProviderType::parse_lossy("openai"), ProviderType::OpenAI);
        assert_eq!(ProviderType::parse_lossy("anthropic"), ProviderType::Anthropic);
        assert_eq!(ProviderType::parse_lossy("unknown"), ProviderType::Custom);
    }

    #[test]
    fn test_registry_register() {
        // 这个测试需要 mock provider，实际测试有限
        let registry = ProviderRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_parse_extra_provider_env_json() {
        let cfg = parse_extra_provider_env(
            "demo",
            r#"{"type":"openai","api_key":"k1","base_url":"https://api.example.com/v1","model":"gpt-4o-mini"}"#,
        )
        .expect("should parse json");
        assert_eq!(cfg.name, "demo");
        assert_eq!(cfg.provider_type, ProviderType::OpenAI);
        assert_eq!(cfg.api_key, "k1");
        assert_eq!(cfg.base_url.as_deref(), Some("https://api.example.com/v1"));
        assert_eq!(cfg.default_model, "gpt-4o-mini");
    }

    #[test]
    fn test_parse_extra_provider_env_legacy_url_with_model_separator() {
        let cfg =
            parse_extra_provider_env("demo", "openai:k1:https://api.example.com/v1|gpt-4o-mini")
                .expect("should parse legacy");
        assert_eq!(cfg.provider_type, ProviderType::OpenAI);
        assert_eq!(cfg.api_key, "k1");
        assert_eq!(cfg.base_url.as_deref(), Some("https://api.example.com/v1"));
        assert_eq!(cfg.default_model, "gpt-4o-mini");
    }
}

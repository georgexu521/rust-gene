//! LLM Provider 注册表
//!
//! 支持动态注册和选择多个 LLM Provider

use crate::services::api::{
    provider_manifest::{ProviderManifest, ProviderManifestLoader},
    provider_protocol::{ProviderCapabilities, ProviderProtocolFamily},
    LlmProvider,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// Provider 配置
#[derive(Clone)]
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

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("name", &self.name)
            .field("provider_type", &self.provider_type)
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("enabled", &self.enabled)
            .finish()
    }
}

/// Provider 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    /// Kimi / Moonshot
    Kimi,
    /// Kimi Code Plan
    KimiCode,
    /// DeepSeek
    DeepSeek,
    /// GLM / Zhipu / Z.AI
    Glm,
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
            "kimi_code" | "kimi-code" | "kimi_code_plan" | "kimi-code-plan" => {
                ProviderType::KimiCode
            }
            "deepseek" => ProviderType::DeepSeek,
            "glm" | "zai" | "z.ai" | "zhipu" | "zhipuai" | "bigmodel" => ProviderType::Glm,
            "openai" => ProviderType::OpenAI,
            "openai_compat" | "openai-compatible" => ProviderType::OpenAICompat,
            "minimax" => ProviderType::Minimax,
            "anthropic" => ProviderType::Anthropic,
            "google" | "gemini" => ProviderType::Google,
            "azure" | "azure_openai" => ProviderType::Azure,
            _ => ProviderType::Custom,
        }
    }

    pub fn protocol_family(&self) -> ProviderProtocolFamily {
        match self {
            ProviderType::Kimi | ProviderType::KimiCode => ProviderProtocolFamily::Kimi,
            ProviderType::Minimax => ProviderProtocolFamily::MiniMax,
            ProviderType::Anthropic => ProviderProtocolFamily::AnthropicLike,
            ProviderType::OpenAI
            | ProviderType::OpenAICompat
            | ProviderType::DeepSeek
            | ProviderType::Glm
            | ProviderType::Google
            | ProviderType::Azure
            | ProviderType::Custom => ProviderProtocolFamily::OpenAiCompatible,
        }
    }

    pub fn capabilities(&self) -> ProviderCapabilities {
        match self {
            ProviderType::DeepSeek => {
                ProviderCapabilities::detect(DEEPSEEK_DEFAULT_BASE_URL, "deepseek-v4-flash")
            }
            _ => ProviderCapabilities::for_family(self.protocol_family()),
        }
    }
}

impl ProviderConfig {
    pub fn capabilities(&self) -> ProviderCapabilities {
        let detected = ProviderCapabilities::detect(
            self.base_url.as_deref().unwrap_or_default(),
            &self.default_model,
        );
        if detected.protocol_family == ProviderProtocolFamily::OpenAiCompatible {
            self.provider_type.capabilities()
        } else {
            detected
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse_lossy(s))
    }
}

pub const MINIMAX_DEFAULT_BASE_URL: &str = "https://api.minimax.io/v1";
pub const KIMI_DEFAULT_BASE_URL: &str = "https://api.moonshot.cn/v1";
pub const KIMI_CODE_DEFAULT_BASE_URL: &str = "https://api.kimi.com/coding/v1";
pub const DEEPSEEK_DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
pub const GLM_DEFAULT_BASE_URL: &str = "https://open.bigmodel.cn/api/paas/v4";
pub const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// Build a comma-separated hint of primary API key env vars for built-in providers.
pub fn provider_key_env_hint() -> String {
    ProviderManifestLoader::load_merged()
        .provider
        .iter()
        .filter_map(|entry| entry.env.first().cloned())
        .collect::<Vec<_>>()
        .join(", ")
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

        let test_provider_active = crate::services::api::test_provider::TestProvider::from_env()
            .map(|provider| {
                let config = ProviderConfig {
                    name: "test-fixture".to_string(),
                    provider_type: ProviderType::Custom,
                    api_key: "test-fixture".to_string(),
                    base_url: Some("test://provider".to_string()),
                    default_model: "test-fixture-model".to_string(),
                    enabled: true,
                };
                registry.register(
                    "test-fixture".to_string(),
                    Arc::new(provider) as Arc<dyn LlmProvider>,
                    config,
                );
                registry.select("test-fixture".to_string());
                true
            })
            .unwrap_or(false);

        let manifest = ProviderManifestLoader::load_merged();
        for entry in manifest.provider {
            if let Some(config) = provider_config_from_manifest(&entry) {
                if let Some(provider) = Self::create_provider(&config) {
                    let name = entry.id.clone();
                    registry.register(name.clone(), provider, config);
                    if registry.selected().is_none() {
                        registry.select(name);
                    }
                }
            }
        }

        // 支持 PRIORITY_AGENT_PROVIDER_<NAME> 环境变量配置额外 Provider
        for (key, value) in std::env::vars() {
            if key.starts_with("PRIORITY_AGENT_PROVIDER_") {
                let name = key
                    .strip_prefix("PRIORITY_AGENT_PROVIDER_")
                    .expect("provider env key must have PRIORITY_AGENT_PROVIDER_ prefix")
                    .to_lowercase();
                if !name.is_empty() && !value.is_empty() {
                    // 不记录原始 value，避免 API key 泄露到日志
                    info!("Configuring extra provider: {}", name);
                    if let Some(config) = parse_extra_provider_env(&name, &value) {
                        if let Some(provider) = Self::create_provider(&config) {
                            let select_name = name.clone();
                            registry.register(name, provider, config);
                            if registry.selected().is_none() {
                                registry.select(select_name);
                            }
                        }
                    } else {
                        warn!("Invalid provider env format for '{}'", name);
                    }
                }
            }
        }

        if !test_provider_active {
            if let Some(preferred) = env_non_empty("PRIORITY_AGENT_DEFAULT_PROVIDER") {
                let preferred = preferred.to_ascii_lowercase();
                if registry.providers.contains_key(&preferred) {
                    registry.select(preferred);
                } else {
                    warn!(
                    "PRIORITY_AGENT_DEFAULT_PROVIDER is set to '{}', but that provider is not configured",
                    preferred
                );
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
                        .unwrap_or_else(|| KIMI_DEFAULT_BASE_URL.to_string()),
                    default_model: config.default_model.clone(),
                    thinking_enabled: true,
                    thinking_budget: None,
                };
                Some(
                    Arc::new(crate::services::api::kimi::KimiClient::new(kimi_config))
                        as Arc<dyn LlmProvider>,
                )
            }
            ProviderType::OpenAI
            | ProviderType::OpenAICompat
            | ProviderType::KimiCode
            | ProviderType::DeepSeek
            | ProviderType::Glm => Some(Arc::new(
                crate::services::api::openai::OpenAiClient::new_with_label(
                    &config.name,
                    &config.api_key,
                    config.base_url.as_deref(),
                    Some(&config.default_model),
                ),
            ) as Arc<dyn LlmProvider>),
            ProviderType::Minimax => {
                // Minimax 也使用 OpenAI 兼容方式
                Some(Arc::new(crate::services::api::minimax::MiniMaxClient::new(
                    &config.api_key,
                    Some(
                        config
                            .base_url
                            .as_deref()
                            .unwrap_or(MINIMAX_DEFAULT_BASE_URL),
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

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn provider_config_from_manifest(entry: &ProviderManifest) -> Option<ProviderConfig> {
    let api_key = entry.resolve_api_key()?;
    let base_url = entry.resolve_base_url();
    let model = entry.resolve_model();
    let provider_type = ProviderType::parse_lossy(&entry.id);

    Some(ProviderConfig {
        name: entry.id.clone(),
        provider_type,
        api_key,
        base_url: Some(base_url),
        default_model: model,
        enabled: true,
    })
}

fn default_base_url_for_provider_type(provider_type: ProviderType) -> Option<&'static str> {
    match provider_type {
        ProviderType::Kimi => Some(KIMI_DEFAULT_BASE_URL),
        ProviderType::KimiCode => Some(KIMI_CODE_DEFAULT_BASE_URL),
        ProviderType::DeepSeek => Some(DEEPSEEK_DEFAULT_BASE_URL),
        ProviderType::Glm => Some(GLM_DEFAULT_BASE_URL),
        ProviderType::OpenAI => Some(OPENAI_DEFAULT_BASE_URL),
        ProviderType::Minimax => Some(MINIMAX_DEFAULT_BASE_URL),
        ProviderType::OpenAICompat
        | ProviderType::Anthropic
        | ProviderType::Google
        | ProviderType::Azure
        | ProviderType::Custom => None,
    }
}

fn default_model_for_provider_type(provider_type: ProviderType) -> &'static str {
    match provider_type {
        ProviderType::Kimi => "kimi-k2.5",
        ProviderType::KimiCode => "kimi-for-coding",
        ProviderType::DeepSeek => "deepseek-v4-flash",
        ProviderType::Glm => "glm-5.1",
        ProviderType::Minimax => "MiniMax-M3",
        ProviderType::OpenAI
        | ProviderType::OpenAICompat
        | ProviderType::Anthropic
        | ProviderType::Google
        | ProviderType::Azure
        | ProviderType::Custom => "gpt-4o",
    }
}

fn parse_extra_provider_env(name: &str, value: &str) -> Option<ProviderConfig> {
    // 首选 JSON 格式，避免分隔符歧义:
    // {"type":"openai","api_key":"...","base_url":"https://...","model":"gpt-4o"}
    if value.trim_start().starts_with('{') {
        let json = serde_json::from_str::<serde_json::Value>(value).ok()?;
        let provider_type = ProviderType::parse_lossy(json.get("type")?.as_str()?.trim());
        let api_key = json.get("api_key")?.as_str()?.trim().to_string();
        if api_key.is_empty() {
            return None;
        }
        let base_url = json
            .get("base_url")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .or_else(|| default_base_url_for_provider_type(provider_type).map(ToString::to_string));
        let default_model = json
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_model_for_provider_type(provider_type))
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
    let p_type = parts.next()?.trim();
    let p_key = parts.next()?.trim();
    let rest = parts.next().map(str::trim).filter(|s| !s.is_empty());
    if p_type.is_empty() || p_key.is_empty() {
        return None;
    }

    let provider_type = ProviderType::parse_lossy(p_type);
    let api_key = p_key.to_string();
    let (base_url, default_model) = match rest {
        None => (
            default_base_url_for_provider_type(provider_type).map(ToString::to_string),
            default_model_for_provider_type(provider_type).to_string(),
        ),
        Some(rem) if rem.starts_with("http://") || rem.starts_with("https://") => {
            if let Some((url, model)) = rem.rsplit_once('|') {
                let model = model.trim();
                (
                    Some(url.trim().to_string()),
                    if model.is_empty() {
                        default_model_for_provider_type(provider_type)
                    } else {
                        model
                    }
                    .to_string(),
                )
            } else {
                (
                    Some(rem.to_string()),
                    default_model_for_provider_type(provider_type).to_string(),
                )
            }
        }
        Some(rem) => {
            if let Some((url, model)) = rem.split_once(':') {
                let model = model.trim();
                (
                    Some(url.trim().to_string()),
                    if model.is_empty() {
                        default_model_for_provider_type(provider_type)
                    } else {
                        model
                    }
                    .to_string(),
                )
            } else {
                (
                    Some(rem.to_string()),
                    default_model_for_provider_type(provider_type).to_string(),
                )
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
    use crate::test_utils::env_guard::EnvVarGuard;

    #[test]
    fn test_provider_type_from_str() {
        assert_eq!(ProviderType::parse_lossy("kimi"), ProviderType::Kimi);
        assert_eq!(ProviderType::parse_lossy("moonshot"), ProviderType::Kimi);
        assert_eq!(
            ProviderType::parse_lossy("kimi-code-plan"),
            ProviderType::KimiCode
        );
        assert_eq!(
            ProviderType::parse_lossy("deepseek"),
            ProviderType::DeepSeek
        );
        assert_eq!(ProviderType::parse_lossy("zhipuai"), ProviderType::Glm);
        assert_eq!(ProviderType::parse_lossy("openai"), ProviderType::OpenAI);
        assert_eq!(
            ProviderType::parse_lossy("anthropic"),
            ProviderType::Anthropic
        );
        assert_eq!(ProviderType::parse_lossy("unknown"), ProviderType::Custom);
    }

    #[test]
    fn test_provider_type_capabilities() {
        let minimax = ProviderType::Minimax.capabilities();
        assert_eq!(minimax.protocol_family, ProviderProtocolFamily::MiniMax);
        assert!(minimax.requires_nonstreaming_tool_calls);

        let kimi_code = ProviderType::KimiCode.capabilities();
        assert_eq!(kimi_code.protocol_family, ProviderProtocolFamily::Kimi);

        let deepseek = ProviderType::DeepSeek.capabilities();
        assert_eq!(
            deepseek.protocol_family,
            ProviderProtocolFamily::OpenAiCompatible
        );
        assert!(!deepseek.supports_streaming_tool_calls);
        assert!(deepseek.requires_nonstreaming_tool_calls);

        let openai = ProviderType::OpenAI.capabilities();
        assert_eq!(
            openai.protocol_family,
            ProviderProtocolFamily::OpenAiCompatible
        );
        assert!(!openai.requires_nonstreaming_tool_calls);
    }

    #[test]
    fn test_provider_config_capabilities_detect_model_and_base_url() {
        let cfg = ProviderConfig {
            name: "custom-minimax".to_string(),
            provider_type: ProviderType::OpenAICompat,
            api_key: "k".to_string(),
            base_url: Some("https://api.minimaxi.com/v1".to_string()),
            default_model: "MiniMax-M3".to_string(),
            enabled: true,
        };

        let capabilities = cfg.capabilities();

        assert_eq!(
            capabilities.protocol_family,
            ProviderProtocolFamily::MiniMax
        );
        assert!(capabilities.requires_nonstreaming_tool_calls);
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
    fn test_parse_extra_provider_env_json_rejects_blank_api_key() {
        assert!(parse_extra_provider_env(
            "demo",
            r#"{"type":"openai","api_key":"   ","base_url":"https://api.example.com/v1"}"#,
        )
        .is_none());
    }

    #[test]
    fn test_parse_extra_provider_env_json_trims_optional_values() {
        let cfg = parse_extra_provider_env(
            "demo",
            r#"{"type":" openai ","api_key":" k1 ","base_url":" https://api.example.com/v1 ","model":"  "}"#,
        )
        .expect("should parse json");
        assert_eq!(cfg.provider_type, ProviderType::OpenAI);
        assert_eq!(cfg.api_key, "k1");
        assert_eq!(cfg.base_url.as_deref(), Some("https://api.example.com/v1"));
        assert_eq!(cfg.default_model, "gpt-4o");
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

    #[test]
    fn test_parse_extra_provider_env_legacy_rejects_blank_api_key() {
        assert!(
            parse_extra_provider_env("demo", "openai:   :https://api.example.com/v1").is_none()
        );
    }

    fn clear_default_provider_env(env: &mut EnvVarGuard) {
        for entry in ProviderManifestLoader::load_merged().provider {
            for key in entry
                .env
                .iter()
                .chain(entry.base_url_env.iter())
                .chain(entry.model_env.iter())
            {
                env.remove(key);
            }
        }
        env.remove("PRIORITY_AGENT_DEFAULT_PROVIDER");
        env.remove("PRIORITY_AGENT_TEST_PROVIDER_SCENARIO");
        env.remove("PRIORITY_AGENT_TEST_PROVIDER_SLEEP_SECS");
    }

    #[test]
    fn test_from_env_prefers_minimax_when_configured() {
        let mut env = EnvVarGuard::acquire_blocking();
        clear_default_provider_env(&mut env);
        env.set("MINIMAX_API_KEY", "minimax-key");
        env.set("MINIMAX_BASE_URL", "https://minimax.example/v1");
        env.set("MINIMAX_MODEL", "MiniMax-Test");
        env.set("OPENAI_API_KEY", "openai-key");
        env.set("OPENAI_MODEL", "gpt-test");

        let registry = ProviderRegistry::from_env();
        assert_eq!(registry.selected(), Some("minimax"));
        let cfg = registry.get_config("minimax").expect("minimax config");
        assert_eq!(cfg.provider_type, ProviderType::Minimax);
        assert_eq!(cfg.base_url.as_deref(), Some("https://minimax.example/v1"));
        assert_eq!(cfg.default_model, "MiniMax-Test");
        assert!(registry.get("openai").is_some());
    }

    #[test]
    fn test_from_env_registers_new_coding_providers() {
        let mut env = EnvVarGuard::acquire_blocking();
        clear_default_provider_env(&mut env);
        env.set("KIMI_CODE_API_KEY", "kimi-code-key");
        env.set("DEEPSEEK_API_KEY", "deepseek-key");
        env.set("GLM_API_KEY", "glm-key");

        let registry = ProviderRegistry::from_env();

        assert_eq!(registry.selected(), Some("kimi-code"));
        assert_eq!(
            registry
                .get_config("kimi-code")
                .expect("kimi-code config")
                .default_model
                .as_str(),
            "kimi-for-coding"
        );
        assert_eq!(
            registry
                .get_config("deepseek")
                .expect("deepseek config")
                .base_url
                .as_deref(),
            Some(DEEPSEEK_DEFAULT_BASE_URL)
        );
        assert_eq!(
            registry
                .get_config("glm")
                .expect("glm config")
                .provider_type,
            ProviderType::Glm
        );
    }

    #[test]
    fn test_from_env_allows_default_provider_override() {
        let mut env = EnvVarGuard::acquire_blocking();
        clear_default_provider_env(&mut env);
        env.set("MINIMAX_API_KEY", "minimax-key");
        env.set("DEEPSEEK_API_KEY", "deepseek-key");
        env.set("PRIORITY_AGENT_DEFAULT_PROVIDER", "deepseek");

        let registry = ProviderRegistry::from_env();

        assert_eq!(registry.selected(), Some("deepseek"));
    }

    #[test]
    fn test_from_env_registers_test_provider_when_scenario_is_set() {
        let mut env = EnvVarGuard::acquire_blocking();
        clear_default_provider_env(&mut env);
        env.set("PRIORITY_AGENT_TEST_PROVIDER_SCENARIO", "tool-pwd");

        let registry = ProviderRegistry::from_env();

        assert_eq!(registry.selected(), Some("test-fixture"));
        assert!(registry.get("test-fixture").is_some());
        let cfg = registry
            .get_config("test-fixture")
            .expect("test fixture config");
        assert_eq!(cfg.default_model, "test-fixture-model");
        assert_eq!(cfg.base_url.as_deref(), Some("test://provider"));
    }

    #[test]
    fn test_from_env_test_provider_scenario_beats_default_provider_override() {
        let mut env = EnvVarGuard::acquire_blocking();
        clear_default_provider_env(&mut env);
        env.set("PRIORITY_AGENT_TEST_PROVIDER_SCENARIO", "tool-pwd");
        env.set("MINIMAX_API_KEY", "minimax-key");
        env.set("PRIORITY_AGENT_DEFAULT_PROVIDER", "minimax");

        let registry = ProviderRegistry::from_env();

        assert_eq!(registry.selected(), Some("test-fixture"));
        assert!(registry.get("minimax").is_some());
    }

    #[test]
    fn test_from_env_ignores_empty_provider_keys() {
        let mut env = EnvVarGuard::acquire_blocking();
        clear_default_provider_env(&mut env);
        env.set("MINIMAX_API_KEY", "");
        env.set("OPENAI_API_KEY", "   ");
        env.set("MOONSHOT_API_KEY", "");
        env.set("DEEPSEEK_API_KEY", "");

        let registry = ProviderRegistry::from_env();

        assert!(registry.is_empty());
        assert_eq!(registry.selected(), None);
    }
}

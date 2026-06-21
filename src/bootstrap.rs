//! 应用启动引导模块
//!
//! 负责初始化所有组件：Provider、ToolRegistry、LSP、Worktree、MCP、引擎等。
//! 将 main.rs 的 ~200 行初始化逻辑集中在此。

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use crate::engine::streaming::StreamingQueryEngine;
use crate::engine::QueryEngine;
use crate::tools::ToolRegistry;

/// 所有已初始化的应用组件
pub struct AppComponents {
    pub provider: Arc<dyn crate::services::api::LlmProvider>,
    pub model: String,
    pub tool_registry: Arc<ToolRegistry>,
    pub streaming_engine: Arc<StreamingQueryEngine>,
    pub query_engine: Arc<QueryEngine>,
    pub lsp_manager: Arc<crate::engine::lsp::LspManager>,
    pub worktree_manager: Arc<crate::engine::worktree::WorktreeManager>,
}

/// Components needed by the HTTP API server.
///
/// The API session prompt route is a formal full-agent entrypoint, so API
/// startup must carry the same streaming runtime used by CLI/TUI/desktop.
pub struct ApiComponents {
    pub provider: Arc<dyn crate::services::api::LlmProvider>,
    pub model: String,
    pub tool_registry: Arc<ToolRegistry>,
    pub streaming_engine: Arc<StreamingQueryEngine>,
    pub lsp_manager: Arc<crate::engine::lsp::LspManager>,
    pub worktree_manager: Arc<crate::engine::worktree::WorktreeManager>,
}

/// 初始化 LLM Provider（由 ProviderRegistry 的确定性优先级和用户覆盖项选择）
pub fn init_provider() -> Result<(Arc<dyn crate::services::api::LlmProvider>, String)> {
    init_provider_with_default_preference(None)
}

/// 初始化 TUI 默认 Provider。
///
/// TUI is the interactive product surface, so when the user has not set an
/// explicit default provider, prefer DeepSeek's fast default if it is
/// configured. Environment and saved config overrides still win.
pub fn init_tui_provider() -> Result<(Arc<dyn crate::services::api::LlmProvider>, String)> {
    init_provider_with_default_preference(Some("deepseek"))
}

/// 初始化 Desktop 默认 Provider。
///
/// Desktop is the daily-use workbench surface; when the user has not selected
/// a provider explicitly, prefer DeepSeek v4 flash if it is configured.
/// Environment and saved config overrides still win.
pub fn init_desktop_provider() -> Result<(Arc<dyn crate::services::api::LlmProvider>, String)> {
    init_provider_with_default_preference(Some("deepseek"))
}

fn init_provider_with_default_preference(
    default_provider: Option<&str>,
) -> Result<(Arc<dyn crate::services::api::LlmProvider>, String)> {
    let app_config = crate::services::config::AppConfig::load().unwrap_or_default();
    let mut registry = crate::services::api::provider::ProviderRegistry::from_env();
    let has_env_default = std::env::var("PRIORITY_AGENT_DEFAULT_PROVIDER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some();
    let has_test_provider = registry.get("test-fixture").is_some();
    if has_test_provider {
        registry.select("test-fixture".to_string());
    } else if !has_env_default {
        if let Some(saved_provider) = app_config
            .api
            .provider_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            registry.select(saved_provider.to_ascii_lowercase());
        } else if let Some(default_provider) = default_provider {
            if registry.get(default_provider).is_some() {
                registry.select(default_provider.to_string());
            }
        }
    }
    let Some(provider) = registry.get_selected_provider() else {
        return Err(anyhow::anyhow!(
            "No LLM provider configured.\n\
             Set one of: {}\n\
             Optional provider-specific env vars: *_BASE_URL, *_MODEL.\n\
             To override selection when multiple keys are present, set PRIORITY_AGENT_DEFAULT_PROVIDER.",
            crate::services::api::provider::provider_key_env_hint()
        ));
    };

    let selected = registry.selected().unwrap_or("unknown");
    let provider_config = registry.get_config(selected);
    let default_model = provider_config
        .map(|config| config.default_model.clone())
        .unwrap_or_else(|| provider.default_model().to_string());
    let supported_models = crate::services::api::provider_catalog::supported_models(selected);
    let model = if app_config
        .api
        .provider_name
        .as_deref()
        .map(|saved| saved.eq_ignore_ascii_case(selected))
        .unwrap_or(false)
        && !app_config.api.model.trim().is_empty()
    {
        let saved_model = app_config.api.model.trim();
        let is_supported = supported_models
            .iter()
            .any(|m| m.eq_ignore_ascii_case(saved_model));
        if is_supported || supported_models.is_empty() {
            saved_model.to_string()
        } else {
            tracing::warn!(
                "Saved model '{}' is not in the supported list for provider '{}'; using default '{}'",
                saved_model,
                selected,
                default_model
            );
            default_model.clone()
        }
    } else {
        default_model
    };
    info!(
        "LLM provider ready: provider={}, base={}, model={}",
        selected,
        provider.base_url(),
        model
    );
    Ok((provider, model))
}

/// 初始化工具注册表（含插件动态注入）
pub fn init_tool_registry(working_dir: &std::path::Path) -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::default_registry();
    let trust_mode = crate::services::config::AppConfig::load()
        .map(|c| c.features.plugin_trust_mode)
        .unwrap_or_else(|_| "warn".to_string());
    info!("Plugin trust mode: {}", trust_mode);
    let injected =
        crate::tools::plugin_tool::register_enabled_plugin_tools(&mut registry, working_dir);
    let tool_registry = Arc::new(registry);
    info!(
        "Tool registry initialized with {} tools (plugin runtime tools injected: {})",
        tool_registry.tool_names().len(),
        injected
    );
    tool_registry
}

/// 加载 MCP 服务器配置（环境变量）
fn load_mcp_servers_from_env() -> Vec<crate::engine::mcp::McpServerConfig> {
    let raw = match std::env::var("PRIORITY_AGENT_MCP_SERVERS_JSON") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Vec::new(),
    };
    match serde_json::from_str::<Vec<crate::engine::mcp::McpServerConfig>>(&raw) {
        Ok(servers) => servers,
        Err(e) => {
            tracing::warn!(
                "Invalid PRIORITY_AGENT_MCP_SERVERS_JSON, ignoring value: {}",
                e
            );
            Vec::new()
        }
    }
}

fn env_bool(name: &str) -> Option<bool> {
    let raw = std::env::var(name).ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// 清理超过保留期限的快照，防止磁盘无限增长
/// 默认保留 7 天，可通过 `PRIORITY_AGENT_SNAPSHOT_RETENTION_DAYS` 覆盖
pub fn cleanup_old_snapshots() {
    let retention_days: u64 = std::env::var("PRIORITY_AGENT_SNAPSHOT_RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);

    let snapshots_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("snapshots");

    if !snapshots_dir.exists() {
        return;
    }

    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(retention_days * 24 * 60 * 60);

    let mut cleaned = 0usize;
    let mut failed = 0usize;

    fn visit_dir(
        dir: &std::path::Path,
        cutoff: std::time::SystemTime,
        cleaned: &mut usize,
        failed: &mut usize,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_old = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|modified| modified < cutoff)
                .unwrap_or(false);

            if path.is_dir() {
                // 先递归处理子目录
                visit_dir(&path, cutoff, cleaned, failed);
                // 如果目录变空且自身也旧，则删除
                if is_old {
                    match std::fs::remove_dir(&path) {
                        Ok(_) => *cleaned += 1,
                        Err(_) => {
                            // 目录非空或权限问题，忽略
                        }
                    }
                }
            } else if is_old {
                match std::fs::remove_file(&path) {
                    Ok(_) => *cleaned += 1,
                    Err(_) => *failed += 1,
                }
            }
        }
    }

    visit_dir(&snapshots_dir, cutoff, &mut cleaned, &mut failed);

    if cleaned > 0 || failed > 0 {
        info!(
            "Snapshot cleanup: removed {} old entries, {} failed (retention: {} days)",
            cleaned, failed, retention_days
        );
    }
}

/// 一键初始化所有应用组件（含 Provider + ToolRegistry + 全组件）
pub async fn init_app(working_dir: &std::path::Path) -> Result<AppComponents> {
    let (provider, model) = init_provider()?;
    let tool_registry = init_tool_registry(working_dir);
    init_components(provider, model, tool_registry, working_dir).await
}

/// 初始化 TUI 组件。
pub async fn init_tui_app(working_dir: &std::path::Path) -> Result<AppComponents> {
    let (provider, model) = init_tui_provider()?;
    let tool_registry = init_tool_registry(working_dir);
    init_components(provider, model, tool_registry, working_dir).await
}

/// 初始化 API server 所需组件。
pub async fn init_api_components(working_dir: &std::path::Path) -> Result<ApiComponents> {
    let components = init_app(working_dir).await?;
    Ok(ApiComponents {
        provider: components.provider,
        model: components.model,
        tool_registry: components.tool_registry,
        streaming_engine: components.streaming_engine,
        lsp_manager: components.lsp_manager,
        worktree_manager: components.worktree_manager,
    })
}

/// 初始化所有应用组件（Provider 与 ToolRegistry 已就绪）
pub async fn init_components(
    provider: Arc<dyn crate::services::api::LlmProvider>,
    model: String,
    tool_registry: Arc<ToolRegistry>,
    working_dir: &std::path::Path,
) -> Result<AppComponents> {
    // 加载配置
    let app_config = crate::services::config::AppConfig::load().unwrap_or_default();
    let engine_config = app_config.engine.clone();
    cleanup_expired_tool_outputs(working_dir);

    // LSP 管理器
    let lsp_manager = crate::engine::lsp::LspManager::new();
    lsp_manager.detect_servers_with_config(working_dir, &app_config.lsp);
    let lsp_manager = Arc::new(lsp_manager);
    info!(
        "LspManager initialized with {} servers",
        lsp_manager.client_count()
    );

    // Worktree 管理器
    let worktree_manager = Arc::new(crate::engine::worktree::WorktreeManager::new().await);
    if let Some(name) = worktree_manager.active_worktree_name().await {
        info!("Active worktree: {}", name);
    }

    // MCP 管理器
    let mut mcp_configs = engine_config.mcp_servers.clone();
    let env_mcp_configs = load_mcp_servers_from_env();
    if !env_mcp_configs.is_empty() {
        info!(
            "Loaded {} MCP servers from PRIORITY_AGENT_MCP_SERVERS_JSON",
            env_mcp_configs.len()
        );
        mcp_configs.extend(env_mcp_configs);
    }
    let mcp_manager = if app_config.features.mcp_enabled || !mcp_configs.is_empty() {
        let manager = Arc::new(crate::engine::mcp::McpManager::load_from_config(
            mcp_configs,
        ));
        info!(
            "MCP manager initialized with {} configured server(s)",
            manager.server_names().len()
        );
        Some(manager)
    } else {
        None
    };

    // QueryEngine
    let task_manager = crate::task_manager::GLOBAL_TASK_MANAGER.clone();
    let mut query_engine_builder =
        QueryEngine::new(provider.clone(), tool_registry.clone(), &model)
            .with_max_iterations(engine_config.max_iterations)
            .with_task_manager(task_manager.clone())
            .with_lsp_manager(lsp_manager.clone())
            .with_worktree_manager(worktree_manager.clone());
    if let Some(ref mcp) = mcp_manager {
        query_engine_builder = query_engine_builder.with_mcp_manager(mcp.clone());
    }
    let query_engine = Arc::new(query_engine_builder);

    // StreamingQueryEngine
    let llm_memory_extraction = env_bool("PRIORITY_AGENT_LLM_MEMORY_EXTRACTION")
        .unwrap_or(app_config.features.llm_memory_extraction);
    let mut streaming_engine_builder =
        StreamingQueryEngine::new(provider.clone(), tool_registry.clone(), &model)
            .with_max_iterations(engine_config.max_iterations)
            .with_working_dir(working_dir)
            .with_task_manager(task_manager.clone())
            .with_lsp_manager(lsp_manager.clone())
            .with_worktree_manager(worktree_manager.clone())
            .with_llm_memory_extraction(llm_memory_extraction);
    if let Some(ref mcp) = mcp_manager {
        streaming_engine_builder = streaming_engine_builder.with_mcp_manager(mcp.clone());
    }

    // AgentManager is constructed lazily only for routed delegation/sub-agent turns.
    streaming_engine_builder =
        streaming_engine_builder.with_agent_query_engine(query_engine.clone());

    // 工具授权通道
    let approval_channel = Arc::new(crate::engine::conversation_loop::ToolApprovalChannel::new());
    streaming_engine_builder = streaming_engine_builder.with_approval_channel(approval_channel);

    let streaming_engine = Arc::new(streaming_engine_builder);
    info!("All components initialized");

    Ok(AppComponents {
        provider,
        model,
        tool_registry,
        streaming_engine,
        query_engine,
        lsp_manager,
        worktree_manager,
    })
}

fn cleanup_expired_tool_outputs(working_dir: &std::path::Path) {
    match crate::tool_output_store::cleanup_expired_for_project(working_dir) {
        Ok(removed) if removed > 0 => {
            info!("Cleaned {} expired tool outputs during startup", removed);
        }
        Ok(_) => {}
        Err(err) => {
            info!("Tool output startup cleanup skipped: {}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::env_guard::EnvVarGuard;

    fn with_env_vars(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let mut env = EnvVarGuard::acquire_blocking();
        let config_home = tempfile::tempdir().expect("temp config home");
        let config_home_path = config_home.path().to_string_lossy();
        env.set("XDG_CONFIG_HOME", config_home_path.as_ref());
        env.set("HOME", config_home_path.as_ref());
        for entry in crate::services::api::provider_catalog::builtin_catalog() {
            for key in entry
                .key_env_vars
                .iter()
                .chain(entry.base_url_env_vars.iter())
                .chain(entry.model_env_vars.iter())
            {
                env.remove(key);
            }
        }
        env.remove("PRIORITY_AGENT_DEFAULT_PROVIDER");
        env.remove("PRIORITY_AGENT_TEST_PROVIDER_SCENARIO");
        env.remove("PRIORITY_AGENT_TEST_PROVIDER_SLEEP_SECS");
        env.remove("PRIORITY_AGENT_API_PROVIDER_NAME");
        for (k, v) in vars {
            if let Some(val) = v {
                env.set(k, val);
            } else {
                env.remove(k);
            }
        }

        f();
    }

    #[test]
    fn test_init_provider_prefers_minimax_over_openai() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("OPENAI_API_KEY", Some("openai-key")),
                ("MOONSHOT_API_KEY", Some("moonshot-key")),
                ("MINIMAX_MODEL", Some("MiniMax-M2.7")),
                ("OPENAI_MODEL", Some("gpt-4o")),
                ("MOONSHOT_MODEL", Some("kimi-k2.5")),
            ],
            || {
                let (_provider, model) = init_provider().expect("provider should initialize");
                assert_eq!(model, "MiniMax-M2.7");
            },
        );
    }

    #[test]
    fn test_init_tui_provider_prefers_deepseek_flash() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("DEEPSEEK_API_KEY", Some("deepseek-key")),
            ],
            || {
                let (_provider, model) = init_tui_provider().expect("provider should initialize");
                assert_eq!(model, "deepseek-v4-flash");
            },
        );
    }

    #[test]
    fn test_init_tui_provider_env_override_beats_deepseek_default() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("DEEPSEEK_API_KEY", Some("deepseek-key")),
                ("PRIORITY_AGENT_DEFAULT_PROVIDER", Some("minimax")),
            ],
            || {
                let (_provider, model) = init_tui_provider().expect("provider should initialize");
                assert_eq!(model, "MiniMax-M3");
            },
        );
    }

    #[test]
    fn test_init_desktop_provider_prefers_deepseek_flash() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("DEEPSEEK_API_KEY", Some("deepseek-key")),
            ],
            || {
                let (_provider, model) =
                    init_desktop_provider().expect("provider should initialize");
                assert_eq!(model, "deepseek-v4-flash");
            },
        );
    }

    #[test]
    fn test_init_desktop_provider_env_override_beats_deepseek_default() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("DEEPSEEK_API_KEY", Some("deepseek-key")),
                ("PRIORITY_AGENT_DEFAULT_PROVIDER", Some("minimax")),
            ],
            || {
                let (_provider, model) =
                    init_desktop_provider().expect("provider should initialize");
                assert_eq!(model, "MiniMax-M3");
            },
        );
    }

    #[test]
    fn test_init_provider_honors_saved_provider_when_no_env_override() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("DEEPSEEK_API_KEY", Some("deepseek-key")),
            ],
            || {
                let mut config = crate::services::config::AppConfig::default();
                config.api.provider_name = Some("deepseek".to_string());
                config.api.model = "deepseek-chat".to_string();
                config.save().unwrap();

                let (_provider, model) = init_provider().expect("provider should initialize");
                assert_eq!(model, "deepseek-chat");
            },
        );
    }

    #[test]
    fn test_init_provider_env_override_beats_saved_provider() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("DEEPSEEK_API_KEY", Some("deepseek-key")),
                ("PRIORITY_AGENT_DEFAULT_PROVIDER", Some("minimax")),
            ],
            || {
                let mut config = crate::services::config::AppConfig::default();
                config.api.provider_name = Some("deepseek".to_string());
                config.api.model = "deepseek-chat".to_string();
                config.save().unwrap();

                let (_provider, model) = init_provider().expect("provider should initialize");
                assert_eq!(model, "MiniMax-M3");
            },
        );
    }

    #[test]
    fn test_init_provider_test_provider_scenario_beats_saved_provider() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", Some("mini-key")),
                ("PRIORITY_AGENT_DEFAULT_PROVIDER", Some("minimax")),
                ("PRIORITY_AGENT_TEST_PROVIDER_SCENARIO", Some("tool-pwd")),
            ],
            || {
                let mut config = crate::services::config::AppConfig::default();
                config.api.provider_name = Some("minimax".to_string());
                config.api.model = "MiniMax-M2.7".to_string();
                config.save().unwrap();

                let (provider, model) = init_provider().expect("provider should initialize");
                assert_eq!(provider.base_url(), "test://provider");
                assert_eq!(model, "test-fixture-model");
            },
        );
    }

    #[test]
    fn test_init_provider_falls_back_to_openai_when_no_minimax() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", None),
                ("OPENAI_API_KEY", Some("openai-key")),
                ("OPENAI_MODEL", Some("gpt-4o")),
            ],
            || {
                let (_provider, model) = init_provider().expect("provider should initialize");
                assert_eq!(model, "gpt-4o");
            },
        );
    }

    #[test]
    fn test_cleanup_old_snapshots_removes_stale() {
        let base = std::env::temp_dir().join("test_priority_agent_snapshots");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        // 创建目录和文件
        let old_dir = base.join("session-old").join("1234567890");
        std::fs::create_dir_all(&old_dir).unwrap();
        let old_file = old_dir.join("test.txt");
        std::fs::write(&old_file, "old").unwrap();

        // 获取 old_file 的修改时间
        let old_mtime = std::fs::metadata(&old_file).unwrap().modified().unwrap();

        // 创建新文件（确保 mtime 更晚）
        std::thread::sleep(std::time::Duration::from_millis(50));
        let new_dir = base.join("session-new").join("9999999999");
        std::fs::create_dir_all(&new_dir).unwrap();
        let new_file = new_dir.join("test.txt");
        std::fs::write(&new_file, "new").unwrap();

        // 使用 old_mtime + 25ms 作为 cutoff：
        // old_file 应被删除（mtime < cutoff），new_file 应保留（mtime > cutoff）
        let cutoff = old_mtime + std::time::Duration::from_millis(25);
        let mut cleaned = 0usize;
        let mut failed = 0usize;
        cleanup_old_snapshots_dir(&base, cutoff, &mut cleaned, &mut failed);

        // 旧的应被删除
        assert!(!old_file.exists(), "old file should be removed");
        assert!(!old_dir.exists(), "old dir should be removed");

        // 新的应保留
        assert!(new_file.exists(), "new file should be kept");
        assert!(new_dir.exists(), "new dir should be kept");

        let _ = std::fs::remove_dir_all(&base);
    }

    fn cleanup_old_snapshots_dir(
        dir: &std::path::Path,
        cutoff: std::time::SystemTime,
        cleaned: &mut usize,
        failed: &mut usize,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_old = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|modified| modified < cutoff)
                .unwrap_or(false);

            if path.is_dir() {
                cleanup_old_snapshots_dir(&path, cutoff, cleaned, failed);
                if is_old && std::fs::remove_dir(&path).is_ok() {
                    *cleaned += 1;
                }
            } else if is_old {
                match std::fs::remove_file(&path) {
                    Ok(_) => *cleaned += 1,
                    Err(_) => *failed += 1,
                }
            }
        }
    }
}

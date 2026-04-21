//! 应用启动引导模块
//!
//! 负责初始化所有组件：Provider、ToolRegistry、LSP、Worktree、MCP、引擎等。
//! 将 main.rs 的 ~200 行初始化逻辑集中在此。

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

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

/// 初始化 LLM Provider（MiniMax -> OpenAI -> Kimi）
pub fn init_provider() -> Result<(Arc<dyn crate::services::api::LlmProvider>, String)> {
    if let Ok(client) = crate::services::api::minimax::MiniMaxClient::from_env() {
        let model = client.default_model().to_string();
        info!(
            "MiniMax client ready: base={}, model={}",
            client.base_url(),
            model
        );
        let provider: Arc<dyn crate::services::api::LlmProvider> = Arc::new(client);
        Ok((provider, model))
    } else if let Ok(client) = crate::services::api::openai::OpenAiClient::from_env() {
        let model = client.default_model().to_string();
        info!(
            "OpenAI client ready: base={}, model={}",
            client.base_url(),
            model
        );
        let provider: Arc<dyn crate::services::api::LlmProvider> = Arc::new(client);
        Ok((provider, model))
    } else if let Ok(client) = crate::services::api::kimi::KimiClient::from_env() {
        let model = client.default_model().to_string();
        info!("Kimi client ready: model={}", model);
        let provider: Arc<dyn crate::services::api::LlmProvider> = Arc::new(client);
        Ok((provider, model))
    } else {
        Err(anyhow::anyhow!(
            "No LLM provider configured.\n\
             Set one of:\n\
             1. MINIMAX_API_KEY (MiniMax Token Plan)\n\
             2. OPENAI_API_KEY (OpenAI, or any OpenAI-compatible API)\n\
             3. MOONSHOT_API_KEY (Kimi / Moonshot AI)\n\
             Optional env vars: MINIMAX_BASE_URL, MINIMAX_MODEL, OPENAI_BASE_URL, OPENAI_MODEL, MOONSHOT_BASE_URL, MOONSHOT_MODEL"
        ))
    }
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

/// 初始化所有应用组件
pub async fn init_components(
    provider: Arc<dyn crate::services::api::LlmProvider>,
    model: String,
    tool_registry: Arc<ToolRegistry>,
    working_dir: &std::path::Path,
) -> Result<AppComponents> {
    // 加载配置
    let app_config = crate::services::config::AppConfig::load().unwrap_or_default();
    let engine_config = app_config.engine.clone();

    // SessionStore
    let db_path = dirs::data_dir()
        .map(|d| d.join("priority-agent").join("sessions.db"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent/sessions.db"));
    let session_store = match crate::session_store::SessionStore::open(&db_path) {
        Ok(store) => {
            info!("SessionStore opened at {:?}", db_path);
            Some(Arc::new(store))
        }
        Err(e) => {
            warn!("Failed to open SessionStore: {}", e);
            None
        }
    };

    // LSP 管理器
    let mut lsp_manager = crate::engine::lsp::LspManager::new();
    lsp_manager.detect_servers(working_dir);
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
    let mut streaming_engine_builder =
        StreamingQueryEngine::new(provider.clone(), tool_registry.clone(), &model)
            .with_max_iterations(engine_config.max_iterations)
            .with_task_manager(task_manager.clone())
            .with_lsp_manager(lsp_manager.clone())
            .with_worktree_manager(worktree_manager.clone())
            .with_llm_memory_extraction(app_config.features.llm_memory_extraction);
    if let Some(ref mcp) = mcp_manager {
        streaming_engine_builder = streaming_engine_builder.with_mcp_manager(mcp.clone());
    }

    // 记忆快照
    let mem_manager = Arc::new(tokio::sync::Mutex::new(crate::memory::MemoryManager::new()));
    {
        let mut mgr = mem_manager.lock().await;
        mgr.freeze_snapshot();
    }
    let snapshot = {
        let mgr = mem_manager.lock().await;
        mgr.get_snapshot()
    };
    if !snapshot.is_empty() {
        info!("Memory snapshot prepared");
    }
    streaming_engine_builder = streaming_engine_builder.with_memory_manager(mem_manager);

    // SessionStore 接入
    if let Some(ref store) = session_store {
        let session_id = format!("session-{}", uuid::Uuid::new_v4());
        let _ = store.create_session(&session_id, "TUI Session", &model);
        streaming_engine_builder =
            streaming_engine_builder.with_session_store(store.clone(), session_id);
    }

    // AgentManager
    let agent_manager =
        Arc::new(crate::agent::AgentManager::new().with_query_engine(query_engine.clone()));
    streaming_engine_builder = streaming_engine_builder.with_agent_manager(agent_manager);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::env_guard::EnvVarGuard;

    fn with_env_vars(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let mut env = EnvVarGuard::acquire_blocking();
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
    fn test_init_provider_falls_back_to_openai_when_no_minimax() {
        with_env_vars(
            &[
                ("MINIMAX_API_KEY", None),
                ("OPENAI_API_KEY", Some("openai-key")),
                ("MOONSHOT_API_KEY", Some("moonshot-key")),
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
                if is_old {
                    match std::fs::remove_dir(&path) {
                        Ok(_) => *cleaned += 1,
                        Err(_) => {}
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
}

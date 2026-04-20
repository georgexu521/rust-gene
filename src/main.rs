//! Priority Agent - 加权优先级桌面 Agent
//!
//! 解决 AI Agent 抓不住重点的问题，通过显式的权重系统让 AI 始终专注于最重要的事项。
//! 高密度思考 = 高密度 Q&A — Agent 应不断提问/解答来深化推理。

#![allow(dead_code)]

// ─── Core Modules ────────────────────────────────────────────────────
pub mod agent;
pub mod ai_analyzer;
pub mod api;
pub mod bootstrap;
pub mod bridge;
pub mod cli;
pub mod context_manager;
pub mod cost_tracker;
pub mod diagnostics;
pub mod engine;
pub mod errors;
pub mod github;
pub mod ide;
pub mod instructions;
pub mod memory;
pub mod migrations;
pub mod onboarding;
pub mod permissions;
pub mod platform;
pub mod plugins;
pub mod priority;
pub mod remote;
pub mod services;
pub mod session_store;
pub mod skills;
pub mod state;
pub mod task_analyzer;
pub mod task_manager;
pub mod team;
pub mod telemetry;
pub mod tools;
pub mod tui;
pub mod voice;
pub mod weight_engine;

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Priority Agent starting...");

    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str());

    match mode {
        Some("--api") => {
            // HTTP API 模式
            #[cfg(feature = "experimental-api-server")]
            {
                let port = args
                    .iter()
                    .position(|a| a == "--port")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or(8787);
                info!("Starting API server on port {}...", port);
                if let Err(e) = api::start_server(port).await {
                    error!("API server failed: {}", e);
                    std::process::exit(1);
                }
            }
            #[cfg(not(feature = "experimental-api-server"))]
            {
                eprintln!("API server requires feature 'experimental-api-server'");
                eprintln!("Run: cargo run --features experimental-api-server -- --api");
                std::process::exit(1);
            }
        }
        Some("--cli") => {
            // 传统 CLI 模式
            #[cfg(feature = "legacy-cli")]
            {
                cli::run_cli().await;
            }
            #[cfg(not(feature = "legacy-cli"))]
            {
                eprintln!("CLI mode requires feature 'legacy-cli'");
                std::process::exit(1);
            }
        }
        _ => {
            // 默认: TUI 模式 (需要 bootstrap 初始化所有组件)
            info!("Starting TUI...");
            let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let (provider, model) = match bootstrap::init_provider() {
                Ok(p) => p,
                Err(e) => {
                    error!("Provider init failed: {}", e);
                    eprintln!("Failed to initialize LLM provider: {}", e);
                    eprintln!("Hint: set MOONSHOT_API_KEY or OPENAI_API_KEY environment variable.");
                    std::process::exit(1);
                }
            };
            let tool_registry = bootstrap::init_tool_registry(&working_dir);
            match bootstrap::init_components(provider, model, tool_registry, &working_dir).await {
                Ok(components) => {
                    if let Err(e) = tui::run_tui(
                        components.streaming_engine,
                        Some(components.lsp_manager),
                        Some(components.worktree_manager),
                    )
                    .await
                    {
                        error!("TUI failed: {}", e);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!("Bootstrap failed: {}", e);
                    eprintln!("Failed to initialize components: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    info!("Priority Agent exiting.");
}

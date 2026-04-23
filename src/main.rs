//! Priority Agent - 加权优先级桌面 Agent
//!
//! 解决 AI Agent 抓不住重点的问题，通过显式的权重系统让 AI 始终专注于最重要的事项。
//! 高密度思考 = 高密度 Q&A — Agent 应不断提问/解答来深化推理。

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
pub mod security;
pub mod services;
pub mod session_store;
pub mod skills;
pub mod state;
pub mod task_analyzer;
pub mod task_manager;
pub mod team;
pub mod telemetry;
pub mod changelog;
pub mod quality_gates;
pub mod slo;
pub mod version;
#[cfg(test)]
pub mod test_utils;
pub mod tools;
pub mod tui;
pub mod voice;

use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupMode {
    Help,
    Api,
    Cli,
    Tui,
}

fn is_cli_subcommand(arg: &str) -> bool {
    matches!(
        arg,
        "init"
            | "add"
            | "list"
            | "next"
            | "done"
            | "progress"
            | "analyze"
            | "snapshot"
            | "restore"
            | "interactive"
            | "i"
            | "chat"
            | "help"
    )
}

fn detect_startup_mode(args: &[String]) -> StartupMode {
    let argv0 = std::env::args().next().unwrap_or_default();
    let invoked_as_pa = argv0.ends_with("pa") || argv0.ends_with("pa.exe");

    let mode = args.get(1).map(|s| s.as_str());
    match mode {
        Some("--help") | Some("-h") | Some("help") => StartupMode::Help,
        Some("--api") => StartupMode::Api,
        Some("--cli") => StartupMode::Cli,
        Some("--tui") => StartupMode::Tui,
        Some(sub) if is_cli_subcommand(sub) => StartupMode::Cli,
        _ if invoked_as_pa => StartupMode::Cli,
        _ => StartupMode::Tui,
    }
}

fn print_help() {
    let argv0 = std::env::args().next().unwrap_or_else(|| "priority-agent".into());
    let is_pa = argv0.ends_with("pa") || argv0.ends_with("pa.exe");
    let bin = if is_pa { "pa" } else { "priority-agent" };

    println!("Priority Agent");
    println!();
    println!("Usage:");
    println!("  {bin} [--api [--port <PORT>]] [--cli] [--tui] [--help]");
    println!();
    println!("Modes:");
    println!("  --api    Start HTTP API server (feature: experimental-api-server)");
    println!("  --cli    Run chat CLI mode (feature: legacy-cli)");
    println!("  --tui    Start TUI mode");
    println!("  (none)   Default: TUI for 'priority-agent', CLI for 'pa'");
    println!();
    println!("Examples:");
    println!("  {bin}                  # Default mode");
    println!("  {bin} --api --port 8787 # HTTP API server");
    println!("  {bin} --tui            # Force TUI mode");
    println!("  priority-agent --cli   # Force chat CLI mode");
    println!("  priority-agent chat    # Enter chat CLI directly");
}

#[tokio::main]
async fn main() {
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    let startup_mode = detect_startup_mode(&args);

    // 初始化日志（交互模式默认降噪，仍可通过 RUST_LOG 覆盖）
    let default_level = match startup_mode {
        StartupMode::Api => "info",
        StartupMode::Help | StartupMode::Cli | StartupMode::Tui => "warn",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Priority Agent starting...");

    // 加载 .env 文件（如果存在）
    if let Err(e) = dotenvy::dotenv() {
        debug!(".env file not loaded: {}", e);
    }

    match startup_mode {
        StartupMode::Help => {
            print_help();
        }
        StartupMode::Api => {
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
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let (provider, model) = match bootstrap::init_provider() {
                    Ok(p) => p,
                    Err(e) => {
                        error!("Provider init failed: {}", e);
                        eprintln!("Failed to initialize LLM provider: {}", e);
                        eprintln!(
                            "Hint: set MOONSHOT_API_KEY or OPENAI_API_KEY environment variable."
                        );
                        std::process::exit(1);
                    }
                };
                let tool_registry = bootstrap::init_tool_registry(&working_dir);
                let mut lsp_manager = crate::engine::lsp::LspManager::new();
                lsp_manager.detect_servers(&working_dir);
                let lsp_manager = std::sync::Arc::new(lsp_manager);
                let worktree_manager =
                    std::sync::Arc::new(crate::engine::worktree::WorktreeManager::new().await);

                if let Err(e) = api::start_server(
                    provider,
                    model,
                    tool_registry,
                    port,
                    Some(lsp_manager),
                    Some(worktree_manager),
                )
                .await
                {
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
        StartupMode::Cli => {
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
        StartupMode::Tui => {
            // 默认: TUI 模式 (需要 bootstrap 初始化所有组件)
            // 检测是否在交互式终端中运行
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                eprintln!("Error: TUI mode requires an interactive terminal.");
                eprintln!("       Use --api to start the HTTP API server,");
                eprintln!("       or --cli for the legacy command-line interface.");
                eprintln!();
                print_help();
                std::process::exit(1);
            }
            info!("Starting TUI...");
            let working_dir =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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

#[cfg(test)]
mod tests {
    use super::{detect_startup_mode, StartupMode};

    #[test]
    fn test_detect_startup_mode_help_variants() {
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--help".into()]),
            StartupMode::Help
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "-h".into()]),
            StartupMode::Help
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "help".into()]),
            StartupMode::Help
        );
    }

    #[test]
    fn test_detect_startup_mode_api_cli_tui() {
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--api".into()]),
            StartupMode::Api
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--cli".into()]),
            StartupMode::Cli
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into()]),
            StartupMode::Tui
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--unknown".into()]),
            StartupMode::Tui
        );
    }

    #[test]
    fn test_detect_startup_mode_cli_subcommands() {
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "chat".into()]),
            StartupMode::Cli
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "init".into()]),
            StartupMode::Cli
        );
    }
}

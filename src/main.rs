//! Priority Agent - 加权优先级桌面 Agent
//!
//! 解决 AI Agent 抓不住重点的问题，通过显式的权重系统让 AI 始终专注于最重要的事项。
//! 高密度思考 = 高密度 Q&A — Agent 应不断提问/解答来深化推理。

// ─── Core Modules ────────────────────────────────────────────────────
pub mod agent;
#[cfg(feature = "experimental-task-analyzer")]
pub mod ai_analyzer;
#[cfg(feature = "experimental-api-server")]
pub mod api;
pub mod bootstrap;
pub mod bridge;
pub mod changelog;
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
#[cfg(feature = "experimental-platform")]
pub mod platform;
pub mod plugins;
#[cfg(feature = "experimental-priority")]
pub mod priority;
#[cfg(feature = "experimental-priority")]
pub mod quality_gates;
pub mod remote;
pub mod security;
pub mod services;
pub mod session_store;
pub mod shell;
pub mod skills;
pub mod slo;
pub mod state;
#[cfg(feature = "experimental-task-analyzer")]
pub mod task_analyzer;
pub mod task_manager;
pub mod team;
pub mod telemetry;
#[cfg(test)]
pub mod test_utils;
pub mod tools;
pub mod tui;
pub mod version;
#[cfg(feature = "voice")]
pub mod voice;

use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupMode {
    Help,
    Api,
    Cli,
    Tui,
    EvalRun,
    ProviderHealth,
}

fn detect_startup_mode(args: &[String]) -> StartupMode {
    let mode = args.get(1).map(|s| s.as_str());
    match mode {
        Some("--help") | Some("-h") | Some("help") => StartupMode::Help,
        Some("--api") => StartupMode::Api,
        Some("--cli") => StartupMode::Cli,
        Some("--tui") => StartupMode::Tui,
        Some("--eval-run") => StartupMode::EvalRun,
        Some("--provider-health") => StartupMode::ProviderHealth,
        _ => StartupMode::Cli,
    }
}

fn print_help() {
    let argv0 = std::env::args()
        .next()
        .unwrap_or_else(|| "priority-agent".into());
    let is_pa = argv0.ends_with("pa") || argv0.ends_with("pa.exe");
    let bin = if is_pa { "pa" } else { "priority-agent" };

    println!("Priority Agent");
    println!();
    println!("Usage:");
    println!("  {bin} [--api [--port <PORT>]] [--cli] [--tui] [--help]");
    println!();
    println!("Modes:");
    println!("  --api    Start HTTP API server (feature: experimental-api-server)");
    println!("  --cli    Start Priority Agent (default)");
    println!("  --tui    Start the full-screen terminal interface");
    println!("  --eval-run --prompt-file <PATH> [--output <PATH>] [--events <PATH>]");
    println!("           Run one non-interactive evaluation task");
    println!("  --provider-health [--output <PATH>] [--timeout <SECS>]");
    println!("           Probe provider chat, tool-call, and tool-result continuation");
    println!("  (none)   Default: start Priority Agent");
    println!();
    println!("Examples:");
    println!("  {bin}                  # Default mode");
    println!("  {bin} --api --port 8787 # HTTP API server");
    println!("  {bin} --cli            # Same as default");
    println!("  {bin} --tui            # Full-screen interface");
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|idx| args.get(idx + 1))
        .cloned()
}

fn write_eval_event(
    writer: &mut Option<std::io::BufWriter<std::fs::File>>,
    event: serde_json::Value,
) -> anyhow::Result<()> {
    use std::io::Write;

    if let Some(writer) = writer.as_mut() {
        serde_json::to_writer(&mut *writer, &event)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    Ok(())
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

async fn answer_pending_approval(
    engine: &std::sync::Arc<crate::engine::streaming::StreamingQueryEngine>,
    approved: bool,
) -> bool {
    let Some(channel) = engine.approval_channel() else {
        return false;
    };

    for _ in 0..20 {
        if let Some((_request, tx)) = channel.take_pending().await {
            let response = if approved {
                crate::engine::conversation_loop::ToolApprovalResponse::approved_once()
            } else {
                crate::engine::conversation_loop::ToolApprovalResponse::rejected_once()
            };
            let _ = tx.send(response);
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    false
}

async fn run_eval_task(args: &[String]) -> anyhow::Result<()> {
    use crate::engine::streaming::StreamEvent;
    use futures::StreamExt;
    use serde_json::json;

    let prompt_file = arg_value(args, "--prompt-file")
        .ok_or_else(|| anyhow::anyhow!("--prompt-file is required for --eval-run"))?;
    let output_file = arg_value(args, "--output");
    let events_file =
        arg_value(args, "--events").or_else(|| std::env::var("PRIORITY_AGENT_EVAL_EVENTS").ok());

    let prompt = std::fs::read_to_string(&prompt_file)
        .map_err(|e| anyhow::anyhow!("failed to read prompt file '{}': {}", prompt_file, e))?;

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let (provider, model) = bootstrap::init_provider()?;
    let tool_registry = bootstrap::init_tool_registry(&working_dir);
    let components =
        bootstrap::init_components(provider, model, tool_registry, &working_dir).await?;

    let mut event_writer = if let Some(path) = events_file.as_ref() {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        Some(std::io::BufWriter::new(std::fs::File::create(path)?))
    } else {
        None
    };

    write_eval_event(
        &mut event_writer,
        json!({
            "event": "eval_started",
            "prompt_file": prompt_file,
            "cwd": working_dir,
            "model": components.model.clone(),
        }),
    )?;

    let mut stream = components.streaming_engine.query_stream(prompt).await;
    let mut final_response = String::new();
    let mut error: Option<String> = None;

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::Start => write_eval_event(&mut event_writer, json!({"event": "start"}))?,
            StreamEvent::TextChunk(text) => {
                final_response.push_str(&text);
                write_eval_event(
                    &mut event_writer,
                    json!({"event": "text_chunk", "chars": text.chars().count()}),
                )?;
            }
            StreamEvent::ToolCallStart { id, name } => write_eval_event(
                &mut event_writer,
                json!({"event": "tool_call_start", "id": id, "name": name}),
            )?,
            StreamEvent::ToolCallArgs { id, args_delta } => write_eval_event(
                &mut event_writer,
                json!({"event": "tool_call_args", "id": id, "args_delta": args_delta}),
            )?,
            StreamEvent::ToolCallComplete { id } => write_eval_event(
                &mut event_writer,
                json!({"event": "tool_call_complete", "id": id}),
            )?,
            StreamEvent::ToolExecutionStart { id, name } => write_eval_event(
                &mut event_writer,
                json!({"event": "tool_execution_start", "id": id, "name": name}),
            )?,
            StreamEvent::ToolExecutionProgress { id, progress } => write_eval_event(
                &mut event_writer,
                json!({"event": "tool_execution_progress", "id": id, "progress": progress}),
            )?,
            StreamEvent::ToolExecutionComplete {
                id,
                result,
                metadata,
            } => write_eval_event(
                &mut event_writer,
                json!({
                    "event": "tool_execution_complete",
                    "id": id,
                    "result_chars": result.chars().count(),
                    "result_preview": truncate_chars(&result, 2000),
                    "metadata": metadata,
                }),
            )?,
            StreamEvent::ThinkingStart => {
                write_eval_event(&mut event_writer, json!({"event": "thinking_start"}))?
            }
            StreamEvent::ThinkingChunk(text) => write_eval_event(
                &mut event_writer,
                json!({"event": "thinking_chunk", "chars": text.chars().count()}),
            )?,
            StreamEvent::ThinkingComplete => {
                write_eval_event(&mut event_writer, json!({"event": "thinking_complete"}))?
            }
            StreamEvent::Usage {
                prompt_tokens,
                completion_tokens,
                reasoning_tokens,
                cached_tokens,
            } => write_eval_event(
                &mut event_writer,
                json!({
                    "event": "usage",
                    "prompt_tokens": prompt_tokens,
                    "completion_tokens": completion_tokens,
                    "reasoning_tokens": reasoning_tokens,
                    "cached_tokens": cached_tokens,
                }),
            )?,
            StreamEvent::Complete => {
                write_eval_event(&mut event_writer, json!({"event": "complete"}))?;
                break;
            }
            StreamEvent::OutputTruncated => {
                write_eval_event(&mut event_writer, json!({"event": "output_truncated"}))?
            }
            StreamEvent::Error(message) => {
                write_eval_event(
                    &mut event_writer,
                    json!({"event": "error", "message": message}),
                )?;
                error = Some(message);
                break;
            }
            StreamEvent::PermissionRequest {
                id,
                tool_name,
                arguments,
                prompt,
            } => {
                let answered = answer_pending_approval(&components.streaming_engine, false).await;
                write_eval_event(
                    &mut event_writer,
                    json!({
                        "event": "permission_request",
                        "id": id,
                        "tool_name": tool_name,
                        "arguments": arguments,
                        "prompt": prompt,
                        "auto_response": "deny",
                        "answered": answered,
                    }),
                )?;
            }
        }
    }

    let mut latest_trace = components.streaming_engine.trace_store().latest();
    for _ in 0..20 {
        if latest_trace.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        latest_trace = components.streaming_engine.trace_store().latest();
    }

    if let Some(trace) = latest_trace {
        let trace_id = trace.trace_id.clone();
        let status = format!("{:?}", trace.status);
        let turn_index = trace.turn_index;
        let duration_ms = trace.duration_ms();
        let event_types = trace
            .events
            .iter()
            .map(|event| event.label().to_string())
            .collect::<Vec<_>>();
        write_eval_event(
            &mut event_writer,
            json!({
                "event": "trace_summary",
                "trace_id": trace_id,
                "status": status,
                "turn_index": turn_index,
                "duration_ms": duration_ms,
                "event_count": trace.events.len(),
                "event_types": event_types,
                "trace": trace,
            }),
        )?;
    }

    if let Some(path) = output_file {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &final_response)?;
    } else {
        print!("{final_response}");
    }

    if let Some(message) = error {
        anyhow::bail!(message);
    }

    Ok(())
}

async fn run_provider_health_command(args: &[String]) -> anyhow::Result<()> {
    let timeout_secs = arg_value(args, "--timeout")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(45)
        .clamp(5, 300);
    let output_file = arg_value(args, "--output");
    let (provider, model) = bootstrap::init_provider()?;
    let report = diagnostics::provider_health::run_provider_health(
        provider,
        model,
        std::time::Duration::from_secs(timeout_secs),
    )
    .await;
    let json = serde_json::to_string_pretty(&report)?;

    if let Some(path) = output_file {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
    } else {
        println!("{json}");
    }

    if !report.is_ok() {
        anyhow::bail!("provider health failed: {}", report.failure_summary());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    let startup_mode = detect_startup_mode(&args);

    // 初始化日志（交互模式默认降噪，仍可通过 RUST_LOG 覆盖）
    let default_level = match startup_mode {
        StartupMode::Api => "info",
        StartupMode::Help
        | StartupMode::Cli
        | StartupMode::Tui
        | StartupMode::EvalRun
        | StartupMode::ProviderHealth => "warn",
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
            // Default: scrollback-first Priority Agent CLI.
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                eprintln!("Error: CLI mode requires an interactive terminal.");
                eprintln!("       Use --api to start the HTTP API server.");
                eprintln!();
                print_help();
                std::process::exit(1);
            }
            info!("Starting Priority Agent CLI...");
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
                    if let Err(e) = shell::run_shell(components.streaming_engine).await {
                        error!("Priority Agent CLI failed: {}", e);
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
        StartupMode::Tui => {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                eprintln!("Error: TUI mode requires an interactive terminal.");
                eprintln!("       Use --api to start the HTTP API server.");
                std::process::exit(1);
            }
            info!("Starting full-screen terminal interface...");
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
                        error!("Legacy TUI failed: {}", e);
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
        StartupMode::EvalRun => {
            if let Err(e) = run_eval_task(&args).await {
                error!("Evaluation run failed: {}", e);
                eprintln!("Evaluation run failed: {}", e);
                std::process::exit(1);
            }
        }
        StartupMode::ProviderHealth => {
            if let Err(e) = run_provider_health_command(&args).await {
                error!("Provider health failed: {}", e);
                eprintln!("Provider health failed: {}", e);
                std::process::exit(1);
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
            detect_startup_mode(&["priority-agent".into(), "--tui".into()]),
            StartupMode::Tui
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--eval-run".into()]),
            StartupMode::EvalRun
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--provider-health".into()]),
            StartupMode::ProviderHealth
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into()]),
            StartupMode::Cli
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--unknown".into()]),
            StartupMode::Cli
        );
    }

    #[test]
    fn test_legacy_cli_subcommands_fall_back_to_interactive_cli() {
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

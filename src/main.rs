//! Priority Agent - Rust programming-agent terminal CLI.
//!
//! The runtime keeps local tools, memory, validation, permissions, and closeout
//! evidence explicit while the model owns semantic engineering judgment.

use std::collections::HashSet;
use tracing::{debug, error, info};
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "experimental-api-server")]
use priority_agent::api;
use priority_agent::{bootstrap, diagnostics, entry, tui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupMode {
    Help,
    Api,
    Cli,
    LabCli,
    LabDaemon,
    Tui,
    EvalRun,
    ProviderHealth,
    ContextAttach,
}

fn detect_startup_mode(args: &[String]) -> StartupMode {
    let mode = args.get(1).map(|s| s.as_str());
    match mode {
        Some("--help") | Some("-h") | Some("help") => StartupMode::Help,
        Some("--api") => StartupMode::Api,
        Some("--cli") => StartupMode::Cli,
        Some("lab") | Some("--lab") => StartupMode::LabCli,
        Some("lab-daemon") | Some("--lab-daemon") => StartupMode::LabDaemon,
        Some("--tui") => StartupMode::Tui,
        Some("--eval-run") => StartupMode::EvalRun,
        Some("--provider-health") => StartupMode::ProviderHealth,
        Some("--context") => StartupMode::ContextAttach,
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
    println!(
        "  {bin} [lab|lab-daemon] [--api [--port <PORT>]] [--cli] [--tui] [--no-footer] [--help]"
    );
    println!();
    println!("Modes:");
    println!("  --api    Start HTTP API server (feature: experimental-api-server)");
    println!(
        "  --cli    Start the default terminal interface
  --no-footer  Disable the fixed bottom footer in CLI mode (use plain stdin/stdout)"
    );
    println!("  lab      Start Lab Mode professor intake and LabRun command surface");
    println!(
        "           Use: {bin} lab --command \"dashboard\" for one non-interactive Lab command"
    );
    println!(
        "           Add --with-provider to route provider-backed Lab commands through the active model"
    );
    println!("  lab-daemon  Run one non-interactive Lab daemon worker pass from persisted policy");
    println!("  --tui    Start the legacy full-screen terminal interface (alternative)");
    println!("  --eval-run --prompt-file <PATH> [--output <PATH>] [--events <PATH>] [--allowed-tools <CSV>]");
    println!("           Run one non-interactive evaluation task");
    println!("  --provider-health [--output <PATH>] [--timeout <SECS>]");
    println!("           Probe provider chat, tool-call, and tool-result continuation");
    println!("  --context attach --file <PATH> [--range <L1:L2>]");
    println!("           Write IDE context handoff file for desktop/tui pickup");
    println!("  (none)   Default: start the terminal interface");
    println!();
    println!("Examples:");
    println!("  {bin}                  # Default terminal interface");
    println!("  {bin} lab              # Lab Mode professor intake");
    println!("  {bin} --api --port 8787 # HTTP API server");
    println!("  {bin} --cli            # Same as default");
    println!("  {bin} --tui            # Legacy full-screen interface");
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|idx| args.get(idx + 1))
        .cloned()
}

fn comma_separated_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn run_context_attach(args: &[String]) -> Result<(), String> {
    let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
    if sub != "attach" {
        let bin = args.first().map(|s| s.as_str()).unwrap_or("priority-agent");
        eprintln!("Usage: {bin} --context attach --file <PATH> [--range <L1:L2>]");
        return Ok(());
    }
    let file_path =
        arg_value(args, "--file").ok_or_else(|| "--file <PATH> is required".to_string())?;
    let range = arg_value(args, "--range");
    let file = std::path::Path::new(&file_path);
    if !file.exists() {
        return Err(format!("file not found: {}", file_path));
    }
    let content =
        std::fs::read_to_string(file).map_err(|e| format!("cannot read {}: {}", file_path, e))?;
    let context = serde_json::json!({
        "source": "cli-attach",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "file": file.canonicalize().unwrap_or_else(|_| file.to_path_buf()).display().to_string(),
        "range": range,
        "content_preview": &content[..content.len().min(2000)],
    });
    let dir = std::path::PathBuf::from(".priority-agent");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create .priority-agent: {}", e))?;
    let path = dir.join("context.json");
    let json = serde_json::to_string_pretty(&context).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write context: {}", e))?;
    println!("Context written to {}", path.display());
    Ok(())
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

fn env_flag(name: &str) -> Option<bool> {
    std::env::var(name).ok().map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn exit_success_after_one_shot_command() -> ! {
    use std::io::Write;

    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::process::exit(0);
}

fn configure_eval_memory_isolation(output_file: Option<&str>, events_file: Option<&str>) {
    let base_dir = output_file
        .or(events_file)
        .and_then(|path| {
            std::path::Path::new(path)
                .parent()
                .map(std::path::Path::to_path_buf)
        })
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("target")
                .join("eval-reports")
        });
    let state_dir = base_dir.join("eval-state");
    if std::env::var("PRIORITY_AGENT_MEMORY_PROPOSALS_PATH").is_err() {
        std::env::set_var(
            "PRIORITY_AGENT_MEMORY_PROPOSALS_PATH",
            state_dir.join("memory_proposals.jsonl"),
        );
    }
    if std::env::var("PRIORITY_AGENT_PROJECT_PROGRESS_PATH").is_err() {
        std::env::set_var(
            "PRIORITY_AGENT_PROJECT_PROGRESS_PATH",
            state_dir.join("project_progress.jsonl"),
        );
    }
}

fn default_log_level(startup_mode: StartupMode) -> &'static str {
    match startup_mode {
        StartupMode::Api => "info",
        StartupMode::Tui => "off",
        StartupMode::Help
        | StartupMode::Cli
        | StartupMode::LabCli
        | StartupMode::LabDaemon
        | StartupMode::EvalRun
        | StartupMode::ProviderHealth
        | StartupMode::ContextAttach => "warn",
    }
}

fn suppress_terminal_logs(startup_mode: StartupMode) -> bool {
    matches!(
        startup_mode,
        StartupMode::Tui | StartupMode::Cli | StartupMode::LabCli
    )
}

async fn answer_pending_approval(
    engine: &std::sync::Arc<priority_agent::engine::streaming::StreamingQueryEngine>,
    approved: bool,
    session_scoped: bool,
    working_dir: &std::path::Path,
) -> Option<&'static str> {
    use priority_agent::engine::conversation_loop::ToolApprovalResponse;

    let channel = engine.approval_channel()?;

    for _ in 0..20 {
        if let Some((request, tx)) = channel.take_pending().await {
            let (response, label) = if approved {
                if session_scoped {
                    (ToolApprovalResponse::approved_session(), "approve_session")
                } else {
                    (ToolApprovalResponse::approved_once(), "approve_once")
                }
            } else if eval_safe_validation_tool_call(&request.tool_call, working_dir) {
                (
                    ToolApprovalResponse::approved_once(),
                    "approve_safe_validation",
                )
            } else {
                (ToolApprovalResponse::rejected_once(), "deny")
            };
            let _ = tx.send(response);
            return Some(label);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    None
}

fn eval_safe_validation_tool_call(
    tool_call: &priority_agent::services::api::ToolCall,
    working_dir: &std::path::Path,
) -> bool {
    if tool_call.name != "bash" {
        return false;
    }
    let Some(command) = tool_call
        .arguments
        .get("command")
        .and_then(|value| value.as_str())
    else {
        return false;
    };
    let classification =
        priority_agent::tools::bash_tool::command_classifier::classify_command_with_working_dir(
            command,
            working_dir,
        );
    let desktop_validation_like = eval_allowed_desktop_validation_command(command)
        || classification
            .subcommands
            .iter()
            .any(|subcommand| eval_allowed_desktop_validation_command(&subcommand.command));
    let search_assertion_like = eval_allowed_search_assertion_command(command)
        || classification
            .subcommands
            .iter()
            .any(|subcommand| eval_allowed_search_assertion_command(&subcommand.command));
    let validation_like =
        classification.is_safe_validation() || desktop_validation_like || search_assertion_like;
    if !validation_like || (classification.network_access && !desktop_validation_like) {
        return false;
    }
    if classification.command_plan.has_command_substitution
        || classification.command_plan.has_process_substitution
        || classification.command_plan.has_heredoc
    {
        return false;
    }
    if !eval_safe_validation_shell_controls(&classification) {
        return false;
    }
    if !classification
        .redirections
        .iter()
        .all(|redir| !redir.writes || (redir.operator == "2>" && redir.target.is_none()))
    {
        return false;
    }
    if !classification
        .absolute_path_patterns
        .iter()
        .all(|path| path_is_within(path, working_dir))
    {
        return false;
    }
    classification
        .command_plan
        .cd_targets
        .iter()
        .all(|target| path_is_within(target, working_dir))
}

fn eval_safe_validation_shell_controls(
    classification: &priority_agent::tools::bash_tool::command_classifier::CommandClassification,
) -> bool {
    if !classification
        .shell_control_operators
        .iter()
        .all(|operator| matches!(operator.as_str(), "and" | "redirect" | "pipe"))
    {
        return false;
    }

    if !classification
        .shell_control_operators
        .iter()
        .any(|operator| operator == "pipe")
    {
        return true;
    }

    classification
        .subcommands
        .iter()
        .filter(|subcommand| subcommand.index > 0)
        .all(|subcommand| eval_safe_validation_subcommand_or_pipe_filter(&subcommand.command))
}

fn eval_safe_validation_subcommand_or_pipe_filter(command: &str) -> bool {
    eval_safe_validation_pipe_filter(command) || eval_allowed_validation_subcommand(command)
}

fn eval_safe_validation_pipe_filter(command: &str) -> bool {
    let normalized =
        priority_agent::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    matches!(normalized.as_str(), "head" | "tail" | "rg" | "grep")
        || normalized.starts_with("head ")
        || normalized.starts_with("tail ")
        || normalized.starts_with("rg ")
        || normalized.starts_with("grep ")
}

fn eval_allowed_validation_subcommand(command: &str) -> bool {
    let normalized =
        priority_agent::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    normalized.contains("cargo test")
        || normalized.contains("cargo check")
        || normalized.contains("cargo fmt")
        || normalized.contains("cargo clippy")
        || normalized.contains("npm test")
        || normalized.contains("npm run test")
        || normalized.contains("pnpm test")
        || normalized.contains("pytest")
        || eval_allowed_search_assertion_command(command)
        || eval_allowed_desktop_validation_command(command)
}

fn eval_allowed_search_assertion_command(command: &str) -> bool {
    let normalized =
        priority_agent::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    (normalized.starts_with("! rg ") || normalized.starts_with("! grep "))
        && !normalized.contains('|')
        && !normalized.contains("&&")
        && !normalized.contains(';')
        && !normalized.contains('>')
        && !normalized.contains('<')
}

fn eval_allowed_desktop_validation_command(command: &str) -> bool {
    let normalized =
        priority_agent::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    normalized == "pnpm --dir apps/desktop build"
        || normalized.starts_with("pnpm --dir apps/desktop build ")
        || normalized == "pnpm --dir apps/desktop test:ui-smoke"
        || normalized.starts_with("pnpm --dir apps/desktop test:ui-smoke ")
        || normalized.starts_with("pnpm --dir apps/desktop exec playwright test")
}

fn path_is_within(path: &str, root: &std::path::Path) -> bool {
    let candidate = std::path::Path::new(path);
    let path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    lexical_normalize(&path).starts_with(lexical_normalize(root))
}

fn lexical_normalize(path: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

async fn run_eval_task(
    args: &[String],
    components: &bootstrap::AppComponents,
) -> anyhow::Result<()> {
    use futures::StreamExt;
    use priority_agent::engine::streaming::StreamEvent;
    use serde_json::json;

    let prompt_file = arg_value(args, "--prompt-file")
        .ok_or_else(|| anyhow::anyhow!("--prompt-file is required for --eval-run"))?;
    let output_file = arg_value(args, "--output");
    let events_file =
        arg_value(args, "--events").or_else(|| std::env::var("PRIORITY_AGENT_EVAL_EVENTS").ok());
    let allowed_tools = arg_value(args, "--allowed-tools")
        .or_else(|| std::env::var("PRIORITY_AGENT_EVAL_ALLOWED_TOOLS").ok())
        .map(|raw| {
            comma_separated_values(&raw)
                .into_iter()
                .collect::<HashSet<_>>()
        })
        .filter(|tools| !tools.is_empty());

    let mut prompt = std::fs::read_to_string(&prompt_file)
        .map_err(|e| anyhow::anyhow!("failed to read prompt file '{}': {}", prompt_file, e))?;

    // When eval mutations are enabled, insert a code_change signal prefix
    // so the intent router exposes file_edit/file_write/bash and the
    // permission handler auto-approves mutating tools.
    let allow_mutations = std::env::var("PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS")
        .unwrap_or_else(|_| "0".to_string())
        .trim()
        == "1";
    if allow_mutations && !prompt.trim().is_empty() {
        // Force code_change route: prepend the exact pattern that
        // is_live_coding_code_change_request detects.
        prompt = format!("eval intent: seeded_code_change\n\n{prompt}");
    }

    configure_eval_memory_isolation(output_file.as_deref(), events_file.as_deref());
    let eval_memory_generate = env_flag("PRIORITY_AGENT_EVAL_MEMORY_GENERATE").unwrap_or(false);
    components
        .streaming_engine
        .set_memory_generate(eval_memory_generate);
    components.streaming_engine.set_allowed_tools(allowed_tools);

    // Eval-run optimizations: auto-approve ask_user, disable file cache
    // short-circuit so the model always sees full file content, and skip
    // storm breaker for read-only tools by default.
    //
    // NOTE: PRIORITY_AGENT_AUTO_APPROVE only affects the `ask_user` tool.
    // For tool permission auto-approval (bash, file_write, file_edit, etc.),
    // use PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS (see permission handler below).
    if std::env::var("PRIORITY_AGENT_AUTO_APPROVE").is_err() {
        std::env::set_var("PRIORITY_AGENT_AUTO_APPROVE", "1");
    }
    if std::env::var("PRIORITY_AGENT_EVAL_NO_FILE_CACHE").is_err() {
        std::env::set_var("PRIORITY_AGENT_EVAL_NO_FILE_CACHE", "1");
    }

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

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
            StreamEvent::ToolExecutionStart { id, name, metadata } => {
                let mut event = json!({"event": "tool_execution_start", "id": id, "name": name});
                if let (Some(object), Some(metadata)) = (event.as_object_mut(), metadata) {
                    object.insert("metadata".to_string(), metadata);
                }
                write_eval_event(&mut event_writer, event)?
            }
            StreamEvent::ToolExecutionProgress { id, progress } => write_eval_event(
                &mut event_writer,
                json!({"event": "tool_execution_progress", "id": id, "progress": progress}),
            )?,
            StreamEvent::ToolExecutionComplete {
                id,
                result,
                metadata,
                ..
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
            StreamEvent::ToolResultsReadyForModel { ids } => write_eval_event(
                &mut event_writer,
                json!({"event": "tool_results_ready_for_model", "tool_call_ids": ids}),
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
                cache_write_tokens,
            } => write_eval_event(
                &mut event_writer,
                json!({
                    "event": "usage",
                    "prompt_tokens": prompt_tokens,
                    "completion_tokens": completion_tokens,
                    "reasoning_tokens": reasoning_tokens,
                    "cached_tokens": cached_tokens,
                    "cache_write_tokens": cache_write_tokens,
                }),
            )?,
            StreamEvent::RuntimeDiagnostic { diagnostic } => write_eval_event(
                &mut event_writer,
                json!({
                    "event": "runtime_diagnostic",
                    "diagnostic": diagnostic,
                }),
            )?,
            StreamEvent::Closeout {
                status,
                evidence_summary,
            } => write_eval_event(
                &mut event_writer,
                json!({
                    "event": "closeout",
                    "status": status,
                    "evidence_summary": evidence_summary,
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
                ..
            } => {
                // Eval-run is read-only by default; set PRIORITY_AGENT_EVAL_ALLOW_MUTATIONS=1
                // to auto-approve tool permissions for seeded code-change tasks.
                // Session-scoped approval means bash/file_edit/file_write stay
                // approved across multiple iterations during the eval.
                let answered = answer_pending_approval(
                    &components.streaming_engine,
                    allow_mutations,
                    allow_mutations,
                    &working_dir,
                )
                .await;
                write_eval_event(
                    &mut event_writer,
                    json!({
                        "event": "permission_request",
                        "id": id,
                        "tool_name": tool_name,
                        "arguments": arguments,
                        "prompt": prompt,
                        "auto_response": answered.unwrap_or(if allow_mutations { "approve" } else { "deny" }),
                        "answered": answered.is_some(),
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
    if let Err(err) = diagnostics::provider_health::append_provider_health_ledger(&report) {
        tracing::warn!("failed to append provider health ledger: {}", err);
    }
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

/// 统一初始化应用组件，失败时打印错误并退出进程
async fn init_app_or_exit(
    working_dir: &std::path::Path,
    mode: StartupMode,
) -> Option<bootstrap::AppComponents> {
    let init_result = if matches!(mode, StartupMode::Tui) {
        bootstrap::init_tui_app(working_dir).await
    } else {
        bootstrap::init_app(working_dir).await
    };

    match init_result {
        Ok(components) => Some(components),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("No LLM provider configured") {
                // Only CLI and TUI should reach onboarding; API/eval still exit.
                if matches!(
                    mode,
                    StartupMode::Cli | StartupMode::LabCli | StartupMode::Tui
                ) {
                    eprintln!("No provider configured — starting onboarding.");
                    return None;
                }
                error!("Provider init failed: {}", e);
                eprintln!("Failed to initialize LLM provider: {}", e);
                eprintln!(
                    "Hint: set one provider key: {}.",
                    priority_agent::services::api::provider::provider_key_env_hint()
                );
            } else {
                error!("Bootstrap failed: {}", e);
                eprintln!("Failed to initialize components: {}", e);
            }
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "experimental-api-server")]
async fn init_api_or_exit(working_dir: &std::path::Path) -> bootstrap::ApiComponents {
    match bootstrap::init_api_components(working_dir).await {
        Ok(components) => components,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("No LLM provider configured") {
                error!("Provider init failed: {}", e);
                eprintln!("Failed to initialize LLM provider: {}", e);
                eprintln!(
                    "Hint: set one provider key: {}.",
                    priority_agent::services::api::provider::provider_key_env_hint()
                );
            } else {
                error!("API bootstrap failed: {}", e);
                eprintln!("Failed to initialize API components: {}", e);
            }
            std::process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() {
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    let startup_mode = detect_startup_mode(&args);

    // 初始化日志（交互模式默认降噪，仍可通过 RUST_LOG 覆盖）
    let default_level = default_log_level(startup_mode);
    let log_writer = if suppress_terminal_logs(startup_mode) {
        // CLI/TUI: redirect logs to a file so they never garble the terminal
        // but are still available for debugging.
        let log_dir = dirs::data_local_dir()
            .map(|d| d.join("priority-agent"))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let _ = std::fs::create_dir_all(&log_dir);
        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_dir.join("cli.log"))
        {
            Ok(log_file) => BoxMakeWriter::new(std::sync::Mutex::new(log_file)),
            Err(_) => BoxMakeWriter::new(std::io::sink),
        }
    } else {
        BoxMakeWriter::new(std::io::stderr)
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_writer(log_writer)
        .init();

    info!("Priority Agent starting...");

    // Load product credential env (provider keys saved via /connect).
    if let Err(e) = priority_agent::services::api::credentials::load_product_credential_env() {
        debug!("product credential env not loaded: {}", e);
    }

    // 加载 .env 文件（如果存在）
    if let Err(e) = dotenvy::dotenv() {
        debug!(".env file not loaded: {}", e);
    }

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

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
                let components = init_api_or_exit(&working_dir).await;
                let runtime_controller =
                    priority_agent::engine::runtime_controller::RuntimeController::new(
                        components.streaming_engine,
                    );
                if let Err(e) = api::start_server(
                    components.provider,
                    components.model,
                    components.tool_registry,
                    port,
                    Some(components.lsp_manager),
                    Some(components.worktree_manager),
                    Some(runtime_controller),
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
            let no_footer = has_flag(&args, "--no-footer");
            info!("Starting Priority Agent CLI...");
            let components = init_app_or_exit(&working_dir, startup_mode).await;
            let Some(components) = components else {
                eprintln!("No provider configured. Run /connect <provider> <key>");
                std::process::exit(1);
            };
            if let Err(e) = entry::direct::run_cli(components.streaming_engine, no_footer).await {
                error!("Priority Agent CLI failed: {}", e);
                std::process::exit(1);
            }
        }
        StartupMode::LabCli => {
            if let Some(command) = arg_value(&args, "--command") {
                if has_flag(&args, "--with-provider") {
                    info!("Running one provider-backed non-interactive Lab command...");
                    let components = init_app_or_exit(&working_dir, startup_mode).await;
                    let Some(components) = components else {
                        eprintln!("No provider configured. Run /connect <provider> <key>");
                        std::process::exit(1);
                    };
                    if let Err(e) =
                        entry::lab::run_command_with_components(&command, components).await
                    {
                        error!("Priority Agent provider-backed Lab command failed: {}", e);
                        std::process::exit(1);
                    }
                } else {
                    info!("Running one non-interactive Lab command...");
                    if let Err(e) = entry::lab::run_command(&command).await {
                        error!("Priority Agent Lab command failed: {}", e);
                        std::process::exit(1);
                    }
                }
                exit_success_after_one_shot_command();
            }
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                eprintln!("Error: Lab mode requires an interactive terminal.");
                eprintln!("       Use --api to start the HTTP API server.");
                eprintln!();
                print_help();
                std::process::exit(1);
            }
            let no_footer = has_flag(&args, "--no-footer");
            info!("Starting Priority Agent Lab Mode...");
            let components = init_app_or_exit(&working_dir, startup_mode).await;
            let Some(components) = components else {
                eprintln!("No provider configured. Run /connect <provider> <key>");
                std::process::exit(1);
            };
            if let Err(e) = entry::lab::run_cli(components.streaming_engine, no_footer).await {
                error!("Priority Agent Lab Mode failed: {}", e);
                std::process::exit(1);
            }
        }
        StartupMode::LabDaemon => {
            info!("Starting Priority Agent Lab daemon worker...");
            let components = init_app_or_exit(&working_dir, startup_mode).await;
            let Some(components) = components else {
                eprintln!("No provider configured. Run /connect <provider> <key>");
                std::process::exit(1);
            };
            if let Err(e) = entry::lab::run_daemon_worker(components).await {
                error!("Priority Agent Lab daemon worker failed: {}", e);
                eprintln!("Lab daemon worker failed: {e}");
                std::process::exit(1);
            }
        }
        StartupMode::Tui => {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                eprintln!("Error: TUI mode requires an interactive terminal.");
                eprintln!("       Use --api to start the HTTP API server.");
                std::process::exit(1);
            }
            info!("Starting full-screen terminal interface...");
            let components = init_app_or_exit(&working_dir, startup_mode).await;
            let (engine, lsp, worktree) = match components {
                Some(c) => (
                    Some(c.streaming_engine.clone()),
                    Some(c.lsp_manager.clone()),
                    Some(c.worktree_manager.clone()),
                ),
                None => (None, None, None),
            };
            if let Err(e) = tui::run_tui(engine, lsp, worktree).await {
                error!("Legacy TUI failed: {}", e);
                std::process::exit(1);
            }
        }
        StartupMode::EvalRun => {
            let components = init_app_or_exit(&working_dir, startup_mode).await;
            let Some(components) = components else {
                eprintln!("Evaluation requires a configured provider.");
                std::process::exit(1);
            };
            if let Err(e) = run_eval_task(&args, &components).await {
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
        StartupMode::ContextAttach => {
            if let Err(e) = run_context_attach(&args) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    info!("Priority Agent exiting.");
}

#[cfg(test)]
mod tests {
    use super::{
        configure_eval_memory_isolation, default_log_level, detect_startup_mode, env_flag,
        eval_safe_validation_tool_call, suppress_terminal_logs, StartupMode,
    };
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct TestEnvGuard {
        _lock: MutexGuard<'static, ()>,
        saved: HashMap<String, Option<String>>,
    }

    impl TestEnvGuard {
        fn acquire() -> Self {
            Self {
                _lock: ENV_LOCK.lock().unwrap(),
                saved: HashMap::new(),
            }
        }

        fn set(&mut self, key: &str, value: &str) {
            self.capture_if_needed(key);
            // SAFETY: guarded by process-wide ENV_LOCK in this test module.
            unsafe { std::env::set_var(key, value) };
        }

        fn remove(&mut self, key: &str) {
            self.capture_if_needed(key);
            // SAFETY: guarded by process-wide ENV_LOCK in this test module.
            unsafe { std::env::remove_var(key) };
        }

        fn capture_if_needed(&mut self, key: &str) {
            if self.saved.contains_key(key) {
                return;
            }
            self.saved.insert(key.to_string(), std::env::var(key).ok());
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, old_value) in self.saved.drain() {
                match old_value {
                    Some(value) => {
                        // SAFETY: guarded by process-wide ENV_LOCK in this test module.
                        unsafe { std::env::set_var(key, value) };
                    }
                    None => {
                        // SAFETY: guarded by process-wide ENV_LOCK in this test module.
                        unsafe { std::env::remove_var(key) };
                    }
                }
            }
        }
    }

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
            detect_startup_mode(&["priority-agent".into(), "lab".into()]),
            StartupMode::LabCli
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--lab".into()]),
            StartupMode::LabCli
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "lab-daemon".into()]),
            StartupMode::LabDaemon
        );
        assert_eq!(
            detect_startup_mode(&["priority-agent".into(), "--lab-daemon".into()]),
            StartupMode::LabDaemon
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
    fn test_tui_startup_suppresses_terminal_logs_by_default() {
        assert_eq!(default_log_level(StartupMode::Tui), "off");
        assert!(suppress_terminal_logs(StartupMode::Tui));

        assert_eq!(default_log_level(StartupMode::Cli), "warn");
        assert!(suppress_terminal_logs(StartupMode::Cli));
        assert_eq!(default_log_level(StartupMode::LabCli), "warn");
        assert!(suppress_terminal_logs(StartupMode::LabCli));
        assert_eq!(default_log_level(StartupMode::LabDaemon), "warn");
        assert!(!suppress_terminal_logs(StartupMode::LabDaemon));
        assert_eq!(default_log_level(StartupMode::Api), "info");
        assert!(!suppress_terminal_logs(StartupMode::Api));
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

    #[test]
    fn eval_memory_isolation_sets_report_local_paths() {
        let mut env = TestEnvGuard::acquire();
        env.remove("PRIORITY_AGENT_MEMORY_PROPOSALS_PATH");
        env.remove("PRIORITY_AGENT_PROJECT_PROGRESS_PATH");
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("task.md");

        configure_eval_memory_isolation(output.to_str(), None);

        assert_eq!(
            std::env::var("PRIORITY_AGENT_MEMORY_PROPOSALS_PATH").unwrap(),
            dir.path()
                .join("eval-state")
                .join("memory_proposals.jsonl")
                .to_string_lossy()
        );
        assert_eq!(
            std::env::var("PRIORITY_AGENT_PROJECT_PROGRESS_PATH").unwrap(),
            dir.path()
                .join("eval-state")
                .join("project_progress.jsonl")
                .to_string_lossy()
        );
    }

    #[test]
    fn eval_memory_generate_flag_defaults_off() {
        let mut env = TestEnvGuard::acquire();
        env.remove("PRIORITY_AGENT_EVAL_MEMORY_GENERATE");
        assert_eq!(env_flag("PRIORITY_AGENT_EVAL_MEMORY_GENERATE"), None);
        env.set("PRIORITY_AGENT_EVAL_MEMORY_GENERATE", "1");
        assert_eq!(env_flag("PRIORITY_AGENT_EVAL_MEMORY_GENERATE"), Some(true));
    }

    #[test]
    fn eval_safe_validation_allows_current_worktree_cargo_test() {
        let dir = tempfile::tempdir().unwrap();
        let command = format!(
            "cd {} && cargo test -q --manifest-path fixtures/core_quality/rust_refactor/Cargo.toml 2>&1",
            dir.path().display()
        );
        let call = priority_agent::services::api::ToolCall {
            id: "call_validation".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": command }),
        };

        assert!(eval_safe_validation_tool_call(&call, dir.path()));
    }

    #[test]
    fn eval_safe_validation_allows_bounded_validation_pipe() {
        let dir = tempfile::tempdir().unwrap();
        let command = format!(
            "cd {} && cargo check -q 2>&1 | head -60",
            dir.path().display()
        );
        let call = priority_agent::services::api::ToolCall {
            id: "call_validation_pipe".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": command }),
        };

        assert!(eval_safe_validation_tool_call(&call, dir.path()));
    }

    #[test]
    fn eval_safe_validation_allows_desktop_validation_scripts() {
        let dir = tempfile::tempdir().unwrap();
        for command in [
            "pnpm --dir apps/desktop build",
            "pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts",
            "pnpm --dir apps/desktop test:ui-smoke",
        ] {
            let call = priority_agent::services::api::ToolCall {
                id: format!("call_desktop_{}", command.replace(' ', "_")),
                name: "bash".to_string(),
                arguments: serde_json::json!({ "command": command }),
            };

            assert!(
                eval_safe_validation_tool_call(&call, dir.path()),
                "desktop validation command should be auto-approved: {command}"
            );
        }
    }

    #[test]
    fn eval_safe_validation_allows_desktop_validation_after_cd() {
        let dir = tempfile::tempdir().unwrap();
        let command = format!(
            "cd {} && pnpm --dir apps/desktop build 2>&1",
            dir.path().display()
        );
        let call = priority_agent::services::api::ToolCall {
            id: "call_desktop_cd".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({ "command": command }),
        };
        let classification =
            priority_agent::tools::bash_tool::command_classifier::classify_command_with_working_dir(
                call.arguments["command"].as_str().unwrap(),
                dir.path(),
            );

        assert!(
            eval_safe_validation_tool_call(&call, dir.path()),
            "{classification:#?}"
        );
    }

    #[test]
    fn eval_safe_validation_allows_negative_search_assertion() {
        let dir = tempfile::tempdir().unwrap();
        let call = priority_agent::services::api::ToolCall {
            id: "call_negative_search".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "! rg '&format!' src/engine/conversation_loop/repair_controller.rs"
            }),
        };

        assert!(eval_safe_validation_tool_call(&call, dir.path()));
    }

    #[test]
    fn eval_safe_validation_rejects_external_cd_or_file_redirection() {
        let dir = tempfile::tempdir().unwrap();
        let external = priority_agent::services::api::ToolCall {
            id: "call_external".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cd /tmp && cargo test -q --manifest-path fixtures/core_quality/rust_refactor/Cargo.toml"
            }),
        };
        let file_redirect = priority_agent::services::api::ToolCall {
            id: "call_redirect".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cargo test -q --manifest-path fixtures/core_quality/rust_refactor/Cargo.toml > /tmp/out"
            }),
        };
        let unsafe_pipe = priority_agent::services::api::ToolCall {
            id: "call_unsafe_pipe".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "cargo check -q 2>&1 | sh"
            }),
        };
        let unsafe_negative_search = priority_agent::services::api::ToolCall {
            id: "call_unsafe_negative_search".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "! rg '&format!' src/engine/conversation_loop/repair_controller.rs > /tmp/out"
            }),
        };

        assert!(!eval_safe_validation_tool_call(&external, dir.path()));
        assert!(!eval_safe_validation_tool_call(&file_redirect, dir.path()));
        assert!(!eval_safe_validation_tool_call(&unsafe_pipe, dir.path()));
        assert!(!eval_safe_validation_tool_call(
            &unsafe_negative_search,
            dir.path()
        ));
    }
}

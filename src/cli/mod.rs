//! CLI 模块 - 命令行界面
//!
//! 提供命令解析、交互式提示和输出格式化

pub mod commands;
pub mod display;
pub mod interactive;

pub use commands::{Cli, Commands};
pub use display::{format_progress, format_task_tree, print_banner};
pub use interactive::{prompt_project, prompt_task, select_task};

/// 运行 CLI 模式（legacy-cli 模式入口）
#[cfg(feature = "legacy-cli")]
pub async fn run_cli() {
    use crate::engine::streaming::StreamEvent;
    use crate::cli::commands::Commands;
    use futures::StreamExt;
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;
    use std::collections::BTreeMap;
    use std::io::{self, Write};
    use std::path::PathBuf;

    let cli = Cli::parse();
    match cli.command {
        Commands::Help => crate::cli::commands::print_help(),
        Commands::Init => println!("Initializing project..."),
        Commands::AddTask { name } => println!("Adding task: {}", name),
        Commands::List => println!("Listing tasks..."),
        Commands::Next => println!("Next recommended task..."),
        Commands::CompleteTask { id } => println!("Completing task: {}", id),
        Commands::Progress => println!("Showing progress..."),
        Commands::Analyze => println!("Analyzing project..."),
        Commands::Snapshot { name } => println!("Creating snapshot: {:?}", name),
        Commands::Restore { id } => println!("Restoring: {}", id),
        Commands::Interactive => {
            if let Err(e) = run_interactive().await {
                eprintln!("Interactive error: {}", e);
            }
        }
    }

    async fn run_interactive() -> anyhow::Result<()> {
        let working_dir =
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let (provider, model) = crate::bootstrap::init_provider()?;
        let tool_registry = crate::bootstrap::init_tool_registry(&working_dir);
        let components =
            crate::bootstrap::init_components(provider, model, tool_registry, &working_dir).await?;
        let engine = components.streaming_engine;
        let lsp_manager = components.lsp_manager;
        let mut show_usage = false;
        let mut verbose_tool_logs = false;
        let show_tool_done = std::env::var("PRIORITY_AGENT_CLI_SHOW_TOOL_DONE")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        let mut line_editor = create_line_editor()?;

        println!("Priority Agent Chat");
        println!("输入消息开始对话。/help 查看命令，/exit 退出。");
        println!("提示：行尾使用 `\\` 可继续输入多行。");

        loop {
            let Some(input) = read_user_input(&mut line_editor)? else {
                println!();
                break;
            };
            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            if input.starts_with('/') {
                if handle_slash_command(
                    input,
                    &engine,
                    &mut show_usage,
                    &mut verbose_tool_logs,
                )
                .await?
                {
                    persist_line_editor_history(&mut line_editor)?;
                    break;
                }
                continue;
            }

            print!("assistant: ");
            io::stdout().flush()?;

            let mut stream = engine.query_stream(input.to_string()).await;
            let mut saw_text = false;
            let mut saw_error = false;

            while let Some(event) = stream.next().await {
                match event {
                    StreamEvent::Start => {}
                    StreamEvent::TextChunk(chunk) => {
                        print!("{chunk}");
                        io::stdout().flush()?;
                        saw_text = true;
                    }
                    StreamEvent::ToolExecutionStart { name, .. } => {
                        if saw_text {
                            println!();
                        }
                        println!("\n[tool:start] {name}");
                    }
                    StreamEvent::ToolExecutionProgress { progress, .. } => {
                        if verbose_tool_logs && !progress.trim().is_empty() {
                            println!("[tool] {progress}");
                        }
                    }
                    StreamEvent::ToolExecutionComplete { id, result } => {
                        let is_error = result.contains("Result: ERROR");
                        if is_error {
                            let preview = result.replace('\n', " ");
                            let preview = preview.chars().take(120).collect::<String>();
                            println!("[tool:error] {id}: {preview}");
                            saw_error = true;
                        } else if verbose_tool_logs || show_tool_done {
                            if !result.trim().is_empty() {
                                let preview = result.replace('\n', " ");
                                let preview = preview.chars().take(120).collect::<String>();
                                println!("[tool:done] {id}: {preview}");
                            } else {
                                println!("[tool:done] {id}");
                            }
                        }
                    }
                    StreamEvent::PermissionRequest {
                        tool_name, prompt, ..
                    } => {
                        println!();
                        println!("[permission] `{tool_name}`: {prompt}");
                        let approved = prompt_yes_no("Approve this tool call? [y/N]: ")?;
                        if let Some(channel) = engine.approval_channel() {
                            if let Some((_, tx)) = channel.take_pending().await {
                                let _ = tx.send(approved);
                            }
                        }
                    }
                    StreamEvent::Usage {
                        prompt_tokens,
                        completion_tokens,
                        reasoning_tokens,
                        cached_tokens,
                    } => {
                        if show_usage {
                            println!(
                                "\n[usage] prompt={prompt_tokens}, completion={completion_tokens}, reasoning={}, cached={}",
                                reasoning_tokens
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                                cached_tokens
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "-".to_string())
                            );
                        }
                    }
                    StreamEvent::Error(err) => {
                        println!("\n[error] {err}");
                        saw_error = true;
                    }
                    StreamEvent::Complete => {}
                    StreamEvent::OutputTruncated => {
                        println!("\n[warn] output truncated by max_tokens");
                    }
                    StreamEvent::ToolCallStart { .. }
                    | StreamEvent::ToolCallArgs { .. }
                    | StreamEvent::ToolCallComplete { .. }
                    | StreamEvent::ThinkingStart
                    | StreamEvent::ThinkingChunk(_)
                    | StreamEvent::ThinkingComplete => {}
                }
            }

            if saw_text || saw_error {
                println!();
            } else {
                println!("(no text output)");
            }
        }

        persist_line_editor_history(&mut line_editor)?;

        // Align CLI cleanup behavior with TUI: ensure background managers stop.
        if let Some(mcp_manager) = engine.mcp_manager() {
            mcp_manager.shutdown().await;
        }
        lsp_manager.shutdown().await;

        Ok(())
    }

    fn prompt_yes_no(prompt: &str) -> anyhow::Result<bool> {
        print!("{prompt}");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        let approved = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
        Ok(approved)
    }

    fn read_user_input(editor: &mut DefaultEditor) -> anyhow::Result<Option<String>> {
        let mut full = String::new();
        let mut prompt = "\n> ";

        loop {
            match editor.readline(prompt) {
                Ok(line) => {
                    let trimmed_line = line.trim_end();
                    if trimmed_line.ends_with('\\') {
                        let continued = trimmed_line.trim_end_matches('\\');
                        full.push_str(continued);
                        full.push('\n');
                        prompt = "... ";
                        continue;
                    }
                    full.push_str(trimmed_line);
                    break;
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    return Ok(Some(String::new()));
                }
                Err(ReadlineError::Eof) => {
                    if full.is_empty() {
                        return Ok(None);
                    }
                    break;
                }
                Err(err) => return Err(anyhow::anyhow!("readline failed: {}", err)),
            }
        }

        if !full.trim().is_empty() {
            let _ = editor.add_history_entry(full.as_str());
        }
        Ok(Some(full))
    }

    fn history_file_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join("history")
            .join("repl_history.txt")
    }

    fn create_line_editor() -> anyhow::Result<DefaultEditor> {
        let mut editor = DefaultEditor::new()?;
        let history_path = history_file_path();
        if let Some(parent) = history_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = editor.load_history(&history_path);
        Ok(editor)
    }

    fn persist_line_editor_history(editor: &mut DefaultEditor) -> anyhow::Result<()> {
        let history_path = history_file_path();
        if let Some(parent) = history_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        editor.save_history(&history_path)?;
        Ok(())
    }

    fn tool_group(name: &str, desc: &str) -> &'static str {
        let n = name.to_ascii_lowercase();
        let d = desc.to_ascii_lowercase();

        if n.contains("bash")
            || n.contains("powershell")
            || n.contains("repl")
            || n.contains("git")
            || n.contains("format")
        {
            "shell"
        } else if n.contains("web")
            || n.contains("browser")
            || n.contains("github")
            || n.contains("mcp")
            || d.contains("http")
            || d.contains("web")
        {
            "network"
        } else if n.contains("agent")
            || n.contains("task")
            || n.contains("swarm")
            || n.contains("socratic")
            || n.contains("cron")
            || n.contains("plan")
        {
            "agent"
        } else if n.contains("file")
            || n.contains("glob")
            || n.contains("grep")
            || n.contains("project")
            || n.contains("symbol")
            || n.contains("lsp")
            || n.contains("diff")
            || n.contains("refactor")
            || n.contains("notebook")
            || n.contains("worktree")
        {
            "workspace"
        } else {
            "system"
        }
    }

    async fn handle_slash_command(
        input: &str,
        engine: &std::sync::Arc<crate::engine::streaming::StreamingQueryEngine>,
        show_usage: &mut bool,
        verbose_tool_logs: &mut bool,
    ) -> anyhow::Result<bool> {
        let mut parts = input.split_whitespace();
        let cmd = parts.next().unwrap_or_default();

        match cmd {
            "/help" => {
                println!("Commands:");
                println!("  /help                  Show this help");
                println!("  /tools [keyword]       List available tools");
                println!("  /clear                 Clear conversation history");
                println!("  /new                   Alias of /clear");
                println!("  /model                 Show current provider/model");
                println!("  /cost, /token          Show usage/cost summary");
                println!("  /permission <mode>     Set permission mode");
                println!("                         modes: default|auto_low_risk|auto_all|read_only|once");
                println!("  /usage <on|off>        Toggle per-turn usage lines");
                println!("  /tool-logs <compact|verbose>");
                println!("                         compact hides successful [tool:done] lines");
                println!("  /status                Show basic session status");
                println!("  /exit, /quit           Exit chat");
            }
            "/clear" | "/new" => {
                engine.clear_history().await;
                println!("Conversation history cleared.");
            }
            "/model" => {
                println!(
                    "provider: {}\nmodel: {}",
                    engine.provider_base_url(),
                    engine.model_name()
                );
            }
            "/permission" => {
                let Some(mode) = parts.next() else {
                    println!("Usage: /permission <default|auto_low_risk|auto_all|read_only|once>");
                    return Ok(false);
                };
                let parsed = match mode.to_ascii_lowercase().as_str() {
                    "default" => Some(crate::permissions::PermissionMode::Default),
                    "auto_low_risk" | "autolowrisk" | "low_risk" => {
                        Some(crate::permissions::PermissionMode::AutoLowRisk)
                    }
                    "auto_all" | "autoall" => Some(crate::permissions::PermissionMode::AutoAll),
                    "read_only" | "readonly" => Some(crate::permissions::PermissionMode::ReadOnly),
                    "once" => Some(crate::permissions::PermissionMode::Once),
                    _ => None,
                };

                if let Some(mode) = parsed {
                    engine.set_permission_mode(mode);
                    println!("permission mode set to: {:?}", mode);
                } else {
                    println!("Unknown mode: {mode}");
                }
            }
            "/cost" | "/token" => {
                let tracker = engine.cost_tracker().lock().await;
                let tool_calls: u64 = tracker.tool_usage.values().copied().sum();
                println!("{}", tracker.token_summary());
                println!("{}", tracker.model_usage_summary());
                println!(
                    "cost: ${:.4}, requests: {}, tool_calls: {}",
                    tracker.estimated_cost_usd, tracker.total_requests, tool_calls
                );
            }
            "/tools" => {
                let keyword = parts.next().map(|s| s.to_ascii_lowercase());
                let registry = engine.tool_registry();
                let mut names: Vec<&str> = registry.tool_names();
                names.sort_unstable();

                let mut grouped: BTreeMap<&str, Vec<String>> = BTreeMap::new();
                for name in names {
                    if let Some(tool) = registry.get(name) {
                        let desc = tool.description();
                        let matches = if let Some(ref kw) = keyword {
                            name.to_ascii_lowercase().contains(kw)
                                || desc.to_ascii_lowercase().contains(kw)
                        } else {
                            true
                        };
                        if matches {
                            let group = tool_group(name, desc);
                            grouped
                                .entry(group)
                                .or_default()
                                .push(format!("- {:<20} {}", name, desc));
                        }
                    }
                }
                let count: usize = grouped.values().map(|v| v.len()).sum();
                if count == 0 {
                    println!("No tools matched.");
                } else {
                    for (group, items) in grouped {
                        println!("[{}]", group);
                        for item in items {
                            println!("{}", item);
                        }
                    }
                    println!("total: {}", count);
                }
            }
            "/usage" => {
                let Some(value) = parts.next() else {
                    println!("usage lines: {}", if *show_usage { "on" } else { "off" });
                    return Ok(false);
                };
                match value.to_ascii_lowercase().as_str() {
                    "on" => {
                        *show_usage = true;
                        println!("usage lines enabled.");
                    }
                    "off" => {
                        *show_usage = false;
                        println!("usage lines disabled.");
                    }
                    _ => println!("Usage: /usage <on|off>"),
                }
            }
            "/tool-logs" => {
                let Some(value) = parts.next() else {
                    println!(
                        "tool logs: {}",
                        if *verbose_tool_logs {
                            "verbose"
                        } else {
                            "compact"
                        }
                    );
                    return Ok(false);
                };
                match value.to_ascii_lowercase().as_str() {
                    "compact" => {
                        *verbose_tool_logs = false;
                        println!("tool logs set to compact.");
                    }
                    "verbose" => {
                        *verbose_tool_logs = true;
                        println!("tool logs set to verbose.");
                    }
                    _ => println!("Usage: /tool-logs <compact|verbose>"),
                }
            }
            "/status" => {
                let history_len = engine.get_history().await.len();
                println!(
                    "model: {}\npermission: {}\nhistory_messages: {}\nusage_lines: {}\ntool_logs: {}",
                    engine.model_name(),
                    format!("{:?}", engine.permission_mode()),
                    history_len,
                    if *show_usage { "on" } else { "off" },
                    if *verbose_tool_logs {
                        "verbose"
                    } else {
                        "compact"
                    }
                );
            }
            "/exit" | "/quit" => return Ok(true),
            _ => println!("Unknown command: {cmd}. Use /help."),
        }

        Ok(false)
    }
}

#[cfg(not(feature = "legacy-cli"))]
pub async fn run_cli() {
    eprintln!("CLI mode not compiled in (missing legacy-cli feature)");
}

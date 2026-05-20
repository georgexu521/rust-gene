//! Scrollback-first interactive shell.
//!
//! This renderer intentionally avoids alternate-screen full redraws. The main
//! conversation is appended to the user's real terminal scrollback, while only
//! the short "thinking" line is transient. This matches the interaction model
//! used by mature coding-agent CLIs more closely than a dashboard-style TUI.

use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use crate::services::api::Message;
use crate::session_store::{MessageRecord, SessionRecord, SessionStore};
use crate::tui::tool_view::{upsert_tool_run, with_tool_run, ToolRunView};
use futures::StreamExt;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{history::DefaultHistory, Config};
use rustyline::{Context, Editor, Helper};
use std::borrow::Cow;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";

const LOCAL_COMMANDS: &[ShellCommand] = &[
    ShellCommand::new("/help", "show commands"),
    ShellCommand::new("/commands", "show commands"),
    ShellCommand::new("/resume", "resume a previous conversation"),
    ShellCommand::new("/sessions", "list previous conversations"),
    ShellCommand::new("/status", "show model and context status"),
    ShellCommand::new("/model", "show active model"),
    ShellCommand::new("/clear", "clear terminal"),
    ShellCommand::new("/exit", "quit"),
];

pub async fn run_shell(engine: Arc<StreamingQueryEngine>) -> anyhow::Result<()> {
    if !io::stdin().is_terminal() {
        anyhow::bail!("CLI mode requires an interactive terminal");
    }

    print_welcome(&engine).await;

    let mut editor = build_line_editor()?;
    let history_path = shell_history_path();
    if let Some(path) = history_path.as_ref() {
        let _ = editor.load_history(path);
    }

    loop {
        let message = match editor.readline("› ") {
            Ok(line) => line.trim_end_matches(['\r', '\n']).to_string(),
            Err(ReadlineError::Interrupted) => {
                println!("{DIM}Interrupted. Press Ctrl+D or type /exit to quit.{RESET}");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(err) => return Err(err.into()),
        };

        if message.trim().is_empty() {
            continue;
        }

        let _ = editor.add_history_entry(message.as_str());
        if let Some(path) = history_path.as_ref() {
            let _ = editor.save_history(path);
        }

        if handle_local_command(&engine, &message).await? {
            if matches!(message.trim(), "/exit" | "/quit" | "exit" | "quit") {
                break;
            }
            continue;
        }

        run_turn(engine.clone(), message).await?;
        println!();
    }

    Ok(())
}

async fn print_welcome(engine: &StreamingQueryEngine) {
    let usage = engine.context_usage_report().await;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let dir = compact_home_path(&cwd);
    let model = compact_line(&engine.model_name(), 30);
    let provider = compact_line(&engine.provider_base_url(), 42);
    let mode = permission_mode_label(engine.permission_mode());
    let width = terminal_width().clamp(60, 110);
    let inner = width.saturating_sub(4);

    println!(
        "{BLUE}╭─{RESET} {BOLD}Priority Agent{RESET} {DIM}coding agent{RESET}{}",
        colored_rule(width.saturating_sub(31), BLUE)
    );
    println!(
        "{BLUE}│{RESET}  {BOLD}Welcome back.{RESET} {DIM}Ask for code changes, debugging, reviews, or project inspection.{RESET}"
    );
    println!("{BLUE}│{RESET}");
    println!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}{}",
        "Directory",
        compact_line(&dir, inner.saturating_sub(12))
    );
    println!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}{} {DIM}· provider {RESET}{}",
        "Model", model, provider
    );
    println!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}{} {DIM}· context {RESET}{} / {}",
        "Mode", mode, usage.total_estimated_tokens, usage.max_context_tokens
    );
    println!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}/help commands · /status details · /exit quit",
        "Shortcuts"
    );
    println!("{BLUE}╰{}╯{RESET}", "─".repeat(width.saturating_sub(2)));
    println!();
}

async fn handle_local_command(
    engine: &StreamingQueryEngine,
    message: &str,
) -> anyhow::Result<bool> {
    match message.trim() {
        "/exit" | "/quit" | "exit" | "quit" => {
            engine
                .flush_memory_for_current_history(crate::memory::MemoryFlushReason::Exit)
                .await;
            println!("{DIM}Bye.{RESET}");
            Ok(true)
        }
        "/help" | "/commands" | "/?" | "help" => {
            print_command_help();
            Ok(true)
        }
        "/model" => {
            println!(
                "{BOLD}Model{RESET}\n{DIM}  provider{RESET} {}\n{DIM}  model   {RESET} {}",
                engine.provider_base_url(),
                engine.model_name()
            );
            Ok(true)
        }
        "/sessions" => {
            print_sessions(engine, 20)?;
            Ok(true)
        }
        command if command == "/resume" || command.starts_with("/resume ") => {
            resume_session_command(engine, command).await?;
            Ok(true)
        }
        "/status" => {
            print_status(engine).await;
            Ok(true)
        }
        "/clear" => {
            engine.clear_history().await;
            print!("\x1b[2J\x1b[H");
            io::stdout().flush()?;
            Ok(true)
        }
        "/tui" => {
            println!("{DIM}Run `pa --tui` to open the full-screen terminal interface.{RESET}");
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn print_command_help() {
    println!("{BOLD}Commands{RESET}");
    for command in LOCAL_COMMANDS {
        println!("{DIM}  {:<10}{RESET}{}", command.name, command.description);
    }
    println!("{DIM}  /?        {RESET}alias for /help");
    println!();
    println!(
        "{DIM}Tips: /resume opens prior conversations · ↑/↓ history · Tab complete slash commands{RESET}"
    );
}

fn print_sessions(engine: &StreamingQueryEngine, limit: i64) -> anyhow::Result<()> {
    let Some((store, current_id)) = engine.session_binding() else {
        println!("{DIM}No session store is configured for this run.{RESET}");
        return Ok(());
    };
    let sessions = store.list_sessions(limit)?;
    if sessions.is_empty() {
        println!("{DIM}No previous sessions found.{RESET}");
        return Ok(());
    }
    print_session_list(&store, &sessions, Some(&current_id))?;
    Ok(())
}

async fn resume_session_command(
    engine: &StreamingQueryEngine,
    command: &str,
) -> anyhow::Result<()> {
    let Some((store, current_id)) = engine.session_binding() else {
        println!("{DIM}No session store is configured for this run.{RESET}");
        return Ok(());
    };

    let sessions = resumable_sessions(&store, store.list_sessions(40)?);
    if sessions.is_empty() {
        println!("{DIM}No previous sessions found.{RESET}");
        return Ok(());
    }

    let query = command.strip_prefix("/resume").unwrap_or("").trim();
    let selected = if query.is_empty() {
        print_session_list(&store, &sessions, Some(&current_id))?;
        print!("{DIM}Resume session [number/id/search, empty to cancel] {RESET}");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        let answer = answer.trim();
        if answer.is_empty() {
            println!("{DIM}Resume cancelled.{RESET}");
            return Ok(());
        }
        resolve_session_selection(&store, &sessions, answer)?
    } else {
        resolve_session_selection(&store, &sessions, query)?
    };

    let Some(session) = selected else {
        println!("{YELLOW}No matching session found.{RESET}");
        return Ok(());
    };

    let records = store.get_messages(&session.id)?;
    let messages = records_to_api_messages(&records);
    engine
        .flush_memory_for_current_history(crate::memory::MemoryFlushReason::ResumeSwitch)
        .await;
    engine.set_history(messages).await;
    engine.set_session_id(session.id.clone());

    println!(
        "{GREEN}✓{RESET} Resumed {} {DIM}· {} messages · updated {}{RESET}",
        display_session_title(&session),
        records.len(),
        session.updated_at
    );
    print_recent_session_preview(&records);
    Ok(())
}

fn print_session_list(
    store: &SessionStore,
    sessions: &[SessionRecord],
    current_id: Option<&str>,
) -> anyhow::Result<()> {
    println!("{BOLD}Conversations{RESET}");
    for (idx, session) in sessions.iter().enumerate() {
        let count = store.message_count(&session.id).unwrap_or_default();
        let marker = if current_id == Some(session.id.as_str()) {
            "*"
        } else {
            " "
        };
        println!(
            "{DIM}{:>2}.{}{RESET} {:<42} {DIM}{:>3} msgs · {} · {}{RESET}",
            idx + 1,
            marker,
            compact_line(&display_session_title(session), 42),
            count,
            compact_line(&session.model, 18),
            session.updated_at
        );
        println!("{DIM}    {}{RESET}", session.id);
    }
    Ok(())
}

fn resumable_sessions(store: &SessionStore, sessions: Vec<SessionRecord>) -> Vec<SessionRecord> {
    sessions
        .into_iter()
        .filter(|session| store.message_count(&session.id).unwrap_or_default() > 0)
        .collect()
}

fn resolve_session_selection(
    store: &SessionStore,
    sessions: &[SessionRecord],
    query: &str,
) -> anyhow::Result<Option<SessionRecord>> {
    if matches!(query, "latest" | "last" | "continue") {
        return Ok(sessions.first().cloned());
    }

    if let Ok(index) = query.parse::<usize>() {
        if (1..=sessions.len()).contains(&index) {
            return Ok(Some(sessions[index - 1].clone()));
        }
    }

    let query_lower = query.to_lowercase();
    if let Some(session) = sessions.iter().find(|session| {
        session.id.starts_with(query)
            || session.title.to_lowercase().contains(&query_lower)
            || session.model.to_lowercase().contains(&query_lower)
    }) {
        return Ok(Some(session.clone()));
    }

    let matches = store.search_messages(query, 8).unwrap_or_default();
    for message in matches {
        if let Some(session) = store.get_session(&message.session_id)? {
            return Ok(Some(session));
        }
    }

    Ok(None)
}

fn records_to_api_messages(records: &[MessageRecord]) -> Vec<Message> {
    records
        .iter()
        .map(|record| match record.role.as_str() {
            "user" => Message::user(record.content.clone()),
            "assistant" => {
                let tool_calls = record.tool_calls.as_ref().and_then(|value| {
                    if value.is_array() {
                        serde_json::from_value::<Vec<crate::services::api::ToolCall>>(value.clone())
                            .ok()
                    } else {
                        None
                    }
                });
                if let Some(tool_calls) = tool_calls {
                    Message::assistant_with_tools(record.content.clone(), tool_calls)
                } else {
                    Message::assistant(record.content.clone())
                }
            }
            "tool" => Message::tool(
                record.tool_call_id.clone().unwrap_or_default(),
                record.content.clone(),
            ),
            _ => Message::system(record.content.clone()),
        })
        .collect()
}

fn print_recent_session_preview(records: &[MessageRecord]) {
    let recent = records
        .iter()
        .rev()
        .filter(|record| matches!(record.role.as_str(), "user" | "assistant"))
        .take(4)
        .collect::<Vec<_>>();
    if recent.is_empty() {
        return;
    }
    println!("{DIM}Recent context:{RESET}");
    for record in recent.into_iter().rev() {
        let label = if record.role == "user" {
            "you"
        } else {
            "agent"
        };
        println!(
            "{DIM}  {:<5}{RESET} {}",
            label,
            compact_line(&record.content, 86)
        );
    }
}

fn display_session_title(session: &SessionRecord) -> String {
    if session.title.trim().is_empty() {
        format!("Session {}", &session.id[..8.min(session.id.len())])
    } else {
        session.title.clone()
    }
}

async fn print_status(engine: &StreamingQueryEngine) {
    let usage = engine.context_usage_report().await;
    let session_rules = engine.session_permission_rules();
    let usage_pct = if usage.max_context_tokens > 0 {
        usage.total_estimated_tokens.saturating_mul(100) / usage.max_context_tokens
    } else {
        0
    };
    let context_bar = percent_bar(usage_pct.min(100), 16);
    let memory_label = if usage.relevant_memories.is_empty() {
        "none".to_string()
    } else {
        format!("{} relevant", usage.relevant_memories.len())
    };
    let rule_count = session_rules.always_allow.len()
        + session_rules.always_deny.len()
        + session_rules.always_ask.len();
    let recent_memory = usage
        .relevant_memories
        .first()
        .map(|m| compact_line(&m.snippet, 72));

    println!("{BOLD}Priority Agent{RESET}");
    println!(
        "{DIM}  model      {RESET}{:<24} {DIM}provider{RESET} {}",
        compact_line(&engine.model_name(), 24),
        compact_line(&engine.provider_base_url(), 48)
    );
    println!(
        "{DIM}  context    {RESET}{} {:>3}%  {}/{} tokens",
        context_bar, usage_pct, usage.total_estimated_tokens, usage.max_context_tokens
    );
    println!(
        "{DIM}  request    {RESET}history {} msgs / {} tokens · tools {} / {} tokens",
        usage.history_messages, usage.history_tokens, usage.tool_count, usage.tool_schema_tokens
    );
    println!(
        "{DIM}  policy     {RESET}{} · {} session rules · {} tools registered",
        permission_mode_label(engine.permission_mode()),
        rule_count,
        usage.tool_count
    );
    println!("{DIM}  memory     {RESET}{memory_label}");
    if let Some(memory) = recent_memory {
        println!("{DIM}  recall     {RESET}{memory}");
    }
    println!(
        "{DIM}  prefix     {RESET}{}",
        usage.stable_prefix_fingerprint
    );
}

fn permission_mode_label(mode: crate::permissions::PermissionMode) -> &'static str {
    match mode {
        crate::permissions::PermissionMode::Default => "default",
        crate::permissions::PermissionMode::AutoLowRisk => "auto-low-risk",
        crate::permissions::PermissionMode::AutoAll => "auto",
        crate::permissions::PermissionMode::ReadOnly => "read-only",
        crate::permissions::PermissionMode::Once => "once",
    }
}

fn percent_bar(percent: u64, width: usize) -> String {
    let filled = ((percent as usize) * width).div_ceil(100).min(width);
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(80)
}

fn colored_rule(len: usize, color: &str) -> String {
    if len == 0 {
        String::new()
    } else {
        format!("{color}{}{RESET}", "─".repeat(len))
    }
}

fn compact_home_path(path: &std::path::Path) -> String {
    let home = dirs::home_dir();
    if let Some(home) = home.as_ref() {
        if let Ok(stripped) = path.strip_prefix(home) {
            let suffix = stripped.to_string_lossy();
            if suffix.is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", suffix);
        }
    }
    path.display().to_string()
}

fn compact_line(text: &str, max_chars: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max_chars {
        return text;
    }

    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionChoice {
    AllowOnce,
    DenyOnce,
    AllowSession,
    DenySession,
}

impl PermissionChoice {
    fn approved(self) -> bool {
        matches!(self, Self::AllowOnce | Self::AllowSession)
    }
}

fn prompt_for_permission(
    engine: &StreamingQueryEngine,
    tool_name: &str,
    arguments: &serde_json::Value,
    prompt: &str,
) -> anyhow::Result<bool> {
    println!("{YELLOW}?{RESET} Permission required");
    if !tool_name.is_empty() {
        println!(
            "{DIM}  tool      {RESET}{}",
            permission_scope_summary(tool_name, arguments)
        );
    }
    for line in prompt.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            println!("{DIM}  {trimmed}{RESET}");
        }
    }
    let pattern = if tool_name.is_empty() {
        None
    } else {
        Some(crate::tui::app::permission_rule_pattern(
            tool_name, arguments,
        ))
    };
    if let Some(pattern) = pattern.as_ref() {
        println!("{DIM}  scope     {RESET}{pattern}");
    }
    println!("{DIM}  choices   {RESET}y allow once · n deny · a allow session · d deny session");
    print!("{DIM}  Choice [y/N/a/d] {RESET}");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let choice = match answer.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => PermissionChoice::AllowOnce,
        "a" | "always" | "allow" => PermissionChoice::AllowSession,
        "d" | "deny-session" => PermissionChoice::DenySession,
        _ => PermissionChoice::DenyOnce,
    };

    if let Some(pattern) = pattern.as_ref() {
        match choice {
            PermissionChoice::AllowSession => {
                engine.add_session_permission_rule("allow", pattern);
                println!("{DIM}  saved     allow {pattern} for this session{RESET}");
            }
            PermissionChoice::DenySession => {
                engine.add_session_permission_rule("deny", pattern);
                println!("{DIM}  saved     deny {pattern} for this session{RESET}");
            }
            PermissionChoice::AllowOnce | PermissionChoice::DenyOnce => {}
        }
    }

    Ok(choice.approved())
}

fn permission_scope_summary(tool_name: &str, arguments: &serde_json::Value) -> String {
    if tool_name == "bash" {
        let cmd = arguments["command"]
            .as_str()
            .or_else(|| arguments["cmd"].as_str())
            .unwrap_or("");
        if !cmd.is_empty() {
            return format!("bash · {}", compact_line(cmd, 80));
        }
    }
    if tool_name == "mcp_tool" {
        let server = arguments["server_name"].as_str().unwrap_or("");
        let tool = arguments["tool_name"].as_str().unwrap_or("");
        if !server.is_empty() || !tool.is_empty() {
            return format!("mcp · {server}/{tool}");
        }
    }
    if matches!(tool_name, "file_write" | "file_edit" | "file_read") {
        if let Some(path) = arguments["path"].as_str() {
            return format!("{tool_name} · {}", compact_line(path, 80));
        }
    }
    tool_name.to_string()
}

fn build_line_editor() -> anyhow::Result<Editor<ShellHelper, DefaultHistory>> {
    let config = Config::builder()
        .auto_add_history(false)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut editor = Editor::<ShellHelper, DefaultHistory>::with_config(config)?;
    editor.set_helper(Some(ShellHelper));
    Ok(editor)
}

fn shell_history_path() -> Option<PathBuf> {
    let mut dir = dirs::data_local_dir().or_else(dirs::home_dir)?;
    dir.push("priority-agent");
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    dir.push("shell_history");
    Some(dir)
}

async fn run_turn(engine: Arc<StreamingQueryEngine>, message: String) -> anyhow::Result<()> {
    let mut stream = engine.query_stream(message).await;
    let mut tool_runs: Vec<ToolRunView> = Vec::new();
    let mut assistant_printer = AssistantPrinter::default();
    show_status("Thinking...")?;
    let mut status_visible = true;

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::Start => {}
            StreamEvent::ThinkingStart => {
                show_status("Thinking...")?;
                status_visible = true;
            }
            StreamEvent::ThinkingChunk(_) => {}
            StreamEvent::ThinkingComplete => {
                clear_status_if_visible(&mut status_visible)?;
            }
            StreamEvent::TextChunk(text) => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.push(&text)?;
            }
            StreamEvent::ToolCallStart { id, name } => {
                upsert_tool_run(&mut tool_runs, id, name);
            }
            StreamEvent::ToolCallArgs { id, args_delta } => {
                with_tool_run(&mut tool_runs, &id, |run| run.push_args_delta(&args_delta));
            }
            StreamEvent::ToolCallComplete { .. } => {}
            StreamEvent::ToolExecutionStart { id, name } => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.finish_line_if_needed()?;
                upsert_tool_run(&mut tool_runs, id.clone(), name.clone());
                with_tool_run(&mut tool_runs, &id, |run| run.mark_running(name));
                if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                    println_tool_line("·", YELLOW, &run.render_lines(false).join("\n"), false);
                }
            }
            StreamEvent::ToolExecutionProgress { id, progress } => {
                with_tool_run(&mut tool_runs, &id, |run| run.push_progress(progress));
                if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                    if let Some(line) = tool_progress_line(run) {
                        clear_status_if_visible(&mut status_visible)?;
                        assistant_printer.finish_line_if_needed()?;
                        println_tool_line("…", YELLOW, &line, false);
                    }
                }
            }
            StreamEvent::ToolExecutionComplete {
                id,
                result,
                metadata,
            } => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.finish_line_if_needed()?;
                with_tool_run(&mut tool_runs, &id, |run| {
                    run.mark_complete_with_metadata(result, metadata)
                });
                if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                    let marker = match run.status {
                        crate::tui::tool_view::ToolRunStatus::Failed
                        | crate::tui::tool_view::ToolRunStatus::TimedOut => "✗",
                        crate::tui::tool_view::ToolRunStatus::Cancelled => "×",
                        crate::tui::tool_view::ToolRunStatus::Backgrounded => "↪",
                        _ => "✓",
                    };
                    let color = if marker == "✗" {
                        RED
                    } else if marker == "×" {
                        YELLOW
                    } else {
                        GREEN
                    };
                    println_tool_line(marker, color, &run.render_lines(false).join("\n"), true);
                }
            }
            StreamEvent::PermissionRequest {
                tool_name,
                arguments,
                prompt,
                ..
            } => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.finish_line_if_needed()?;
                let approved = prompt_for_permission(&engine, &tool_name, &arguments, &prompt)?;
                if let Some(channel) = engine.approval_channel() {
                    if let Some((_request, tx)) = channel.take_pending().await {
                        let _ = tx.send(approved);
                    }
                }
                show_status("Thinking...")?;
                status_visible = true;
            }
            StreamEvent::Usage { .. } => {}
            StreamEvent::OutputTruncated => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.finish_line_if_needed()?;
                println!("{YELLOW}Output truncated. Continue if needed.{RESET}");
            }
            StreamEvent::Complete => break,
            StreamEvent::Error(error) => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.finish_line_if_needed()?;
                println!("{RED}Error:{RESET} {error}");
                break;
            }
        }
    }

    clear_status_if_visible(&mut status_visible)?;
    assistant_printer.finish()?;
    Ok(())
}

fn show_status(text: &str) -> io::Result<()> {
    print!("\r\x1b[2K{YELLOW}· {text}{RESET}");
    io::stdout().flush()
}

fn clear_status_if_visible(visible: &mut bool) -> io::Result<()> {
    if *visible {
        print!("\r\x1b[2K");
        io::stdout().flush()?;
        *visible = false;
    }
    Ok(())
}

fn println_tool_line(marker: &str, color: &str, text: &str, first_line_normal: bool) {
    for (idx, line) in text.lines().enumerate() {
        if idx == 0 {
            if first_line_normal {
                println!("{color}{marker}{RESET} {line}");
            } else {
                println!("{color}{marker}{RESET} {DIM}{line}{RESET}");
            }
        } else {
            println!("{DIM}{line}{RESET}");
        }
    }
}

fn tool_progress_line(run: &ToolRunView) -> Option<String> {
    let latest = run.progress.last()?.trim();
    if latest.is_empty() {
        return None;
    }
    Some(format!(
        "{} · {}",
        compact_line(&run.name, 24),
        compact_line(latest, 96)
    ))
}

#[derive(Clone, Copy)]
struct ShellCommand {
    name: &'static str,
    description: &'static str,
}

impl ShellCommand {
    const fn new(name: &'static str, description: &'static str) -> Self {
        Self { name, description }
    }
}

#[derive(Default)]
struct ShellHelper;

impl Helper for ShellHelper {}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let prefix = &line[..pos];
        if !prefix.starts_with('/') || prefix.contains(char::is_whitespace) {
            return Ok((pos, Vec::new()));
        }

        let candidates = LOCAL_COMMANDS
            .iter()
            .filter(|command| command.name.starts_with(prefix))
            .map(|command| Pair {
                display: format!("{:<7} {}", command.name, command.description),
                replacement: command.name.to_string(),
            })
            .collect();

        Ok((0, candidates))
    }
}

impl Hinter for ShellHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        let prefix = &line[..pos];
        if !prefix.starts_with('/') || prefix.contains(char::is_whitespace) {
            return None;
        }

        let command = LOCAL_COMMANDS
            .iter()
            .find(|command| command.name.starts_with(prefix) && command.name != prefix)?;
        Some(command.name[prefix.len()..].to_string())
    }
}

impl Highlighter for ShellHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Owned(format!("{DIM}{prompt}{RESET}"))
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("{DIM}{hint}{RESET}"))
    }
}

impl Validator for ShellHelper {
    fn validate(&self, _ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        Ok(ValidationResult::Valid(None))
    }
}

#[derive(Default)]
struct AssistantPrinter {
    started: bool,
    line: String,
    in_code_block: bool,
    blank_lines: usize,
}

impl AssistantPrinter {
    fn push(&mut self, text: &str) -> io::Result<()> {
        self.ensure_started()?;
        self.line.push_str(text);

        while let Some(newline_idx) = self.line.find('\n') {
            let rest = self.line.split_off(newline_idx + 1);
            let complete = std::mem::replace(&mut self.line, rest);
            let line = complete.trim_end_matches(['\r', '\n']);
            self.print_line(line)?;
        }

        io::stdout().flush()
    }

    fn finish_line_if_needed(&mut self) -> io::Result<()> {
        if self.started && !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            self.print_line(&line)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        self.finish_line_if_needed()?;
        if self.started {
            println!();
        }
        Ok(())
    }

    fn ensure_started(&mut self) -> io::Result<()> {
        if !self.started {
            print!("{CYAN}●{RESET} ");
            self.started = true;
        }
        Ok(())
    }

    fn print_line(&mut self, line: &str) -> io::Result<()> {
        let rendered = render_assistant_line(line, &mut self.in_code_block);
        if rendered.trim().is_empty() {
            if !self.started || self.blank_lines >= 1 {
                return Ok(());
            }
            self.blank_lines += 1;
            println!();
        } else {
            self.blank_lines = 0;
            println!("{rendered}");
        }
        Ok(())
    }
}

fn render_assistant_line(line: &str, in_code_block: &mut bool) -> String {
    let trimmed = line.trim_end();
    if trimmed.trim_start().starts_with("```") {
        let was_in_code_block = *in_code_block;
        *in_code_block = !*in_code_block;
        let label = trimmed.trim_start().trim_start_matches("```").trim();
        if was_in_code_block {
            return format!("{DIM}╰─{RESET}");
        }
        if label.is_empty() {
            return format!("{DIM}╭─ code{RESET}");
        }
        return format!("{DIM}╭─ {label}{RESET}");
    }

    if *in_code_block {
        return format!("{DIM}│{RESET} {trimmed}");
    }

    if let Some(table_line) = render_markdown_table_line(trimmed) {
        return table_line;
    }

    let cleaned = clean_markdown_inline(trimmed);
    let heading = cleaned.trim_start();
    if heading.starts_with('#') {
        let heading_text = heading.trim_start_matches('#').trim_start();
        return format!("{BOLD}{heading_text}{RESET}");
    }
    if let Some(block_quote) = render_block_quote(&cleaned) {
        return block_quote;
    }
    if let Some(list_item) = render_list_item(&cleaned) {
        return list_item;
    }
    if let Some(numbered_item) = render_numbered_item(&cleaned) {
        return numbered_item;
    }
    cleaned
}

fn render_list_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());
    let marker = ["- ", "* ", "• "]
        .iter()
        .find(|marker| trimmed.starts_with(**marker))?;
    let text = trimmed[marker.len()..].trim_start();
    let spaces = " ".repeat(indent.min(6));
    Some(format!("{spaces}{DIM}•{RESET} {text}"))
}

fn render_numbered_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());
    let dot = trimmed.find(". ")?;
    if dot == 0 || dot > 3 {
        return None;
    }
    let number = &trimmed[..dot];
    if !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let text = trimmed[dot + 2..].trim_start();
    let spaces = " ".repeat(indent.min(6));
    Some(format!("{spaces}{DIM}{number}.{RESET} {text}"))
}

fn render_block_quote(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let text = trimmed.strip_prefix("> ")?;
    let indent = line.len().saturating_sub(trimmed.len()).min(6);
    Some(format!("{}{DIM}│ {text}{RESET}", " ".repeat(indent)))
}

fn render_markdown_table_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }

    let cells: Vec<String> = trimmed
        .trim_matches('|')
        .split('|')
        .map(|cell| clean_markdown_inline(cell.trim()))
        .filter(|cell| !cell.is_empty())
        .collect();

    if cells.is_empty() {
        return Some(String::new());
    }

    let is_separator = cells.iter().all(|cell| {
        cell.chars()
            .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
    });
    if is_separator {
        return Some(String::new());
    }

    Some(format!("  {}", cells.join("  ")))
}

fn clean_markdown_inline(line: &str) -> String {
    let mut out = line.replace("**", "");
    out = out.replace('`', "");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_table_separator_is_hidden() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("|---|:---:|", &mut in_code),
            String::new()
        );
    }

    #[test]
    fn markdown_table_is_softened() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("| `gex` | 空文件夹 |", &mut in_code),
            "  gex  空文件夹"
        );
    }

    #[test]
    fn inline_markdown_is_cleaned() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("**文件：** `a.md`", &mut in_code),
            "文件： a.md"
        );
    }

    #[test]
    fn markdown_lists_are_softened() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("- first item", &mut in_code),
            format!("{DIM}•{RESET} first item")
        );
        assert_eq!(
            render_assistant_line("  1. next item", &mut in_code),
            format!("  {DIM}1.{RESET} next item")
        );
    }

    #[test]
    fn markdown_quotes_and_code_blocks_are_softened() {
        let mut in_code = false;
        assert_eq!(
            render_assistant_line("> note", &mut in_code),
            format!("{DIM}│ note{RESET}")
        );
        assert_eq!(
            render_assistant_line("```rust", &mut in_code),
            format!("{DIM}╭─ rust{RESET}")
        );
        assert_eq!(
            render_assistant_line("let x = 1;", &mut in_code),
            format!("{DIM}│{RESET} let x = 1;")
        );
        assert_eq!(
            render_assistant_line("```", &mut in_code),
            format!("{DIM}╰─{RESET}")
        );
    }

    #[test]
    fn percent_bar_renders_fixed_width() {
        assert_eq!(percent_bar(0, 4), "[░░░░]");
        assert_eq!(percent_bar(50, 4), "[██░░]");
        assert_eq!(percent_bar(100, 4), "[████]");
    }

    #[test]
    fn tool_progress_line_shows_latest_progress_compactly() {
        let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        run.push_progress("required validation still running after 30s".to_string());
        let line = tool_progress_line(&run).expect("progress line");
        assert!(line.contains("bash"));
        assert!(line.contains("30s"));
    }

    #[test]
    fn permission_choice_approval_semantics() {
        assert!(PermissionChoice::AllowOnce.approved());
        assert!(PermissionChoice::AllowSession.approved());
        assert!(!PermissionChoice::DenyOnce.approved());
        assert!(!PermissionChoice::DenySession.approved());
    }

    #[test]
    fn slash_completion_lists_matching_commands() {
        let helper = ShellHelper;
        let history = DefaultHistory::new();
        let (start, candidates) = helper
            .complete("/c", 2, &Context::new(&history))
            .expect("completion should succeed");

        assert_eq!(start, 0);
        assert!(candidates.iter().any(|pair| pair.replacement == "/clear"));
        assert!(!candidates.iter().any(|pair| pair.replacement == "/help"));
    }

    #[test]
    fn slash_hint_completes_command_suffix() {
        let helper = ShellHelper;
        let history = DefaultHistory::new();
        assert_eq!(
            helper.hint("/he", 3, &Context::new(&history)),
            Some("lp".to_string())
        );
    }

    #[test]
    fn session_selection_accepts_index_id_and_title() {
        let store = SessionStore::in_memory().expect("store");
        store
            .create_session("session-alpha", "Fix login bug", "model-a")
            .unwrap();
        store
            .create_session("session-beta", "Build dashboard", "model-b")
            .unwrap();
        let sessions = store.list_sessions(10).unwrap();

        let by_index = resolve_session_selection(&store, &sessions, "1")
            .unwrap()
            .unwrap();
        assert!(by_index.id == "session-alpha" || by_index.id == "session-beta");

        let by_id = resolve_session_selection(&store, &sessions, "session-alpha")
            .unwrap()
            .unwrap();
        assert_eq!(by_id.id, "session-alpha");

        let by_title = resolve_session_selection(&store, &sessions, "dashboard")
            .unwrap()
            .unwrap();
        assert_eq!(by_title.id, "session-beta");
    }

    #[test]
    fn message_records_restore_api_history() {
        let records = vec![
            MessageRecord {
                id: 1,
                session_id: "s".to_string(),
                role: "user".to_string(),
                content: "hello".to_string(),
                tool_calls: None,
                tool_call_id: None,
                reasoning: None,
                created_at: "now".to_string(),
            },
            MessageRecord {
                id: 2,
                session_id: "s".to_string(),
                role: "assistant".to_string(),
                content: "hi".to_string(),
                tool_calls: None,
                tool_call_id: None,
                reasoning: None,
                created_at: "now".to_string(),
            },
        ];

        let messages = records_to_api_messages(&records);
        assert!(matches!(messages[0], Message::User { .. }));
        assert!(matches!(messages[1], Message::Assistant { .. }));
    }
}

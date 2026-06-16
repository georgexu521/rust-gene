//! Scrollback-first interactive shell.
//!
//! This renderer intentionally avoids alternate-screen full redraws. The main
//! conversation is appended to the user's real terminal scrollback, while a
//! fixed footer at the bottom shows the prompt and transient status. This
//! matches the interaction model used by mature coding-agent CLIs more closely
//! than a dashboard-style TUI.

pub mod attachment;
pub mod completion;
pub mod footer;
pub mod host;
pub mod interrupt;
pub mod permission_diff;
pub mod prompt;
pub mod question;
pub mod render;
pub mod theme;

pub mod slash;

use crate::components::attachment_token::AttachmentSource;
use crate::engine::runtime_controller::RuntimeController;
use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use crate::session_store::{SessionRecord, SessionStore};
use crate::shell::attachment::AttachmentManager;
use crate::shell::completion::{find_candidates, MentionCandidate};
use crate::shell::footer::{AttachmentLine, FooterMode, FooterRenderer};
use crate::shell::host::{CliHost, ShellHost};
use crate::shell::interrupt::InterruptState;
use crate::shell::prompt::PromptEditor;
use crate::shell::render::render_assistant_line;
use crate::shell::slash::{
    handle_diff, handle_export_data, handle_redo, handle_save_session, handle_undo,
};
use crate::shell::theme::*;
use crate::tui::tool_view::{upsert_tool_run, with_tool_run, ToolRunView};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;

const LOCAL_COMMANDS: &[ShellCommand] = &[
    ShellCommand::new("/help", "show commands"),
    ShellCommand::new("/commands", "show commands"),
    ShellCommand::new("/attach", "attach a file to the next message"),
    ShellCommand::new("/detach", "remove an attachment"),
    ShellCommand::new("/resume", "resume a previous conversation"),
    ShellCommand::new("/sessions", "list previous conversations"),
    ShellCommand::new("/new", "start a new conversation"),
    ShellCommand::new("/status", "show model and context status"),
    ShellCommand::new("/model", "show active model"),
    ShellCommand::new("/provider", "list or switch provider"),
    ShellCommand::new("/tools", "list available tools"),
    ShellCommand::new("/permissions", "show permission rules"),
    ShellCommand::new("/memory", "show or toggle memory settings"),
    ShellCommand::new("/cost", "show token and cost usage"),
    ShellCommand::new("/token", "show token and cost usage"),
    ShellCommand::new("/undo", "undo last file edit"),
    ShellCommand::new("/redo", "redo last file edit"),
    ShellCommand::new("/diff", "show diff for recent edits or file"),
    ShellCommand::new("/export", "export current session"),
    ShellCommand::new("/save", "save current session"),
    ShellCommand::new("/doctor", "show environment diagnostics"),
    ShellCommand::new("/audit", "show token usage or tool audit"),
    ShellCommand::new("/clear", "clear terminal"),
    ShellCommand::new("/tui", "open the full-screen TUI"),
    ShellCommand::new("/exit", "quit"),
];

pub async fn run_shell(engine: Arc<StreamingQueryEngine>) -> anyhow::Result<()> {
    if !io::stdin().is_terminal() {
        anyhow::bail!("CLI mode requires an interactive terminal");
    }

    crossterm::terminal::enable_raw_mode()?;
    let result = run_shell_inner(engine).await;
    let _ = crossterm::terminal::disable_raw_mode();
    result
}

async fn run_shell_inner(engine: Arc<StreamingQueryEngine>) -> anyhow::Result<()> {
    print_welcome(&engine).await;

    let session_manager = build_session_manager(&engine).await?;
    let mut host = CliHost::new(engine.clone(), session_manager);

    let footer_height = 3usize;
    let mut editor = PromptEditor::new();
    let mut attachments = AttachmentManager::new();
    let mut completion_state: Option<(usize, Vec<MentionCandidate>, usize)> = None;
    let mut footer = FooterRenderer::new(footer_height);
    let interrupt = InterruptState::new();
    let controller = RuntimeController::new(engine.clone());

    // Reserve footer space at the bottom of the terminal.
    for _ in 0..footer_height {
        println!();
    }

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    tokio::spawn(event_reader(event_tx));

    render_prompt_footer(&mut footer, &editor, &attachments)?;

    loop {
        let Some(event) = event_rx.recv().await else {
            break;
        };

        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Release {
                continue;
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    if interrupt.request_interrupt() {
                        controller.cancel().await;
                        footer.render(&FooterMode::Interrupt, &editor, terminal_width())?;
                    } else {
                        break;
                    }
                }
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    if !editor.is_empty() || !attachments.is_empty() {
                        let message = editor.text();
                        editor.clear();
                        footer.print_above(&format_user_message(&message))?;

                        if handle_local_command(
                            &mut host,
                            &engine,
                            &message,
                            &mut footer,
                            &mut attachments,
                        )
                        .await?
                        {
                            if matches!(message.trim(), "/exit" | "/quit" | "exit" | "quit") {
                                break;
                            }
                            render_prompt_footer(&mut footer, &editor, &attachments)?;
                            continue;
                        }

                        let submission = attachments.build_submission(&message);
                        attachments.clear();
                        interrupt.start_turn();
                        footer.render(&FooterMode::Thinking, &editor, terminal_width())?;

                        let mut stream = controller.submit_stream_turn(submission).await;
                        run_turn(
                            engine.clone(),
                            &controller,
                            &mut stream,
                            &mut footer,
                            &interrupt,
                            &mut event_rx,
                        )
                        .await?;
                        interrupt.end_turn();
                        render_prompt_footer(&mut footer, &editor, &attachments)?;
                    }
                }
                (KeyModifiers::CONTROL, KeyCode::Char('d'))
                | (KeyModifiers::NONE, KeyCode::Esc) => {
                    if editor.is_empty() && attachments.is_empty() {
                        break;
                    }
                }
                (_, KeyCode::Char(ch)) => {
                    editor.insert(&ch.to_string());
                    completion_state = None;
                    if let Some((_, candidates)) =
                        find_candidates(&editor.text(), current_cursor_col(&editor))
                    {
                        if !candidates.is_empty() {
                            completion_state =
                                Some((current_cursor_col(&editor), candidates.clone(), 0));
                        }
                    }
                    render_prompt_footer_with_completion(
                        &mut footer,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Backspace) => {
                    editor.backspace();
                    completion_state = update_completion_after_edit(&editor, completion_state);
                    render_prompt_footer_with_completion(
                        &mut footer,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Delete) => {
                    editor.delete();
                    completion_state = update_completion_after_edit(&editor, completion_state);
                    render_prompt_footer_with_completion(
                        &mut footer,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (KeyModifiers::CONTROL, KeyCode::Left) | (KeyModifiers::ALT, KeyCode::Left) => {
                    editor.move_word_left();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, prompt_prefix_width())?;
                }
                (KeyModifiers::CONTROL, KeyCode::Right) | (KeyModifiers::ALT, KeyCode::Right) => {
                    editor.move_word_right();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, prompt_prefix_width())?;
                }
                (_, KeyCode::Left) => {
                    editor.move_left();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, prompt_prefix_width())?;
                }
                (_, KeyCode::Right) => {
                    editor.move_right();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, prompt_prefix_width())?;
                }
                (_, KeyCode::Up) => {
                    if let Some((_, _, selected)) = completion_state.as_mut() {
                        *selected = selected.saturating_sub(1);
                        render_prompt_footer_with_completion(
                            &mut footer,
                            &editor,
                            &attachments,
                            completion_state.as_ref(),
                        )?;
                    } else {
                        editor.move_up();
                        render_prompt_footer(&mut footer, &editor, &attachments)?;
                        footer.position_cursor(&editor, prompt_prefix_width())?;
                    }
                }
                (_, KeyCode::Down) => {
                    if let Some((_, candidates, selected)) = completion_state.as_mut() {
                        *selected = (*selected + 1).min(candidates.len().saturating_sub(1));
                        render_prompt_footer_with_completion(
                            &mut footer,
                            &editor,
                            &attachments,
                            completion_state.as_ref(),
                        )?;
                    } else {
                        editor.move_down();
                        render_prompt_footer(&mut footer, &editor, &attachments)?;
                        footer.position_cursor(&editor, prompt_prefix_width())?;
                    }
                }
                (KeyModifiers::NONE, KeyCode::Tab) => {
                    if let Some((start, candidates, selected)) = completion_state.take() {
                        if let Some(candidate) = candidates.get(selected) {
                            replace_word_at_cursor(&mut editor, start, &candidate.replacement);
                        }
                        render_prompt_footer(&mut footer, &editor, &attachments)?;
                    }
                }
                (_, KeyCode::Home) => {
                    editor.move_home();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, prompt_prefix_width())?;
                }
                (_, KeyCode::End) => {
                    editor.move_end();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, prompt_prefix_width())?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn current_cursor_col(editor: &PromptEditor) -> usize {
    let (row, col) = editor.cursor();
    let mut chars_before = 0usize;
    for (idx, line) in editor.lines().iter().enumerate() {
        if idx < row {
            chars_before += line.chars().count() + 1; // +1 for newline
        } else {
            chars_before += line[..col].chars().count();
            break;
        }
    }
    chars_before
}

fn replace_word_at_cursor(editor: &mut PromptEditor, start_col: usize, replacement: &str) {
    let text = editor.text();
    let end_col = current_cursor_col(editor);
    let mut new_text = String::with_capacity(text.len() + replacement.len());
    new_text.push_str(&text[..start_col.min(text.len())]);
    new_text.push_str(replacement);
    new_text.push_str(&text[end_col.min(text.len())..]);
    editor.clear();
    editor.insert(&new_text);
    let new_col = start_col + replacement.chars().count();
    position_editor_cursor(editor, new_col);
}

fn position_editor_cursor(editor: &mut PromptEditor, target_char_col: usize) {
    let text = editor.text();
    let mut chars_seen = 0usize;
    for (row, line) in text.lines().enumerate() {
        let line_chars = line.chars().count();
        if chars_seen + line_chars >= target_char_col {
            let col_in_line = target_char_col.saturating_sub(chars_seen);
            let byte_col = line
                .char_indices()
                .nth(col_in_line)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            // PromptEditor cursor is (row, byte col); move until match
            while editor.cursor().0 > row {
                editor.move_up();
            }
            while editor.cursor().0 < row {
                editor.move_down();
            }
            while editor.cursor().1 > byte_col {
                editor.move_left();
            }
            while editor.cursor().1 < byte_col {
                editor.move_right();
            }
            return;
        }
        chars_seen += line_chars + 1;
    }
    editor.move_end();
}

fn update_completion_after_edit(
    editor: &PromptEditor,
    state: Option<(usize, Vec<MentionCandidate>, usize)>,
) -> Option<(usize, Vec<MentionCandidate>, usize)> {
    let col = current_cursor_col(editor);
    if let Some((start, _, _)) = state {
        if col >= start {
            if let Some((new_start, candidates)) = find_candidates(&editor.text(), col) {
                if !candidates.is_empty() {
                    return Some((new_start, candidates, 0));
                }
            }
        }
    }
    None
}

fn render_prompt_footer_with_completion(
    footer: &mut FooterRenderer,
    editor: &PromptEditor,
    attachments: &AttachmentManager,
    completion: Option<&(usize, Vec<MentionCandidate>, usize)>,
) -> io::Result<()> {
    let mut line = attachments.render_pills(terminal_width().saturating_sub(2));
    if let Some((_, candidates, selected)) = completion {
        let mut comp_line = String::from("Completion: ");
        for (idx, candidate) in candidates.iter().take(6).enumerate() {
            if idx > 0 {
                comp_line.push_str("  ");
            }
            let marker = if idx == *selected { ">" } else { " " };
            comp_line.push_str(&format!("{}{}", marker, candidate.display));
        }
        if !line.is_empty() {
            line.push('\n');
        }
        line.push_str(&comp_line);
    }
    footer.render_with_attachments(
        &FooterMode::Prompt,
        editor,
        terminal_width(),
        &AttachmentLine { text: line },
    )
}

fn render_prompt_footer(
    footer: &mut FooterRenderer,
    editor: &PromptEditor,
    attachments: &AttachmentManager,
) -> io::Result<()> {
    render_prompt_footer_with_completion(footer, editor, attachments, None)
}

fn prompt_prefix_width() -> usize {
    2
}

fn format_user_message(message: &str) -> String {
    message
        .lines()
        .enumerate()
        .map(|(idx, line)| {
            let prefix = if idx == 0 { "" } else { "  " };
            format!("{}{}│{} {}", prefix, DIM, RESET, line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn event_reader(event_tx: tokio::sync::mpsc::UnboundedSender<Event>) {
    while let Ok(Ok(event)) = tokio::task::spawn_blocking(crossterm::event::read).await {
        if event_tx.send(event).is_err() {
            break;
        }
    }
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

async fn build_session_manager(
    engine: &StreamingQueryEngine,
) -> anyhow::Result<crate::tui::session_manager::TuiSessionManager> {
    if let Some((store, session_id)) = engine.session_binding() {
        let title = engine
            .current_session_id()
            .unwrap_or_else(|| "New Session".to_string());
        let model = engine.model_name();
        let workspace = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string());
        crate::tui::session_manager::TuiSessionManager::from_store(
            store,
            session_id,
            title,
            &model,
            workspace.as_deref(),
        )
    } else {
        Ok(crate::tui::session_manager::TuiSessionManager::in_memory()?)
    }
}

async fn handle_local_command(
    host: &mut CliHost,
    engine: &StreamingQueryEngine,
    message: &str,
    footer: &mut FooterRenderer,
    attachments: &mut AttachmentManager,
) -> anyhow::Result<bool> {
    match message.trim() {
        "/exit" | "/quit" | "exit" | "quit" => {
            engine
                .flush_memory_for_current_history(crate::memory::MemoryFlushReason::Exit)
                .await;
            footer.print_above(&format!("{DIM}Bye.{RESET}"))?;
            Ok(true)
        }
        "/help" | "/commands" | "/?" | "help" => {
            print_command_help();
            Ok(true)
        }
        "/help maturity" | "/commands maturity" => {
            let registry = crate::tui::commands::default_command_registry();
            println!("{}", registry.help_text_all());
            Ok(true)
        }
        "/attach" | "/attachments" => {
            print_attachments(attachments);
            Ok(true)
        }
        command if command.starts_with("/attach ") => {
            let args = command.strip_prefix("/attach ").unwrap_or("").trim();
            handle_attach_command(args, attachments)?;
            Ok(true)
        }
        command if command.starts_with("/detach ") => {
            let args = command.strip_prefix("/detach ").unwrap_or("").trim();
            handle_detach_command(args, attachments)?;
            Ok(true)
        }
        "/detach" | "/unattach" => {
            attachments.clear();
            println!("{DIM}Cleared all attachments.{RESET}");
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
            handle_resume_command(host, command).await;
            Ok(true)
        }
        "/new" => {
            let title = "New Session";
            let model = engine.model_name();
            let workspace = std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string());
            match host
                .session_manager
                .start_session(title, &model, workspace.as_deref())
            {
                Ok(session_id) => {
                    engine.set_session_id(session_id.clone());
                    println!("{GREEN}✓{RESET} Started new session {session_id}");
                }
                Err(e) => println!("{RED}✗{RESET} Failed to start session: {e}"),
            }
            Ok(true)
        }
        "/back" => {
            println!(
                "{DIM}Back navigation is a TUI feature; use /resume to switch sessions.{RESET}"
            );
            Ok(true)
        }
        "/status" => {
            print_status(engine).await;
            Ok(true)
        }
        "/provider" => {
            let response = crate::shell::slash::handle_provider(host, "").await;
            println!("{}", response);
            Ok(true)
        }
        command if command.starts_with("/provider ") => {
            let args = command.strip_prefix("/provider ").unwrap_or("").trim();
            let response = crate::shell::slash::handle_provider(host, args).await;
            println!("{}", response);
            Ok(true)
        }
        "/cost" | "/token" => {
            let response = crate::shell::slash::handle_token_cost(engine).await;
            println!("{}", response);
            Ok(true)
        }
        "/clear" => {
            engine.clear_history().await;
            print!("\x1b[2J\x1b[H");
            io::stdout().flush()?;
            Ok(true)
        }
        "/tools" => {
            let registry = engine.tool_registry();
            let tools: Vec<String> = registry
                .tool_names()
                .into_iter()
                .map(|name| format!("  {name}"))
                .collect();
            println!("{BOLD}Available tools{RESET}\n{}", tools.join("\n"));
            Ok(true)
        }
        "/permissions" => {
            let rules = engine.session_permission_rules();
            println!("{BOLD}Permission rules{RESET}");
            println!("{DIM}  always allow:{RESET} {}", rules.always_allow.len());
            println!("{DIM}  always deny:{RESET}  {}", rules.always_deny.len());
            println!("{DIM}  always ask:{RESET}   {}", rules.always_ask.len());
            Ok(true)
        }
        "/memory" => {
            println!(
                "{BOLD}Memory{RESET}\n{DIM}  use     {RESET}{}\n{DIM}  generate{RESET}{}\n{DIM}  recall  {RESET}{}",
                host.memory_use(),
                host.memory_generate(),
                host.memory_recall_mode()
            );
            Ok(true)
        }
        command if command.starts_with("/memory ") => {
            let sub = command.strip_prefix("/memory ").unwrap_or("").trim();
            match sub {
                "on" => {
                    host.set_memory_use(true);
                    host.set_memory_generate(true);
                    println!("{DIM}Memory enabled.{RESET}");
                }
                "off" => {
                    host.set_memory_use(false);
                    host.set_memory_generate(false);
                    println!("{DIM}Memory disabled.{RESET}");
                }
                "use on" => host.set_memory_use(true),
                "use off" => host.set_memory_use(false),
                "generate on" => host.set_memory_generate(true),
                "generate off" => host.set_memory_generate(false),
                _ => println!(
                    "{DIM}Usage: /memory [on|off|use on|use off|generate on|generate off]{RESET}"
                ),
            }
            Ok(true)
        }
        "/undo" => {
            let response = handle_undo(host, "");
            println!("{}", response);
            Ok(true)
        }
        "/redo" => {
            let response = handle_redo(host, "");
            println!("{}", response);
            Ok(true)
        }
        "/validate" => {
            let response = crate::shell::slash::handle_doctor(host, "").await;
            println!("{}", response);
            Ok(true)
        }
        "/diff" => {
            let response = handle_diff(host, "").await;
            println!("{}", response);
            Ok(true)
        }
        command if command.starts_with("/diff ") => {
            let args = command.strip_prefix("/diff ").unwrap_or("").trim();
            let response = handle_diff(host, args).await;
            println!("{}", response);
            Ok(true)
        }
        "/export" => {
            let response = handle_export_data(host, "").await;
            println!("{}", response);
            Ok(true)
        }
        command if command.starts_with("/export ") => {
            let args = command.strip_prefix("/export ").unwrap_or("").trim();
            let response = handle_export_data(host, args).await;
            println!("{}", response);
            Ok(true)
        }
        "/save" => {
            let response = handle_save_session(host);
            println!("{}", response);
            Ok(true)
        }
        "/doctor" => {
            let response = crate::shell::slash::handle_doctor(host, "").await;
            println!("{}", response);
            Ok(true)
        }
        "/audit" => {
            let response = crate::shell::slash::handle_audit(host, "").await;
            println!("{}", response);
            Ok(true)
        }
        command if command.starts_with("/audit ") => {
            let args = command.strip_prefix("/audit ").unwrap_or("").trim();
            let response = crate::shell::slash::handle_audit(host, args).await;
            println!("{}", response);
            Ok(true)
        }
        command if command == "/resume" || command.starts_with("/resume ") => {
            handle_resume_command(host, command).await;
            Ok(true)
        }
        "/tui" => {
            println!("{DIM}Run `pa --tui` to open the full-screen terminal interface.{RESET}");
            Ok(true)
        }
        command if command.starts_with('/') => {
            println!("{DIM}Unknown command: {command}. Use /help for available commands.{RESET}");
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn handle_resume_command(host: &mut CliHost, command: &str) {
    let args = command.strip_prefix("/resume").unwrap_or("").trim();
    let dyn_host: &mut dyn ShellHost = host;
    let response = crate::shell::slash::handle_resume(dyn_host, args).await;
    println!("{}", response);
}

fn handle_attach_command(args: &str, attachments: &mut AttachmentManager) -> anyhow::Result<()> {
    let paths: Vec<&str> = args.split_whitespace().collect();
    if paths.is_empty() {
        println!("{DIM}Usage: /attach <path> [<path> ...]{RESET}");
        return Ok(());
    }

    for path in paths {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !std::path::Path::new(trimmed).exists() {
            println!("{YELLOW}✗{RESET} {DIM}not found:{RESET} {trimmed}");
            continue;
        }
        match attachments.add_file(trimmed, AttachmentSource::File) {
            Some(token) => println!("{GREEN}✓{RESET} {DIM}attached{RESET} {}", token.label),
            None => println!("{YELLOW}·{RESET} {DIM}already attached:{RESET} {trimmed}"),
        }
    }
    Ok(())
}

fn handle_detach_command(args: &str, attachments: &mut AttachmentManager) -> anyhow::Result<()> {
    let target = args.trim();
    if target.is_empty() {
        println!("{DIM}Usage: /detach <index|path|label> or /detach all{RESET}");
        return Ok(());
    }
    if target.eq_ignore_ascii_case("all") {
        attachments.clear();
        println!("{DIM}Cleared all attachments.{RESET}");
        return Ok(());
    }

    if let Ok(index) = target.parse::<usize>() {
        if index > 0 {
            if attachments.remove_by_index(index - 1).is_some() {
                println!("{DIM}Detached attachment #{index}.{RESET}");
            } else {
                println!("{YELLOW}No attachment at index {index}.{RESET}");
            }
            return Ok(());
        }
    }

    if let Some(token) = attachments.remove_file_by_path(target) {
        println!("{DIM}Detached {}.{RESET}", token.label);
        return Ok(());
    }

    // Try matching by label.
    let labels: Vec<String> = attachments.labels();
    let lowered = target.to_lowercase();
    if let Some(idx) = labels
        .iter()
        .position(|label| label.to_lowercase() == lowered)
    {
        if attachments.remove_by_index(idx).is_some() {
            println!("{DIM}Detached {}.{RESET}", labels[idx]);
            return Ok(());
        }
    }

    println!("{YELLOW}No attachment matching '{target}'.{RESET}");
    Ok(())
}

fn print_attachments(attachments: &AttachmentManager) {
    if attachments.is_empty() {
        println!("{DIM}No attachments.{RESET}");
        return;
    }
    println!("{BOLD}Attachments{RESET}");
    for (idx, label) in attachments.labels().iter().enumerate() {
        println!("{DIM}  {:>2}. {}{RESET}", idx + 1, label);
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

async fn prompt_for_permission(
    controller: &RuntimeController,
    request: &crate::engine::conversation_loop::ToolApprovalRequest,
    tool_name: &str,
    arguments: &serde_json::Value,
    prompt_text: &str,
    footer: &mut FooterRenderer,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
) -> anyhow::Result<bool> {
    let mut overlay_text = String::new();
    overlay_text.push_str(&format!("{YELLOW}?{RESET} Permission required\n"));
    if !tool_name.is_empty() {
        overlay_text.push_str(&format!(
            "{DIM}  tool      {RESET}{}\n",
            permission_scope_summary(tool_name, arguments)
        ));
    }
    for line in prompt_text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            overlay_text.push_str(&format!("{DIM}  {trimmed}{RESET}\n"));
        }
    }

    if let Some((title, diff)) = crate::shell::permission_diff::compute_permission_diff(request) {
        overlay_text.push('\n');
        overlay_text.push_str(&format!("{DIM}{title}{RESET}\n"));
        for line in diff.lines().take(24) {
            overlay_text.push_str(&crate::shell::render::colorize_diff_line(line));
            overlay_text.push('\n');
        }
        if diff.lines().count() > 24 {
            overlay_text.push_str(&format!("{DIM}  ...{RESET}\n"));
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
        overlay_text.push_str(&format!("{DIM}  scope     {RESET}{pattern}\n"));
    }

    footer.render(
        &FooterMode::Permission(overlay_text),
        &PromptEditor::new(),
        terminal_width(),
    )?;

    let choice = loop {
        match event_rx.recv().await {
            Some(Event::Key(key)) => {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => break PermissionChoice::AllowOnce,
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        break PermissionChoice::AllowSession
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => break PermissionChoice::DenySession,
                    KeyCode::Char('n') | KeyCode::Char('N') => break PermissionChoice::DenyOnce,
                    KeyCode::Enter => break PermissionChoice::DenyOnce,
                    _ => {}
                }
            }
            _ => break PermissionChoice::DenyOnce,
        }
    };

    if let Some(pattern) = pattern.as_ref() {
        match choice {
            PermissionChoice::AllowSession => {
                controller
                    .engine()
                    .add_session_permission_rule("allow", pattern);
            }
            PermissionChoice::DenySession => {
                controller
                    .engine()
                    .add_session_permission_rule("deny", pattern);
            }
            PermissionChoice::AllowOnce | PermissionChoice::DenyOnce => {}
        }
    }

    controller.approve_pending(choice.approved()).await;
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

#[allow(dead_code)]
fn shell_history_path() -> Option<PathBuf> {
    let mut dir = dirs::data_local_dir().or_else(dirs::home_dir)?;
    dir.push("priority-agent");
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    dir.push("shell_history");
    Some(dir)
}

async fn run_turn(
    _engine: Arc<StreamingQueryEngine>,
    controller: &RuntimeController,
    stream: &mut std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>,
    footer: &mut FooterRenderer,
    interrupt: &InterruptState,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
) -> anyhow::Result<()> {
    let mut tool_runs: Vec<ToolRunView> = Vec::new();
    let mut assistant_printer = AssistantPrinter::default();

    loop {
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(StreamEvent::Start) => {}
                    Some(StreamEvent::ThinkingStart) => {
                        footer.render(&FooterMode::Thinking, &PromptEditor::new(), terminal_width())?;
                    }
                    Some(StreamEvent::ThinkingChunk(_)) => {}
                    Some(StreamEvent::ThinkingComplete) => {
                        footer.render(&FooterMode::Prompt, &PromptEditor::new(), terminal_width())?;
                    }
                    Some(StreamEvent::TextChunk(text)) => {
                        assistant_printer.push(&text, footer)?;
                    }
                    Some(StreamEvent::ToolCallStart { id, name }) => {
                        upsert_tool_run(&mut tool_runs, id, name);
                    }
                    Some(StreamEvent::ToolCallArgs { id, args_delta }) => {
                        with_tool_run(&mut tool_runs, &id, |run| run.push_args_delta(&args_delta));
                    }
                    Some(StreamEvent::ToolCallComplete { .. }) => {}
                    Some(StreamEvent::ToolExecutionStart { id, name, .. }) => {
                        assistant_printer.finish_line_if_needed(footer)?;
                        upsert_tool_run(&mut tool_runs, id.clone(), name.clone());
                        with_tool_run(&mut tool_runs, &id, |run| run.mark_running(name));
                        if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                            let desc = run.render_lines(false).join("\n");
                            footer.render(&FooterMode::ToolRunning(desc), &PromptEditor::new(), terminal_width())?;
                            footer.print_above(&format_tool_line("·", YELLOW, &run.render_lines(false).join("\n"), false))?;
                        }
                    }
                    Some(StreamEvent::ToolExecutionProgress { id, progress }) => {
                        with_tool_run(&mut tool_runs, &id, |run| run.push_progress(progress));
                        if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                            if let Some(line) = tool_progress_line(run) {
                                assistant_printer.finish_line_if_needed(footer)?;
                                footer.print_above(&format_tool_line("…", YELLOW, &line, false))?;
                                footer.render(&FooterMode::ToolRunning(line), &PromptEditor::new(), terminal_width())?;
                            }
                        }
                    }
                    Some(StreamEvent::ToolExecutionComplete { id, result, metadata, .. }) => {
                        assistant_printer.finish_line_if_needed(footer)?;
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
                            footer.print_above(&format_tool_line(marker, color, &run.render_lines(false).join("\n"), true))?;
                        }
                    }
                    Some(StreamEvent::ToolResultsReadyForModel { .. }) => {
                        footer.render(&FooterMode::Thinking, &PromptEditor::new(), terminal_width())?;
                    }
                    Some(StreamEvent::PermissionRequest { id, tool_name, arguments, prompt, metadata: _, review }) => {
                        assistant_printer.finish_line_if_needed(footer)?;
                        let request = crate::engine::conversation_loop::ToolApprovalRequest {
                            tool_call: crate::services::api::ToolCall {
                                id,
                                name: tool_name.clone(),
                                arguments: arguments.clone(),
                            },
                            prompt: prompt.clone(),
                            review: None,
                            audit: review.as_ref().map(|b| (**b).clone()),
                            diff_preview: None,
                        };
                        let approved = prompt_for_permission(controller, &request, &tool_name, &arguments, &prompt, footer, event_rx).await?;
                        footer.render(&FooterMode::Thinking, &PromptEditor::new(), terminal_width())?;
                        if !approved {
                            interrupt.request_interrupt();
                            controller.cancel().await;
                            break;
                        }
                    }
                    Some(StreamEvent::RuntimeDiagnostic { .. }) => {}
                    Some(StreamEvent::Closeout { status, evidence_summary }) => {
                        assistant_printer.finish_line_if_needed(footer)?;
                        let summary = evidence_summary.as_deref().unwrap_or("");
                        footer.print_above(&format!("{DIM}[Closeout: {status}] {summary}{RESET}"))?;
                    }
                    Some(StreamEvent::OutputTruncated) => {
                        assistant_printer.finish_line_if_needed(footer)?;
                        footer.print_above(&format!("{YELLOW}Output truncated. Continue if needed.{RESET}"))?;
                    }
                    Some(StreamEvent::Complete) => break,
                    Some(StreamEvent::Error(error)) => {
                        assistant_printer.finish_line_if_needed(footer)?;
                        footer.print_above(&format!("{RED}Error:{RESET} {error}"))?;
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }

                if interrupt.is_interrupted() {
                    controller.cancel().await;
                    break;
                }
            }
            Some(event) = event_rx.recv() => {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    if matches!((key.modifiers, key.code), (KeyModifiers::CONTROL, KeyCode::Char('c')))
                        && interrupt.request_interrupt()
                    {
                        controller.cancel().await;
                        footer.render(
                            &FooterMode::Interrupt,
                            &PromptEditor::new(),
                            terminal_width(),
                        )?;
                    }
                }

                // Check for pending user questions while a turn is running.
                if let Some(channel) = controller.engine().tool_registry().ask_channel() {
                    if let Some(answer) = crate::shell::question::run_question_ui(
                        footer, event_rx, &channel, terminal_width()
                    ).await? {
                        footer.render(
                            &FooterMode::Thinking,
                            &PromptEditor::new(),
                            terminal_width(),
                        )?;
                        let _ = answer;
                    }
                }
            }
        }
    }

    assistant_printer.finish(footer)?;
    Ok(())
}

fn format_tool_line(marker: &str, color: &str, text: &str, first_line_normal: bool) -> String {
    let mut out = String::new();
    for (idx, line) in text.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if idx == 0 {
            if first_line_normal {
                out.push_str(&format!("{color}{marker}{RESET} {line}"));
            } else {
                out.push_str(&format!("{color}{marker}{RESET} {DIM}{line}{RESET}"));
            }
        } else {
            out.push_str(&format!("{DIM}{line}{RESET}"));
        }
    }
    out
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

#[derive(Default)]
struct AssistantPrinter {
    started: bool,
    line: String,
    in_code_block: bool,
    blank_lines: usize,
}

impl AssistantPrinter {
    fn push(&mut self, text: &str, footer: &mut FooterRenderer) -> io::Result<()> {
        self.ensure_started(footer)?;
        self.line.push_str(text);

        while let Some(newline_idx) = self.line.find('\n') {
            let rest = self.line.split_off(newline_idx + 1);
            let complete = std::mem::replace(&mut self.line, rest);
            let line = complete.trim_end_matches(['\r', '\n']);
            self.print_line(line, footer)?;
        }

        io::stdout().flush()
    }

    fn finish_line_if_needed(&mut self, footer: &mut FooterRenderer) -> io::Result<()> {
        if self.started && !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            self.print_line(&line, footer)?;
        }
        Ok(())
    }

    fn finish(&mut self, footer: &mut FooterRenderer) -> io::Result<()> {
        self.finish_line_if_needed(footer)?;
        if self.started {
            footer.print_above("")?;
        }
        Ok(())
    }

    fn ensure_started(&mut self, footer: &mut FooterRenderer) -> io::Result<()> {
        if !self.started {
            footer.print_above(&format!("{CYAN}●{RESET} "))?;
            self.started = true;
        }
        Ok(())
    }

    fn print_line(&mut self, line: &str, footer: &mut FooterRenderer) -> io::Result<()> {
        let rendered = render_assistant_line(line, &mut self.in_code_block);
        if rendered.trim().is_empty() {
            if !self.started || self.blank_lines >= 1 {
                return Ok(());
            }
            self.blank_lines += 1;
            footer.print_above("")?;
        } else {
            self.blank_lines = 0;
            footer.print_above(&rendered)?;
        }
        Ok(())
    }
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
            format!("{CYAN}┌─ rust{RESET}")
        );
        assert_eq!(
            render_assistant_line("let x = 1;", &mut in_code),
            "let x = 1;"
        );
        assert_eq!(
            render_assistant_line("```", &mut in_code),
            format!("{CYAN}└─{RESET}")
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
}

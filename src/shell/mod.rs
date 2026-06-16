//! Scrollback-first interactive shell.
//!
//! This renderer intentionally avoids alternate-screen full redraws. The main
//! conversation is appended to the user's real terminal scrollback, while a
//! fixed footer at the bottom shows the prompt and transient status. This
//! matches the interaction model used by mature coding-agent CLIs more closely
//! than a dashboard-style TUI.

pub mod attachment;
pub mod completion;
pub mod completion_state;
pub mod constants;
pub mod footer;
pub mod host;
pub mod interrupt;
pub mod permission;
pub mod permission_diff;
pub mod prompt;
pub mod question;
pub mod render;
pub mod text;
pub mod theme;
pub mod turn;

pub mod slash;

use crate::components::attachment_token::AttachmentSource;
use crate::engine::runtime_controller::RuntimeController;
use crate::engine::streaming::StreamingQueryEngine;
use crate::session_store::{SessionRecord, SessionStore};
use crate::shell::attachment::AttachmentManager;
use crate::shell::completion::find_candidates;
use crate::shell::completion_state::CompletionState;
use crate::shell::constants::{
    DEFAULT_FOOTER_HEIGHT, PROMPT_PREFIX_WIDTH, SESSION_LIST_MODEL_WIDTH, SESSION_LIST_TITLE_WIDTH,
    WELCOME_MODEL_WIDTH, WELCOME_PROVIDER_WIDTH, WELCOME_WIDTH_MAX, WELCOME_WIDTH_MIN,
};
use crate::shell::footer::{AttachmentLine, FooterMode, FooterRenderer};
use crate::shell::host::{CliHost, ShellHost};
use crate::shell::interrupt::InterruptState;
use crate::shell::prompt::PromptEditor;
use crate::shell::slash::{
    handle_diff, handle_export_data, handle_redo, handle_save_session, handle_undo,
};
use crate::shell::text::{
    colored_rule, compact_home_path, compact_line, percent_bar, terminal_width,
};
use crate::shell::theme::*;
use crate::shell::turn::run_turn;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
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

    let footer_height = DEFAULT_FOOTER_HEIGHT;
    let mut editor = PromptEditor::new();
    let mut attachments = AttachmentManager::new();
    let mut completion_state: Option<CompletionState> = None;
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
                            completion_state = Some(CompletionState::new(
                                current_cursor_col(&editor),
                                candidates,
                            ));
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
                    completion_state = CompletionState::update_after_edit(
                        &editor,
                        completion_state,
                        current_cursor_col(&editor),
                    );
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
                    footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
                }
                (KeyModifiers::CONTROL, KeyCode::Right) | (KeyModifiers::ALT, KeyCode::Right) => {
                    editor.move_word_right();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
                }
                (_, KeyCode::Left) => {
                    editor.move_left();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
                }
                (_, KeyCode::Right) => {
                    editor.move_right();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
                }
                (_, KeyCode::Up) => {
                    if let Some(ref mut state) = completion_state {
                        state.select_previous();
                        render_prompt_footer_with_completion(
                            &mut footer,
                            &editor,
                            &attachments,
                            completion_state.as_ref(),
                        )?;
                    } else {
                        editor.move_up();
                        render_prompt_footer(&mut footer, &editor, &attachments)?;
                        footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
                    }
                }
                (_, KeyCode::Down) => {
                    if let Some(ref mut state) = completion_state {
                        state.select_next();
                        render_prompt_footer_with_completion(
                            &mut footer,
                            &editor,
                            &attachments,
                            completion_state.as_ref(),
                        )?;
                    } else {
                        editor.move_down();
                        render_prompt_footer(&mut footer, &editor, &attachments)?;
                        footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
                    }
                }
                (KeyModifiers::NONE, KeyCode::Tab) => {
                    if let Some(state) = completion_state.take() {
                        if let Some(candidate) = state.selected_candidate() {
                            replace_word_at_cursor(
                                &mut editor,
                                state.start_col,
                                &candidate.replacement,
                            );
                        }
                        render_prompt_footer(&mut footer, &editor, &attachments)?;
                    }
                }
                (_, KeyCode::Home) => {
                    editor.move_home();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
                }
                (_, KeyCode::End) => {
                    editor.move_end();
                    completion_state = None;
                    render_prompt_footer(&mut footer, &editor, &attachments)?;
                    footer.position_cursor(&editor, PROMPT_PREFIX_WIDTH)?;
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
        // +1 accounts for the newline that joins lines in `editor.text()`.
        if chars_seen + line_chars >= target_char_col {
            let col_in_line = target_char_col.saturating_sub(chars_seen);
            let byte_col = line
                .char_indices()
                .nth(col_in_line)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            editor.set_cursor(row, byte_col);
            return;
        }
        chars_seen += line_chars + 1;
    }
    editor.move_end();
}

fn update_completion_after_edit(
    editor: &PromptEditor,
    state: Option<CompletionState>,
) -> Option<CompletionState> {
    CompletionState::update_after_edit(editor, state, current_cursor_col(editor))
}

fn render_prompt_footer_with_completion(
    footer: &mut FooterRenderer,
    editor: &PromptEditor,
    attachments: &AttachmentManager,
    completion: Option<&CompletionState>,
) -> io::Result<()> {
    let mut line = attachments.render_pills(terminal_width().saturating_sub(2));
    if let Some(state) = completion {
        let mut comp_line = String::from("Completion: ");
        for (idx, candidate) in state.candidates.iter().take(6).enumerate() {
            if idx > 0 {
                comp_line.push_str("  ");
            }
            let marker = if idx == state.selected { ">" } else { " " };
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
    let model = compact_line(&engine.model_name(), WELCOME_MODEL_WIDTH);
    let provider = compact_line(&engine.provider_base_url(), WELCOME_PROVIDER_WIDTH);
    let mode = permission_mode_label(engine.permission_mode());
    let width = terminal_width().clamp(WELCOME_WIDTH_MIN, WELCOME_WIDTH_MAX);
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
            let response = crate::shell::slash::handle_validate(host).await;
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
            let response = handle_save_session(host).await;
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
            compact_line(&display_session_title(session), SESSION_LIST_TITLE_WIDTH),
            count,
            compact_line(&session.model, SESSION_LIST_MODEL_WIDTH),
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
    let context_bar = percent_bar(
        usage_pct.min(100),
        crate::shell::constants::STATUS_CONTEXT_BAR_WIDTH,
    );
    let memory_label = if usage.relevant_memories.is_empty() {
        "none".to_string()
    } else {
        format!("{} relevant", usage.relevant_memories.len())
    };
    let rule_count = session_rules.always_allow.len()
        + session_rules.always_deny.len()
        + session_rules.always_ask.len();
    let recent_memory = usage.relevant_memories.first().map(|m| {
        compact_line(
            &m.snippet,
            crate::shell::constants::RECENT_MEMORY_SNIPPET_WIDTH,
        )
    });

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

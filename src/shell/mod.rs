//! Interactive shell for the Priority Agent CLI.
//!
//! The default renderer uses the terminal's alternate screen buffer and redraws
//! the whole interface on every change. This avoids cursor-dance artifacts when
//! raw-mode input, streaming output, and CJK characters mix. The `--no-footer`
//! flag falls back to a plain stdout mode suitable for pipe/redirection
//! environments.

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
pub mod screen;
pub mod surface;
pub mod text;
pub mod theme;
pub mod turn;

pub mod slash;
pub mod test_support;

use crate::components::attachment_token::AttachmentSource;
use crate::engine::runtime_controller::RuntimeController;
use crate::engine::streaming::StreamingQueryEngine;
use crate::session_store::{SessionRecord, SessionStore};
use crate::shell::attachment::AttachmentManager;
use crate::shell::completion::find_candidates;
use crate::shell::completion_state::CompletionState;
use crate::shell::constants::{
    SESSION_LIST_MODEL_WIDTH, SESSION_LIST_TITLE_WIDTH, WELCOME_MODEL_WIDTH,
    WELCOME_PROVIDER_WIDTH, WELCOME_WIDTH_MAX, WELCOME_WIDTH_MIN,
};
use crate::shell::footer::FooterMode;
use crate::shell::host::{CliHost, ShellHost};
use crate::shell::interrupt::InterruptState;
use crate::shell::prompt::PromptEditor;
use crate::shell::slash::{
    handle_diff, handle_export_data, handle_redo, handle_save_session, handle_undo,
};
use crate::shell::surface::{PlainSurface, Surface};
use crate::shell::text::{colored_rule, compact_home_path, compact_line, terminal_width};
use crate::shell::theme::*;
use crate::shell::turn::run_turn;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use std::io::{self, IsTerminal};
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
    ShellCommand::new("/changes", "show recent file changes"),
    ShellCommand::new("/checkpoints", "list session checkpoints"),
    ShellCommand::new("/compact", "compact context manually"),
    ShellCommand::new("/context", "show context usage"),
    ShellCommand::new("/skills", "list installed skills"),
    ShellCommand::new("/agents", "list active agents"),
    ShellCommand::new("/tasks", "list tasks"),
    ShellCommand::new("/mcp", "show MCP server status"),
    ShellCommand::new("/tui", "open the full-screen TUI"),
    ShellCommand::new("/exit", "quit"),
];

/// Options that control how the CLI shell behaves.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShellOptions {
    /// When true, the shell runs in plain stdin/stdout mode without a fixed
    /// bottom footer. Useful for pipe/redirection environments or minimal
    /// terminals.
    pub no_footer: bool,
}

pub async fn run_shell(engine: Arc<StreamingQueryEngine>) -> anyhow::Result<()> {
    run_shell_with_options(engine, ShellOptions::default()).await
}

pub async fn run_shell_with_options(
    engine: Arc<StreamingQueryEngine>,
    options: ShellOptions,
) -> anyhow::Result<()> {
    if !io::stdin().is_terminal() {
        anyhow::bail!("CLI mode requires an interactive terminal");
    }

    crossterm::terminal::enable_raw_mode()?;
    let result = run_shell_inner(engine, options).await;
    let _ = crossterm::terminal::disable_raw_mode();
    result
}

async fn run_shell_inner(
    engine: Arc<StreamingQueryEngine>,
    options: ShellOptions,
) -> anyhow::Result<()> {
    let session_manager = build_session_manager(&engine).await?;
    let controller = RuntimeController::new(engine.clone());
    let mut host =
        CliHost::new(engine.clone(), session_manager).with_controller(controller.clone());

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    tokio::spawn(event_reader(event_tx));

    let mut editor = PromptEditor::new();
    let mut attachments = AttachmentManager::new();
    let mut completion_state: Option<CompletionState> = None;
    let interrupt = InterruptState::new();

    let welcome = render_welcome(&engine).await;
    for line in welcome.lines() {
        print!("{line}\r\n");
    }
    let _ = io::Write::flush(&mut io::stdout());
    let mut surface = ShellSurface::Plain(PlainSurface::new());

    let surface_ref = surface.as_surface();
    surface_ref.render_footer(
        &FooterMode::Prompt,
        &editor,
        &attachments,
        completion_state.as_ref(),
    )?;

    loop {
        let Some(event) = event_rx.recv().await else {
            break;
        };

        if let Event::Resize(_cols, _rows) = event {
            let _ = surface.as_surface().render_footer(
                &FooterMode::Prompt,
                &editor,
                &attachments,
                completion_state.as_ref(),
            );
            continue;
        }

        if let Event::Mouse(MouseEvent { kind, .. }) = event {
            let delta = match kind {
                MouseEventKind::ScrollUp => -3,
                MouseEventKind::ScrollDown => 3,
                _ => 0,
            };
            if delta != 0 {
                let _ = surface.as_surface().scroll_by(delta);
            }
            continue;
        }

        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Release {
                continue;
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    if interrupt.request_interrupt() {
                        controller.cancel().await;
                        surface.as_surface().render_footer(
                            &FooterMode::Interrupt,
                            &editor,
                            &attachments,
                            completion_state.as_ref(),
                        )?;
                    } else {
                        break;
                    }
                }
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    if !editor.is_empty() || !attachments.is_empty() {
                        surface.as_surface().scroll_to_bottom()?;
                        let message = editor.text();
                        editor.clear();
                        surface
                            .as_surface()
                            .push_line(&format_user_message(&message))?;

                        if handle_local_command(
                            &mut host,
                            &engine,
                            &message,
                            surface.as_surface(),
                            &mut attachments,
                        )
                        .await?
                        {
                            if matches!(message.trim(), "/exit" | "/quit" | "exit" | "quit") {
                                break;
                            }
                            surface.as_surface().render_footer(
                                &FooterMode::Prompt,
                                &editor,
                                &attachments,
                                completion_state.as_ref(),
                            )?;
                            continue;
                        }

                        let submission = attachments.build_submission(&message);
                        attachments.clear();
                        interrupt.start_turn();
                        surface.as_surface().render_footer(
                            &FooterMode::Thinking,
                            &editor,
                            &attachments,
                            completion_state.as_ref(),
                        )?;

                        let mut stream = controller.submit_stream_turn(submission.clone()).await;
                        let continuation = run_turn(
                            engine.clone(),
                            &controller,
                            &mut stream,
                            surface.as_surface(),
                            &interrupt,
                            &mut event_rx,
                            if options.no_footer {
                                None
                            } else {
                                Some(&mut editor)
                            },
                        )
                        .await?;
                        if let Some(continue_message) = continuation {
                            let mut stream = controller
                                .submit_stream_turn(format!("{submission}\n\n{continue_message}"))
                                .await;
                            run_turn(
                                engine.clone(),
                                &controller,
                                &mut stream,
                                surface.as_surface(),
                                &interrupt,
                                &mut event_rx,
                                if options.no_footer {
                                    None
                                } else {
                                    Some(&mut editor)
                                },
                            )
                            .await?;
                        }
                        interrupt.end_turn();
                        surface.as_surface().render_footer(
                            &FooterMode::Prompt,
                            &editor,
                            &attachments,
                            completion_state.as_ref(),
                        )?;
                    }
                }
                (KeyModifiers::CONTROL, KeyCode::Char('d'))
                | (KeyModifiers::NONE, KeyCode::Esc) => {
                    if editor.is_empty() && attachments.is_empty() {
                        break;
                    }
                }
                (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
                    surface.as_surface().push_line("")?;
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
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
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
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Delete) => {
                    editor.delete();
                    completion_state = update_completion_after_edit(&editor, completion_state);
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (KeyModifiers::CONTROL, KeyCode::Left) | (KeyModifiers::ALT, KeyCode::Left) => {
                    editor.move_word_left();
                    completion_state = None;
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (KeyModifiers::CONTROL, KeyCode::Right) | (KeyModifiers::ALT, KeyCode::Right) => {
                    editor.move_word_right();
                    completion_state = None;
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Left) => {
                    editor.move_left();
                    completion_state = None;
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Right) => {
                    editor.move_right();
                    completion_state = None;
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Up) => {
                    if let Some(ref mut state) = completion_state {
                        state.select_previous();
                    } else {
                        editor.move_up();
                    }
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Down) => {
                    if let Some(ref mut state) = completion_state {
                        state.select_next();
                    } else {
                        editor.move_down();
                    }
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
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
                    }
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::Home) => {
                    editor.move_home();
                    completion_state = None;
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                (_, KeyCode::End) => {
                    editor.move_end();
                    completion_state = None;
                    surface.as_surface().render_footer(
                        &FooterMode::Prompt,
                        &editor,
                        &attachments,
                        completion_state.as_ref(),
                    )?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

enum ShellSurface {
    Plain(PlainSurface),
}

impl ShellSurface {
    fn as_surface(&mut self) -> &mut dyn Surface {
        match self {
            ShellSurface::Plain(s) => s,
        }
    }
}

async fn event_reader(event_tx: tokio::sync::mpsc::UnboundedSender<Event>) {
    while let Ok(Ok(event)) = tokio::task::spawn_blocking(crossterm::event::read).await {
        if event_tx.send(event).is_err() {
            break;
        }
    }
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

async fn render_welcome(engine: &StreamingQueryEngine) -> String {
    let usage = engine.context_usage_report().await;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let dir = compact_home_path(&cwd);
    let model = compact_line(&engine.model_name(), WELCOME_MODEL_WIDTH);
    let provider = compact_line(&engine.provider_base_url(), WELCOME_PROVIDER_WIDTH);
    let mode = permission_mode_label(engine.permission_mode());
    let width = terminal_width();
    let mut out = String::new();

    if width < WELCOME_WIDTH_MIN {
        out.push_str(&format!(
            "{BLUE}Priority Agent{RESET} {DIM}coding agent{RESET}\n"
        ));
        out.push_str(&format!("{DIM}Dir{RESET} {}\n", compact_line(&dir, 40)));
        out.push_str(&format!(
            "{DIM}Model{RESET} {} {DIM}·{RESET} {}\n",
            model, provider
        ));
        out.push_str(&format!(
            "{DIM}Mode{RESET} {} {DIM}·{RESET} context {} / {}\n",
            mode, usage.total_estimated_tokens, usage.max_context_tokens
        ));
        out.push_str(&format!(
            "{DIM}/help{RESET} commands {DIM}·{RESET} /status {DIM}·{RESET} /exit\n"
        ));
        out.push('\n');
        return out;
    }

    let width = width.clamp(WELCOME_WIDTH_MIN, WELCOME_WIDTH_MAX);
    let inner = width.saturating_sub(4);

    out.push_str(&format!(
        "{BLUE}╭─{RESET} {BOLD}Priority Agent{RESET} {DIM}coding agent{RESET}{}\n",
        colored_rule(width.saturating_sub(31), BLUE)
    ));
    out.push_str(&format!(
        "{BLUE}│{RESET}  {BOLD}Welcome back.{RESET} {DIM}Ask for code changes, debugging, reviews, or project inspection.{RESET}\n"
    ));
    out.push_str(&format!("{BLUE}│{RESET}\n"));
    out.push_str(&format!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}{}\n",
        "Directory",
        compact_line(&dir, inner.saturating_sub(12))
    ));
    out.push_str(&format!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}{} {DIM}· provider {RESET}{}\n",
        "Model", model, provider
    ));
    out.push_str(&format!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}{} {DIM}· context {RESET}{} / {}\n",
        "Mode", mode, usage.total_estimated_tokens, usage.max_context_tokens
    ));
    out.push_str(&format!(
        "{BLUE}│{RESET}  {DIM}{:<10}{RESET}/help commands · /status details · /exit quit\n",
        "Shortcuts"
    ));
    out.push_str(&format!(
        "{BLUE}╰{}╯{RESET}\n",
        "─".repeat(width.saturating_sub(2))
    ));
    out.push('\n');
    out
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
    surface: &mut dyn Surface,
    attachments: &mut AttachmentManager,
) -> anyhow::Result<bool> {
    match message.trim() {
        "/exit" | "/quit" | "exit" | "quit" => {
            engine
                .flush_memory_for_current_history(crate::memory::MemoryFlushReason::Exit)
                .await;
            surface.push_line(&format!("{DIM}Bye.{RESET}"))?;
            Ok(true)
        }
        "/help" | "/commands" | "/?" | "help" => {
            print_command_help(surface)?;
            Ok(true)
        }
        "/help maturity" | "/commands maturity" => {
            let registry = crate::tui::commands::default_command_registry();
            surface.push_line(&registry.help_text_all())?;
            Ok(true)
        }
        "/attach" | "/attachments" => {
            print_attachments(surface, attachments)?;
            Ok(true)
        }
        command if command.starts_with("/attach ") => {
            let args = command.strip_prefix("/attach ").unwrap_or("").trim();
            handle_attach_command(surface, args, attachments)?;
            Ok(true)
        }
        command if command.starts_with("/detach ") => {
            let args = command.strip_prefix("/detach ").unwrap_or("").trim();
            handle_detach_command(surface, args, attachments)?;
            Ok(true)
        }
        "/detach" | "/unattach" => {
            attachments.clear();
            surface.push_line(&format!("{DIM}Cleared all attachments.{RESET}"))?;
            Ok(true)
        }
        "/model" => {
            let response = crate::shell::slash::handle_model(host, "").await;
            surface.push_line(&response)?;
            Ok(true)
        }
        command if command.starts_with("/model ") => {
            let args = command.strip_prefix("/model ").unwrap_or("").trim();
            let response = crate::shell::slash::handle_model(host, args).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/resume" => {
            let response = crate::shell::slash::handle_resume(host, "").await;
            surface.push_line(&response)?;
            Ok(true)
        }
        command if command.starts_with("/resume ") => {
            let args = command.strip_prefix("/resume ").unwrap_or("").trim();
            let response = crate::shell::slash::handle_resume(host, args).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/sessions" => {
            print_sessions(surface, engine, 20)?;
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
                    surface
                        .push_line(&format!("{GREEN}✓{RESET} Started new session {session_id}"))?;
                }
                Err(e) => {
                    surface.push_line(&format!("{RED}✗{RESET} Failed to start session: {e}"))?
                }
            }
            Ok(true)
        }
        "/back" => {
            surface.push_line(&format!(
                "{DIM}Back navigation is a TUI feature; use /resume to switch sessions.{RESET}"
            ))?;
            Ok(true)
        }
        "/status" => {
            let response = crate::shell::slash::handle_status(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/provider" => {
            let response = crate::shell::slash::handle_provider(host, "").await;
            surface.push_line(&response)?;
            Ok(true)
        }
        command if command.starts_with("/provider ") => {
            let args = command.strip_prefix("/provider ").unwrap_or("").trim();
            let response = crate::shell::slash::handle_provider(host, args).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/cost" | "/token" => {
            let response = crate::shell::slash::handle_token_cost(engine).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/clear" => {
            engine.clear_history().await;
            surface.clear()?;
            Ok(true)
        }
        "/tools" => {
            let registry = engine.tool_registry();
            let tools: Vec<String> = registry
                .tool_names()
                .into_iter()
                .map(|name| format!("  {name}"))
                .collect();
            surface.push_line(&format!(
                "{BOLD}Available tools{RESET}\n{}",
                tools.join("\n")
            ))?;
            Ok(true)
        }
        "/permissions" => {
            let rules = engine.session_permission_rules();
            surface.push_line(&format!(
                "{BOLD}Permission rules{RESET}\n{DIM}  always allow:{RESET} {}\n{DIM}  always deny:{RESET}  {}\n{DIM}  always ask:{RESET}   {}",
                rules.always_allow.len(),
                rules.always_deny.len(),
                rules.always_ask.len(),
            ))?;
            Ok(true)
        }
        "/memory" => {
            surface.push_line(&format!(
                "{BOLD}Memory{RESET}\n{DIM}  use     {RESET}{}\n{DIM}  generate{RESET}{}\n{DIM}  recall  {RESET}{}",
                host.memory_use(),
                host.memory_generate(),
                host.memory_recall_mode()
            ))?;
            Ok(true)
        }
        command if command.starts_with("/memory ") => {
            let sub = command.strip_prefix("/memory ").unwrap_or("").trim();
            match sub {
                "on" => {
                    host.set_memory_use(true);
                    host.set_memory_generate(true);
                    surface.push_line(&format!("{DIM}Memory enabled.{RESET}"))?;
                }
                "off" => {
                    host.set_memory_use(false);
                    host.set_memory_generate(false);
                    surface.push_line(&format!("{DIM}Memory disabled.{RESET}"))?;
                }
                "use on" => host.set_memory_use(true),
                "use off" => host.set_memory_use(false),
                "generate on" => host.set_memory_generate(true),
                "generate off" => host.set_memory_generate(false),
                _ => surface.push_line(&format!(
                    "{DIM}Usage: /memory [on|off|use on|use off|generate on|generate off]{RESET}"
                ))?,
            }
            Ok(true)
        }
        "/undo" => {
            let response = handle_undo(host, "");
            surface.push_line(&response)?;
            Ok(true)
        }
        "/redo" => {
            let response = handle_redo(host, "");
            surface.push_line(&response)?;
            Ok(true)
        }
        "/validate" => {
            let response = crate::shell::slash::handle_validate(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/diff" => {
            let response = handle_diff(host, "").await;
            surface.push_line(&response)?;
            Ok(true)
        }
        command if command.starts_with("/diff ") => {
            let args = command.strip_prefix("/diff ").unwrap_or("").trim();
            let response = handle_diff(host, args).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/export" => {
            let response = handle_export_data(host, "").await;
            surface.push_line(&response)?;
            Ok(true)
        }
        command if command.starts_with("/export ") => {
            let args = command.strip_prefix("/export ").unwrap_or("").trim();
            let response = handle_export_data(host, args).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/save" => {
            let response = handle_save_session(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/doctor" => {
            let response = crate::shell::slash::handle_doctor(host, "").await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/audit" => {
            let response = crate::shell::slash::handle_audit(host, "").await;
            surface.push_line(&response)?;
            Ok(true)
        }
        command if command.starts_with("/audit ") => {
            let args = command.strip_prefix("/audit ").unwrap_or("").trim();
            let response = crate::shell::slash::handle_audit(host, args).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/tui" => {
            surface.push_line(&format!(
                "{DIM}Run `pa --tui` to open the full-screen terminal interface.{RESET}"
            ))?;
            Ok(true)
        }
        "/changes" => {
            let response = crate::shell::slash::handle_changes(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/checkpoints" => {
            let response = crate::shell::slash::handle_checkpoints(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/compact" => {
            let response = crate::shell::slash::handle_compact(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/context" => {
            let response = crate::shell::slash::handle_context(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/skills" => {
            let response = crate::shell::slash::handle_skills(host);
            surface.push_line(&response)?;
            Ok(true)
        }
        "/agents" => {
            let response = crate::shell::slash::handle_agents(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/tasks" => {
            let response = crate::shell::slash::handle_tasks(host).await;
            surface.push_line(&response)?;
            Ok(true)
        }
        "/mcp" => {
            let response = crate::shell::slash::handle_mcp(host);
            surface.push_line(&response)?;
            Ok(true)
        }
        command if command.starts_with('/') => {
            surface.push_line(&format!(
                "{DIM}Unknown command: {command}. Use /help for available commands.{RESET}"
            ))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_attach_command(
    surface: &mut dyn Surface,
    args: &str,
    attachments: &mut AttachmentManager,
) -> io::Result<()> {
    let paths: Vec<&str> = args.split_whitespace().collect();
    if paths.is_empty() {
        surface.push_line(&format!("{DIM}Usage: /attach <path> [<path> ...]{RESET}"))?;
        return Ok(());
    }

    for path in paths {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !std::path::Path::new(trimmed).exists() {
            surface.push_line(&format!(
                "{YELLOW}✗{RESET} {DIM}not found:{RESET} {trimmed}"
            ))?;
            continue;
        }
        match attachments.add_file(trimmed, AttachmentSource::File) {
            Some(token) => surface.push_line(&format!(
                "{GREEN}✓{RESET} {DIM}attached{RESET} {}",
                token.label
            ))?,
            None => surface.push_line(&format!(
                "{YELLOW}·{RESET} {DIM}already attached:{RESET} {trimmed}"
            ))?,
        }
    }
    Ok(())
}

fn handle_detach_command(
    surface: &mut dyn Surface,
    args: &str,
    attachments: &mut AttachmentManager,
) -> io::Result<()> {
    let target = args.trim();
    if target.is_empty() {
        surface.push_line(&format!(
            "{DIM}Usage: /detach <index|path|label> or /detach all{RESET}"
        ))?;
        return Ok(());
    }
    if target.eq_ignore_ascii_case("all") {
        attachments.clear();
        surface.push_line(&format!("{DIM}Cleared all attachments.{RESET}"))?;
        return Ok(());
    }

    if let Ok(index) = target.parse::<usize>() {
        if index > 0 {
            if attachments.remove_by_index(index - 1).is_some() {
                surface.push_line(&format!("{DIM}Detached attachment #{index}.{RESET}"))?;
            } else {
                surface.push_line(&format!("{YELLOW}No attachment at index {index}.{RESET}"))?;
            }
            return Ok(());
        }
    }

    if let Some(token) = attachments.remove_file_by_path(target) {
        surface.push_line(&format!("{DIM}Detached {}.{RESET}", token.label))?;
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
            surface.push_line(&format!("{DIM}Detached {}.{RESET}", labels[idx]))?;
            return Ok(());
        }
    }

    surface.push_line(&format!(
        "{YELLOW}No attachment matching '{target}'.{RESET}"
    ))?;
    Ok(())
}

fn print_attachments(surface: &mut dyn Surface, attachments: &AttachmentManager) -> io::Result<()> {
    if attachments.is_empty() {
        surface.push_line(&format!("{DIM}No attachments.{RESET}"))?;
        return Ok(());
    }
    surface.push_line(&format!("{BOLD}Attachments{RESET}"))?;
    for (idx, label) in attachments.labels().iter().enumerate() {
        surface.push_line(&format!("{DIM}  {:>2}. {}{RESET}", idx + 1, label))?;
    }
    Ok(())
}

fn print_command_help(surface: &mut dyn Surface) -> io::Result<()> {
    surface.push_line(&format!("{BOLD}Commands{RESET}"))?;
    for command in LOCAL_COMMANDS {
        surface.push_line(&format!(
            "{DIM}  {:<10}{RESET}{}",
            command.name, command.description
        ))?;
    }
    surface.push_line(&format!("{DIM}  /?        {RESET}alias for /help"))?;
    surface.push_line("")?;
    surface.push_line(&format!(
        "{DIM}Tips: /resume opens prior conversations · ↑/↓ history · Tab complete slash commands{RESET}"
    ))?;
    Ok(())
}

fn print_sessions(
    surface: &mut dyn Surface,
    engine: &StreamingQueryEngine,
    limit: i64,
) -> anyhow::Result<()> {
    let Some((store, current_id)) = engine.session_binding() else {
        surface.push_line(&format!(
            "{DIM}No session store is configured for this run.{RESET}"
        ))?;
        return Ok(());
    };
    let sessions = store.list_sessions(limit)?;
    if sessions.is_empty() {
        surface.push_line(&format!("{DIM}No previous sessions found.{RESET}"))?;
        return Ok(());
    }
    print_session_list(surface, &store, &sessions, Some(&current_id))?;
    Ok(())
}

fn print_session_list(
    surface: &mut dyn Surface,
    store: &SessionStore,
    sessions: &[SessionRecord],
    current_id: Option<&str>,
) -> anyhow::Result<()> {
    surface.push_line(&format!("{BOLD}Conversations{RESET}"))?;
    for (idx, session) in sessions.iter().enumerate() {
        let count = store.message_count(&session.id).unwrap_or_default();
        let marker = if current_id == Some(session.id.as_str()) {
            "*"
        } else {
            " "
        };
        surface.push_line(&format!(
            "{DIM}{:>2}.{}{RESET} {:<42} {DIM}{:>3} msgs · {} · {}{RESET}",
            idx + 1,
            marker,
            compact_line(&display_session_title(session), SESSION_LIST_TITLE_WIDTH),
            count,
            compact_line(&session.model, SESSION_LIST_MODEL_WIDTH),
            session.updated_at
        ))?;
        surface.push_line(&format!("{DIM}    {}{RESET}", session.id))?;
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

fn permission_mode_label(mode: crate::permissions::PermissionMode) -> &'static str {
    match mode {
        crate::permissions::PermissionMode::Default => "default",
        crate::permissions::PermissionMode::AutoLowRisk => "auto-low-risk",
        crate::permissions::PermissionMode::AutoAll => "auto",
        crate::permissions::PermissionMode::ReadOnly => "read-only",
        crate::permissions::PermissionMode::Once => "once",
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
    use crate::shell::surface::TestSurface;
    use crate::shell::test_support::{test_cli_host, test_engine};
    #[tokio::test]
    async fn handle_help_command_prints_commands() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed =
            handle_local_command(&mut host, &engine, "/help", &mut surface, &mut attachments)
                .await
                .unwrap();

        assert!(consumed, "/help should be consumed");
        assert!(attachments.is_empty());
    }

    #[tokio::test]
    async fn handle_unknown_slash_command_is_consumed() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed = handle_local_command(
            &mut host,
            &engine,
            "/notacommand",
            &mut surface,
            &mut attachments,
        )
        .await
        .unwrap();

        assert!(consumed, "unknown slash commands are still consumed");
    }

    #[tokio::test]
    async fn handle_plain_message_is_not_consumed() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed =
            handle_local_command(&mut host, &engine, "hello", &mut surface, &mut attachments)
                .await
                .unwrap();

        assert!(!consumed, "plain messages should not be consumed");
    }

    #[tokio::test]
    async fn handle_exit_command_is_consumed() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed =
            handle_local_command(&mut host, &engine, "/exit", &mut surface, &mut attachments)
                .await
                .unwrap();

        assert!(consumed, "/exit should be consumed");
    }

    #[tokio::test]
    async fn handle_new_command_creates_session() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed =
            handle_local_command(&mut host, &engine, "/new", &mut surface, &mut attachments)
                .await
                .unwrap();

        assert!(consumed);
        let sid = host.session_manager.current_session_id();
        assert!(sid.is_some(), "/new should create a session");
        assert_eq!(sid, engine.current_session_id().as_deref());
    }

    #[tokio::test]
    async fn handle_clear_command_is_consumed() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed =
            handle_local_command(&mut host, &engine, "/clear", &mut surface, &mut attachments)
                .await
                .unwrap();

        assert!(consumed, "/clear should be consumed");
    }

    #[tokio::test]
    async fn handle_model_command_is_consumed() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed =
            handle_local_command(&mut host, &engine, "/model", &mut surface, &mut attachments)
                .await
                .unwrap();

        assert!(consumed, "/model should be consumed");
    }

    #[tokio::test]
    async fn handle_status_command_is_consumed() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let consumed = handle_local_command(
            &mut host,
            &engine,
            "/status",
            &mut surface,
            &mut attachments,
        )
        .await
        .unwrap();

        assert!(consumed, "/status should be consumed");
    }

    #[tokio::test]
    async fn handle_attach_and_detach_commands() {
        let engine = test_engine();
        let mut host = test_cli_host(engine.clone());
        let mut surface = TestSurface::new();
        let mut attachments = AttachmentManager::new();

        let file_path = std::env::current_dir().unwrap().join("Cargo.toml");
        let cmd = format!("/attach {}", file_path.display());
        let consumed =
            handle_local_command(&mut host, &engine, &cmd, &mut surface, &mut attachments)
                .await
                .unwrap();
        assert!(consumed, "/attach should be consumed");
        assert_eq!(attachments.count(), 1);

        let consumed = handle_local_command(
            &mut host,
            &engine,
            "/detach all",
            &mut surface,
            &mut attachments,
        )
        .await
        .unwrap();
        assert!(consumed, "/detach all should be consumed");
        assert!(attachments.is_empty());
    }
}

//! Scrollback-first interactive shell.
//!
//! This renderer intentionally avoids alternate-screen full redraws. The main
//! conversation is appended to the user's real terminal scrollback, while only
//! the short "thinking" line is transient. This matches the interaction model
//! used by mature coding-agent CLIs more closely than a dashboard-style TUI.

use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
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

const LOCAL_COMMANDS: &[ShellCommand] = &[
    ShellCommand::new("/help", "show commands"),
    ShellCommand::new("/commands", "show commands"),
    ShellCommand::new("/status", "show model and context status"),
    ShellCommand::new("/model", "show active model"),
    ShellCommand::new("/clear", "clear terminal"),
    ShellCommand::new("/exit", "quit"),
    ShellCommand::new("/tui", "open legacy full-screen UI"),
];

pub async fn run_shell(engine: Arc<StreamingQueryEngine>) -> anyhow::Result<()> {
    if !io::stdin().is_terminal() {
        anyhow::bail!("CLI mode requires an interactive terminal");
    }

    print_welcome();

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

fn print_welcome() {
    println!("{DIM}Priority Agent · /help commands · /exit quit · /tui legacy full-screen{RESET}");
    println!();
}

async fn handle_local_command(
    engine: &StreamingQueryEngine,
    message: &str,
) -> anyhow::Result<bool> {
    match message.trim() {
        "/exit" | "/quit" | "exit" | "quit" => {
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
        "/status" => {
            print_status(engine).await;
            Ok(true)
        }
        "/clear" => {
            print!("\x1b[2J\x1b[H");
            io::stdout().flush()?;
            Ok(true)
        }
        "/tui" => {
            println!("{DIM}Run `pa --tui` to open the legacy full-screen interface.{RESET}");
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
    println!("{DIM}Tips: ↑/↓ history · Tab complete slash commands · Ctrl+C interrupt line{RESET}");
}

async fn print_status(engine: &StreamingQueryEngine) {
    let usage = engine.context_usage_report().await;
    let usage_pct = if usage.max_context_tokens > 0 {
        usage.total_estimated_tokens.saturating_mul(100) / usage.max_context_tokens
    } else {
        0
    };

    println!("{BOLD}Status{RESET}");
    println!("{DIM}  model     {RESET}{}", engine.model_name());
    println!("{DIM}  provider  {RESET}{}", engine.provider_base_url());
    println!(
        "{DIM}  context   {RESET}{} / {} tokens ({}%)",
        usage.total_estimated_tokens, usage.max_context_tokens, usage_pct
    );
    println!(
        "{DIM}  history   {RESET}{} messages · {} tokens",
        usage.history_messages, usage.history_tokens
    );
    println!(
        "{DIM}  tools     {RESET}{} tools · {} schema tokens",
        usage.tool_count, usage.tool_schema_tokens
    );
    if !usage.relevant_memories.is_empty() {
        println!(
            "{DIM}  memory    {RESET}{} relevant memories",
            usage.relevant_memories.len()
        );
    }
}

fn build_line_editor() -> anyhow::Result<Editor<ShellHelper, DefaultHistory>> {
    let config = Config::builder()
        .auto_add_history(false)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut editor = Editor::<ShellHelper, DefaultHistory>::with_config(config)?;
    editor.set_helper(Some(ShellHelper::default()));
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
                    println_tool_line("·", YELLOW, &run.summary(), false);
                }
            }
            StreamEvent::ToolExecutionProgress { id, progress } => {
                with_tool_run(&mut tool_runs, &id, |run| run.push_progress(progress));
            }
            StreamEvent::ToolExecutionComplete { id, result } => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.finish_line_if_needed()?;
                with_tool_run(&mut tool_runs, &id, |run| run.mark_complete(result));
                if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                    let marker = if run.status == crate::tui::tool_view::ToolRunStatus::Failed {
                        "✗"
                    } else {
                        "✓"
                    };
                    let color = if marker == "✗" { RED } else { GREEN };
                    println_tool_line(marker, color, &run.render_lines(false).join("\n  "), true);
                }
            }
            StreamEvent::PermissionRequest { prompt, .. } => {
                clear_status_if_visible(&mut status_visible)?;
                assistant_printer.finish_line_if_needed()?;
                let approved = prompt_for_permission(&prompt)?;
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
            println!("{DIM}  {line}{RESET}");
        }
    }
}

fn prompt_for_permission(prompt: &str) -> anyhow::Result<bool> {
    println!("{YELLOW}?{RESET} Permission required");
    for line in prompt.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            println!("{DIM}  {trimmed}{RESET}");
        }
    }
    print!("{DIM}  Allow? [y/N] {RESET}");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
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
            print!("{BLUE}●{RESET} ");
            self.started = true;
        }
        Ok(())
    }

    fn print_line(&mut self, line: &str) -> io::Result<()> {
        let rendered = render_assistant_line(line, &mut self.in_code_block);
        if rendered.is_empty() {
            println!();
        } else {
            println!("{rendered}");
        }
        Ok(())
    }
}

fn render_assistant_line(line: &str, in_code_block: &mut bool) -> String {
    let trimmed = line.trim_end();
    if trimmed.trim_start().starts_with("```") {
        *in_code_block = !*in_code_block;
        return format!("{DIM}{trimmed}{RESET}");
    }

    if *in_code_block {
        return trimmed.to_string();
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
    cleaned
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
}

//! Streaming turn rendering for the CLI.
//!
//! Consumes `StreamEvent`s from `RuntimeController::submit_stream_turn` and
//! renders assistant text, tool status lines, permission prompts, and closeout
//! markers into the scrollback/footer.

use crate::engine::runtime_controller::RuntimeController;
use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use crate::shell::constants::{TOOL_PROGRESS_LATEST_WIDTH, TOOL_PROGRESS_NAME_WIDTH};
use crate::shell::footer::{FooterMode, FooterRenderer};
use crate::shell::interrupt::InterruptState;
use crate::shell::permission::prompt_for_permission;
use crate::shell::prompt::PromptEditor;
use crate::shell::render::render_assistant_line;
use crate::shell::text::{compact_line, terminal_width};
use crate::shell::theme::{CYAN, DIM, GREEN, RED, RESET, YELLOW};
use crate::tui::tool_view::{upsert_tool_run, with_tool_run, ToolRunView};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use std::io::{self, Write};
use std::sync::Arc;

pub(crate) async fn run_turn(
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
                        let approved = prompt_for_permission(
                            controller, &request, &tool_name, &arguments, &prompt,
                            footer, event_rx, terminal_width(),
                        ).await?;
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
        compact_line(&run.name, TOOL_PROGRESS_NAME_WIDTH),
        compact_line(latest, TOOL_PROGRESS_LATEST_WIDTH)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_progress_line_shows_latest_progress_compactly() {
        let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        run.push_progress("required validation still running after 30s".to_string());
        let line = tool_progress_line(&run).expect("progress line");
        assert!(line.contains("bash"));
        assert!(line.contains("30s"));
    }
}

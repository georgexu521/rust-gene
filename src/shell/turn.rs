//! Streaming turn rendering for the CLI.
//!
//! Consumes `StreamEvent`s from `RuntimeController::submit_stream_turn` and
//! renders assistant text, tool status lines, permission prompts, and closeout
//! markers into the active surface.

use crate::engine::runtime_controller::RuntimeController;
use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use crate::shell::constants::{TOOL_PROGRESS_LATEST_WIDTH, TOOL_PROGRESS_NAME_WIDTH};
use crate::shell::footer::FooterMode;
use crate::shell::prompt::PromptEditor;
use crate::shell::render::render_assistant_line;
use crate::shell::surface::Surface;
use crate::shell::theme::{CYAN, DIM, GREEN, RED, RESET, YELLOW};
use crate::tui::tool_view::{upsert_tool_run, with_tool_run, ToolRunView};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use std::io;
use std::sync::Arc;

pub(crate) async fn run_turn(
    engine: Arc<StreamingQueryEngine>,
    controller: &RuntimeController,
    stream: &mut std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>,
    surface: &mut dyn Surface,
    interrupt: &crate::shell::interrupt::InterruptState,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    continuation_editor: Option<&mut PromptEditor>,
) -> anyhow::Result<Option<String>> {
    let mut tool_runs: Vec<ToolRunView> = Vec::new();
    let mut assistant_printer = AssistantPrinter::default();

    loop {
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(StreamEvent::Start) => {}
                    Some(StreamEvent::ThinkingStart) => {
                        surface.render_footer(
                            &FooterMode::Thinking,
                            &PromptEditor::new(),
                            &crate::shell::attachment::AttachmentManager::new(),
                        None,
                        )?;
                    }
                    Some(StreamEvent::ThinkingChunk(_)) => {}
                    Some(StreamEvent::ThinkingComplete) => {
                        surface.render_footer(
                            &FooterMode::Prompt,
                            &PromptEditor::new(),
                            &crate::shell::attachment::AttachmentManager::new(),
                        None,
                        )?;
                    }
                    Some(StreamEvent::TextChunk(text)) => {
                        assistant_printer.push(&text, surface)?;
                    }
                    Some(StreamEvent::ToolCallStart { id, name }) => {
                        upsert_tool_run(&mut tool_runs, id, name);
                    }
                    Some(StreamEvent::ToolCallArgs { id, args_delta }) => {
                        with_tool_run(&mut tool_runs, &id, |run| run.push_args_delta(&args_delta));
                    }
                    Some(StreamEvent::ToolCallComplete { .. }) => {}
                    Some(StreamEvent::ToolExecutionStart { id, name, .. }) => {
                        assistant_printer.finish_line_if_needed(surface)?;
                        upsert_tool_run(&mut tool_runs, id.clone(), name.clone());
                        with_tool_run(&mut tool_runs, &id, |run| run.mark_running(name));
                        if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                            let desc = run.render_lines(false).join("\n");
                            surface.render_footer(
                                &FooterMode::ToolRunning(desc.clone()),
                                &PromptEditor::new(),
                                &crate::shell::attachment::AttachmentManager::new(),
                            None,
                            )?;
                            surface.push_line(&format_tool_line("·", YELLOW, &desc, false)
                            )?;
                        }
                    }
                    Some(StreamEvent::ToolExecutionProgress { id, progress }) => {
                        with_tool_run(&mut tool_runs, &id, |run| run.push_progress(progress));
                        if let Some(run) = tool_runs.iter().find(|run| run.id == id) {
                            if let Some(line) = tool_progress_line(run) {
                                assistant_printer.finish_line_if_needed(surface)?;
                                surface.push_line(&format_tool_line("…", YELLOW, &line, false)
                                )?;
                                surface.render_footer(
                                    &FooterMode::ToolRunning(line),
                                    &PromptEditor::new(),
                                    &crate::shell::attachment::AttachmentManager::new(),
                                None,
                                )?;
                            }
                        }
                    }
                    Some(StreamEvent::ToolExecutionComplete { id, result, metadata, .. }) => {
                        assistant_printer.finish_line_if_needed(surface)?;
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
                            surface.push_line(&format_tool_line(marker, color, &run.render_lines(false).join("\n"), true)
                            )?;
                        }
                    }
                    Some(StreamEvent::ToolResultsReadyForModel { .. }) => {
                        surface.render_footer(
                            &FooterMode::Thinking,
                            &PromptEditor::new(),
                            &crate::shell::attachment::AttachmentManager::new(),
                        None,
                        )?;
                    }
                    Some(StreamEvent::PermissionRequest { id, tool_name, arguments, prompt, metadata: _, review }) => {
                        assistant_printer.finish_line_if_needed(surface)?;
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
                        let width = surface.terminal_width();
                        let approved = crate::shell::permission::prompt_for_permission(
                            controller, &request, &tool_name, &arguments, &prompt,
                            surface, event_rx, width,
                        ).await?;
                        surface.render_footer(
                            &FooterMode::Thinking,
                            &PromptEditor::new(),
                            &crate::shell::attachment::AttachmentManager::new(),
                        None,
                        )?;
                        if !approved {
                            interrupt.request_interrupt();
                            controller.cancel().await;
                            break;
                        }
                    }
                    Some(StreamEvent::RuntimeDiagnostic { .. }) => {}
                    Some(StreamEvent::Closeout { status, evidence_summary }) => {
                        assistant_printer.finish_line_if_needed(surface)?;
                        let summary = evidence_summary.as_deref().unwrap_or("");
                        surface.push_line(&format!("{DIM}[Closeout: {status}] {summary}{RESET}"))?;
                    }
                    Some(StreamEvent::OutputTruncated) => {
                        assistant_printer.finish_line_if_needed(surface)?;
                        surface.push_line(&format!("{YELLOW}Output truncated.{RESET}"))?;
                        if let Some(ref prompt) = continuation_editor {
                            surface.render_footer(
                                &FooterMode::Prompt,
                                prompt,
                                &crate::shell::attachment::AttachmentManager::new(),
                            None,
                            )?;
                            if let Some(text) = read_single_line(surface, event_rx).await? {
                                if !text.trim().is_empty() {
                                    let summary = build_context_summary(&engine).await;
                                    return Ok(Some(format!("{}\n\n{}", text.trim(), summary)));
                                }
                            }
                        }
                    }
                    Some(StreamEvent::Complete) => break,
                    Some(StreamEvent::Error(error)) => {
                        assistant_printer.finish_line_if_needed(surface)?;
                        surface.push_line(&format!("{RED}Error:{RESET} {error}"))?;
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
                        surface.render_footer(
                            &FooterMode::Interrupt,
                            &PromptEditor::new(),
                            &crate::shell::attachment::AttachmentManager::new(),
                        None,
                        )?;
                    }
                }

                // Check for pending user questions while a turn is running.
                if let Some(channel) = controller.engine().tool_registry().ask_channel() {
                    let width = surface.terminal_width();
                    if let Some(answer) = crate::shell::question::run_question_ui(
                        surface, event_rx, &channel, width
                    ).await? {
                        surface.render_footer(
                            &FooterMode::Thinking,
                            &PromptEditor::new(),
                            &crate::shell::attachment::AttachmentManager::new(),
                        None,
                        )?;
                        let _ = answer;
                    }
                }
            }
        }
    }

    assistant_printer.finish(surface)?;
    Ok(None)
}

async fn build_context_summary(engine: &StreamingQueryEngine) -> String {
    let usage = engine.context_usage_report().await;
    format!(
        "[context summary] messages={} tokens={}/{} tools={} fingerprint={}",
        usage.history_messages,
        usage.total_estimated_tokens,
        usage.max_context_tokens,
        usage.tool_count,
        usage.stable_prefix_fingerprint
    )
}

async fn read_single_line(
    surface: &mut dyn Surface,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
) -> anyhow::Result<Option<String>> {
    let mut editor = PromptEditor::new();
    loop {
        surface.render_footer(
            &FooterMode::Prompt,
            &editor,
            &crate::shell::attachment::AttachmentManager::new(),
            None,
        )?;
        let Some(event) = event_rx.recv().await else {
            return Ok(None);
        };
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    return Ok(Some(editor.text()));
                }
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    return Ok(None);
                }
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    return Ok(None);
                }
                (_, KeyCode::Char(ch)) => {
                    editor.insert(&ch.to_string());
                }
                (_, KeyCode::Backspace) => {
                    editor.backspace();
                }
                (_, KeyCode::Delete) => {
                    editor.delete();
                }
                (_, KeyCode::Left) => editor.move_left(),
                (_, KeyCode::Right) => editor.move_right(),
                _ => {}
            }
        }
    }
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
        crate::shell::text::compact_line(&run.name, TOOL_PROGRESS_NAME_WIDTH),
        crate::shell::text::compact_line(latest, TOOL_PROGRESS_LATEST_WIDTH)
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
    fn push(&mut self, text: &str, surface: &mut dyn Surface) -> io::Result<()> {
        self.line.push_str(text);

        while let Some(newline_idx) = self.line.find('\n') {
            let rest = self.line.split_off(newline_idx + 1);
            let complete = std::mem::replace(&mut self.line, rest);
            let line = complete.trim_end_matches(['\r', '\n']);
            self.print_line(line, surface)?;
        }

        surface.flush()
    }

    fn finish_line_if_needed(&mut self, surface: &mut dyn Surface) -> io::Result<()> {
        if !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            self.print_line(&line, surface)?;
        }
        Ok(())
    }

    fn finish(&mut self, surface: &mut dyn Surface) -> io::Result<()> {
        self.finish_line_if_needed(surface)?;
        if self.started {
            surface.push_line("")?;
        }
        Ok(())
    }

    fn print_line(&mut self, line: &str, surface: &mut dyn Surface) -> io::Result<()> {
        let rendered = render_assistant_line(line, &mut self.in_code_block);
        if rendered.trim().is_empty() {
            if !self.started || self.blank_lines >= 1 {
                return Ok(());
            }
            self.blank_lines += 1;
            surface.push_line("")?;
        } else {
            self.blank_lines = 0;
            if !self.started {
                self.started = true;
                surface.push_line(&format!("{CYAN}●{RESET} {rendered}"))?;
            } else {
                surface.push_line(&rendered)?;
            }
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

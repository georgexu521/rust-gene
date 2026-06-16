//! Permission prompt UI for the CLI.
//!
//! Displays a tool-approval request in the footer, optionally with a diff
//! preview, and reads a single-key response from the user.

use crate::engine::runtime_controller::RuntimeController;
use crate::shell::constants::PERMISSION_SCOPE_MAX_LEN;
use crate::shell::footer::{FooterMode, FooterRenderer};
use crate::shell::prompt::PromptEditor;
use crate::shell::theme::{DIM, RESET, YELLOW};
use crossterm::event::{Event, KeyCode, KeyEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionChoice {
    AllowOnce,
    DenyOnce,
    AllowSession,
    DenySession,
}

impl PermissionChoice {
    pub fn approved(self) -> bool {
        matches!(self, Self::AllowOnce | Self::AllowSession)
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn prompt_for_permission(
    controller: &RuntimeController,
    request: &crate::engine::conversation_loop::ToolApprovalRequest,
    tool_name: &str,
    arguments: &serde_json::Value,
    prompt_text: &str,
    footer: &mut FooterRenderer,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    terminal_width: usize,
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
        terminal_width,
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
            return format!(
                "bash · {}",
                crate::shell::text::compact_line(cmd, PERMISSION_SCOPE_MAX_LEN)
            );
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
            return format!(
                "{tool_name} · {}",
                crate::shell::text::compact_line(path, PERMISSION_SCOPE_MAX_LEN)
            );
        }
    }
    tool_name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_choice_approval_semantics() {
        assert!(PermissionChoice::AllowOnce.approved());
        assert!(PermissionChoice::AllowSession.approved());
        assert!(!PermissionChoice::DenyOnce.approved());
        assert!(!PermissionChoice::DenySession.approved());
    }
}

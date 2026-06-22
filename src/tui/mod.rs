//! Interactive terminal CLI 模块
//!
//! 使用 ratatui 实现类似 Claude Code 的终端交互体验

pub mod app;
pub mod commands;
pub mod components;
mod input;
pub mod keybindings;
pub mod notify;
pub(crate) mod part_projection;
pub mod render_session;
pub mod runtime_panels;
pub mod screens;
pub mod session_manager;
pub mod slash_handler;
pub mod sync_store;
pub mod theme;
pub mod tool_view;
pub mod view_model;

pub use app::TuiApp;

use crate::engine::lsp::LspManager;
use crate::engine::streaming::StreamingQueryEngine;
use crate::engine::worktree::WorktreeManager;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseEventKind,
};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear},
    Frame, Terminal,
};
use std::io;
use std::sync::Arc;
use tracing::{debug, info};

/// 主交互式 CLI 运行函数
pub async fn run_tui(
    engine: Option<Arc<StreamingQueryEngine>>,
    lsp_manager: Option<Arc<LspManager>>,
    worktree_manager: Option<Arc<WorktreeManager>>,
) -> anyhow::Result<()> {
    info!("Starting interactive terminal CLI...");

    // 初始化终端
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        EnableBracketedPaste
    )?;

    // 设置 panic hook：panic 时自动恢复终端，防止卡终端
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
            DisableBracketedPaste
        );
        default_panic_hook(info);
    }));

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建应用状态（传入流式引擎、LSP 管理器和 Worktree 管理器）
    let mut app = TuiApp::with_engine(engine, lsp_manager, worktree_manager);

    // 会话启动时扫描工作目录并缓存文件元数据
    let worktree_dir = if let Some(ref wt) = app.worktree_manager {
        wt.current_worktree().await.unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
    } else {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    };
    let cache = crate::tools::file_cache::GLOBAL_FILE_CACHE.clone();
    tokio::task::spawn_blocking(move || {
        crate::tools::file_cache::scan_project(&cache, &worktree_dir, true);
    });

    // 主循环
    let result = run_app(&mut terminal, &mut app).await;

    // 清理资源：关闭 LSP 客户端
    if let Some(ref lsp) = app.lsp_manager {
        lsp.shutdown().await;
    }

    // 恢复默认 panic hook（避免离开 TUI 后还持有终端恢复逻辑）
    let _ = std::panic::take_hook();

    // 恢复终端
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    result
}

/// 应用主循环
async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut TuiApp) -> anyhow::Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(250);

    loop {
        if let Some(reason) = app.provider_wait_timeout_reason() {
            app.timeout_active_run_immediate(&reason);
        }
        // 绘制界面
        terminal.draw(|f| draw_ui(f, app))?;

        // 处理事件
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if input::handle_key_event(key, app).await? {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => app.scroll_up(),
                    MouseEventKind::ScrollDown => app.scroll_down(),
                    _ => {}
                },
                Event::Paste(text) => app.insert_paste(text),
                _ => {}
            }
        }

        // 定时更新
        if last_tick.elapsed() >= tick_rate {
            if tokio::time::timeout(std::time::Duration::from_millis(100), app.on_tick())
                .await
                .is_err()
            {
                tracing::warn!("TUI tick exceeded 100ms; yielding to input loop");
            }
            last_tick = std::time::Instant::now();
        }
    }
}

/// 绘制 UI
fn draw_ui(f: &mut Frame, app: &mut TuiApp) {
    match app.mode {
        app::AppMode::Settings => {
            // 设置模式
            if let Some(ref settings_state) = app.settings_state {
                components::settings::render_settings(f, settings_state, f.area(), &app.theme);
            }
        }
        app::AppMode::PlanApproval => {
            // 计划审批模式：先渲染底层聊天界面，再叠加审批弹窗
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(5),
                    Constraint::Length(1),
                ])
                .split(f.area());

            screens::main_screen::render_chat_area(f, app, main_chunks[0]);
            screens::main_screen::render_input_area(f, app, main_chunks[1]);
            screens::main_screen::render_status_bar(f, app, main_chunks[2]);

            if let Some(ref plan) = app.pending_plan {
                screens::main_screen::render_plan_approval(f, plan, f.area(), &app.theme);
            }
        }
        app::AppMode::PermissionApproval => {
            // 权限审批模式：先渲染底层聊天界面，再叠加审批弹窗
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(5),
                    Constraint::Length(1),
                ])
                .split(f.area());

            screens::main_screen::render_chat_area(f, app, main_chunks[0]);
            screens::main_screen::render_input_area(f, app, main_chunks[1]);
            screens::main_screen::render_status_bar(f, app, main_chunks[2]);

            if let Some(ref req) = app.pending_permission_request {
                screens::main_screen::render_permission_approval(f, req, f.area(), &app.theme);
            }
        }
        app::AppMode::AskUser => {
            // 用户问答模式：先渲染底层聊天界面，再叠加问答弹窗
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(5),
                    Constraint::Length(1),
                ])
                .split(f.area());

            screens::main_screen::render_chat_area(f, app, main_chunks[0]);
            screens::main_screen::render_input_area(f, app, main_chunks[1]);
            screens::main_screen::render_status_bar(f, app, main_chunks[2]);

            if let Some(ref question) = app.pending_question {
                screens::main_screen::render_ask_user(
                    f,
                    question,
                    &app.pending_question_options,
                    f.area(),
                );
            }
        }
        app::AppMode::Chat
        | app::AppMode::VimNormal
        | app::AppMode::CommandPalette
        | app::AppMode::ShortcutHelp
        | app::AppMode::PromptHistory
        | app::AppMode::ModelSelect
        | app::AppMode::ProviderSelect
        | app::AppMode::ConnectWizard
        | app::AppMode::FilePicker
        | app::AppMode::WorkspaceSwitcher => {
            if app.sidebar_visible {
                match sidebar_layout(f.area()) {
                    SidebarLayout::Inline { sidebar, main } => {
                        screens::main_screen::render_sidebar(f, app, sidebar);

                        let main_chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Min(3),
                                Constraint::Length(5),
                                Constraint::Length(1),
                            ])
                            .split(main);

                        screens::main_screen::render_chat_area(f, app, main_chunks[0]);
                        screens::main_screen::render_input_area(f, app, main_chunks[1]);
                        screens::main_screen::render_status_bar(f, app, main_chunks[2]);
                    }
                    SidebarLayout::Overlay { sidebar, main } => {
                        let main_chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Min(3),
                                Constraint::Length(5),
                                Constraint::Length(1),
                            ])
                            .split(main);

                        screens::main_screen::render_chat_area(f, app, main_chunks[0]);
                        screens::main_screen::render_input_area(f, app, main_chunks[1]);
                        screens::main_screen::render_status_bar(f, app, main_chunks[2]);
                        f.render_widget(Clear, overlay_backdrop_area(sidebar, main));
                        screens::main_screen::render_sidebar(f, app, sidebar);
                    }
                }
            } else {
                // 主布局：垂直分为消息区和输入区
                let main_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(3),
                        Constraint::Length(5),
                        Constraint::Length(1),
                    ])
                    .split(f.area());

                screens::main_screen::render_chat_area(f, app, main_chunks[0]);
                screens::main_screen::render_input_area(f, app, main_chunks[1]);
                screens::main_screen::render_status_bar(f, app, main_chunks[2]);
            }

            match app.mode {
                app::AppMode::CommandPalette => {
                    screens::main_screen::render_command_palette(f, app, f.area());
                }
                app::AppMode::ShortcutHelp => {
                    screens::main_screen::render_shortcut_help(f, app, f.area());
                }
                app::AppMode::PromptHistory => {
                    screens::main_screen::render_prompt_picker(f, app, f.area());
                }
                app::AppMode::ModelSelect => {
                    screens::main_screen::render_model_select(f, app, f.area());
                }
                app::AppMode::ProviderSelect => {
                    screens::main_screen::render_provider_select(f, app, f.area());
                }
                app::AppMode::ConnectWizard => {
                    screens::main_screen::render_connect_wizard(f, app, f.area());
                }
                app::AppMode::FilePicker => {
                    screens::main_screen::render_file_picker(f, app, f.area());
                }
                app::AppMode::WorkspaceSwitcher => {
                    screens::main_screen::render_workspace_switcher(f, app, f.area());
                }
                _ => {}
            }
        }
        app::AppMode::Onboarding => {
            // 引导模式：全屏渲染引导弹窗
            if let Some(ref state) = app.onboarding_state {
                screens::main_screen::render_onboarding(f, state, f.area(), &app.theme);
            }
        }
        app::AppMode::DiffViewer => {
            // Diff 查看器模式：先渲染底层聊天界面，再叠加 Diff 弹窗
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(5),
                    Constraint::Length(1),
                ])
                .split(f.area());

            screens::main_screen::render_chat_area(f, app, main_chunks[0]);
            screens::main_screen::render_input_area(f, app, main_chunks[1]);
            screens::main_screen::render_status_bar(f, app, main_chunks[2]);

            let (total_lines, hunk_count) = components::diff_viewer::render_diff_viewer(
                f,
                &app.diff_content,
                &app.diff_title,
                app.diff_scroll_offset,
                f.area(),
                &app.theme,
            );
            let _ = (total_lines, hunk_count); // consumed by status bar updates
        }
        app::AppMode::ToolViewer => {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(5),
                    Constraint::Length(1),
                ])
                .split(f.area());

            screens::main_screen::render_chat_area(f, app, main_chunks[0]);
            screens::main_screen::render_input_area(f, app, main_chunks[1]);
            screens::main_screen::render_status_bar(f, app, main_chunks[2]);

            screens::main_screen::render_tool_viewer(f, app, f.area());
        }
        app::AppMode::MessageSearch => {
            // 消息搜索模式：先渲染底层聊天界面，再叠加搜索弹窗
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(5),
                    Constraint::Length(1),
                ])
                .split(f.area());

            screens::main_screen::render_chat_area(f, app, main_chunks[0]);
            screens::main_screen::render_input_area(f, app, main_chunks[1]);
            screens::main_screen::render_status_bar(f, app, main_chunks[2]);

            screens::main_screen::render_message_search(f, app, f.area());
        }
    }
}

fn overlay_backdrop_area(
    sidebar: ratatui::layout::Rect,
    main: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    ratatui::layout::Rect {
        x: main.x,
        y: sidebar.y,
        width: main.width,
        height: sidebar.height,
    }
}

const INLINE_SIDEBAR_MIN_WIDTH: u16 = 140;
const INLINE_SIDEBAR_WIDTH: u16 = 40;
const OVERLAY_SIDEBAR_MAX_WIDTH: u16 = 44;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarLayout {
    Inline {
        sidebar: ratatui::layout::Rect,
        main: ratatui::layout::Rect,
    },
    Overlay {
        sidebar: ratatui::layout::Rect,
        main: ratatui::layout::Rect,
    },
}

fn sidebar_layout(area: ratatui::layout::Rect) -> SidebarLayout {
    if area.width >= INLINE_SIDEBAR_MIN_WIDTH {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(INLINE_SIDEBAR_WIDTH),
                Constraint::Min(80),
            ])
            .split(area);
        SidebarLayout::Inline {
            sidebar: chunks[0],
            main: chunks[1],
        }
    } else {
        let width = if area.width <= 28 {
            area.width
        } else {
            area.width
                .saturating_sub(4)
                .clamp(24, OVERLAY_SIDEBAR_MAX_WIDTH)
        };
        let height = if area.height > 8 {
            area.height.saturating_sub(6)
        } else {
            area.height
        };
        SidebarLayout::Overlay {
            sidebar: ratatui::layout::Rect {
                x: area.x,
                y: area.y,
                width,
                height,
            },
            main: area,
        }
    }
}

/// 工具函数：创建带标题的块
pub fn titled_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::DarkGray))
}

#[cfg(test)]
mod tests {
    use super::input::handle_key_event;
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::Path;
    use unicode_width::UnicodeWidthStr;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn render_ui_text(app: &mut TuiApp, width: u16, height: u16) -> String {
        render_ui_lines(app, width, height).join("\n")
    }

    fn render_ui_lines(app: &mut TuiApp, width: u16, height: u16) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_ui(frame, app))
            .expect("render ui");
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| rendered_buffer_line(buffer, width, y))
            .collect()
    }

    fn rendered_cell_fg_for_text(
        app: &mut TuiApp,
        width: u16,
        height: u16,
        needle: &str,
    ) -> Option<ratatui::style::Color> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_ui(frame, app))
            .expect("render ui");
        let buffer = terminal.backend().buffer();
        for y in 0..height {
            let line = rendered_buffer_line(buffer, width, y);
            if let Some(byte_idx) = line.find(needle) {
                let x = UnicodeWidthStr::width(&line[..byte_idx]) as u16;
                return Some(buffer[(x, y)].fg);
            }
        }
        None
    }

    fn rendered_buffer_line(buffer: &ratatui::buffer::Buffer, width: u16, y: u16) -> String {
        let mut line = String::new();
        let mut x = 0u16;
        while x < width {
            let symbol = buffer[(x, y)].symbol();
            line.push_str(symbol);
            let symbol_width = UnicodeWidthStr::width(symbol).max(1) as u16;
            x = x.saturating_add(symbol_width);
        }
        line.trim_end().to_string()
    }

    fn write_snapshot_if_requested(name: &str, lines: &[String]) {
        let Ok(dir) = std::env::var("PRIORITY_AGENT_TUI_SNAPSHOT_DIR") else {
            return;
        };
        let dir = Path::new(&dir);
        std::fs::create_dir_all(dir).expect("create snapshot directory");
        let normalized = normalize_snapshot_lines(lines);
        std::fs::write(dir.join(format!("{name}.txt")), normalized.join("\n"))
            .expect("write snapshot");
    }

    fn normalize_snapshot_lines(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .map(|line| normalize_snapshot_line(line))
            .collect()
    }

    fn normalize_snapshot_line(line: &str) -> String {
        let mut normalized = String::with_capacity(line.len());
        let mut rest = line;
        while let Some(idx) = rest.find("sess_") {
            normalized.push_str(&rest[..idx]);
            let candidate = &rest[idx..];
            let suffix_len = candidate["sess_".len()..]
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric())
                .map(char::len_utf8)
                .sum::<usize>();
            if suffix_len == 0 {
                normalized.push_str("sess_");
                rest = &candidate["sess_".len()..];
            } else {
                normalized.push_str("sess_demo");
                rest = &candidate["sess_".len() + suffix_len..];
            }
        }
        normalized.push_str(rest);
        normalized
    }

    fn assert_snapshot_display_width(lines: &[String], width: u16, height: u16) {
        assert_eq!(lines.len(), usize::from(height));
        for (line_index, line) in lines.iter().enumerate() {
            let display_width = UnicodeWidthStr::width(line.as_str());
            assert!(
                display_width <= usize::from(width),
                "{}x{} snapshot line {} has display width {}: {:?}",
                width,
                height,
                line_index + 1,
                display_width,
                line
            );
        }
    }

    fn assert_no_sidebar_overlay_bleed(lines: &[String], width: u16, height: u16) {
        if !matches!(
            sidebar_layout(ratatui::layout::Rect {
                x: 0,
                y: 0,
                width,
                height,
            }),
            SidebarLayout::Overlay { .. }
        ) {
            return;
        }

        for line in lines {
            let trimmed_start = line.trim_start();
            if !trimmed_start.starts_with('│') {
                continue;
            }
            let Some(right_border) = line.rfind('│') else {
                continue;
            };
            assert!(
                line[right_border + '│'.len_utf8()..]
                    .chars()
                    .all(char::is_whitespace),
                "sidebar overlay should not leave transcript text after its right border: {line:?}"
            );
        }
    }

    fn opencode_alignment_fixture() -> TuiApp {
        let mut app = TuiApp::new();
        app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
        let _session_id = app
            .session_manager
            .start_session("TUI visual review", "deepseek-v4-flash", None)
            .unwrap();
        app.session_manager
            .add_message(
                crate::state::MessageRole::User,
                "Please inspect the current TUI layout.",
            )
            .unwrap();
        app.session_manager
            .add_message(
                crate::state::MessageRole::Assistant,
                "I am checking the project status and terminal layout.",
            )
            .unwrap();
        app.sidebar_visible = true;
        app.is_querying = true;
        app.facade_snapshot.provider_request.phase =
            crate::engine::runtime_facade::ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family = Some("openai_compatible".into());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".into());
        app.facade_snapshot.provider_request.elapsed_ms = 3_200;
        app.history.push_back("cargo check current TUI".to_string());
        app.prompt_stash = Some("review the latest TUI run".to_string());
        app.composer.add_file(
            "Cargo.toml",
            crate::components::attachment_token::AttachmentSource::File,
        );
        app.composer
            .text
            .insert_str("Check the TUI layout and report any regressions.");
        app.messages.push(crate::state::MessageItem {
            id: "user_snapshot".to_string(),
            role: crate::state::MessageRole::User,
            content: "Please inspect the current TUI layout.".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        });
        app.messages.push(crate::state::MessageItem {
            id: "assistant_snapshot".to_string(),
            role: crate::state::MessageRole::Assistant,
            content: "<think>internal reasoning should not dominate the screen</think>\nI am checking the project status and terminal layout.".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: [
                ("model".to_string(), "deepseek-v4-flash".to_string()),
                ("total_tokens".to_string(), "1889".to_string()),
                ("elapsed_ms".to_string(), "2700".to_string()),
            ]
            .into_iter()
            .collect(),
        });

        app.sync_snapshot = {
            let mut store = crate::tui::sync_store::TuiSyncStore::new();
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                    message_id: Some("assistant_snapshot".to_string()),
                    text: app.messages.last().unwrap().content.clone(),
                    streaming: false,
                },
            );
            store.mark_completed();
            store.snapshot()
        };

        let mut running = crate::tui::tool_view::ToolRunView::new(
            "tool_snapshot_check".to_string(),
            "bash".to_string(),
        );
        running.push_args_delta(r#"{"command":"cargo check -q"}"#);
        running.mark_running("bash".to_string());
        running.push_progress("waiting for cargo metadata".to_string());
        app.sync_snapshot
            .set_tool_runs_for_message("user_snapshot_tool".to_string(), vec![running]);
        app.runtime_state_snapshot
            .tool_uses
            .push(crate::state::RuntimeToolUse {
                id: "tool_snapshot_check".to_string(),
                name: "bash".to_string(),
                summary: "Running cargo".to_string(),
                status: crate::state::RuntimeToolStatus::Running,
                active: true,
                arguments: None,
                latest_progress: Some("waiting for cargo metadata".to_string()),
                result_preview: None,
                elapsed_ms: Some(1_200),
                operation_kind: None,
                ui_render_kind: None,
                read_only: None,
                concurrency_safe: None,
                destructive: None,
                input_paths: Vec::new(),
                transcript_summary: Some("[Shell] Running cargo".to_string()),
            });
        app
    }

    fn completed_tool_turn_fixture() -> TuiApp {
        let mut app = TuiApp::new();
        app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
        let _session_id = app
            .session_manager
            .start_session("Cargo validation pass", "deepseek-v4-flash", None)
            .unwrap();
        app.session_manager
            .add_message(
                crate::state::MessageRole::User,
                "Run cargo check and summarize the result.",
            )
            .unwrap();
        app.session_manager
            .add_message(
                crate::state::MessageRole::Assistant,
                "cargo check completed successfully.",
            )
            .unwrap();
        app.sidebar_visible = false;
        app.facade_snapshot.provider_request.provider_family = Some("openai_compatible".into());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".into());
        app.history.push_back("cargo check -q".to_string());
        app.messages.push(crate::state::MessageItem {
            id: "user_completed_tool".to_string(),
            role: crate::state::MessageRole::User,
            content: "Run cargo check and summarize the result.".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        });

        let mut completed = crate::tui::tool_view::ToolRunView::new(
            "tool_completed_check".to_string(),
            "bash".to_string(),
        );
        completed.push_args_delta(r#"{"command":"cargo check -q"}"#);
        completed.mark_running("bash".to_string());
        completed.mark_complete("Result: OK\ncargo check finished successfully".to_string());
        app.messages.push(crate::state::MessageItem {
            id: "assistant_completed_tool".to_string(),
            role: crate::state::MessageRole::Assistant,
            content: "Done. `cargo check -q` completed successfully, and there are no compile errors in this validation pass.".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: [
                ("model_label".to_string(), "deepseek-v4-flash".to_string()),
                ("completion_tokens".to_string(), "42".to_string()),
                ("elapsed_ms".to_string(), "1800".to_string()),
                ("tool_count".to_string(), "1".to_string()),
                ("validation_status".to_string(), "passed".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        app.sync_snapshot = {
            let mut store = crate::tui::sync_store::TuiSyncStore::new();
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::TurnStarted {
                    user_message_id: "user_completed_tool".to_string(),
                    assistant_message_id: "assistant_completed_tool".to_string(),
                },
            );
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::ToolCallStarted {
                    message_id: Some("user_completed_tool".to_string()),
                    tool_call_id: "tool_completed_check".to_string(),
                    tool_name: "bash".to_string(),
                },
            );
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::ToolArgumentsDelta {
                    tool_call_id: "tool_completed_check".to_string(),
                    arguments_delta: r#"{"command":"cargo check -q"}"#.to_string(),
                },
            );
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::ToolExecutionStarted {
                    message_id: Some("user_completed_tool".to_string()),
                    tool_call_id: "tool_completed_check".to_string(),
                    tool_name: "bash".to_string(),
                    metadata: None,
                },
            );
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::ToolExecutionCompleted {
                    tool_call_id: "tool_completed_check".to_string(),
                    result: "Result: OK
cargo check finished successfully"
                        .to_string(),
                    metadata: None,
                    result_data: None,
                },
            );
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                    message_id: Some("assistant_completed_tool".to_string()),
                    text: app.messages.last().unwrap().content.clone(),
                    streaming: false,
                },
            );
            store.apply_projection_event(&crate::session_store::SessionProjectionEvent::Completed);
            store.snapshot()
        };
        app
    }

    fn provider_failure_turn_fixture() -> TuiApp {
        let mut app = TuiApp::new();
        app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
        let _session_id = app
            .session_manager
            .start_session("DeepSeek provider failure", "deepseek-v4-flash", None)
            .unwrap();
        app.session_manager
            .add_message(crate::state::MessageRole::User, "你好")
            .unwrap();
        app.session_manager
            .add_message(
                crate::state::MessageRole::Assistant,
                "[Error: Failed to get response from deepseek API]",
            )
            .unwrap();
        app.sidebar_visible = false;
        app.facade_snapshot.provider_request.provider_family = Some("openai_compatible".into());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".into());
        app.history.push_back("你好".to_string());
        app.messages.push(crate::state::MessageItem {
            id: "user_provider_failure".to_string(),
            role: crate::state::MessageRole::User,
            content: "你好".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        });
        app.messages.push(crate::state::MessageItem {
            id: "assistant_provider_failure".to_string(),
            role: crate::state::MessageRole::Assistant,
            content: "[Error: Failed to get response from deepseek API]".to_string(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: [
                ("model_label".to_string(), "deepseek-v4-flash".to_string()),
                ("provider_phase".to_string(), "provider error".to_string()),
            ]
            .into_iter()
            .collect(),
        });
        app.sync_snapshot = {
            let mut store = crate::tui::sync_store::TuiSyncStore::new();
            store.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                    message_id: Some("assistant_provider_failure".to_string()),
                    text: app.messages.last().unwrap().content.clone(),
                    streaming: false,
                },
            );
            store.snapshot()
        };
        app
    }

    fn empty_state_fixture() -> TuiApp {
        let mut app = TuiApp::new();
        app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
        let _session_id = app
            .session_manager
            .start_session("Empty state", "deepseek-v4-flash", None)
            .unwrap();
        app.facade_snapshot.provider_request.provider_family = Some("openai_compatible".into());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".into());
        app
    }

    fn sidebar_sessions_fixture() -> TuiApp {
        let mut app = TuiApp::new();
        app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
        let current_id = app
            .session_manager
            .start_session("Current work", "deepseek-v4-flash", Some("/tmp"))
            .unwrap();
        app.session_manager
            .add_message(crate::state::MessageRole::User, "Current task")
            .unwrap();
        let _other_id = app
            .session_manager
            .start_session("Other project", "claude-sonnet", Some("/home"))
            .unwrap();
        app.session_manager
            .add_message(crate::state::MessageRole::User, "Other task")
            .unwrap();
        app.session_manager.switch_to_session(&current_id).unwrap();
        app.sidebar_visible = true;
        app.sidebar_selected = 0;
        app.workspace = crate::workspace::Workspace::detect(std::path::Path::new("/tmp"));
        app
    }

    fn composer_with_attachments_fixture() -> TuiApp {
        let mut app = TuiApp::new();
        app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
        let _session_id = app
            .session_manager
            .start_session("Composer test", "deepseek-v4-flash", None)
            .unwrap();
        app.facade_snapshot.provider_request.provider_family = Some("openai_compatible".into());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".into());
        app.composer.add_file(
            "Cargo.toml",
            crate::components::attachment_token::AttachmentSource::File,
        );
        app.composer.add_file(
            "src/main.rs",
            crate::components::attachment_token::AttachmentSource::File,
        );
        app.composer
            .text
            .insert_str("Check these files and report any issues.");
        app
    }

    #[test]
    fn empty_state_snapshot_is_clean_and_hides_raw_provider_names() {
        for (width, height) in [(80, 24), (120, 35)] {
            let mut app = empty_state_fixture();
            let lines = render_ui_lines(&mut app, width, height);
            let rendered = lines.join("\n");
            write_snapshot_if_requested(&format!("empty-state-{width}x{height}"), &lines);

            assert_snapshot_display_width(&lines, width, height);
            assert!(!rendered.contains("openai_compatible"));
            assert!(!rendered.contains("async_openai::error"));
            assert!(!rendered.contains("failed deserialization of"));
            assert!(rendered.contains("? shortcuts"));
            assert!(
                rendered.contains("DeepSeek / deepseek-v4-flash")
                    || rendered.contains("deepseek-v4-flash")
            );
        }
    }

    #[test]
    fn sidebar_sessions_snapshot_groups_by_workspace_and_shows_status() {
        let mut app = sidebar_sessions_fixture();
        let lines = render_ui_lines(&mut app, 40, 20);
        let rendered = lines.join("\n");
        write_snapshot_if_requested("sidebar-sessions-40x20", &lines);

        assert_snapshot_display_width(&lines, 40, 20);
        assert!(rendered.contains("Current work"));
        assert!(rendered.contains("Other project"));
        assert!(rendered.contains("deepseek-v4"));
        assert!(rendered.contains("claude-sonnet"));
        assert!(rendered.contains("●") || rendered.contains("◆") || rendered.contains("○"));
    }

    #[test]
    fn composer_with_attachments_snapshot_shows_pills_and_prompt() {
        let mut app = composer_with_attachments_fixture();
        let lines = render_ui_lines(&mut app, 120, 20);
        let rendered = lines.join("\n");
        write_snapshot_if_requested("composer-with-attachments-120x20", &lines);

        assert_snapshot_display_width(&lines, 120, 20);
        assert!(rendered.contains("Cargo.toml"));
        assert!(rendered.contains("src/main.rs"));
        assert!(rendered.contains("Check these files"));
        assert!(rendered.contains("files:2"));
        assert!(!rendered.contains("[[file:"));
    }

    #[test]
    fn snapshot_normalization_replaces_volatile_session_ids() {
        let lines = vec![
            "◈ sess_abc · deepseek-v4-flash · auto".to_string(),
            "● auto · sess_123def · openai_compatible".to_string(),
            "no session marker here".to_string(),
        ];

        let normalized = normalize_snapshot_lines(&lines).join("\n");

        assert!(normalized.contains("sess_demo · deepseek-v4-flash"));
        assert!(normalized.contains("● auto · sess_demo · openai_compatible"));
        assert!(normalized.contains("no session marker here"));
        assert!(!normalized.contains("sess_abc"));
        assert!(!normalized.contains("sess_123def"));
    }

    #[test]
    fn snapshot_width_assertion_uses_terminal_display_width() {
        let cjk_wide_line = vec!["你好abc".to_string()];

        assert!(std::panic::catch_unwind(|| {
            assert_snapshot_display_width(&cjk_wide_line, 5, 1)
        })
        .is_err());
        assert_snapshot_display_width(&cjk_wide_line, 7, 1);
    }

    #[test]
    fn rendered_snapshot_lines_do_not_insert_placeholder_spaces_after_cjk() {
        let mut app = provider_failure_turn_fixture();

        let rendered = render_ui_text(&mut app, 100, 30);

        assert!(rendered.contains("你好"));
        assert!(!rendered.contains("你 好"));
    }

    #[tokio::test]
    async fn shortcut_help_slash_enters_filter_mode() {
        let mut app = TuiApp::new();
        app.open_shortcut_help();

        handle_key_event(key(KeyCode::Char('/')), &mut app)
            .await
            .unwrap();
        assert_eq!(app.mode, app::AppMode::ShortcutHelp);
        assert!(app.filtering_shortcut_help);

        handle_key_event(key(KeyCode::Char('d')), &mut app)
            .await
            .unwrap();
        assert_eq!(app.shortcut_help_filter, "d");
        assert_eq!(app.mode, app::AppMode::ShortcutHelp);
    }

    #[tokio::test]
    async fn ctrl_c_cancels_active_query_without_quitting() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.messages.push(crate::state::MessageItem {
            id: "assistant_active".to_string(),
            role: crate::state::MessageRole::Assistant,
            content: String::new(),
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            metadata: Default::default(),
        });

        let should_quit = handle_key_event(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut app,
        )
        .await
        .unwrap();

        assert!(!should_quit);
        assert!(!app.is_querying);
        assert_eq!(app.mode, app::AppMode::Chat);
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("[Cancelled: Run interrupted]"));
    }

    #[tokio::test]
    async fn ctrl_tab_switches_sidebar_panel_before_plain_tab_collapse() {
        let mut app = TuiApp::new();
        assert_eq!(app.sidebar_panel, app::SidebarPanel::Sessions);

        handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL), &mut app)
            .await
            .unwrap();

        assert_eq!(app.sidebar_panel, app::SidebarPanel::Context);
    }

    #[tokio::test]
    async fn plain_tab_does_not_switch_sidebar_panel() {
        let mut app = TuiApp::new();
        assert_eq!(app.sidebar_panel, app::SidebarPanel::Sessions);

        handle_key_event(key(KeyCode::Tab), &mut app).await.unwrap();

        assert_eq!(app.sidebar_panel, app::SidebarPanel::Sessions);
    }

    #[tokio::test]
    async fn chat_backspace_removes_last_attachment_when_input_empty() {
        let mut app = TuiApp::new();
        app.attach_context_path("Cargo.toml").unwrap();

        handle_key_event(key(KeyCode::Backspace), &mut app)
            .await
            .unwrap();

        assert_eq!(app.composer_attachment_count(), 0);
        assert!(app
            .toasts
            .iter()
            .any(|toast| toast.message.contains("Removed attachment: Cargo.toml")));
    }

    #[tokio::test]
    async fn chat_backspace_keeps_attachments_when_editing_text() {
        let mut app = TuiApp::new();
        app.attach_context_path("Cargo.toml").unwrap();
        app.composer.text.insert_str("abc");

        handle_key_event(key(KeyCode::Backspace), &mut app)
            .await
            .unwrap();

        assert_eq!(app.composer.text.value(), "ab");
        assert_eq!(app.composer_attachment_count(), 1);
    }

    #[tokio::test]
    async fn ctrl_r_prompt_picker_restores_selected_prompt() {
        let mut app = TuiApp::new();
        app.prompt_stash = Some("stashed draft".to_string());
        app.history.push_back("older prompt".to_string());
        app.history.push_back("newer prompt".to_string());

        handle_key_event(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            &mut app,
        )
        .await
        .unwrap();

        assert_eq!(app.mode, app::AppMode::PromptHistory);
        assert_eq!(app.prompt_picker_items()[0].0, "stash");

        handle_key_event(key(KeyCode::Down), &mut app)
            .await
            .unwrap();
        handle_key_event(key(KeyCode::Enter), &mut app)
            .await
            .unwrap();

        assert_eq!(app.mode, app::AppMode::Chat);
        assert_eq!(app.composer.text.value(), "newer prompt");
        assert_eq!(app.prompt_stash.as_deref(), Some("stashed draft"));
    }

    #[tokio::test]
    async fn sidebar_enter_uses_filtered_visible_sessions() {
        let mut app = TuiApp::new();
        app.session_manager = crate::tui::session_manager::TuiSessionManager::in_memory().unwrap();
        let _alpha = app
            .session_manager
            .start_session("Alpha Session", "model", None)
            .unwrap();
        let beta = app
            .session_manager
            .start_session("Beta Session", "model", None)
            .unwrap();
        let _gamma = app
            .session_manager
            .start_session("Gamma Session", "model", None)
            .unwrap();

        app.mode = app::AppMode::VimNormal;
        app.sidebar_visible = true;
        app.sidebar_filter = "Beta".to_string();
        app.sidebar_selected = 0;

        handle_key_event(key(KeyCode::Enter), &mut app)
            .await
            .unwrap();

        assert_eq!(
            app.session_manager.current_session_id(),
            Some(beta.as_str())
        );
    }

    #[test]
    fn sidebar_layout_overlays_on_narrow_terminals() {
        let area = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };

        let layout = sidebar_layout(area);

        match layout {
            SidebarLayout::Overlay { sidebar, main } => {
                assert_eq!(main, area);
                assert!(sidebar.width <= 44);
                assert_eq!(sidebar.height, 24);
            }
            SidebarLayout::Inline { .. } => panic!("narrow terminal should use overlay sidebar"),
        }
    }

    #[test]
    fn sidebar_layout_inlines_on_wide_terminals() {
        let area = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 160,
            height: 45,
        };

        let layout = sidebar_layout(area);

        match layout {
            SidebarLayout::Inline { sidebar, main } => {
                assert_eq!(sidebar.width, INLINE_SIDEBAR_WIDTH);
                assert!(main.width >= 120);
            }
            SidebarLayout::Overlay { .. } => panic!("wide terminal should use inline sidebar"),
        }
    }

    #[test]
    fn sidebar_layout_overlays_at_120_columns_to_protect_timeline() {
        let area = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 35,
        };

        let layout = sidebar_layout(area);

        match layout {
            SidebarLayout::Overlay { sidebar, main } => {
                assert_eq!(main, area);
                assert!(sidebar.width >= 40);
            }
            SidebarLayout::Inline { .. } => {
                panic!("120-column terminal should use overlay sidebar")
            }
        }
    }

    #[test]
    fn rendered_query_state_has_one_active_wait_label_at_100x30() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.facade_snapshot.provider_request.phase =
            crate::engine::runtime_facade::ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family =
            Some("openai_compatible".to_string());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
        app.facade_snapshot.provider_request.elapsed_ms = 2_700;
        app.sidebar_visible = true;

        let rendered = render_ui_text(&mut app, 100, 30);

        assert_eq!(rendered.matches("waiting on DeepSeek").count(), 1);
        assert!(!rendered.contains("Thinking..."));
        assert!(rendered.contains("? shortcuts"));
    }

    #[test]
    fn rendered_mid_width_keeps_composer_footer_clear_at_120x35() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.facade_snapshot.provider_request.phase =
            crate::engine::runtime_facade::ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
        app.sidebar_visible = true;

        let rendered = render_ui_text(&mut app, 120, 35);

        assert_eq!(rendered.matches("waiting on DeepSeek").count(), 1);
        assert!(!rendered.contains("Thinking..."));
        assert!(rendered.contains("Message Priority Agent"));
        assert!(rendered.contains("? shortcuts"));
    }

    #[test]
    fn rendered_wide_sidebar_keeps_footer_and_composer_metadata() {
        let mut app = TuiApp::new();
        app.sidebar_visible = true;
        app.history.push_back("previous prompt".to_string());
        app.prompt_stash = Some("stashed prompt".to_string());

        let rendered = render_ui_text(&mut app, 160, 45);

        assert!(rendered.contains("hist:1"));
        assert!(rendered.contains("stash"));
        assert!(rendered.contains("? shortcuts"));
    }

    #[test]
    fn rendered_turn_visual_state_stays_clean_across_common_viewports() {
        for (width, height) in [(100, 30), (120, 35), (160, 45)] {
            let mut app = opencode_alignment_fixture();
            let rendered = render_ui_text(&mut app, width, height);

            assert_eq!(
                rendered.matches("Running cargo").count(),
                1,
                "{width}x{height} should have one active tool label"
            );
            assert_eq!(
                rendered.matches("waiting on ").count(),
                0,
                "{width}x{height} should not duplicate provider wait while a concrete tool is active"
            );
            assert!(
                !rendered.contains("Thinking..."),
                "{width}x{height} should not show the old placeholder"
            );
            assert!(
                rendered.lines().all(|line| !line
                    .trim()
                    .starts_with("internal reasoning should not dominate")),
                "{width}x{height} should keep hidden reasoning collapsed"
            );
            assert!(
                rendered.contains("files:1") && rendered.contains("/attach preview"),
                "{width}x{height} should keep attachment affordances visible"
            );
            assert!(
                rendered.contains("? shortcuts"),
                "{width}x{height} should keep footer visible"
            );
        }
    }

    #[test]
    fn opencode_alignment_snapshots_can_be_dumped_for_visual_review() {
        for (width, height) in [(100, 30), (120, 35), (160, 45)] {
            let mut app = opencode_alignment_fixture();
            let lines = render_ui_lines(&mut app, width, height);
            let rendered = lines.join("\n");
            write_snapshot_if_requested(&format!("opencode-alignment-{width}x{height}"), &lines);

            assert_snapshot_display_width(&lines, width, height);
            assert_eq!(rendered.matches("Running cargo").count(), 1);
            assert_eq!(rendered.matches("waiting on ").count(), 0);
            assert!(rendered.contains("1.2s · esc to interrupt"));
            assert!(!rendered.contains("ms · esc to interrupt"));
            assert_no_sidebar_overlay_bleed(&lines, width, height);
            assert!(!rendered.contains("failed deserialization of"));
            assert!(!rendered.contains("async_openai::error"));
            assert!(!rendered.contains("Thinking..."));
            assert!(!rendered.contains("sess_demo · auto ·"));
            assert!(!rendered.contains("auto · DeepSeek"));
            assert!(!rendered.contains("deepseek-v4-flash · auto"));
            assert!(!rendered.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
            assert!(!rendered.contains("openai_compatible"));
            assert!(rendered.contains("deepseek-v4-flash"));
            if width >= 140 {
                assert!(rendered.lines().any(|line| {
                    line.contains("◈ ") && line.contains(" · DeepSeek / deepseek-v4-flash")
                }));
            }
            assert!(rendered.contains("DeepSeek / deepseek-v4-flash"));
            assert!(rendered.contains("files:1"));
            assert!(rendered.contains("? shortcuts"));
        }
    }

    #[test]
    fn completed_tool_turn_snapshots_can_be_dumped_for_visual_review() {
        for (width, height) in [(100, 30), (120, 35), (160, 45)] {
            let mut app = completed_tool_turn_fixture();
            let lines = render_ui_lines(&mut app, width, height);
            let rendered = lines.join("\n");
            write_snapshot_if_requested(&format!("completed-tool-turn-{width}x{height}"), &lines);

            assert_snapshot_display_width(&lines, width, height);
            assert!(
                rendered.matches("cargo check").count() >= 2,
                "{width}x{height} should show the command and final summary"
            );
            assert!(
                rendered.contains("done")
                    || rendered.contains("✓ [Shell]")
                    || rendered.contains("completed successfully"),
                "{width}x{height} should show completion status"
            );
            assert!(
                rendered.contains("validation passed") || rendered.contains("no compile errors"),
                "{width}x{height} should show validation status"
            );
            assert!(
                rendered.contains("1 tools") || rendered.matches("cargo check").count() >= 2,
                "{width}x{height} should show tool count or command summary"
            );
            assert!(rendered.contains("deepseek-v4-flash"));
            assert!(rendered.lines().any(|line| {
                line.contains("◈ ") && line.contains(" · DeepSeek / deepseek-v4-flash")
            }));
            assert!(rendered.contains("DeepSeek / deepseek-v4-flash"));
            assert!(!rendered.contains("unknown"));
            assert!(!rendered.contains("sess_demo · auto ·"));
            assert!(!rendered.contains("auto · DeepSeek"));
            assert!(!rendered.contains("deepseek-v4-flash · auto"));
            assert!(!rendered.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
            assert!(!rendered.contains("openai_compatible"));
            assert!(!rendered.contains("waiting on "));
            assert!(!rendered.contains("Thinking..."));
            assert!(!rendered.contains("async_openai::error"));
            assert!(rendered.contains("? shortcuts"));
        }
    }

    #[test]
    fn provider_failure_turn_snapshots_stay_product_shaped() {
        for (width, height) in [(100, 30), (120, 35), (160, 45)] {
            let mut app = provider_failure_turn_fixture();
            let lines = render_ui_lines(&mut app, width, height);
            let rendered = lines.join("\n");
            write_snapshot_if_requested(&format!("provider-failure-turn-{width}x{height}"), &lines);

            assert_snapshot_display_width(&lines, width, height);
            assert!(rendered.contains("Error"));
            assert!(rendered.contains("Failed to get response from deepseek API"));
            assert!(
                rendered.contains("provider error")
                    || rendered.contains("deepseek API")
                    || rendered.contains("Error"),
                "{width}x{height} should show provider failure reason"
            );
            assert!(rendered.contains("deepseek-v4-flash"));
            assert!(rendered.lines().any(|line| {
                line.contains("◈ ") && line.contains(" · DeepSeek / deepseek-v4-flash")
            }));
            assert!(rendered.contains("DeepSeek / deepseek-v4-flash"));
            assert!(!rendered.contains("Reply"));
            assert!(!rendered.contains("[Error:"));
            assert!(!rendered.contains("failed deserialization of"));
            assert!(!rendered.contains("async_openai::error"));
            assert!(!rendered.contains("Thinking..."));
            assert!(!rendered.contains("sess_demo · auto ·"));
            assert!(!rendered.contains("auto · DeepSeek"));
            assert!(!rendered.contains("deepseek-v4-flash · auto"));
            assert!(!rendered.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
            assert!(!rendered.contains("openai_compatible"));
            assert!(!rendered.contains("waiting on "));
            assert!(rendered.contains("? shortcuts"));
        }
    }

    #[test]
    fn completed_tool_turn_uses_semantic_styles() {
        let mut app = completed_tool_turn_fixture();

        assert_eq!(
            rendered_cell_fg_for_text(&mut app, 120, 35, "Ran cargo"),
            Some(app.theme.tokens.fg.faint)
        );
        assert_eq!(
            rendered_cell_fg_for_text(&mut app, 120, 35, "Reply"),
            Some(app.theme.tokens.tone.ok)
        );
        assert_eq!(
            rendered_cell_fg_for_text(&mut app, 120, 35, "DeepSeek"),
            Some(app.theme.tokens.fg.faint)
        );
    }

    #[test]
    fn provider_failure_turn_uses_error_semantic_style() {
        let mut app = provider_failure_turn_fixture();

        assert_eq!(
            rendered_cell_fg_for_text(&mut app, 120, 35, "Error"),
            Some(app.theme.tokens.card.error.color)
        );
        assert_ne!(
            rendered_cell_fg_for_text(&mut app, 120, 35, "Error"),
            Some(app.theme.tokens.tone.ok)
        );
    }
}

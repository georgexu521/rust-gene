//! Interactive terminal CLI 模块
//!
//! 使用 ratatui 实现类似 Claude Code 的终端交互体验

pub mod app;
pub mod commands;
pub mod components;
pub mod keybindings;
pub mod notify;
pub mod screens;
pub mod session_manager;
pub mod slash_handler;
pub mod theme;
pub mod tool_view;

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
    widgets::{Block, Borders},
    Frame, Terminal,
};
use std::io;
use std::sync::Arc;
use tracing::{debug, info};

/// 主交互式 CLI 运行函数
pub async fn run_tui(
    engine: Arc<StreamingQueryEngine>,
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
        // 绘制界面
        terminal.draw(|f| draw_ui(f, app))?;

        // 处理事件
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key_event(key, app).await? {
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
            app.on_tick().await;
            last_tick = std::time::Instant::now();
        }
    }
}

/// 绘制 UI
fn draw_ui(f: &mut Frame, app: &TuiApp) {
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
                screens::main_screen::render_plan_approval(f, plan, f.area());
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
                screens::main_screen::render_permission_approval(f, req, f.area());
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
        | app::AppMode::ModelSelect
        | app::AppMode::ProviderSelect => {
            if app.sidebar_visible {
                // 侧边栏 + 主区域布局
                let h_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(f.area());

                screens::main_screen::render_sidebar(f, app, h_chunks[0]);

                let main_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(3),
                        Constraint::Length(5),
                        Constraint::Length(1),
                    ])
                    .split(h_chunks[1]);

                screens::main_screen::render_chat_area(f, app, main_chunks[0]);
                screens::main_screen::render_input_area(f, app, main_chunks[1]);
                screens::main_screen::render_status_bar(f, app, main_chunks[2]);
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
                app::AppMode::ModelSelect => {
                    screens::main_screen::render_model_select(f, app, f.area());
                }
                app::AppMode::ProviderSelect => {
                    screens::main_screen::render_provider_select(f, app, f.area());
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

            components::diff_viewer::render_diff_viewer(
                f,
                &app.diff_content,
                &app.diff_title,
                app.diff_scroll_offset,
                f.area(),
                &app.theme,
            );
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

/// 处理键盘事件
/// 返回 true 表示退出应用
async fn handle_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    debug!("Key event: {:?}", key);
    use crate::tui::keybindings::AppAction;

    // AskUser 模式特殊处理
    if app.mode == app::AppMode::AskUser {
        return handle_ask_user_key_event(key, app).await;
    }

    if app.mode == app::AppMode::CommandPalette {
        return handle_command_palette_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ShortcutHelp {
        app.close_shortcut_help();
        return Ok(false);
    }

    if app.mode == app::AppMode::ModelSelect {
        return handle_model_select_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ProviderSelect {
        return handle_provider_select_key_event(key, app).await;
    }

    if app.mode == app::AppMode::PermissionApproval {
        match key.code {
            KeyCode::Esc => {
                app.respond_to_permission(false);
                return Ok(false);
            }
            KeyCode::Char('s') => {
                app.respond_to_permission_with_rule(
                    true,
                    Some("allow"),
                    Some(crate::permissions::RuleSource::User),
                );
                return Ok(false);
            }
            KeyCode::Char('p') => {
                app.respond_to_permission_with_rule(
                    true,
                    Some("allow"),
                    Some(crate::permissions::RuleSource::Project),
                );
                return Ok(false);
            }
            KeyCode::Char('a') => {
                app.respond_to_permission_with_rule(
                    true,
                    Some("allow"),
                    Some(crate::permissions::RuleSource::Global),
                );
                return Ok(false);
            }
            KeyCode::Char('x') => {
                app.respond_to_permission_with_rule(
                    false,
                    Some("deny"),
                    Some(crate::permissions::RuleSource::Global),
                );
                return Ok(false);
            }
            _ => {}
        }
    }

    if app.mode == app::AppMode::ToolViewer {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                app.mode = if app.vim_mode {
                    app::AppMode::VimNormal
                } else {
                    app::AppMode::Chat
                };
            }
            KeyCode::Up => {
                app.tool_viewer_scroll_offset = app.tool_viewer_scroll_offset.saturating_sub(1)
            }
            KeyCode::Down => {
                app.tool_viewer_scroll_offset = app.tool_viewer_scroll_offset.saturating_add(1)
            }
            KeyCode::PageUp => {
                app.tool_viewer_scroll_offset = app.tool_viewer_scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                app.tool_viewer_scroll_offset = app.tool_viewer_scroll_offset.saturating_add(10);
            }
            KeyCode::Home => app.tool_viewer_scroll_offset = 0,
            KeyCode::End => app.tool_viewer_scroll_offset = u16::MAX,
            _ => {}
        }
        return Ok(false);
    }

    // Diff 查看器模式：滚动 + Cancel/Quit
    if app.mode == app::AppMode::DiffViewer {
        let action = app.keybindings.action_for(key, app.mode);
        match action {
            AppAction::Cancel | AppAction::Quit => {
                if app.pending_permission_request.is_some() {
                    app.mode = app::AppMode::PermissionApproval;
                } else {
                    app.mode = app::AppMode::Chat;
                }
            }
            AppAction::ScrollUp => {
                app.diff_scroll_offset = app.diff_scroll_offset.saturating_sub(1);
            }
            AppAction::ScrollDown => {
                app.diff_scroll_offset = app.diff_scroll_offset.saturating_add(1);
            }
            AppAction::ScrollTop => {
                app.diff_scroll_offset = 0;
            }
            AppAction::ScrollBottom => {
                // 设置一个较大的值，渲染时会 clamp
                app.diff_scroll_offset = u16::MAX;
            }
            _ => {}
        }
        // 也处理 PageUp/PageDown（直接通过 KeyCode 检查，不依赖 keybindings）
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::PageUp => {
                app.diff_scroll_offset = app.diff_scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                app.diff_scroll_offset = app.diff_scroll_offset.saturating_add(10);
            }
            _ => {}
        }
        return Ok(false);
    }

    // Onboarding 引导模式
    if app.mode == app::AppMode::Onboarding {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Enter | KeyCode::Right => {
                if let Some(ref mut state) = app.onboarding_state {
                    if !state.next_step() {
                        // 已完成所有步骤
                        let _ = state.complete();
                        app.mode = app::AppMode::Chat;
                        app.onboarding_state = None;
                    }
                } else {
                    app.mode = app::AppMode::Chat;
                }
            }
            KeyCode::Left => {
                if let Some(ref mut state) = app.onboarding_state {
                    state.prev_step();
                }
            }
            KeyCode::Esc => {
                // 跳过引导
                if let Some(ref state) = app.onboarding_state {
                    let _ = state.complete();
                }
                app.mode = app::AppMode::Chat;
                app.onboarding_state = None;
            }
            _ => {}
        }
        return Ok(false);
    }

    // 设置模式特殊处理（编辑模式不使用 action 映射）
    if app.mode == app::AppMode::Settings {
        return handle_settings_key_event(key, app).await;
    }

    if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.open_command_palette();
        return Ok(false);
    }

    if key.code == KeyCode::Char('m') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.open_model_select();
        return Ok(false);
    }

    if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.open_provider_select();
        return Ok(false);
    }

    if matches!(key.code, KeyCode::F(1)) || (key.code == KeyCode::Char('?') && app.input.is_empty())
    {
        app.open_shortcut_help();
        return Ok(false);
    }

    if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.cycle_expanded_tool_run();
        return Ok(false);
    }

    if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if !app.open_tool_viewer() {
            app.add_system_message("No tool output to view yet.".to_string());
        }
        return Ok(false);
    }

    // VimNormal 模式：添加 Ctrl+D/U 半页滚动（直接处理，不经过 action_for）
    if app.mode == app::AppMode::VimNormal {
        use crossterm::event::{KeyCode, KeyModifiers};
        if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
            app.scroll_down_half_page();
            return Ok(false);
        }
        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            app.scroll_up_half_page();
            return Ok(false);
        }
        if key.code == KeyCode::Char('/') {
            app.message_search_state.activate();
            app.mode = app::AppMode::MessageSearch;
            return Ok(false);
        }
        if key.code == KeyCode::Tab {
            // Toggle collapse for the message at scroll_offset
            let idx = app.scroll_offset;
            if idx < app.messages.len() {
                if app.collapsed_indices.contains(&idx) {
                    app.collapsed_indices.remove(&idx);
                } else {
                    app.collapsed_indices.insert(idx);
                }
            }
            return Ok(false);
        }
        if key.code == KeyCode::Char('b') {
            app.sidebar_visible = !app.sidebar_visible;
            return Ok(false);
        }
    }

    // 消息搜索模式特殊处理
    if app.mode == app::AppMode::MessageSearch {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                app.message_search_state.deactivate();
                app.mode = app::AppMode::VimNormal;
                return Ok(false);
            }
            KeyCode::Enter => {
                // Jump to selected result
                if let Some(result) = app.message_search_state.selected_result() {
                    app.scroll_offset = result.message_index;
                }
                app.message_search_state.deactivate();
                app.mode = app::AppMode::VimNormal;
                return Ok(false);
            }
            KeyCode::Up => {
                app.message_search_state.prev_result();
                return Ok(false);
            }
            KeyCode::Down => {
                app.message_search_state.next_result();
                return Ok(false);
            }
            KeyCode::Char('k') if app.message_search_state.query.is_empty() => {
                app.message_search_state.prev_result();
                return Ok(false);
            }
            KeyCode::Char('j') if app.message_search_state.query.is_empty() => {
                app.message_search_state.next_result();
                return Ok(false);
            }
            KeyCode::Char('n') if app.message_search_state.query.is_empty() => {
                app.message_search_state.toggle_case_sensitive();
                let contents: Vec<String> =
                    app.messages.iter().map(|m| m.content.clone()).collect();
                app.message_search_state.search(&contents);
                return Ok(false);
            }
            KeyCode::Backspace => {
                app.message_search_state.pop_char();
                let contents: Vec<String> =
                    app.messages.iter().map(|m| m.content.clone()).collect();
                app.message_search_state.search(&contents);
                return Ok(false);
            }
            KeyCode::Char(c) => {
                app.message_search_state.push_char(c);
                let contents: Vec<String> =
                    app.messages.iter().map(|m| m.content.clone()).collect();
                app.message_search_state.search(&contents);
                return Ok(false);
            }
            _ => {}
        }
        return Ok(false);
    }

    // 其他模式统一通过 action_for 分发
    let action = app.keybindings.action_for(key, app.mode);

    match action {
        AppAction::Quit => {
            info!("Quit keybinding pressed, exiting...");
            // 退出前 flush 记忆
            if let Some(ref engine) = app.streaming_engine {
                if let Some(mem) = engine.memory_manager() {
                    let _messages = engine.get_history().await;
                    let api_messages: Vec<crate::services::api::Message> = Vec::new();
                    let mut mem = mem.lock().await;
                    mem.flush_session(&api_messages);
                }
            }
            // 退出前写入 telemetry 会话统计（仅用户开启时生效）
            if let Some(ref engine) = app.streaming_engine {
                let tracker = engine.cost_tracker().lock().await.clone();
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let started_at_ms = tracker
                    .session_start
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let mut tool_durations = std::collections::HashMap::new();
                let mut total_tool_calls = 0_u64;
                let mut total_tool_success = 0_u64;
                let mut total_tool_failed = 0_u64;
                for (name, stats) in &tracker.tool_metrics {
                    total_tool_calls += stats.calls;
                    total_tool_success += stats.success;
                    total_tool_failed += stats.failed;
                    let avg = if stats.calls > 0 {
                        stats.total_duration_ms / stats.calls
                    } else {
                        0
                    };
                    tool_durations.insert(name.clone(), avg);
                }

                let session = crate::telemetry::SessionTelemetry {
                    session_id: app
                        .session_manager
                        .current_session_id()
                        .unwrap_or("tui")
                        .to_string(),
                    started_at_ms,
                    ended_at_ms: Some(now_ms),
                    total_requests: tracker.total_requests,
                    total_tokens: tracker.total_tokens.total,
                    tool_calls: total_tool_calls,
                    tool_success: total_tool_success,
                    tool_failed: total_tool_failed,
                    estimated_cost_usd: tracker.estimated_cost_usd,
                    tool_durations,
                    errors: Vec::new(),
                    coding_rounds: tracker.coding_quality.file_change_rounds,
                    first_pass_successes: tracker.coding_quality.first_pass_successes,
                    verify_failures: tracker.coding_quality.verify_failures,
                    repair_cycles: tracker.coding_quality.repair_cycles,
                };
                let collector = crate::telemetry::TelemetryCollector::new();
                collector.record_session(session);
            }
            // 清理 Agent
            if let Some(ref engine) = app.streaming_engine {
                if let Some(manager) = engine.agent_manager() {
                    manager.cleanup().await;
                }
            }
            return Ok(true);
        }
        AppAction::Submit => {
            if !app.input.is_empty() {
                app.submit_message().await;
            }
        }
        AppAction::InsertNewline => {
            app.input.insert_newline();
        }
        AppAction::ToggleVimMode => {
            app.vim_mode = !app.vim_mode;
            app.mode = if app.vim_mode {
                app::AppMode::VimNormal
            } else {
                app::AppMode::Chat
            };
        }
        AppAction::Cancel => {
            if app.vim_mode && app.mode == app::AppMode::Chat {
                app.mode = app::AppMode::VimNormal;
            }
        }
        AppAction::ScrollUp => app.scroll_up(),
        AppAction::ScrollDown => app.scroll_down(),
        AppAction::ScrollTop => app.scroll_offset = 0,
        AppAction::ScrollBottom => app.scroll_to_bottom(),
        AppAction::VimInsert => app.mode = app::AppMode::Chat,
        AppAction::VimCommand => {
            app.mode = app::AppMode::Chat;
            app.input.insert(':');
        }
        AppAction::PlanApprove => {
            app.respond_to_plan(crate::engine::plan_mode::PlanApproval::Approved);
        }
        AppAction::PlanReject => {
            app.respond_to_plan(crate::engine::plan_mode::PlanApproval::Rejected);
        }
        AppAction::PlanModify => {
            app.respond_to_plan(crate::engine::plan_mode::PlanApproval::Modified(
                "Please revise the plan".to_string(),
            ));
        }
        AppAction::PermissionApprove => app.respond_to_permission(true),
        AppAction::PermissionReject => app.respond_to_permission(false),
        AppAction::PermissionViewDiff => {
            if let Some((title, diff)) = app.compute_permission_diff() {
                app.diff_title = title;
                app.diff_content = diff;
                app.mode = app::AppMode::DiffViewer;
            }
        }
        AppAction::None => {
            return handle_fallback_key_event(key, app).await;
        }
        _ => {}
    }

    Ok(false)
}

/// 处理 AskUser 模式的键盘事件
async fn handle_ask_user_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Enter => {
            let answer = app.input.value().to_string();
            app.respond_to_question(answer);
        }
        KeyCode::Esc => {
            app.respond_to_question("User cancelled".to_string());
        }
        KeyCode::Char(c) => {
            // 支持数字键快速选择选项
            if let Some(digit) = c.to_digit(10) {
                let idx = digit as usize;
                if idx > 0 && idx <= app.pending_question_options.len() {
                    let answer = app.pending_question_options[idx - 1].clone();
                    app.respond_to_question(answer);
                    return Ok(false);
                }
            }
            app.input.insert(c);
        }
        KeyCode::Backspace => app.input.delete_char_before_cursor(),
        KeyCode::Delete => app.input.delete_char_at_cursor(),
        KeyCode::Left => app.input.move_cursor_left(),
        KeyCode::Right => app.input.move_cursor_right(),
        KeyCode::Home => app.input.move_cursor_to_start(),
        KeyCode::End => app.input.move_cursor_to_end(),
        _ => {}
    }
    Ok(false)
}

async fn handle_command_palette_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Esc => app.close_command_palette(),
        KeyCode::Enter => app.accept_command_palette_selection().await,
        KeyCode::Up => app.command_palette_prev(),
        KeyCode::Down => app.command_palette_next(),
        KeyCode::Backspace => app.command_palette_backspace(),
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.close_command_palette();
        }
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            app.command_palette_push(c);
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_model_select_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Esc => app.close_model_select(),
        KeyCode::Enter => app.accept_model_selection(),
        KeyCode::Up => app.model_select_prev(),
        KeyCode::Down => app.model_select_next(),
        KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.close_model_select();
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_provider_select_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Esc => app.close_provider_select(),
        KeyCode::Enter => {
            let result = app.accept_provider_selection();
            app.add_system_message(result);
        }
        KeyCode::Up => app.provider_select_prev(),
        KeyCode::Down => app.provider_select_next(),
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.close_provider_select();
        }
        _ => {}
    }
    Ok(false)
}

/// 未被 action_for 捕获的键走默认行为（主要用于 Chat 模式的字符输入和光标移动）
async fn handle_fallback_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    if app.mode != app::AppMode::Chat {
        return Ok(false);
    }

    match key.code {
        KeyCode::Char(c) => app.input.insert(c),
        KeyCode::Backspace => app.input.delete_char_before_cursor(),
        KeyCode::Delete => app.input.delete_char_at_cursor(),
        KeyCode::Left => app.input.move_cursor_left(),
        KeyCode::Right => app.input.move_cursor_right(),
        KeyCode::Home => app.input.move_cursor_to_start(),
        KeyCode::End => app.input.move_cursor_to_end(),
        KeyCode::Up => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.history_prev();
            } else if app.input.is_cursor_on_first_line() {
                app.scroll_up();
            } else {
                app.input.move_cursor_up();
            }
        }
        KeyCode::Down => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.history_next();
            } else if app.input.is_cursor_on_last_line() {
                app.scroll_down();
            } else {
                app.input.move_cursor_down();
            }
        }
        KeyCode::PageUp => app.scroll_up_half_page(),
        KeyCode::PageDown => app.scroll_down_half_page(),
        _ => {}
    }
    Ok(false)
}

/// 处理设置模式的键盘事件
async fn handle_settings_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    use crate::tui::keybindings::AppAction;

    let is_edit_mode = app
        .settings_state
        .as_ref()
        .map(|s| s.edit_mode)
        .unwrap_or(false);

    if is_edit_mode {
        // 编辑模式：固定键位（不通过 keybindings）
        if let Some(ref mut state) = app.settings_state {
            match key.code {
                KeyCode::Enter => state.save_edit(),
                KeyCode::Esc => state.cancel_edit(),
                KeyCode::Char(c) => state.edit_buffer.push(c),
                KeyCode::Backspace => {
                    state.edit_buffer.pop();
                }
                _ => {}
            }
        }
        return Ok(false);
    }

    // 导航模式：通过 keybindings 分发
    let action = app.keybindings.action_for(key, app.mode);

    match action {
        AppAction::Quit => app.exit_settings(),
        AppAction::SettingsSave => {
            let result = app.save_settings();
            if let Some(ref mut state) = app.settings_state {
                if let Err(e) = result {
                    state.show_message(format!("Save failed: {}", e));
                }
            }
        }
        AppAction::SettingsNextPage => {
            if let Some(ref mut state) = app.settings_state {
                state.next_page();
            }
        }
        AppAction::SettingsPrevPage => {
            if let Some(ref mut state) = app.settings_state {
                state.prev_page();
            }
        }
        AppAction::SettingsNextItem => {
            if let Some(ref mut state) = app.settings_state {
                state.next_item();
            }
        }
        AppAction::SettingsPrevItem => {
            if let Some(ref mut state) = app.settings_state {
                state.prev_item();
            }
        }
        AppAction::SettingsEdit | AppAction::Submit => {
            if let Some(ref mut state) = app.settings_state {
                state.start_edit();
            }
        }
        AppAction::SettingsToggleBool => {
            if let Some(ref mut state) = app.settings_state {
                state.toggle_bool();
            }
        }
        _ => {
            // 保留硬编码方向键作为兜底
            match key.code {
                KeyCode::Right => {
                    if let Some(ref mut state) = app.settings_state {
                        state.next_page();
                    }
                }
                KeyCode::Left => {
                    if let Some(ref mut state) = app.settings_state {
                        state.prev_page();
                    }
                }
                KeyCode::Down => {
                    if let Some(ref mut state) = app.settings_state {
                        state.next_item();
                    }
                }
                KeyCode::Up => {
                    if let Some(ref mut state) = app.settings_state {
                        state.prev_item();
                    }
                }
                KeyCode::Enter => {
                    if let Some(ref mut state) = app.settings_state {
                        state.start_edit();
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(ref mut state) = app.settings_state {
                        state.toggle_bool();
                    }
                }
                _ => {}
            }
        }
    }

    Ok(false)
}

/// 工具函数：创建带标题的块
pub fn titled_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::DarkGray))
}

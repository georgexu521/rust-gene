use super::*;

async fn handle_leader_sequence(key: KeyEvent, app: &mut TuiApp) -> Option<bool> {
    use crossterm::event::KeyCode;

    if let Some(state) = &app.leader_state {
        if state.started_at.elapsed().as_millis() as u64 >= app.keybindings.leader_timeout_ms {
            app.clear_leader_sequence();
            return None;
        }
        app.clear_leader_sequence();
        match key.code {
            KeyCode::Char('p') => {
                app.open_command_palette();
                return Some(false);
            }
            KeyCode::Char('s') => {
                app.sidebar_visible = true;
                app.sidebar_panel = app::SidebarPanel::Sessions;
                return Some(false);
            }
            KeyCode::Char('d') => {
                if !app.open_tool_viewer() {
                    app.add_system_message("No diff/tool output to view yet.".to_string());
                }
                return Some(false);
            }
            KeyCode::Char('g') => {
                app.cycle_recent_session_forward().await;
                return Some(false);
            }
            KeyCode::Char('w') => {
                app.open_workspace_switcher();
                return Some(false);
            }
            _ => return None,
        }
    }

    if app.keybindings.leader.matches(key)
        && matches!(app.mode, app::AppMode::Chat | app::AppMode::VimNormal)
    {
        app.begin_leader_sequence();
        return Some(false);
    }

    None
}

/// 返回 true 表示退出应用
pub(super) async fn handle_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    debug!("Key event: {:?}", key);
    use crate::tui::keybindings::AppAction;

    if let Some(handled) = handle_leader_sequence(key, app).await {
        return Ok(handled);
    }

    // AskUser 模式特殊处理
    if app.mode == app::AppMode::AskUser {
        return handle_ask_user_key_event(key, app).await;
    }

    if app.mode == app::AppMode::CommandPalette {
        return handle_command_palette_key_event(key, app).await;
    }

    if app.mode == app::AppMode::PromptHistory {
        return handle_prompt_picker_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ModelSelect {
        return handle_model_select_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ProviderSelect {
        return handle_provider_select_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ConnectWizard {
        return handle_connect_wizard_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ShortcutHelp {
        if key.code == KeyCode::Char('/') && !app.filtering_shortcut_help {
            app.shortcut_help_filter.clear();
            app.filtering_shortcut_help = true;
            return Ok(false);
        }
        if app.filtering_shortcut_help {
            match key.code {
                KeyCode::Esc => {
                    app.shortcut_help_filter.clear();
                    app.filtering_shortcut_help = false;
                    return Ok(false);
                }
                KeyCode::Enter => {
                    app.filtering_shortcut_help = false;
                    return Ok(false);
                }
                KeyCode::Backspace => {
                    app.shortcut_help_filter.pop();
                    return Ok(false);
                }
                KeyCode::Char(c) => {
                    app.shortcut_help_filter.push(c);
                    return Ok(false);
                }
                _ => {}
            }
        }
        app.close_shortcut_help();
        return Ok(false);
    }

    if app.mode == app::AppMode::PromptHistory {
        return handle_prompt_picker_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ModelSelect {
        return handle_model_select_key_event(key, app).await;
    }

    if app.mode == app::AppMode::ProviderSelect {
        return handle_provider_select_key_event(key, app).await;
    }

    if app.mode == app::AppMode::FilePicker {
        return handle_file_picker_key_event(key, app).await;
    }

    if app.mode == app::AppMode::WorkspaceSwitcher {
        return handle_workspace_switcher_key_event(key, app).await;
    }

    if app.has_interruptible_run()
        && matches!(
            app.mode,
            app::AppMode::Chat
                | app::AppMode::VimNormal
                | app::AppMode::DiffViewer
                | app::AppMode::ToolViewer
        )
        && (key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)))
    {
        app.cancel_active_run("Run interrupted").await;
        return Ok(false);
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
                app.pop_mode();
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
                app.pop_mode();
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
            KeyCode::Char('n') => {
                if let Some(line) = components::diff_renderer::find_next_hunk_line(
                    &app.diff_content,
                    app.diff_scroll_offset,
                ) {
                    app.diff_scroll_offset = line as u16;
                }
            }
            KeyCode::Char('p') => {
                if let Some(line) = components::diff_renderer::find_prev_hunk_line(
                    &app.diff_content,
                    app.diff_scroll_offset,
                ) {
                    app.diff_scroll_offset = line as u16;
                }
            }
            KeyCode::Tab => {
                if let Some(line) = components::diff_renderer::find_next_file_line(
                    &app.diff_content,
                    app.diff_scroll_offset,
                ) {
                    app.diff_scroll_offset = line as u16;
                }
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

    if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.sidebar_panel = app.sidebar_panel.next();
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
            app.push_mode(app::AppMode::MessageSearch);
            return Ok(false);
        }
        if key.code == KeyCode::Tab && key.modifiers.is_empty() {
            app.toggle_collapse_at_scroll_anchor();
            return Ok(false);
        }
        if key.code == KeyCode::Char('b') {
            app.sidebar_visible = !app.sidebar_visible;
            return Ok(false);
        }
        // Sidebar navigation (only when sidebar is visible)
        if app.sidebar_visible {
            // Rename mode takes precedence over navigation
            if app.renaming_session_id.is_some() {
                match key.code {
                    KeyCode::Enter => {
                        if let Some(ref id) = app.renaming_session_id.take() {
                            let new_title = app.rename_buffer.trim().to_string();
                            if !new_title.is_empty() {
                                let _ = app.session_manager.update_session_title(id, &new_title);
                            }
                            app.rename_buffer.clear();
                        }
                        return Ok(false);
                    }
                    KeyCode::Esc => {
                        app.renaming_session_id = None;
                        app.rename_buffer.clear();
                        return Ok(false);
                    }
                    KeyCode::Char(c) => {
                        app.rename_buffer.push(c);
                        return Ok(false);
                    }
                    KeyCode::Backspace => {
                        app.rename_buffer.pop();
                        return Ok(false);
                    }
                    _ => {}
                }
            }

            let sessions = app.visible_sidebar_sessions(50);
            let max = sessions.len().saturating_sub(1);
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if app.sidebar_selected < max {
                        app.sidebar_selected += 1;
                    }
                    return Ok(false);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    app.sidebar_selected = app.sidebar_selected.saturating_sub(1);
                    return Ok(false);
                }
                KeyCode::Enter => {
                    if let Some(session) = sessions.get(app.sidebar_selected) {
                        let id = session.id.clone();
                        let _ = app.restore_session(&id).await;
                        app.sidebar_selected = 0;
                    }
                    return Ok(false);
                }
                KeyCode::Char('p') => {
                    if let Some(session) = sessions.get(app.sidebar_selected) {
                        app.toggle_pinned_session(&session.id);
                    }
                    return Ok(false);
                }
                KeyCode::Char('d') => {
                    if let Some(session) = sessions.get(app.sidebar_selected) {
                        if app.confirm_delete_session_id.as_deref() == Some(&session.id) {
                            // Second press: confirm delete
                            let _ = app.session_manager.delete_session(&session.id);
                            app.confirm_delete_session_id = None;
                            app.sidebar_selected = app.sidebar_selected.min(max.saturating_sub(1));
                        } else {
                            app.confirm_delete_session_id = Some(session.id.clone());
                        }
                    }
                    return Ok(false);
                }
                KeyCode::Char('r') => {
                    if let Some(session) = sessions.get(app.sidebar_selected) {
                        app.renaming_session_id = Some(session.id.clone());
                        app.rename_buffer = session.title.clone();
                    }
                    return Ok(false);
                }
                KeyCode::Char('/') => {
                    app.sidebar_filter.clear();
                    app.filtering_sidebar = true;
                    app.sidebar_selected = 0;
                    return Ok(false);
                }
                KeyCode::Esc => {
                    if app.filtering_sidebar {
                        app.sidebar_filter.clear();
                        app.filtering_sidebar = false;
                        app.sidebar_selected = 0;
                    }
                    return Ok(false);
                }
                _ => {}
            }
            // Handle sidebar filter text input
            if app.filtering_sidebar {
                match key.code {
                    KeyCode::Char(c) => {
                        app.sidebar_filter.push(c);
                        app.sidebar_selected = 0;
                        return Ok(false);
                    }
                    KeyCode::Backspace => {
                        app.sidebar_filter.pop();
                        app.sidebar_selected = 0;
                        return Ok(false);
                    }
                    KeyCode::Enter => {
                        app.filtering_sidebar = false;
                        app.sidebar_selected = 0;
                        return Ok(false);
                    }
                    _ => {}
                }
            }
        }
    }

    // 消息搜索模式特殊处理
    if app.mode == app::AppMode::MessageSearch {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                app.message_search_state.deactivate();
                app.pop_mode();
                return Ok(false);
            }
            KeyCode::Enter => {
                // Jump to selected result
                if let Some(result) = app.message_search_state.selected_result() {
                    app.scroll_to_message_index(result.message_index);
                }
                app.message_search_state.deactivate();
                app.pop_mode();
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
    if let Some(handled) = handle_rich_input_shortcuts(key, app) {
        return Ok(handled);
    }

    // Toggle inline expand/collapse for the focused message or tool body when
    // the composer is empty.
    if key.code == KeyCode::Enter
        && app.composer.text.is_empty()
        && matches!(app.mode, app::AppMode::Chat | app::AppMode::VimNormal)
        && app.toggle_collapsible_at_scroll_anchor()
    {
        return Ok(false);
    }

    let action = app.keybindings.action_for(key, app.mode);

    match action {
        AppAction::Quit => {
            info!("Quit keybinding pressed, exiting...");
            let interrupted_active_run = app.cancel_active_run("Run interrupted before exit").await;
            // 退出前 flush 记忆
            if !interrupted_active_run {
                if let Some(ref engine) = app.streaming_engine {
                    engine
                        .flush_memory_for_current_history(crate::memory::MemoryFlushReason::Exit)
                        .await;
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
            if !interrupted_active_run {
                if let Some(ref engine) = app.streaming_engine {
                    if let Some(manager) = engine.agent_manager() {
                        manager.cleanup().await;
                    }
                }
            }

            // Persist current workspace so the next TUI restart restores grouping.
            if let Err(err) = app.kv_store.set_string(
                "ui.last_workspace_root",
                &app.workspace.root.to_string_lossy(),
            ) {
                tracing::warn!("Failed to persist workspace on exit: {err}");
            }

            return Ok(true);
        }
        AppAction::Submit => {
            if !app.composer.text.is_empty() {
                app.submit_message().await;
            }
        }
        AppAction::InsertNewline => {
            app.composer.text.insert_newline();
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
            if app.cancel_active_run("Run interrupted").await {
                return Ok(false);
            }
            if app.vim_mode && app.mode == app::AppMode::Chat {
                app.mode = app::AppMode::VimNormal;
            }
        }
        AppAction::ScrollUp => app.scroll_up(),
        AppAction::ScrollDown => app.scroll_down(),
        AppAction::ScrollTop => app.scroll_to_top(),
        AppAction::ScrollBottom => app.scroll_to_bottom(),
        AppAction::VimInsert => app.mode = app::AppMode::Chat,
        AppAction::VimCommand => {
            app.mode = app::AppMode::Chat;
            app.composer.text.insert(':');
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
                app.push_mode(app::AppMode::DiffViewer);
            }
        }
        AppAction::OpenCommandPalette => app.open_command_palette(),
        AppAction::OpenPromptHistory => app.open_prompt_picker(),
        AppAction::OpenModelSelect => {
            app.refresh_discovered_models().await;
            app.open_model_select();
        }
        AppAction::OpenProviderSelect => app.open_provider_select(),
        AppAction::OpenShortcutHelp => app.open_shortcut_help(),
        AppAction::ToggleExpandDetails => {
            if !app.toggle_reasoning_at_scroll_anchor() {
                app.cycle_expanded_tool_run();
            }
        }
        AppAction::OpenToolOutput => {
            if !app.open_tool_viewer() {
                app.add_system_message("No tool output to view yet.".to_string());
            }
        }
        AppAction::CycleStatusBarDensity => {
            let density = app.cycle_status_bar_density();
            app.add_system_message(format!("Status bar density: {}", density.name()));
        }
        AppAction::ToggleSidebar => app.sidebar_visible = !app.sidebar_visible,
        AppAction::OpenMessageSearch => {
            app.message_search_state.activate();
            app.push_mode(app::AppMode::MessageSearch);
        }
        AppAction::LeaderPalette => {
            app.begin_leader_sequence();
            return Ok(false);
        }
        AppAction::LeaderSidebar => {
            app.sidebar_visible = !app.sidebar_visible;
            return Ok(false);
        }
        AppAction::LeaderToolDiff => {
            if !app.toggle_reasoning_at_scroll_anchor() {
                app.cycle_expanded_tool_run();
            }
            return Ok(false);
        }
        AppAction::LeaderSessionCycle => {
            app.cycle_recent_session_forward().await;
            return Ok(false);
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
            let answer = app.composer.text.value().to_string();
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
            app.composer.text.insert(c);
        }
        KeyCode::Backspace => app.composer.text.delete_char_before_cursor(),
        KeyCode::Delete => app.composer.text.delete_char_at_cursor(),
        KeyCode::Left => app.composer.text.move_cursor_left(),
        KeyCode::Right => app.composer.text.move_cursor_right(),
        KeyCode::Home => app.composer.text.move_cursor_to_start(),
        KeyCode::End => app.composer.text.move_cursor_to_end(),
        _ => {}
    }
    Ok(false)
}

async fn handle_command_palette_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    if app.keybindings.global_command_palette.matches(key) {
        app.close_command_palette();
        return Ok(false);
    }
    match key.code {
        KeyCode::Esc => app.close_command_palette(),
        KeyCode::Enter => app.accept_command_palette_selection().await,
        KeyCode::Up => app.command_palette_prev(),
        KeyCode::Down => app.command_palette_next(),
        KeyCode::Backspace => app.command_palette_backspace(),
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            app.command_palette_push(c);
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_prompt_picker_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    if app.keybindings.global_prompt_history.matches(key) {
        app.close_prompt_picker();
        return Ok(false);
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_prompt_picker(),
        KeyCode::Enter => {
            app.accept_prompt_picker_selection();
        }
        KeyCode::Up | KeyCode::Char('k') => app.prompt_picker_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.prompt_picker_next(),
        _ => {}
    }
    Ok(false)
}

async fn handle_model_select_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    if app.keybindings.global_model_select.matches(key) {
        app.close_model_select();
        return Ok(false);
    }
    match key.code {
        KeyCode::Esc => app.close_model_select(),
        KeyCode::Enter => app.accept_model_selection(),
        KeyCode::Up => app.model_select_prev(),
        KeyCode::Down => app.model_select_next(),
        KeyCode::Backspace => app.model_select_backspace(),
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            app.model_select_push(c);
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_provider_select_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    if app.keybindings.global_provider_select.matches(key) {
        app.close_provider_select();
        return Ok(false);
    }
    match key.code {
        KeyCode::Esc => app.close_provider_select(),
        KeyCode::Enter => {
            let result = app.accept_provider_selection().await;
            app.add_system_message(result);
        }
        KeyCode::Up => app.provider_select_prev(),
        KeyCode::Down => app.provider_select_next(),
        KeyCode::Backspace => app.provider_select_backspace(),
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            app.provider_select_push(c);
        }
        _ => {}
    }
    Ok(false)
}

async fn handle_connect_wizard_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    use crate::tui::app::connect_wizard::{ConnectStep, WizardStatus};

    let Some(mut wizard) = app.connect_wizard_state.take() else {
        app.pop_mode();
        return Ok(false);
    };

    match wizard.step {
        ConnectStep::SelectProvider => match key.code {
            KeyCode::Esc => {
                app.close_connect_wizard();
                return Ok(false);
            }
            KeyCode::Enter => {
                wizard.confirm_provider();
                app.connect_wizard_state = Some(wizard);
                return Ok(false);
            }
            KeyCode::Up => wizard.select_prev(),
            KeyCode::Down => wizard.select_next(),
            KeyCode::Backspace => wizard.backspace_query(),
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                wizard.push_query(c);
            }
            _ => {}
        },
        ConnectStep::InputKey => match key.code {
            KeyCode::Esc => {
                app.close_connect_wizard();
                return Ok(false);
            }
            KeyCode::Enter => {
                let provider_id = wizard.provider_id.clone().unwrap_or_default();
                let env_var = wizard.selected_key_env_var().unwrap_or_default();
                let key_value = wizard.input_buffer.clone();
                wizard.start_validating();
                app.connect_wizard_state = Some(wizard);
                let result = save_key_and_activate(app, &provider_id, &env_var, &key_value).await;
                if let Some(wizard) = app.connect_wizard_state.as_mut() {
                    match result {
                        Ok(msg) => wizard.finish(WizardStatus::Success(msg)),
                        Err(err) => wizard.finish(WizardStatus::Error(err.to_string())),
                    }
                }
                return Ok(false);
            }
            KeyCode::Backspace => wizard.backspace_key(),
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                wizard.mask_input = !wizard.mask_input;
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                wizard.push_key(c);
            }
            _ => {}
        },
        ConnectStep::Validating => {}
        ConnectStep::Done => match key.code {
            KeyCode::Esc => {
                app.close_connect_wizard();
                return Ok(false);
            }
            KeyCode::Enter => {
                if matches!(wizard.status, WizardStatus::Error(_)) {
                    wizard.step = ConnectStep::InputKey;
                    wizard.status = WizardStatus::None;
                    app.connect_wizard_state = Some(wizard);
                } else {
                    app.close_connect_wizard();
                }
                return Ok(false);
            }
            _ => {}
        },
    }

    app.connect_wizard_state = Some(wizard);
    Ok(false)
}

async fn save_key_and_activate(
    app: &mut TuiApp,
    provider_id: &str,
    env_var: &str,
    key: &str,
) -> anyhow::Result<String> {
    use crate::services::api::provider_manager::ProviderManager;

    let manager = ProviderManager::new();
    let validation = manager
        .save_and_validate(provider_id, env_var, key)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if !validation.is_success() {
        return Err(anyhow::anyhow!("{}", validation.into_message()));
    }

    std::env::set_var("PRIORITY_AGENT_DEFAULT_PROVIDER", provider_id);
    let result = app
        .activate_provider_runtime(provider_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(format!("Provider is active with model {}.", result))
}

async fn handle_workspace_switcher_key_event(
    key: KeyEvent,
    app: &mut TuiApp,
) -> anyhow::Result<bool> {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.close_workspace_switcher();
        }
        KeyCode::Enter => {
            let message = app.accept_workspace_switcher();
            app.add_system_message(message);
        }
        KeyCode::Up | KeyCode::Char('k') => app.workspace_switcher_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.workspace_switcher_next(),
        _ => {}
    }
    Ok(false)
}

async fn handle_file_picker_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    if app.file_picker_filtering {
        match key.code {
            KeyCode::Esc => app.finish_file_picker_filter(),
            KeyCode::Enter => app.finish_file_picker_filter(),
            KeyCode::Backspace => app.pop_file_picker_filter_char(),
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                app.push_file_picker_filter_char(c);
            }
            _ => {}
        }
        return Ok(false);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_composer_file_picker(),
        KeyCode::Char('/') => app.start_file_picker_filter(),
        KeyCode::Enter => {
            let message = app.accept_file_picker_selection();
            if app.mode != app::AppMode::FilePicker {
                app.add_system_message(message);
            }
        }
        KeyCode::Char(' ') => {
            if let Some(state) = &mut app.file_picker_state {
                if state.selection_mode()
                    == crate::tui::components::file_browser::FileSelectionMode::Multi
                {
                    state.toggle_selection();
                } else {
                    state.toggle_current();
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => app.file_picker_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.file_picker_next(),
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(state) = &mut app.file_picker_state {
                state.toggle_current();
            }
        }
        _ => {}
    }
    Ok(false)
}

/// 富文本输入快捷键（选择、剪贴板、撤销/重做）。
/// 仅当处于 Chat 或 VimNormal 模式且没有 overlay 时生效。
fn handle_rich_input_shortcuts(key: KeyEvent, app: &mut TuiApp) -> Option<bool> {
    use crossterm::event::{KeyCode, KeyModifiers};

    if !matches!(app.mode, app::AppMode::Chat | app::AppMode::VimNormal) {
        return None;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Char('a') if ctrl => {
            app.composer.text.select_all();
            return Some(false);
        }
        KeyCode::Char('z') if ctrl && shift => {
            app.composer.text.redo();
            return Some(false);
        }
        KeyCode::Char('z') if ctrl => {
            app.composer.text.undo();
            return Some(false);
        }
        KeyCode::Char('c') if ctrl => {
            if app.composer.text.has_selection() {
                let _ = app.composer.text.copy_selection();
                return Some(false);
            }
        }
        KeyCode::Char('x') if ctrl => {
            if app.composer.text.has_selection() {
                let _ = app.composer.text.cut_selection();
                return Some(false);
            }
        }
        KeyCode::Char('v') if ctrl => {
            let _ = app.composer.text.paste_from_clipboard();
            return Some(false);
        }
        KeyCode::Left if shift => {
            app.composer.text.select_left();
            return Some(false);
        }
        KeyCode::Right if shift => {
            app.composer.text.select_right();
            return Some(false);
        }
        KeyCode::Up if shift => {
            app.composer.text.select_up();
            return Some(false);
        }
        KeyCode::Down if shift => {
            app.composer.text.select_down();
            return Some(false);
        }
        _ => {}
    }

    None
}

/// 未被 action_for 捕获的键走默认行为（主要用于 Chat 模式的字符输入和光标移动）
async fn handle_fallback_key_event(key: KeyEvent, app: &mut TuiApp) -> anyhow::Result<bool> {
    if app.mode != app::AppMode::Chat {
        return Ok(false);
    }

    match key.code {
        KeyCode::Char('@') => {
            app.open_composer_file_picker_with_filter(Some("."), true);
        }
        KeyCode::Backspace => {
            if app.composer.text.is_empty() {
                if let Some(path) = app.remove_last_composer_attachment() {
                    app.add_toast(format!("Removed attachment: {}", path), "-");
                }
            } else {
                app.composer.text.delete_char_before_cursor();
            }
        }
        KeyCode::Delete => app.composer.text.delete_char_at_cursor(),
        KeyCode::Left => app.composer.text.move_cursor_left(),
        KeyCode::Right => app.composer.text.move_cursor_right(),
        KeyCode::Home => app.composer.text.move_cursor_to_start(),
        KeyCode::End => app.composer.text.move_cursor_to_end(),
        KeyCode::Up => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.history_prev();
            } else if app.composer.text.is_cursor_on_first_line() {
                app.scroll_up();
            } else {
                app.composer.text.move_cursor_up();
            }
        }
        KeyCode::Down => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.history_next();
            } else if app.composer.text.is_cursor_on_last_line() {
                app.scroll_down();
            } else {
                app.composer.text.move_cursor_down();
            }
        }
        KeyCode::PageUp => app.scroll_up_half_page(),
        KeyCode::PageDown => app.scroll_down_half_page(),
        KeyCode::Char(c) => app.composer.text.insert(c),
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

use super::*;

impl TuiApp {
    pub fn has_interruptible_run(&self) -> bool {
        self.is_querying
            || self.stream_handle.is_some()
            || self.run_coordinator.is_active()
            || self
                .sync_snapshot
                .all_tool_runs()
                .iter()
                .any(crate::tui::tool_view::ToolRunView::is_active)
            || self.facade_snapshot.tool_turns.iter().any(|turn| {
                !matches!(
                    turn.phase,
                    crate::engine::runtime_facade::ToolTurnPhase::Persisted
                        | crate::engine::runtime_facade::ToolTurnPhase::Failed
                        | crate::engine::runtime_facade::ToolTurnPhase::Cancelled
                        | crate::engine::runtime_facade::ToolTurnPhase::TimedOut
                        | crate::engine::runtime_facade::ToolTurnPhase::FinalAnswer
                )
            })
    }

    pub(in crate::tui::app) fn upsert_assistant_message(
        &mut self,
        message_id: String,
        content: String,
    ) {
        if let Some(message) = self
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            if message.role == crate::state::MessageRole::Assistant {
                message.content = content;
            }
        } else {
            self.messages.push(crate::state::MessageItem {
                id: message_id,
                role: crate::state::MessageRole::Assistant,
                content,
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            });
        }
    }

    pub async fn cancel_active_run(&mut self, reason: &str) -> bool {
        if !self.has_interruptible_run() {
            return false;
        }

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }
        self.stream_done
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.is_querying = false;
        self.run_coordinator.finish_run();
        let runtime_state = self.runtime_facade_state.clone();
        let _ = tokio::time::timeout(std::time::Duration::from_millis(75), async move {
            runtime_state.mark_cancelled().await;
            runtime_state.set_querying(false).await;
        })
        .await;
        self.stream_started_at = None;
        self.post_tool_turn_wait_started_at = None;
        self.current_turn_event_start_seq = None;

        let cancellation_message = format!("[Cancelled: {reason}]");
        let assistant_message_id = self
            .sync_snapshot
            .active_assistant_message_id
            .clone()
            .or_else(|| {
                self.messages
                    .iter()
                    .rev()
                    .find(|message| message.role == crate::state::MessageRole::Assistant)
                    .map(|message| message.id.clone())
            })
            .unwrap_or_else(|| format!("msg_{}", self.messages.len()));
        {
            let mut sync = self.sync_store.lock().await;
            sync.mark_active_tools_with_result("Result: ERROR\nTool run is cancelled.".to_string());
            sync.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                    message_id: Some(assistant_message_id.clone()),
                    text: cancellation_message.clone(),
                    streaming: false,
                },
            );
            self.sync_snapshot = sync.snapshot();
        }
        self.current_tool_anchor_id = None;
        self.settle_unfinished_tool_parts(reason);

        self.upsert_assistant_message(assistant_message_id, cancellation_message);
        self.add_toast(reason.to_string(), "!");
        true
    }

    pub async fn timeout_active_run(&mut self, reason: &str) -> bool {
        if !self.has_interruptible_run() {
            return false;
        }

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }
        self.stream_done
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.is_querying = false;
        self.run_coordinator.finish_run();
        let error_message = format!("[Error: {reason}]");
        let assistant_message_id = self
            .sync_snapshot
            .active_assistant_message_id
            .clone()
            .or_else(|| {
                self.messages
                    .iter()
                    .rev()
                    .find(|message| message.role == crate::state::MessageRole::Assistant)
                    .map(|message| message.id.clone())
            })
            .unwrap_or_else(|| format!("msg_{}", self.messages.len()));
        let runtime_state = self.runtime_facade_state.clone();
        let reason_for_runtime = reason.to_string();
        let _ = tokio::time::timeout(std::time::Duration::from_millis(75), async move {
            runtime_state
                .mark_active_tool_turns_timed_out(&reason_for_runtime)
                .await;
            runtime_state.set_querying(false).await;
        })
        .await;
        self.stream_started_at = None;
        self.post_tool_turn_wait_started_at = None;
        self.current_turn_event_start_seq = None;

        {
            let mut sync = self.sync_store.lock().await;
            sync.mark_active_tools_with_result(format!("Result: ERROR\n{reason}"));
            sync.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                    message_id: Some(assistant_message_id.clone()),
                    text: error_message.clone(),
                    streaming: false,
                },
            );
            self.sync_snapshot = sync.snapshot();
        }
        self.current_tool_anchor_id = None;
        self.settle_unfinished_tool_parts(reason);

        self.upsert_assistant_message(assistant_message_id, error_message.clone());
        self.typewriter_position = error_message.chars().count();
        self.add_toast(reason.to_string(), "!");
        true
    }

    fn settle_unfinished_tool_parts(&self, reason: &str) {
        let Some(session_id) = self.session_manager.current_session_id() else {
            return;
        };
        match self
            .session_manager
            .settle_unfinished_tool_parts(session_id, reason)
        {
            Ok(0) => {}
            Ok(count) => tracing::debug!(
                "Settled {} unfinished tool part(s) after TUI cancellation",
                count
            ),
            Err(err) => tracing::warn!("Failed to settle unfinished tool parts: {}", err),
        }
    }

    pub fn timeout_active_run_immediate(&mut self, reason: &str) -> bool {
        if !self.has_interruptible_run() {
            return false;
        }

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }
        self.stream_done
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.is_querying = false;
        self.run_coordinator.finish_run();
        self.stream_started_at = None;
        self.post_tool_turn_wait_started_at = None;
        self.current_turn_event_start_seq = None;
        self.current_tool_anchor_id = None;
        self.settle_unfinished_tool_parts(reason);

        let error_message = format!("[Error: {reason}]");
        let assistant_message_id = self
            .sync_snapshot
            .active_assistant_message_id
            .clone()
            .or_else(|| {
                self.messages
                    .iter()
                    .rev()
                    .find(|message| message.role == crate::state::MessageRole::Assistant)
                    .map(|message| message.id.clone())
            })
            .unwrap_or_else(|| format!("msg_{}", self.messages.len()));
        if let Ok(mut sync) = self.sync_store.try_lock() {
            sync.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                    message_id: Some(assistant_message_id.clone()),
                    text: error_message.clone(),
                    streaming: false,
                },
            );
            self.sync_snapshot = sync.snapshot();
        }
        self.upsert_assistant_message(assistant_message_id, error_message);
        self.add_toast(reason.to_string(), "!");
        true
    }

    /// 检查是否有待回答的用户问题
    pub(super) async fn check_pending_question(&mut self) {
        let Some(ref engine) = self.streaming_engine else {
            return;
        };
        let Some(ref channel) = engine.tool_registry().ask_channel() else {
            return;
        };

        if let Some((question, options, tx)) = channel.take_pending().await {
            info!("CLI received pending question: {}", question);
            self.pending_question = Some(question);
            self.pending_question_options = options;
            self.question_response_tx = Some(tx);
            self.mode = AppMode::AskUser;
            self.composer.text.clear();
        }
    }

    /// 响应用户问题
    pub fn respond_to_question(&mut self, answer: String) {
        if let Some(tx) = self.question_response_tx.take() {
            let _ = tx.send(answer);
        }
        self.pending_question = None;
        self.pending_question_options.clear();
        self.mode = AppMode::Chat;
        self.composer.text.clear();
    }

    /// 构建工具上下文（复用 LSP / Worktree 管理器注入）
    pub(crate) async fn build_tool_context(&self) -> crate::tools::ToolContext {
        let session_id = self.session_manager.current_session_id().unwrap_or("tui");
        let working_dir = if let Some(ref wt) = self.worktree_manager {
            wt.current_worktree().await.unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            })
        } else {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        };
        let mut context = crate::tools::ToolContext::new(working_dir, session_id);
        context = context.with_session_store(self.session_manager.store());
        if let Some(ref lsp) = self.lsp_manager {
            context = context.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            context = context.with_worktree_manager(wt.clone());
        }
        context = context.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        if let Some(ref engine) = self.streaming_engine {
            context.permission_context.mode = engine.permission_mode();
            let session_rules = engine.session_permission_rules();
            context
                .permission_context
                .rules
                .always_allow
                .extend(session_rules.always_allow);
            context
                .permission_context
                .rules
                .always_deny
                .extend(session_rules.always_deny);
            context
                .permission_context
                .rules
                .always_ask
                .extend(session_rules.always_ask);
            context = context.with_cost_tracker(engine.cost_tracker().clone());
            context = context
                .with_llm_provider(engine.provider())
                .with_model(engine.model_name());
            if let Some(agent_manager) = engine.agent_manager() {
                context = context.with_agent_manager(agent_manager);
            }
            if let Some(mcp_manager) = engine.mcp_manager() {
                context = context.with_mcp_manager(mcp_manager);
            }
            if let Some(memory_manager) = engine.memory_manager() {
                context = context.with_memory_manager(memory_manager);
            }
            if let Some(tracker) = engine.read_tracker() {
                context = context.with_read_tracker(tracker.clone());
            }
        }
        context
    }

    /// 退出设置模式
    pub fn exit_settings(&mut self) {
        self.mode = AppMode::Chat;
        self.settings_state = None;
    }

    /// 保存设置
    pub fn save_settings(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut state) = self.settings_state {
            state.save_config()?;
            // 如果主题发生变化，同步更新 TuiApp 的主题
            self.theme = Arc::new(crate::tui::theme::Theme::from_name(&state.config.ui.theme));
        }
        Ok(())
    }

    /// 历史记录：上一条
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => Some(self.history.len() - 1),
            Some(0) => Some(0),
            Some(i) => Some(i - 1),
        };

        if let Some(idx) = new_index {
            self.history_index = new_index;
            if let Some(cmd) = self.history.get(idx) {
                self.composer.text.set_value(cmd.clone());
            }
        }
    }

    /// 历史记录：下一条
    pub fn history_next(&mut self) {
        if self.history.is_empty() {
            self.history_index = None;
            return;
        }
        match self.history_index {
            None => {}
            Some(i) if i + 1 >= self.history.len() => {
                self.history_index = None;
                self.composer.text.set_value(String::new());
            }
            Some(i) => {
                let new_i = i + 1;
                self.history_index = Some(new_i);
                if let Some(cmd) = self.history.get(new_i) {
                    self.composer.text.set_value(cmd.clone());
                }
            }
        }
    }

    pub fn prompt_history_lines(&self, limit: usize) -> Vec<String> {
        let limit = limit.max(1);
        let start = self.history.len().saturating_sub(limit);
        self.history
            .iter()
            .enumerate()
            .skip(start)
            .map(|(idx, prompt)| format!("{}. {}", idx + 1, compact_prompt_line(prompt, 120)))
            .collect()
    }

    pub fn save_prompt_stash_from_input(&mut self) -> bool {
        let draft = self.composer.text.value().trim();
        if draft.is_empty() || draft.starts_with("/prompt-stash") {
            return false;
        }
        self.prompt_stash = Some(self.composer.text.value().to_string());
        self.composer.text.clear();
        true
    }

    pub fn restore_prompt_stash_to_input(&mut self) -> bool {
        let Some(stash) = self.prompt_stash.take() else {
            return false;
        };
        self.composer.text.set_value(stash);
        true
    }

    pub fn clear_prompt_stash(&mut self) -> bool {
        self.prompt_stash.take().is_some()
    }

    pub fn prompt_stash_summary(&self) -> Option<String> {
        self.prompt_stash
            .as_deref()
            .map(|stash| compact_prompt_line(stash, 120))
    }

    pub fn open_prompt_picker(&mut self) {
        self.prompt_picker_selected = 0;
        self.push_mode(AppMode::PromptHistory);
    }

    pub fn close_prompt_picker(&mut self) {
        self.prompt_picker_selected = 0;
        self.pop_mode();
    }

    pub fn prompt_picker_items(&self) -> Vec<(String, String, String)> {
        let mut items = Vec::new();
        if let Some(stash) = &self.prompt_stash {
            items.push((
                "stash".to_string(),
                compact_prompt_line(stash, 96),
                stash.clone(),
            ));
        }
        for (idx, prompt) in self.history.iter().enumerate().rev().take(12) {
            items.push((
                format!("#{}", idx + 1),
                compact_prompt_line(prompt, 96),
                prompt.clone(),
            ));
        }
        items
    }

    pub fn prompt_picker_next(&mut self) {
        let len = self.prompt_picker_items().len();
        if len > 0 {
            self.prompt_picker_selected = (self.prompt_picker_selected + 1).min(len - 1);
        }
    }

    pub fn prompt_picker_prev(&mut self) {
        self.prompt_picker_selected = self.prompt_picker_selected.saturating_sub(1);
    }

    pub fn accept_prompt_picker_selection(&mut self) -> bool {
        let Some((_, _, content)) = self
            .prompt_picker_items()
            .get(self.prompt_picker_selected)
            .cloned()
        else {
            self.close_prompt_picker();
            return false;
        };
        self.composer.text.set_value(content);
        self.close_prompt_picker();
        true
    }

    pub fn open_composer_file_picker(&mut self, root: Option<&str>, multi_select: bool) -> String {
        let root = root
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(".");
        let path = std::path::Path::new(root);
        if !path.exists() {
            return format!("File picker root not found: {root}");
        }
        let mut state = crate::tui::components::file_browser::FileBrowserState::new(path);
        state.set_selection_mode(if multi_select {
            crate::tui::components::file_browser::FileSelectionMode::Multi
        } else {
            crate::tui::components::file_browser::FileSelectionMode::Single
        });
        self.file_picker_state = Some(state);
        self.file_picker_filtering = false;
        self.push_mode(AppMode::FilePicker);
        if multi_select {
            format!("Multi-select file picker opened at {root}. Space toggles, Enter confirms.")
        } else {
            format!("File picker opened at {root}.")
        }
    }

    pub fn open_composer_multi_file_picker(&mut self, root: Option<&str>) -> String {
        self.open_composer_file_picker(root, true)
    }

    pub fn open_composer_file_picker_with_filter(
        &mut self,
        root: Option<&str>,
        multi_select: bool,
    ) -> String {
        let message = self.open_composer_file_picker(root, multi_select);
        self.start_file_picker_filter();
        message
    }

    pub fn close_composer_file_picker(&mut self) {
        self.file_picker_state = None;
        self.file_picker_filtering = false;
        self.pop_mode();
    }

    pub fn file_picker_next(&mut self) {
        if let Some(state) = &mut self.file_picker_state {
            state.next();
        }
    }

    pub fn file_picker_prev(&mut self) {
        if let Some(state) = &mut self.file_picker_state {
            state.prev();
        }
    }

    pub fn accept_file_picker_selection(&mut self) -> String {
        let Some(state) = &mut self.file_picker_state else {
            return "File picker is not open.".to_string();
        };

        if state.selected_is_dir() {
            state.toggle_current();
            return "Toggled directory.".to_string();
        }

        if state.selection_mode() == crate::tui::components::file_browser::FileSelectionMode::Multi
        {
            let paths: Vec<std::path::PathBuf> = state.selected_paths().iter().cloned().collect();
            if paths.is_empty() {
                let Some(path) = state.selected_path().cloned() else {
                    return "No file selected.".to_string();
                };
                let result =
                    self.add_attachment_token_from_path(path, AttachmentSource::Autocomplete);
                self.close_composer_file_picker();
                return match result {
                    Some(label) => format!("Attached context: {label}"),
                    None => "File already attached.".to_string(),
                };
            }
            let mut attached = 0;
            for path in paths {
                if self
                    .add_attachment_token_from_path(path, AttachmentSource::Autocomplete)
                    .is_some()
                {
                    attached += 1;
                }
            }
            self.close_composer_file_picker();
            return format!("Attached {attached} file(s).");
        }

        let Some(path) = state.selected_path().cloned() else {
            return "No file selected.".to_string();
        };
        let result = self.add_attachment_token_from_path(path, AttachmentSource::Autocomplete);
        self.close_composer_file_picker();
        match result {
            Some(label) => format!("Attached context: {label}"),
            None => "File already attached.".to_string(),
        }
    }

    pub fn start_file_picker_filter(&mut self) {
        self.file_picker_filtering = true;
    }

    pub fn finish_file_picker_filter(&mut self) {
        self.file_picker_filtering = false;
    }

    pub fn push_file_picker_filter_char(&mut self, ch: char) {
        if let Some(state) = &mut self.file_picker_state {
            state.push_filter_char(ch);
        }
    }

    pub fn pop_file_picker_filter_char(&mut self) {
        if let Some(state) = &mut self.file_picker_state {
            state.pop_filter_char();
        }
    }

    pub fn clear_file_picker_filter(&mut self) {
        if let Some(state) = &mut self.file_picker_state {
            state.clear_filter();
        }
        self.file_picker_filtering = false;
    }

    /// 添加助手响应
    pub async fn add_assistant_response(&mut self, content: String) {
        self.is_querying = false;
        self.runtime_facade_state.set_querying(false).await;
        self.stream_started_at = None;

        // 保存助手消息到数据库。流式引擎绑定同一会话时由引擎负责持久化。
        if self.should_persist_messages_from_tui() {
            if let Err(e) = self
                .session_manager
                .add_message(MessageRole::Assistant, &content)
            {
                warn!("Failed to save assistant message: {}", e);
            }
        }

        let assistant_msg = MessageItem {
            id: format!("msg_{}", self.messages.len()),
            role: MessageRole::Assistant,
            content,
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(assistant_msg);
        self.scroll_to_bottom();
    }

    /// 添加系统消息
    pub fn add_system_message(&mut self, content: String) {
        let system_msg = MessageItem {
            id: format!("msg_{}", self.messages.len()),
            role: MessageRole::System,
            content,
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(system_msg);
        self.scroll_to_bottom();
    }

    /// 添加工具消息
    /// Add a Reasonix-style auto-dismissing toast notification
    pub fn add_toast(&mut self, message: impl Into<String>, glyph: &'static str) {
        self.toasts.push(Toast {
            message: message.into(),
            glyph,
            color: self.theme.tokens.tone.info,
            expires_at_tick: self.tick_count + 60,
        });
    }

    pub fn add_tool_message(&mut self, tool_call_id: String, content: String) {
        let tool_msg = MessageItem {
            id: format!("msg_{}", self.messages.len()),
            role: MessageRole::Tool,
            content,
            timestamp: std::time::SystemTime::now(),
            metadata: {
                let mut map = std::collections::HashMap::new();
                map.insert("tool_call_id".to_string(), tool_call_id);
                map
            },
        };
        self.messages.push(tool_msg);
        self.scroll_to_bottom();
    }

    /// 向上滚动
    pub fn scroll_up(&mut self) {
        let resolved = self.current_timeline_anchor_index();
        let next_offset = resolved.saturating_sub(1);
        self.set_manual_scroll_offset(next_offset);
    }

    /// 向下滚动
    pub fn scroll_down(&mut self) {
        let resolved = self.current_timeline_anchor_index();
        self.scroll_offset = resolved.saturating_add(1);
        // Re-pin if scrolled past the last timeline item.
        if self.scroll_offset >= self.timeline_item_count() {
            self.scroll_to_bottom();
        } else {
            self.pinned_to_bottom = false;
            self.scroll_anchor_id = self.timeline_stable_id_at(self.scroll_offset);
        }
    }

    /// 滚动到底部（显示最新消息）
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.timeline_item_count();
        self.scroll_anchor_id = None;
        self.pinned_to_bottom = true;
    }

    pub fn timeline_item_count(&self) -> usize {
        let render_session = self.render_session();
        crate::tui::view_model::timeline::timeline_items(&render_session).len()
    }

    fn render_session(&self) -> crate::tui::render_session::TuiRenderSession {
        let projected_messages = self.visible_timeline_messages();
        self.sync_snapshot.render_session(&projected_messages)
    }

    pub fn jump_to_timeline_target(&mut self, target: &str) -> String {
        let normalized = target.trim().to_ascii_lowercase();
        if normalized.is_empty() || matches!(normalized.as_str(), "bottom" | "latest" | "end") {
            self.scroll_to_bottom();
            return "Jumped to latest message.".to_string();
        }

        let target_index = {
            let render_session = self.render_session();
            let timeline = crate::tui::view_model::timeline::timeline_items(&render_session);
            match normalized.as_str() {
                "user" | "prompt" => timeline
                    .iter()
                    .rposition(crate::tui::view_model::timeline::TimelineItem::is_user_message),
                "failed" | "failure" | "error" => timeline.iter().rposition(|item| {
                    matches!(
                        item,
                        crate::tui::view_model::timeline::TimelineItem::Message { parts: Some(parts), .. }
                            if crate::tui::view_model::timeline::tool_runs_from_parts(parts)
                                .iter()
                                .any(|run| matches!(
                                run.status,
                                crate::tui::tool_view::ToolRunStatus::Failed
                                    | crate::tui::tool_view::ToolRunStatus::TimedOut
                                    | crate::tui::tool_view::ToolRunStatus::Cancelled
                            ))
                    )
                }),
                "edit" | "change" | "write" => timeline.iter().rposition(|item| {
                    matches!(
                        item,
                        crate::tui::view_model::timeline::TimelineItem::Message { parts: Some(parts), .. }
                            if crate::tui::view_model::timeline::tool_runs_from_parts(parts)
                                .iter()
                                .any(|run| matches!(
                                run.name.as_str(),
                                "file_write" | "file_edit" | "file_patch" | "format"
                            ))
                    )
                }),
                other => {
                    return format!(
                        "Unknown jump target: {other}. Use user, failed, edit, or latest."
                    );
                }
            }
        };

        if let Some(index) = target_index {
            self.set_manual_scroll_offset(index);
            format!("Jumped to {normalized} timeline item.")
        } else {
            format!("No {normalized} timeline item found.")
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.set_manual_scroll_offset(0);
    }

    pub fn scroll_to_message_index(&mut self, target_message_index: usize) -> bool {
        let target_index = {
            let render_session = self.render_session();
            let timeline = crate::tui::view_model::timeline::timeline_items(&render_session);
            timeline.iter().position(|item| {
                matches!(
                    item,
                    crate::tui::view_model::timeline::TimelineItem::Message { message_index, .. }
                        if *message_index == target_message_index
                )
            })
        };

        let Some(index) = target_index else {
            return false;
        };
        self.set_manual_scroll_offset(index);
        true
    }

    /// 向上滚动半页（Vim Ctrl+U）
    pub fn scroll_up_half_page(&mut self) {
        let resolved = self.current_timeline_anchor_index();
        self.set_manual_scroll_offset(resolved.saturating_sub(5));
    }

    /// 向下滚动半页（Vim Ctrl+D）
    pub fn scroll_down_half_page(&mut self) {
        let resolved = self.current_timeline_anchor_index();
        self.scroll_offset = resolved.saturating_add(5);
        if self.scroll_offset >= self.timeline_item_count() {
            self.scroll_to_bottom();
        } else {
            self.pinned_to_bottom = false;
            self.scroll_anchor_id = self.timeline_stable_id_at(self.scroll_offset);
        }
    }

    pub fn toggle_collapse_at_scroll_anchor(&mut self) -> bool {
        let message_index = {
            let render_session = self.render_session();
            let timeline = crate::tui::view_model::timeline::timeline_items(&render_session);
            if timeline.is_empty() {
                return false;
            }

            let anchor = crate::tui::view_model::timeline::resolve_scroll_offset(
                &timeline,
                self.scroll_offset,
                self.scroll_anchor_id.as_deref(),
            )
            .min(timeline.len().saturating_sub(1));
            timeline
                .iter()
                .take(anchor + 1)
                .rev()
                .map(|item| match item {
                    crate::tui::view_model::timeline::TimelineItem::Message {
                        message_index,
                        ..
                    } => *message_index,
                })
                .next()
        };

        let Some(idx) = message_index else {
            return false;
        };

        if self.collapsed_indices.contains(&idx) {
            self.collapsed_indices.remove(&idx);
        } else {
            self.collapsed_indices.insert(idx);
        }
        true
    }

    pub fn toggle_reasoning_at_scroll_anchor(&mut self) -> bool {
        let message_id = {
            let render_session = self.render_session();
            let timeline = crate::tui::view_model::timeline::timeline_items(&render_session);
            if timeline.is_empty() {
                return false;
            }

            let anchor = crate::tui::view_model::timeline::resolve_scroll_offset(
                &timeline,
                self.scroll_offset,
                self.scroll_anchor_id.as_deref(),
            )
            .min(timeline.len().saturating_sub(1));
            timeline
                .iter()
                .take(anchor + 1)
                .rev()
                .find_map(|item| match item {
                    crate::tui::view_model::timeline::TimelineItem::Message {
                        id,
                        role,
                        parts,
                        ..
                    } if *role == crate::tui::render_session::TuiRenderRole::Assistant
                        && assistant_has_hidden_reasoning(id, *parts) =>
                    {
                        Some(id.to_string())
                    }
                    crate::tui::view_model::timeline::TimelineItem::Message { .. } => None,
                })
        };

        let Some(message_id) = message_id else {
            return false;
        };

        if self.expanded_reasoning_message_id.as_deref() == Some(message_id.as_str()) {
            self.expanded_reasoning_message_id = None;
        } else {
            self.expanded_reasoning_message_id = Some(message_id);
        }
        true
    }

    pub fn toggle_collapsible_at_scroll_anchor(&mut self) -> bool {
        let render_session = self.render_session();
        let timeline = crate::tui::view_model::timeline::timeline_items(&render_session);
        if timeline.is_empty() {
            return false;
        }

        let anchor = crate::tui::view_model::timeline::resolve_scroll_offset(
            &timeline,
            self.scroll_offset,
            self.scroll_anchor_id.as_deref(),
        )
        .min(timeline.len().saturating_sub(1));

        for item in timeline.iter().take(anchor + 1).rev() {
            match item {
                crate::tui::view_model::timeline::TimelineItem::Message {
                    id,
                    role,
                    content,
                    parts,
                    ..
                } => {
                    if *role == crate::tui::render_session::TuiRenderRole::Assistant
                        && !content.trim().is_empty()
                    {
                        let part_id = parts
                            .and_then(|ps| {
                                ps.iter()
                                    .find(|p| p.kind == crate::tui::sync_store::TuiPartKind::Text)
                                    .map(|p| p.id.clone())
                            })
                            .unwrap_or_else(|| {
                                crate::tui::sync_store::part_id_for(
                                    id,
                                    crate::tui::sync_store::TuiPartKind::Text,
                                )
                            });
                        if self.expanded_inline_message_part_ids.contains(&part_id) {
                            self.expanded_inline_message_part_ids.remove(&part_id);
                        } else {
                            self.expanded_inline_message_part_ids.insert(part_id);
                        }
                        return true;
                    }

                    if *role == crate::tui::render_session::TuiRenderRole::User {
                        if let Some(parts) = parts {
                            let runs =
                                crate::tui::view_model::timeline::tool_runs_from_parts(parts);
                            if let Some(run) = runs.iter().find(|r| {
                                crate::tui::view_model::tool_rows::tool_row_for_run(r, 100).visible
                            }) {
                                let id = run.id.clone();
                                if self.expanded_inline_tool_ids.contains(&id) {
                                    self.expanded_inline_tool_ids.remove(&id);
                                } else {
                                    self.expanded_inline_tool_ids.insert(id);
                                }
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    pub fn current_timeline_anchor_index(&self) -> usize {
        let render_session = self.render_session();
        let timeline = crate::tui::view_model::timeline::timeline_items(&render_session);
        crate::tui::view_model::timeline::resolve_scroll_offset(
            &timeline,
            self.scroll_offset,
            self.scroll_anchor_id.as_deref(),
        )
    }

    fn set_manual_scroll_offset(&mut self, offset: usize) {
        let count = self.timeline_item_count();
        if offset >= count {
            self.scroll_to_bottom();
            return;
        }
        self.scroll_offset = offset;
        self.scroll_anchor_id = self.timeline_stable_id_at(offset);
        self.pinned_to_bottom = false;
    }

    fn timeline_stable_id_at(&self, index: usize) -> Option<String> {
        let render_session = self.render_session();
        let timeline = crate::tui::view_model::timeline::timeline_items(&render_session);
        timeline.get(index).map(|item| item.stable_id().to_string())
    }

    /// 获取可见消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 当前模型名称（用于状态展示）
    pub fn clear_tool_transcript(&mut self) {
        self.sync_snapshot.clear_tool_parts();
        self.current_tool_anchor_id = None;
        self.expanded_tool_run_id = None;
        self.stream_usage_snapshot = None;
    }

    /// 获取消息（考虑滚动）
    pub fn visible_messages(&self) -> &[MessageItem] {
        &self.messages
    }

    /// 获取 timeline 使用的消息投影。
    ///
    /// Historical messages come from the session/app state, while the active
    /// streaming assistant message is projected from the TUI sync store
    /// message/part model.
    pub fn visible_timeline_messages(&self) -> Vec<MessageItem> {
        self.sync_snapshot.project_message_items(&self.messages)
    }

    /// 设置错误信息
    pub fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
        self.is_querying = false;
        self.stream_started_at = None;
    }

    /// 清除错误
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }
}

fn assistant_has_hidden_reasoning(
    content: &str,
    parts: Option<&[crate::tui::sync_store::TuiMessagePart]>,
) -> bool {
    if let Some(parts) = parts {
        let has_thinking = parts.iter().any(|p| {
            p.kind == crate::tui::sync_store::TuiPartKind::Thinking && !p.text.trim().is_empty()
        });
        if has_thinking {
            return true;
        }
    }
    crate::tui::view_model::reasoning::assistant_reasoning_view(content).has_hidden_reasoning()
}

fn compact_prompt_line(prompt: &str, max_chars: usize) -> String {
    let normalized = prompt
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    let source = if normalized.is_empty() {
        prompt.trim()
    } else {
        normalized.as_str()
    };
    let mut out = String::new();
    for ch in source.chars().take(max_chars) {
        out.push(ch);
    }
    if source.chars().count() > max_chars {
        out.push('…');
    }
    out
}

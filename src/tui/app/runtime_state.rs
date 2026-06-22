use super::*;

impl TuiApp {
    pub fn insert_paste(&mut self, text: String) {
        if text.is_empty() {
            return;
        }

        let trimmed = text.trim();
        let char_count = text.chars().count();
        let line_count = text.lines().count().max(1);

        if trimmed.starts_with("data:image") {
            return self.insert_image_paste(text);
        }

        if line_count == 1 && !trimmed.is_empty() {
            if let Some(_label) =
                self.add_attachment_token_from_path(trimmed, AttachmentSource::Pasted)
            {
                return;
            }
        }

        if char_count < LONG_PASTE_CHAR_THRESHOLD && line_count < LONG_PASTE_LINE_THRESHOLD {
            self.composer.text.insert_str(&text);
            return;
        }

        let paste_id = self
            .composer
            .parts
            .iter()
            .filter(|part| matches!(part, ComposerPart::PastedText { .. }))
            .count()
            + 1;
        let placeholder = format!(
            "[[paste:{} {} lines {} chars]]",
            paste_id, line_count, char_count
        );
        self.composer
            .add_pasted_text(format!("paste {}", paste_id), placeholder.clone(), text);
        self.composer.text.insert_str(&placeholder);
    }

    pub(super) fn insert_image_paste(&mut self, text: String) {
        let paste_id = self
            .composer
            .parts
            .iter()
            .filter(|part| matches!(part, ComposerPart::Image { .. }))
            .count()
            + 1;
        let char_count = text.chars().count();
        let placeholder = format!("[[image:{} {} chars]]", paste_id, char_count);
        self.composer.add_image(placeholder.clone(), text);
        self.composer.text.insert_str(&placeholder);
    }

    pub fn pasted_block_count(&self) -> usize {
        self.composer
            .parts
            .iter()
            .filter(|part| match part {
                ComposerPart::PastedText { placeholder, .. } => {
                    self.composer.text.value().contains(placeholder)
                }
                ComposerPart::Image { label, .. } => self.composer.text.value().contains(label),
                _ => false,
            })
            .count()
    }

    pub fn pasted_block_summaries(&self) -> Vec<String> {
        self.composer
            .parts
            .iter()
            .filter(|part| match part {
                ComposerPart::PastedText { placeholder, .. } => {
                    self.composer.text.value().contains(placeholder)
                }
                ComposerPart::Image { label, .. } => self.composer.text.value().contains(label),
                _ => false,
            })
            .map(|part| match part {
                ComposerPart::PastedText { content, .. } => {
                    let line_count = content.lines().count().max(1);
                    let char_count = content.chars().count();
                    format!("{} lines / {} chars", line_count, char_count)
                }
                ComposerPart::Image { content, .. } => {
                    format!("{} chars", content.chars().count())
                }
                _ => String::new(),
            })
            .collect()
    }

    pub fn open_paste_viewer(&mut self, index: Option<usize>) -> bool {
        let active_blocks: Vec<_> = self
            .composer
            .parts
            .iter()
            .filter(|part| match part {
                ComposerPart::PastedText { placeholder, .. } => {
                    self.composer.text.value().contains(placeholder)
                }
                ComposerPart::Image { label, .. } => self.composer.text.value().contains(label),
                _ => false,
            })
            .collect();
        if active_blocks.is_empty() {
            return false;
        }
        let selected = index.unwrap_or(1).saturating_sub(1);
        let Some(block) = active_blocks.get(selected) else {
            return false;
        };
        match block {
            ComposerPart::PastedText { content, .. } => {
                let line_count = content.lines().count().max(1);
                let char_count = content.chars().count();
                self.tool_viewer_title = format!(
                    "Paste {} ({} lines / {} chars)",
                    selected + 1,
                    line_count,
                    char_count
                );
                self.tool_viewer_content = content.clone();
            }
            ComposerPart::Image { content, .. } => {
                let char_count = content.chars().count();
                self.tool_viewer_title = format!("Image {} ({} chars)", selected + 1, char_count);
                self.tool_viewer_content = content.clone();
            }
            _ => return false,
        }
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub fn attach_context_path(&mut self, raw_path: &str) -> Result<String, String> {
        let raw_path = raw_path.trim();
        if raw_path.is_empty() {
            return Err("Usage: /attach <path>|remove <n>|clear|list".to_string());
        }
        if self.composer.attachment_count() >= 12 {
            return Err("Attachment limit reached for this prompt.".to_string());
        }

        let token = AttachmentToken::from_path(raw_path, AttachmentSource::File);
        let display = token.label.clone();
        if self.composer.has_file(&token.path) {
            return Ok(format!("Already attached: {display}"));
        }

        self.composer.add_file(raw_path, AttachmentSource::File);
        Ok(format!("Attached context: {display}"))
    }

    pub fn remove_composer_attachment(&mut self, one_based_index: usize) -> Option<String> {
        if one_based_index == 0 {
            return None;
        }
        let target = self
            .composer
            .attachment_paths()
            .get(one_based_index - 1)
            .cloned()?;

        if let Some(token) = self.composer.remove_file_by_path(&target) {
            return Some(token.label);
        }

        let index = self.composer.parts.iter().position(|part| match part {
            ComposerPart::File(token) => token.label == target || token.path == target,
            ComposerPart::PastedText {
                label, placeholder, ..
            } => label == &target || placeholder == &target,
            ComposerPart::Image { label, .. } => label == &target,
        })?;
        match self.composer.parts.remove(index) {
            ComposerPart::File(token) => Some(token.label),
            ComposerPart::PastedText { label, .. } | ComposerPart::Image { label, .. } => {
                Some(label)
            }
        }
    }

    pub fn remove_last_composer_attachment(&mut self) -> Option<String> {
        self.composer.remove_last_part().map(|part| match part {
            ComposerPart::File(token) => token.label,
            ComposerPart::PastedText { label, .. } | ComposerPart::Image { label, .. } => label,
        })
    }

    pub fn clear_composer_attachments(&mut self) -> usize {
        let count = self.composer.attachment_count();
        self.composer.parts.clear();
        count
    }

    pub fn composer_attachment_summaries(&self) -> Vec<String> {
        self.composer
            .attachment_paths()
            .iter()
            .enumerate()
            .map(|(idx, path)| format!("[{}] {}", idx + 1, attachment_summary(path, 44)))
            .collect()
    }

    pub fn composer_attachment_count(&self) -> usize {
        self.composer.attachment_count()
    }

    pub fn toggle_pinned_session(&mut self, session_id: &str) -> bool {
        let pinned = toggle_pinned_session_list(&mut self.pinned_sessions, session_id, 9);
        if let Err(err) = self.persist_pinned_sessions() {
            warn!("Failed to persist pinned sessions: {}", err);
            self.add_toast(
                format!("Pinned sessions kept for this run; save failed: {}", err),
                "!",
            );
        }
        pinned
    }

    pub(super) fn persist_pinned_sessions(&self) -> anyhow::Result<()> {
        let mut config = crate::services::config::AppConfig::load().unwrap_or_default();
        config.ui.pinned_sessions = self.pinned_sessions.clone();
        config.save()?;
        crate::services::config::init_runtime_config(config);
        Ok(())
    }

    pub fn composer_attachment_tokens(&self) -> Vec<AttachmentToken> {
        self.composer.attachment_tokens()
    }

    pub fn composer_attachments(&self) -> Vec<String> {
        self.composer.attachment_paths()
    }

    /// Insert an attachment token (for paste/autocomplete intake) if not duplicate.
    pub fn add_attachment_token_from_path(
        &mut self,
        path: impl AsRef<std::path::Path>,
        source: AttachmentSource,
    ) -> Option<String> {
        if self.composer.attachment_count() >= 12 {
            return None;
        }
        let token = self.composer.add_file(path, source)?;
        Some(token.label.clone())
    }

    pub fn remove_composer_attachment_token(&mut self, id: &str) -> Option<AttachmentToken> {
        let index = self.composer.parts.iter().position(|part| match part {
            ComposerPart::File(token) => token.id == id,
            _ => false,
        })?;
        match self.composer.parts.remove(index) {
            ComposerPart::File(token) => Some(token),
            other => {
                self.composer.parts.insert(index, other);
                None
            }
        }
    }

    pub fn open_attachment_viewer(&mut self, index: Option<usize>) -> bool {
        let paths = self.composer_attachment_paths();
        if paths.is_empty() {
            return false;
        }
        let selected = index.unwrap_or(1).saturating_sub(1);
        let Some(path) = paths.get(selected).cloned() else {
            return false;
        };
        let absolute = resolve_attachment_path(&path);
        let Ok(metadata) = std::fs::metadata(&absolute) else {
            self.tool_viewer_title = format!("Attachment {}: {}", selected + 1, path);
            self.tool_viewer_content = format!("Attachment path is no longer available:\n{path}");
            self.tool_viewer_scroll_offset = 0;
            self.mode = AppMode::ToolViewer;
            return true;
        };

        self.tool_viewer_title = format!("Attachment {}: {}", selected + 1, path);
        self.tool_viewer_content = if metadata.is_dir() {
            attachment_directory_preview(&absolute)
        } else {
            attachment_file_preview(&absolute, metadata.len())
        };
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub(super) fn composer_attachment_paths(&self) -> Vec<String> {
        self.composer.attachment_paths()
    }

    #[cfg(test)]
    pub(super) fn expand_paste_placeholders(&self, content: &str) -> String {
        let mut expanded = content.to_string();
        for (_id, _label, placeholder, paste_content) in self.composer.pasted_text_parts() {
            expanded = expanded.replace(placeholder, paste_content);
        }
        expanded
    }

    pub(super) fn clear_active_skill_rules(&mut self) {
        let Some(engine) = &self.streaming_engine else {
            self.active_skill_permission_rules.clear();
            return;
        };
        for (decision, pattern) in self.active_skill_permission_rules.drain(..) {
            engine.remove_session_permission_rule(&decision, &pattern);
        }
    }

    pub(super) fn apply_skill_invocation_policy(
        &mut self,
        invocation: &crate::skills::SkillInvocation,
    ) {
        self.clear_active_skill_rules();
        let Some(engine) = &self.streaming_engine else {
            return;
        };
        for pattern in &invocation.allowed_tools {
            engine.add_session_permission_rule("allow", pattern);
            self.active_skill_permission_rules
                .push(("allow".to_string(), pattern.to_string()));
        }
        for pattern in &invocation.disallowed_tools {
            engine.add_session_permission_rule("deny", pattern);
            self.active_skill_permission_rules
                .push(("deny".to_string(), pattern.to_string()));
        }
    }

    /// 发送消息到 LLM（核心逻辑，可被 skill 调用复用）
    pub(crate) async fn send_message(&mut self, content: String) {
        if content.trim().is_empty() {
            return;
        }
        if self.paused {
            self.add_system_message(
                "Agent is paused. Use `/pause resume` to continue sending messages.".to_string(),
            );
            return;
        }

        debug!("Submitting message: {}", content);

        if self.run_coordinator.is_active() || self.is_querying {
            let queued = self.persist_queued_session_input(&content);
            let message = if queued {
                "A run is already active. Queued your message for the next turn.".to_string()
            } else {
                "A run is already active. Wait for it to complete or press Esc to cancel."
                    .to_string()
            };
            self.add_system_message(message);
            return;
        }

        // 取消之前的流式任务（如果有）
        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
            self.runtime_facade_state.mark_cancelled().await;
        }

        // 添加到历史
        self.history.push_back(content.clone());
        if self.history.len() > 100 {
            self.history.pop_front();
        }

        // 添加用户消息
        let user_msg_id = format!("msg_{}", self.messages.len());
        let user_msg = MessageItem {
            id: user_msg_id.clone(),
            role: MessageRole::User,
            content: content.clone(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(user_msg);

        // 如果流式引擎已经绑定同一条持久化会话，消息由引擎统一写入，
        // 避免 UI 和引擎重复插入 user/assistant 历史。
        if self.should_persist_messages_from_tui() {
            if let Err(e) = self
                .session_manager
                .add_message(MessageRole::User, &content)
            {
                warn!("Failed to save user message: {}", e);
            }
        }

        // 更新会话标题（基于第一条用户消息）
        if self.session_manager.current_session_title() == "New Session" {
            let title = self.session_manager.generate_title(&self.messages);
            if let Err(e) = self.session_manager.update_title(&title) {
                warn!("Failed to update session title: {}", e);
            }
        }

        // 标记正在查询
        if !self.run_coordinator.start_run() {
            self.add_system_message(
                "A run is already active. Wait for it to complete or press Esc to cancel."
                    .to_string(),
            );
            return;
        }
        self.is_querying = true;
        self.stream_started_at = Some(std::time::Instant::now());
        self.post_tool_turn_wait_started_at = None;
        self.current_turn_event_start_seq = Some(self.current_session_max_event_seq());

        // Only auto-scroll when pinned
        if self.pinned_to_bottom {
            self.scroll_to_bottom();
        }

        // 使用流式引擎发送查询
        if let Some(engine) = self.streaming_engine.clone() {
            {
                let mut usage = self.stream_usage.lock().await;
                *usage = None;
            }
            self.runtime_facade_state.reset().await;
            self.runtime_facade_state.set_querying(true).await;
            self.runtime_facade_state.set_stream_usage(None).await;
            self.current_tool_anchor_id = Some(user_msg_id.clone());
            self.stream_usage_snapshot = None;
            self.runtime_state_snapshot = self.build_runtime_state_snapshot();
            self.sync_context_runtime_state().await;
            // 标记流未完成
            self.stream_done.store(false, Ordering::SeqCst);

            // 创建助手消息占位符
            let assistant_msg = MessageItem {
                id: format!("msg_{}", self.messages.len()),
                role: MessageRole::Assistant,
                content: String::new(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            };
            let assistant_msg_id = assistant_msg.id.clone();
            self.messages.push(assistant_msg);
            self.scroll_to_bottom();
            {
                let mut sync = self.sync_store.lock().await;
                sync.start_turn(user_msg_id.clone(), assistant_msg_id.clone());
                self.sync_snapshot = sync.snapshot();
            }

            // 启动流式查询（在后台任务中）
            let controller = RuntimeController::with_runtime_state(
                engine.clone(),
                Arc::new(self.runtime_facade_state.clone()),
            );
            let sync_store_clone = self.sync_store.clone();
            let usage_clone = self.stream_usage.clone();
            let runtime_facade_state_clone = self.runtime_facade_state.clone();
            let done_flag = self.stream_done.clone();
            let user_msg = content.clone();
            let parent_message_id = user_msg_id.clone();
            let assistant_message_id = assistant_msg_id.clone();
            let agent_mode = self.agent_mode;

            controller.set_memory_policy(
                self.memory_use,
                self.memory_generate,
                self.memory_recall_mode.clone(),
            );

            let handle = tokio::spawn(async move {
                let mut stream = controller
                    .submit_stream_turn_with_agent_mode_and_parent_message_id(
                        user_msg,
                        agent_mode,
                        parent_message_id.clone(),
                    )
                    .await;
                let mut projection_bus = crate::session_store::SessionProjectionEventBus::new();

                while let Some(event) = stream.next().await {
                    let projection_event =
                        crate::session_store::SessionProjectionEvent::from_stream_event(
                            &event,
                            Some(&parent_message_id),
                            None,
                        );
                    let projection_envelope = projection_bus.publish(projection_event);
                    {
                        let mut sync = sync_store_clone.lock().await;
                        sync.apply_projection_envelope(&projection_envelope);
                    }
                    runtime_facade_state_clone
                        .process_projection_event(&projection_envelope.event)
                        .await;
                    match &projection_envelope.event {
                        crate::session_store::SessionProjectionEvent::Completed => {
                            runtime_facade_state_clone.set_querying(false).await;
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        crate::session_store::SessionProjectionEvent::Usage {
                            prompt_tokens,
                            completion_tokens,
                            reasoning_tokens,
                            cached_tokens,
                            cache_write_tokens,
                        } => {
                            let mut usage = usage_clone.lock().await;
                            *usage = Some(StreamUsageSnapshot {
                                prompt_tokens: *prompt_tokens,
                                completion_tokens: *completion_tokens,
                                reasoning_tokens: *reasoning_tokens,
                                cached_tokens: *cached_tokens,
                                cache_write_tokens: *cache_write_tokens,
                            });
                            runtime_facade_state_clone
                                .set_stream_usage(Some(
                                    crate::engine::runtime_facade::StreamUsageSnapshot {
                                        prompt_tokens: *prompt_tokens,
                                        completion_tokens: *completion_tokens,
                                        reasoning_tokens: *reasoning_tokens,
                                        cached_tokens: *cached_tokens,
                                        cache_write_tokens: *cache_write_tokens,
                                    },
                                ))
                                .await;
                        }
                        crate::session_store::SessionProjectionEvent::Error { message } => {
                            let mut sync = sync_store_clone.lock().await;
                            sync.apply_projection_event(
                                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                                    message_id: Some(assistant_message_id.clone()),
                                    text: format!("[Error: {message}]"),
                                    streaming: false,
                                },
                            );
                            drop(sync);
                            runtime_facade_state_clone.set_querying(false).await;
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        crate::session_store::SessionProjectionEvent::RuntimeDiagnostic {
                            diagnostic,
                        } => {
                            runtime_facade_state_clone
                                .process_diagnostic(diagnostic)
                                .await;
                        }
                        _ => {}
                    }
                }
                // 确保即使流结束也标记完成
                {
                    let mut sync = sync_store_clone.lock().await;
                    sync.mark_stream_closed();
                }
                runtime_facade_state_clone.set_querying(false).await;
                done_flag.store(true, Ordering::SeqCst);
            });
            self.stream_handle = Some(handle);
        } else {
            // 没有引擎，使用占位响应
            self.add_assistant_response(format!(
                "AI engine not available. Set one provider key: {}.",
                crate::services::api::provider::provider_key_env_hint()
            ))
            .await;
        }
    }

    pub(super) fn should_persist_messages_from_tui(&self) -> bool {
        let Some(engine) = &self.streaming_engine else {
            return true;
        };
        let Some((_store, session_id)) = engine.session_binding() else {
            return true;
        };
        !self.session_manager.is_current_session(&session_id)
    }

    pub(super) fn persist_queued_session_input(&self, content: &str) -> bool {
        let Some(engine) = &self.streaming_engine else {
            return false;
        };
        let Some((store, session_id)) = engine.session_binding() else {
            return false;
        };
        let delivery = if self.has_active_goal() {
            crate::engine::run_coordinator::InputDelivery::Steer
        } else {
            crate::engine::run_coordinator::InputDelivery::Queue
        };
        let conn = store.shared_conn();
        let conn = conn.lock().expect("tui app sqlite conn lock poisoned");
        crate::engine::run_coordinator::persist_session_input(&conn, &session_id, content, delivery)
            .is_ok()
    }

    pub(super) fn persist_goal_continuation(&self, content: &str) -> bool {
        let Some(engine) = &self.streaming_engine else {
            return false;
        };
        let Some((store, session_id)) = engine.session_binding() else {
            return false;
        };
        let conn = store.shared_conn();
        let conn = conn.lock().expect("tui app sqlite conn lock poisoned");
        crate::engine::run_coordinator::persist_session_input(
            &conn,
            &session_id,
            content,
            crate::engine::run_coordinator::InputDelivery::Queue,
        )
        .is_ok()
    }

    pub(super) fn has_active_goal(&self) -> bool {
        self.goal_runner
            .as_ref()
            .and_then(|runner| {
                let session_id = self.session_manager.current_session_id()?;
                runner.has_active_goal(session_id).ok()
            })
            .unwrap_or(false)
    }

    pub(super) fn promote_queued_session_input(&self) -> Option<String> {
        let engine = self.streaming_engine.as_ref()?;
        let (store, session_id) = engine.session_binding()?;
        let conn = store.shared_conn();
        let conn = conn.lock().expect("tui app sqlite conn lock poisoned");
        crate::engine::run_coordinator::promote_session_input(&conn, &session_id)
            .ok()
            .flatten()
    }

    /// 刷新当前响应（从缓冲区读取最新的流式内容，带打字机效果）
    pub async fn refresh_response(&mut self) {
        if !self.is_querying {
            return;
        }

        self.sync_snapshot = self.sync_store.lock().await.snapshot();
        // 读取响应长度（最小化锁持有时间，避免克隆整个字符串）
        let total_chars = self.sync_snapshot.assistant_message_content.chars().count();

        // 更新打字机位置
        if self.typewriter_position < total_chars {
            let remaining = total_chars - self.typewriter_position;
            self.typewriter_position += remaining.min(12); // ~48 chars/sec at 4Hz tick
        }

        // 读取需要显示的内容和工具状态
        let display_response: String = self
            .sync_snapshot
            .assistant_message_content
            .chars()
            .take(self.typewriter_position)
            .collect();
        self.stream_usage_snapshot = self.sync_snapshot.usage;
        self.runtime_facade_state.check_slow_warning().await;
        if self.runtime_facade_state.check_timeout().await {
            self.facade_snapshot = self.runtime_facade_state.snapshot().await;
            let reason = self
                .facade_snapshot
                .provider_request
                .message
                .clone()
                .unwrap_or_else(|| "provider request timed out".to_string());
            self.timeout_active_run(&reason).await;
            return;
        }
        self.facade_snapshot = self.runtime_facade_state.snapshot().await;
        self.sync_tool_runs_from_spine_snapshot();
        if self.recover_persisted_final_answer_if_available().await {
            return;
        }
        if let Some(reason) = self.provider_wait_timeout_reason() {
            self.timeout_active_run(&reason).await;
            return;
        }

        // 更新最后一条助手消息
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.content = display_response;
            }
        }

        self.scroll_to_bottom();
    }

    pub(super) fn sync_tool_runs_from_spine_snapshot(&mut self) {
        let turns = self.facade_snapshot.tool_turns.clone();
        let mut sync_store = TuiSyncStore::from_snapshot(self.sync_snapshot.clone());
        let mut projection_bus = crate::session_store::SessionProjectionEventBus::from_seq(
            self.sync_snapshot.last_projection_seq,
        );
        for turn in turns {
            let Some(parent_message_id) = turn.parent_message_id.clone() else {
                continue;
            };
            let event = crate::session_store::SessionProjectionEvent::ToolPartUpdated {
                message_id: Some(parent_message_id),
                tool_call_id: turn.id.clone(),
                tool_name: turn.name.clone(),
                status: Some(tool_part_status_from_turn_phase(turn.phase).to_string()),
                input_args: turn.arguments_preview.clone(),
                result: turn.result_preview.clone().or_else(|| turn.failure.clone()),
                metadata: Some(serde_json::json!({
                    "replay_source": "runtime_facade_spine",
                    "phase": turn.phase.label(),
                    "success": !matches!(
                        turn.phase,
                        ToolTurnPhase::Failed | ToolTurnPhase::Cancelled | ToolTurnPhase::TimedOut
                    ),
                })),
                result_data: None,
            };
            let envelope = projection_bus.publish(event);
            sync_store.apply_projection_envelope(&envelope);
        }
        self.sync_snapshot = sync_store.snapshot();
    }

    pub(super) async fn recover_persisted_final_answer_if_available(&mut self) -> bool {
        if std::env::var("PRIORITY_AGENT_TUI_DISABLE_DB_FINAL_RECOVERY")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        {
            return false;
        }
        if !self.is_querying || !self.has_observed_tool_result_in_spine() {
            return false;
        }

        let Some(current_user_content) = self
            .messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::User)
            .map(|message| message.content.trim().to_string())
        else {
            return false;
        };
        let Some(final_answer) = self.recoverable_persisted_final_answer(&current_user_content)
        else {
            return false;
        };

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }
        self.typewriter_position = final_answer.chars().count();
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.content = final_answer.clone();
                let mut sync_store = TuiSyncStore::from_snapshot(self.sync_snapshot.clone());
                let mut projection_bus = crate::session_store::SessionProjectionEventBus::from_seq(
                    self.sync_snapshot.last_projection_seq,
                );
                let envelope = projection_bus.publish(
                    crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                        message_id: Some(last_msg.id.clone()),
                        text: final_answer.clone(),
                        streaming: false,
                    },
                );
                sync_store.apply_projection_envelope(&envelope);
                self.sync_snapshot = sync_store.snapshot();
            }
        }
        self.runtime_facade_state.mark_tool_turns_persisted().await;
        self.runtime_facade_state.set_querying(false).await;
        self.facade_snapshot = self.runtime_facade_state.snapshot().await;
        if let Some(session_id) = self
            .session_manager
            .current_session_id()
            .map(str::to_string)
        {
            if let Err(err) = self.hydrate_persisted_projection_for_session(&session_id) {
                warn!(
                    "Failed to hydrate persisted projection during recovery: {}",
                    err
                );
                self.sync_tool_runs_from_spine_snapshot();
            }
        } else {
            self.sync_tool_runs_from_spine_snapshot();
        }
        self.stream_done.store(true, Ordering::SeqCst);
        self.is_querying = false;
        self.run_coordinator.finish_run();
        self.stream_started_at = None;
        self.current_tool_anchor_id = None;
        self.scroll_to_bottom();
        true
    }

    pub(super) fn has_observed_tool_result_in_spine(&self) -> bool {
        self.facade_snapshot.tool_turns.iter().any(|turn| {
            matches!(
                turn.phase,
                ToolTurnPhase::ResultObserved
                    | ToolTurnPhase::SentBackToModel
                    | ToolTurnPhase::FinalAnswer
                    | ToolTurnPhase::Persisted
            )
        })
    }

    pub(super) fn recoverable_persisted_final_answer(
        &self,
        current_user_content: &str,
    ) -> Option<String> {
        if let Some(messages) = self.load_bound_session_messages_for_recovery() {
            if let Some(answer) = persisted_final_answer_for_user(&messages, current_user_content) {
                return Some(answer);
            }
        }

        let current_prompt = normalize_turn_prompt(current_user_content);
        for session in self.session_manager.list_sessions(8).ok()? {
            if normalize_turn_prompt(&session.title) != current_prompt {
                continue;
            }
            let messages = self.session_manager.load_messages(&session.id).ok()?;
            if let Some(answer) = persisted_final_answer_for_user(&messages, current_user_content) {
                return Some(answer);
            }
        }
        None
    }

    pub(super) fn load_bound_session_messages_for_recovery(&self) -> Option<Vec<MessageItem>> {
        if let Some(engine) = &self.streaming_engine {
            if let Some((store, session_id)) = engine.session_binding() {
                let records = store.get_messages(&session_id).ok()?;
                return Some(
                    records
                        .into_iter()
                        .map(|record| MessageItem {
                            id: format!("msg_{}", record.id),
                            role: match record.role.as_str() {
                                "user" => MessageRole::User,
                                "assistant" => MessageRole::Assistant,
                                "tool" => MessageRole::Tool,
                                _ => MessageRole::System,
                            },
                            content: record.content,
                            timestamp: std::time::SystemTime::now(),
                            metadata: Default::default(),
                        })
                        .collect(),
                );
            }
        }
        let session_id = self.session_manager.current_session_id()?.to_string();
        self.session_manager.load_messages(&session_id).ok()
    }

    pub(crate) fn provider_wait_timeout_reason(&self) -> Option<String> {
        let provider = &self.facade_snapshot.provider_request;
        if !self.is_querying && self.stream_handle.is_none() {
            return None;
        }
        if self.projected_tool_runs().iter().any(|run| {
            matches!(
                run.status,
                ToolRunStatus::Running | ToolRunStatus::WaitingPermission
            )
        }) {
            return None;
        }
        let runtime_config = crate::services::config::runtime_config();
        let explicit_timeout_ms = runtime_config
            .explicit_llm_request_timeout()
            .map(|timeout| timeout.as_millis() as u64);
        let fallback_timeout_ms = runtime_config.llm_request_timeout().as_millis() as u64;
        let timeout_ms = if provider.timeout_ms > 0 {
            explicit_timeout_ms
                .map(|explicit| explicit.min(provider.timeout_ms))
                .unwrap_or(provider.timeout_ms)
        } else {
            explicit_timeout_ms.unwrap_or(fallback_timeout_ms)
        };
        if timeout_ms == 0 {
            return None;
        }
        let local_post_tool_elapsed_ms = self
            .post_tool_turn_wait_started_at
            .map(|started| started.elapsed().as_millis() as u64)
            .unwrap_or_default();
        if matches!(
            provider.phase,
            crate::engine::runtime_facade::ProviderPhase::Completed
                | crate::engine::runtime_facade::ProviderPhase::Cancelled
        ) {
            if !self.post_tool_turn_wait_has_timed_out(
                timeout_ms,
                provider.elapsed_ms.max(local_post_tool_elapsed_ms),
            ) {
                return None;
            }
            return Some(format!(
                "tool turn stalled after result observation for {:.1}s",
                timeout_ms as f64 / 1000.0
            ));
        }
        let local_elapsed_ms = self
            .stream_started_at
            .map(|started| started.elapsed().as_millis() as u64)
            .unwrap_or_default();
        let elapsed_ms = if provider.phase.is_active() {
            provider.elapsed_ms
        } else {
            provider.elapsed_ms.max(local_elapsed_ms)
        };
        if elapsed_ms < timeout_ms {
            return None;
        }
        Some(provider.message.clone().unwrap_or_else(|| {
            format!(
                "provider request timed out after {:.1}s",
                timeout_ms as f64 / 1000.0
            )
        }))
    }

    pub(super) fn post_tool_turn_wait_has_timed_out(
        &self,
        timeout_ms: u64,
        provider_elapsed_ms: u64,
    ) -> bool {
        if timeout_ms == 0 {
            return false;
        }
        let has_waiting_tool_turn = self.facade_snapshot.tool_turns.iter().any(|turn| {
            matches!(
                turn.phase,
                crate::engine::runtime_facade::ToolTurnPhase::ResultObserved
                    | crate::engine::runtime_facade::ToolTurnPhase::SentBackToModel
            )
        });
        if !has_waiting_tool_turn {
            return false;
        }
        provider_elapsed_ms >= timeout_ms
    }

    pub(super) fn refresh_post_tool_turn_wait_clock(&mut self) {
        if !self.is_querying {
            self.post_tool_turn_wait_started_at = None;
            return;
        }
        let has_waiting_post_tool_turn = self.facade_snapshot.tool_turns.iter().any(|turn| {
            matches!(
                turn.phase,
                crate::engine::runtime_facade::ToolTurnPhase::ResultObserved
                    | crate::engine::runtime_facade::ToolTurnPhase::SentBackToModel
            )
        });
        if has_waiting_post_tool_turn {
            self.post_tool_turn_wait_started_at
                .get_or_insert_with(std::time::Instant::now);
        } else {
            self.post_tool_turn_wait_started_at = None;
        }
    }

    pub(super) fn current_session_max_event_seq(&self) -> i64 {
        let Some(session_id) = self.session_manager.current_session_id() else {
            return 0;
        };
        self.session_manager
            .store()
            .get_session_events_after(session_id, 0)
            .ok()
            .and_then(|events| events.last().map(|event| event.seq))
            .unwrap_or(0)
    }

    pub(super) fn latest_persisted_session_error_for_current_turn(&self) -> Option<String> {
        let session_id = self.session_manager.current_session_id()?;
        let after_seq = self.current_turn_event_start_seq.unwrap_or(0);
        let events = self
            .session_manager
            .store()
            .get_session_events_after(session_id, after_seq)
            .ok()?;
        events
            .iter()
            .rev()
            .find(|event| event.event_type == "error")
            .and_then(|event| serde_json::from_str::<serde_json::Value>(&event.payload).ok())
            .and_then(|payload| {
                payload
                    .get("error")
                    .or_else(|| payload.get("message"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
    }

    pub(super) async fn recover_persisted_error_if_available(&mut self) -> bool {
        if !self.is_querying {
            return false;
        }
        let Some(error) = self.latest_persisted_session_error_for_current_turn() else {
            return false;
        };

        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }
        self.stream_done
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.is_querying = false;
        self.run_coordinator.finish_run();
        self.runtime_facade_state.set_querying(false).await;
        self.stream_started_at = None;
        self.post_tool_turn_wait_started_at = None;
        self.current_turn_event_start_seq = None;
        self.current_tool_anchor_id = None;

        let error_text = format!("[Error: {error}]");
        let assistant_message_id = self
            .sync_snapshot
            .active_assistant_message_id
            .clone()
            .or_else(|| {
                self.messages
                    .iter()
                    .rev()
                    .find(|message| message.role == MessageRole::Assistant)
                    .map(|message| message.id.clone())
            })
            .unwrap_or_else(|| format!("msg_{}", self.messages.len()));

        {
            let mut sync = self.sync_store.lock().await;
            sync.apply_projection_event(&crate::session_store::SessionProjectionEvent::Error {
                message: error.clone(),
            });
            sync.apply_projection_event(
                &crate::session_store::SessionProjectionEvent::AssistantTextUpdated {
                    message_id: Some(assistant_message_id.clone()),
                    text: error_text.clone(),
                    streaming: false,
                },
            );
            self.sync_snapshot = sync.snapshot();
        }

        self.upsert_assistant_message(assistant_message_id, error_text.clone());
        self.typewriter_position = error_text.chars().count();
        if self.error_message.as_deref() != Some(error.as_str()) {
            self.add_toast(error.clone(), "!");
        }
        self.error_message = Some(error);
        self.scroll_to_bottom();
        true
    }
}

pub(super) fn compact_attachment_line(path: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in path.chars().take(max_chars) {
        out.push(ch);
    }
    if path.chars().count() > max_chars {
        out.push('…');
    }
    out
}

pub(super) fn attachment_summary(path: &str, max_path_chars: usize) -> String {
    let compact_path = compact_attachment_line(path, max_path_chars);
    let absolute = resolve_attachment_path(path);
    match std::fs::metadata(&absolute) {
        Ok(metadata) if metadata.is_dir() => {
            format!(
                "{compact_path} (dir, {} items)",
                directory_item_count(&absolute)
            )
        }
        Ok(metadata) => format!(
            "{compact_path} (file, {})",
            format_byte_size(metadata.len())
        ),
        Err(_) => format!("{compact_path} (missing)"),
    }
}

pub(super) fn format_byte_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    let bytes_f = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes_f < MIB {
        format!("{:.1} KiB", bytes_f / KIB)
    } else {
        format!("{:.1} MiB", bytes_f / MIB)
    }
}

pub(super) fn directory_item_count(path: &std::path::Path) -> usize {
    std::fs::read_dir(path)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

pub(super) fn toggle_pinned_session_list(
    pinned_sessions: &mut Vec<String>,
    session_id: &str,
    max: usize,
) -> bool {
    if pinned_sessions.iter().any(|id| id == session_id) {
        pinned_sessions.retain(|id| id != session_id);
        return false;
    }
    if pinned_sessions.len() >= max {
        return false;
    }
    pinned_sessions.push(session_id.to_string());
    true
}

pub(super) fn tool_part_status_from_turn_phase(phase: ToolTurnPhase) -> &'static str {
    match phase {
        ToolTurnPhase::Requested | ToolTurnPhase::Accepted | ToolTurnPhase::Executing => "running",
        ToolTurnPhase::ResultObserved
        | ToolTurnPhase::SentBackToModel
        | ToolTurnPhase::FinalAnswer
        | ToolTurnPhase::Persisted => "completed",
        ToolTurnPhase::Failed => "failed",
        ToolTurnPhase::Cancelled => "cancelled",
        ToolTurnPhase::TimedOut => "timed_out",
    }
}

pub(super) fn persisted_final_answer_for_user(
    messages: &[MessageItem],
    current_user_content: &str,
) -> Option<String> {
    let assistant_idx = messages.iter().rposition(|message| {
        message.role == MessageRole::Assistant
            && !message.content.trim().is_empty()
            && !message.content.trim_start().starts_with("[Error:")
            && !message.content.trim_start().starts_with("[Cancelled:")
    })?;
    let has_tool_after = messages[assistant_idx + 1..]
        .iter()
        .any(|message| message.role == MessageRole::Tool);
    if has_tool_after {
        return None;
    }
    let prior_user_idx = messages[..assistant_idx]
        .iter()
        .rposition(|message| message.role == MessageRole::User)?;
    let prior_user = &messages[prior_user_idx];
    let latest_user_idx = messages
        .iter()
        .rposition(|message| message.role == MessageRole::User)?;
    (latest_user_idx == prior_user_idx
        && (normalize_turn_prompt(&prior_user.content)
            == normalize_turn_prompt(current_user_content)
            || assistant_idx > latest_user_idx))
        .then(|| messages[assistant_idx].content.clone())
}

pub(super) fn normalize_turn_prompt(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn resolve_attachment_path(path: &str) -> std::path::PathBuf {
    let candidate = std::path::Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(candidate)
    }
}

pub(super) fn attachment_file_preview(path: &std::path::Path, byte_len: u64) -> String {
    const MAX_PREVIEW_BYTES: usize = 64 * 1024;
    match std::fs::read(path) {
        Ok(bytes) => {
            let shown_len = bytes.len().min(MAX_PREVIEW_BYTES);
            let mut preview = String::from_utf8_lossy(&bytes[..shown_len]).to_string();
            if bytes.len() > shown_len {
                preview.push_str(&format!(
                    "\n\n... truncated: showing {} of {} bytes",
                    shown_len, byte_len
                ));
            }
            preview
        }
        Err(err) => format!("Failed to read attachment:\n{}", err),
    }
}

pub(super) fn attachment_directory_preview(path: &std::path::Path) -> String {
    let mut lines = vec![format!("Directory: {}", path.display())];
    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries.into_iter().take(200) {
                let kind = if entry.path().is_dir() {
                    "dir "
                } else {
                    "file"
                };
                lines.push(format!("{}  {}", kind, entry.file_name().to_string_lossy()));
            }
        }
        Err(err) => lines.push(format!("Failed to read directory: {}", err)),
    }
    lines.join("\n")
}

use super::*;

impl TuiApp {
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
            self.input.clear();
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
        self.input.clear();
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
                self.input.set_value(cmd.clone());
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
                self.input.set_value(String::new());
            }
            Some(i) => {
                let new_i = i + 1;
                self.history_index = Some(new_i);
                if let Some(cmd) = self.history.get(new_i) {
                    self.input.set_value(cmd.clone());
                }
            }
        }
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
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
        self.pinned_to_bottom = false;
    }

    /// 向下滚动
    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
        // Re-pin if scrolled past the last message
        if self.scroll_offset >= self.messages.len() {
            self.pinned_to_bottom = true;
        }
    }

    /// 滚动到底部（显示最新消息）
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.messages.len();
        self.pinned_to_bottom = true;
    }

    /// 向上滚动半页（Vim Ctrl+U）
    pub fn scroll_up_half_page(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(5);
        self.pinned_to_bottom = false;
    }

    /// 向下滚动半页（Vim Ctrl+D）
    pub fn scroll_down_half_page(&mut self) {
        self.scroll_offset += 5;
    }

    /// 获取可见消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 当前模型名称（用于状态展示）
    pub fn clear_tool_transcript(&mut self) {
        self.tool_runs_snapshot.clear();
        self.tool_runs_by_message_id.clear();
        self.current_tool_anchor_id = None;
        self.expanded_tool_run_id = None;
        self.stream_usage_snapshot = None;
    }

    /// 获取消息（考虑滚动）
    pub fn visible_messages(&self) -> &[MessageItem] {
        &self.messages
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

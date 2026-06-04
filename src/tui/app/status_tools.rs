use super::*;

impl TuiApp {
    pub fn current_model_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine.model_name().to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 当前 Provider 名称（用于状态展示）
    pub fn current_provider_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            provider_name_from_base_url(&engine.provider_base_url()).to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 当前 Provider Base URL（用于状态展示）
    pub fn current_provider_base_url(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine.provider_base_url()
        } else {
            "unknown".to_string()
        }
    }

    pub fn current_agent_mode_label(&self) -> &'static str {
        self.agent_mode.label()
    }

    pub fn set_agent_mode(&mut self, mode: AgentMode) {
        self.agent_mode = mode;
    }

    pub fn workspace_status_label(&self) -> String {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let name = cwd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        if let Some(branch) = read_git_branch_fast(&cwd) {
            format!("{}@{}", name, branch)
        } else {
            name.to_string()
        }
    }

    pub fn current_permission_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            permission_mode_name(engine.permission_mode()).replace('_', "-")
        } else {
            "unknown".to_string()
        }
    }

    fn runtime_permission_state(&self) -> RuntimePermissionState {
        let mut state = RuntimePermissionState {
            mode: self.current_permission_label(),
            ..RuntimePermissionState::default()
        };
        if let Some(request) = self.pending_permission_request.as_ref() {
            state.pending_call_id = Some(request.tool_call.id.clone());
            state.pending_tool = Some(request.tool_call.name.clone());
            state.pending_prompt = Some(request.prompt.clone());
        }
        state
    }

    fn runtime_mcp_state(&self) -> RuntimeMcpState {
        let Some(engine) = self.streaming_engine.as_ref() else {
            return RuntimeMcpState::default();
        };
        let Some(manager) = engine.mcp_manager() else {
            return RuntimeMcpState::default();
        };
        let diagnostics = manager.health_diagnostics();
        let available_count = diagnostics
            .iter()
            .filter(|diag| {
                diag.approved && diag.health == crate::engine::mcp::McpHealthStatus::Healthy
            })
            .count();
        let repair_hints = diagnostics
            .iter()
            .filter(|diag| diag.repair_hint != "none")
            .map(|diag| format!("{}=>{}", diag.name, diag.repair_hint))
            .collect::<Vec<_>>();
        RuntimeMcpState {
            server_count: diagnostics.len(),
            available_count,
            repair_hints,
        }
    }

    fn runtime_bridge_state(&self) -> RuntimeBridgeState {
        let bridge = crate::bridge::runtime_snapshot();
        let remote_env = crate::remote::RemoteEnvDetector::detect();
        let saved_session_count = crate::remote::RemoteSessionManager::new()
            .list_sessions()
            .len();
        let remote_trigger_tool_available = self
            .streaming_engine
            .as_ref()
            .map(|engine| engine.tool_registry().has("remote_trigger"))
            .unwrap_or(false);
        let remote_dev_tool_available = self
            .streaming_engine
            .as_ref()
            .map(|engine| engine.tool_registry().has("remote_dev"))
            .unwrap_or(false);

        RuntimeBridgeState {
            bridge_url_configured: bridge.bridge_url.is_some(),
            bridge_url_source: bridge.bridge_url_source,
            auth_token_configured: bridge.auth_token_configured,
            tenant_configured: bridge.tenant_id.is_some(),
            cursor_count: bridge.cursor_count,
            saved_session_count,
            remote_env_type: remote_env.env_type.to_string(),
            is_remote_env: remote_env.is_remote,
            remote_trigger_tool_available,
            remote_dev_tool_available,
        }
    }

    pub(in crate::tui::app) fn build_runtime_state_snapshot(&self) -> RuntimeAppState {
        let tool_uses = self
            .tool_runs_snapshot
            .iter()
            .map(runtime_tool_use_from_view)
            .collect();
        let terminal_tasks = self
            .tool_runs_snapshot
            .iter()
            .filter_map(runtime_terminal_task_from_view)
            .collect();
        RuntimeAppState {
            tool_uses,
            terminal_tasks,
            permission: self.runtime_permission_state(),
            mcp: self.runtime_mcp_state(),
            bridge: self.runtime_bridge_state(),
        }
    }

    pub(in crate::tui::app) async fn sync_context_runtime_state(&self) {
        let runtime = self.runtime_state_snapshot.clone();
        let messages = self.messages.clone();
        let is_querying = self.is_querying;
        let last_error = self.error_message.clone();
        self.context
            .set_state(move |state| {
                state.messages = messages;
                state.is_querying = is_querying;
                state.last_error = last_error;
                state.runtime = runtime;
            })
            .await;
    }

    pub async fn runtime_status_snapshot(&self) -> RuntimeStatusSnapshot {
        let mut state = self.context.get_state().await;
        state.messages = self.messages.clone();
        state.is_querying = self.is_querying;
        state.last_error = self.error_message.clone();
        state.runtime = self.build_runtime_state_snapshot();
        select_runtime_status(&state)
    }

    pub fn runtime_status_snapshot_now(&self) -> RuntimeStatusSnapshot {
        let mut state = AppState::new();
        state.messages = self.messages.clone();
        state.is_querying = self.is_querying;
        state.last_error = self.error_message.clone();
        state.runtime = self.build_runtime_state_snapshot();
        for task in &self.tasks {
            state.tasks.insert(task.id.clone(), task.clone());
        }
        select_runtime_status(&state)
    }

    pub fn current_goal_label(&self) -> Option<String> {
        self.streaming_engine
            .as_ref()
            .and_then(|engine| engine.goal_manager().current())
            .map(|goal| {
                let max_chars = 36;
                let mut title = goal.title.chars().take(max_chars).collect::<String>();
                if goal.title.chars().count() > max_chars {
                    title.push_str("...");
                }
                format!("goal:{}", title)
            })
    }

    pub fn cycle_status_bar_density(&mut self) -> StatusBarDensity {
        self.status_bar_density = self.status_bar_density.next();
        self.status_bar_density
    }

    pub fn set_status_bar_density(&mut self, density: StatusBarDensity) {
        self.status_bar_density = density;
    }

    pub fn active_tool_count(&self) -> usize {
        if self.runtime_state_snapshot.tool_uses.is_empty() {
            self.tool_runs_snapshot
                .iter()
                .filter(|run| run.is_active())
                .count()
        } else {
            self.runtime_state_snapshot
                .tool_uses
                .iter()
                .filter(|tool| tool.active)
                .count()
        }
    }

    pub fn current_tool_status_label(&self) -> Option<String> {
        if let Some(tool) = self
            .runtime_state_snapshot
            .tool_uses
            .iter()
            .rev()
            .find(|tool| tool.active)
        {
            let elapsed = tool.elapsed_ms.unwrap_or_default() / 1000;
            return Some(format!("{} {}s", tool.summary, elapsed));
        }
        let active = self
            .tool_runs_snapshot
            .iter()
            .rev()
            .find(|run| run.is_active())?;
        Some(format!(
            "{} {}s",
            active.summary(),
            active.elapsed().as_secs()
        ))
    }

    pub fn terminal_task_status_label(&self) -> Option<String> {
        let terminal_count = self.runtime_state_snapshot.terminal_tasks.len();
        let running = self
            .runtime_state_snapshot
            .terminal_tasks
            .iter()
            .filter(|task| task.status == "running")
            .count();
        let backgrounded = self
            .runtime_state_snapshot
            .tool_uses
            .iter()
            .filter(|tool| tool.status == RuntimeToolStatus::Backgrounded)
            .count();
        let pty = self
            .runtime_state_snapshot
            .terminal_tasks
            .iter()
            .filter(|task| task.terminal_kind.as_deref() == Some("pty_shell"))
            .count();
        if terminal_count == 0 && backgrounded == 0 {
            return None;
        }
        let mut parts = vec![format!("terminal:{}", terminal_count.max(backgrounded))];
        if running > 0 || backgrounded > 0 {
            parts.push(format!("running:{}", running.max(backgrounded)));
        }
        if pty > 0 {
            parts.push(format!("pty:{}", pty));
        }
        Some(parts.join(" "))
    }

    pub fn stream_usage_label(&self) -> Option<String> {
        let usage = self.stream_usage_snapshot?;
        let mut label = format!("{} tokens", usage.total_tokens());
        if let Some(reasoning) = usage.reasoning_tokens {
            label.push_str(&format!(" / {} reasoning", reasoning));
        }
        if let Some(cached) = usage.cached_tokens {
            label.push_str(&format!(" / {} cached", cached));
            if let Some(miss) = usage.cache_miss_tokens() {
                label.push_str(&format!(" / {} miss", miss));
            }
            if let Some(hit_rate) = usage.cache_hit_rate_percent() {
                label.push_str(&format!(" / {:.1}% hit", hit_rate));
            }
        }
        Some(label)
    }

    pub fn toggle_transcript_expanded(&mut self) {
        self.transcript_expanded = !self.transcript_expanded;
        self.expanded_tool_run_id = None;
    }

    pub fn cycle_expanded_tool_run(&mut self) {
        let ids = self
            .visible_tool_run_ids()
            .into_iter()
            .collect::<Vec<String>>();
        if ids.is_empty() {
            self.transcript_expanded = !self.transcript_expanded;
            self.expanded_tool_run_id = None;
            return;
        }

        self.transcript_expanded = false;
        self.expanded_tool_run_id = match self.expanded_tool_run_id.as_deref() {
            None => ids.first().cloned(),
            Some(current) => ids
                .iter()
                .position(|id| id == current)
                .and_then(|idx| ids.get(idx + 1).cloned()),
        };
    }

    pub fn open_tool_viewer(&mut self) -> bool {
        let runtime_selected_id = select_tool_viewer_tool_id(
            &self.runtime_state_snapshot,
            self.expanded_tool_run_id.as_deref(),
        );
        let selected = runtime_selected_id
            .as_deref()
            .and_then(|id| self.find_visible_tool_run(id))
            .or_else(|| {
                self.expanded_tool_run_id
                    .as_deref()
                    .and_then(|id| self.find_visible_tool_run(id))
            })
            .or_else(|| self.visible_tool_runs().into_iter().next_back());

        let Some(run) = selected else {
            return false;
        };

        let title = run.summary();
        let content = run.full_details();
        self.tool_viewer_title = title;
        self.tool_viewer_content = content;
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub fn open_tool_viewer_for(&mut self, id: &str) -> bool {
        let Some((title, content)) = self
            .find_visible_tool_run(id)
            .map(|run| (run.summary(), run.full_details()))
        else {
            return false;
        };
        self.tool_viewer_title = title;
        self.tool_viewer_content = content;
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub fn tool_output_index_lines(&self) -> Vec<String> {
        self.visible_tool_runs()
            .into_iter()
            .map(|run| {
                format!(
                    "- {} [{}] {}",
                    run.id,
                    tool_run_status_label(run.status),
                    run.summary()
                )
            })
            .collect()
    }

    fn find_visible_tool_run(&self, id: &str) -> Option<&ToolRunView> {
        self.visible_tool_runs()
            .into_iter()
            .find(|run| run.id.as_str() == id)
    }

    fn visible_tool_runs(&self) -> Vec<&ToolRunView> {
        let mut runs = Vec::new();
        for msg in &self.messages {
            if let Some(group) = self.tool_runs_for_message(&msg.id) {
                runs.extend(group.iter());
            }
        }
        runs
    }

    fn visible_tool_run_ids(&self) -> Vec<String> {
        self.visible_tool_runs()
            .into_iter()
            .map(|run| run.id.clone())
            .collect()
    }

    pub fn is_tool_run_expanded(&self, run: &ToolRunView) -> bool {
        self.transcript_expanded || self.expanded_tool_run_id.as_deref() == Some(run.id.as_str())
    }

    pub fn tool_runs_for_message(&self, message_id: &str) -> Option<&[ToolRunView]> {
        self.tool_runs_by_message_id
            .get(message_id)
            .map(Vec::as_slice)
    }
}

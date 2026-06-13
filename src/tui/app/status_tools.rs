use super::*;

impl TuiApp {
    pub fn current_model_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine.model_name().to_string()
        } else if let Some(model) = self.facade_snapshot.provider_request.model.as_deref() {
            model.to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 当前 Provider 名称（用于状态展示）
    pub fn current_provider_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            provider_name_from_base_url(&engine.provider_base_url()).to_string()
        } else if let Some(provider) = self
            .facade_snapshot
            .provider_request
            .provider_family
            .as_deref()
        {
            display_provider_label(
                provider,
                self.facade_snapshot.provider_request.model.as_deref(),
            )
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
            permission_mode_name(crate::permissions::PermissionMode::default()).replace('_', "-")
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
        let tool_runs = self.projected_tool_runs();
        let tool_uses = tool_runs.iter().map(runtime_tool_use_from_view).collect();
        let terminal_tasks = tool_runs
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
            self.projected_tool_runs()
                .into_iter()
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
            .projected_tool_runs()
            .into_iter()
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
        let (title, content) = if let Some((title, content)) = self
            .find_visible_tool_run(id)
            .map(|run| (run.summary(), run.full_details()))
        {
            (title, content)
        } else if let Some((title, content)) = self.read_stored_tool_output(id) {
            (title, content)
        } else {
            return false;
        };
        self.tool_viewer_title = title;
        self.tool_viewer_content = content;
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub fn tool_output_index_lines(&self) -> Vec<String> {
        let mut lines = self
            .visible_tool_runs()
            .into_iter()
            .map(|run| {
                format!(
                    "- {} [{}] {}",
                    run.id,
                    tool_run_status_label(run.status),
                    run.summary()
                )
            })
            .collect::<Vec<_>>();
        if let Some(session_id) = self.session_manager.current_session_id() {
            if let Ok(metas) =
                crate::tool_output_store::ToolOutputStore::new().list_for_session(session_id)
            {
                for meta in metas {
                    lines.push(format!(
                        "- {} [stored] {} · {} bytes · {}",
                        meta.id,
                        meta.tool_name,
                        meta.original_bytes,
                        meta.uri()
                    ));
                }
            }
        }
        lines
    }

    pub fn reload_persisted_tool_runs_for_session(
        &mut self,
        session_id: &str,
    ) -> Result<usize, String> {
        let parts = self
            .session_manager
            .load_session_parts(session_id)
            .map_err(|err| err.to_string())?;
        let runs = parts
            .into_iter()
            .filter_map(persisted_part_to_tool_run)
            .collect::<Vec<_>>();
        if runs.is_empty() {
            return Ok(0);
        }

        let anchor_id = self
            .messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::User)
            .or_else(|| self.messages.last())
            .map(|message| message.id.clone())
            .unwrap_or_else(|| format!("session-parts-{session_id}"));
        self.sync_snapshot
            .set_tool_runs_for_message(anchor_id, runs.clone());
        Ok(self.projected_tool_runs().len())
    }

    fn find_visible_tool_run(&self, id: &str) -> Option<ToolRunView> {
        self.visible_tool_runs()
            .into_iter()
            .find(|run| run.id.as_str() == id)
    }

    fn visible_tool_runs(&self) -> Vec<ToolRunView> {
        let mut runs = Vec::new();
        for msg in &self.messages {
            if let Some(group) = self.tool_runs_for_message(&msg.id) {
                runs.extend(group);
            }
        }
        runs
    }

    pub fn projected_tool_runs(&self) -> Vec<ToolRunView> {
        let mut runs = self.visible_tool_runs();
        let mut seen = runs
            .iter()
            .map(|run| run.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for run in self.sync_snapshot.all_tool_runs() {
            if seen.insert(run.id.clone()) {
                runs.push(run);
            }
        }
        runs
    }

    fn read_stored_tool_output(&self, id_or_uri: &str) -> Option<(String, String)> {
        let session_id = self.session_manager.current_session_id()?;
        let store = crate::tool_output_store::ToolOutputStore::new();
        let page = store.read_page(session_id, id_or_uri, 0, 64 * 1024).ok()?;
        let meta = store.read_meta(id_or_uri).ok();
        let title = meta
            .as_ref()
            .map(|m| format!("{} · {}", m.tool_name, m.uri()))
            .unwrap_or_else(|| id_or_uri.to_string());
        let mut content = page.content;
        if page.has_more {
            content.push_str(&format!(
                "\n\n[Showing first {} of {} bytes. Full output: {}]",
                page.limit.min(page.total_bytes),
                page.total_bytes,
                id_or_uri
            ));
        }
        Some((title, content))
    }

    /// Clean up stored tool outputs that exceed the retention threshold.
    pub fn clean_tool_outputs(&self) -> String {
        let session_id = self.session_manager.current_session_id();
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let policy = crate::tool_output_store::ToolOutputPolicy::from_project_env(&cwd);
        let store = crate::tool_output_store::ToolOutputStore::new();

        match session_id {
            Some(sid) => {
                let removed = store
                    .cleanup_older_than(policy.cleanup_threshold_ms())
                    .unwrap_or(0);
                let session_removed = store.cleanup_session(sid).unwrap_or(0);
                if removed > 0 || session_removed > 0 {
                    format!(
                        "Cleaned {} tool outputs ({} from current session). Retention: {} days.",
                        removed + session_removed,
                        session_removed,
                        policy.retention_days
                    )
                } else {
                    "No tool outputs to clean.".to_string()
                }
            }
            None => {
                let removed = store
                    .cleanup_older_than(policy.cleanup_threshold_ms())
                    .unwrap_or(0);
                if removed > 0 {
                    format!(
                        "Cleaned {} expired tool outputs (retention: {} days).",
                        removed, policy.retention_days
                    )
                } else {
                    "No tool outputs to clean.".to_string()
                }
            }
        }
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

    pub fn tool_runs_for_message(&self, message_id: &str) -> Option<Vec<ToolRunView>> {
        self.sync_snapshot.tool_runs_for_message(message_id)
    }
}

fn persisted_part_to_tool_run(
    part: crate::session_store::PersistedSessionPart,
) -> Option<ToolRunView> {
    if part.kind != "tool" && part.kind != "shell" {
        return None;
    }

    let payload = part.payload;
    let tool_name = part
        .tool_name
        .clone()
        .or_else(|| payload["tool_name"].as_str().map(str::to_string))
        .unwrap_or_else(|| {
            if part.kind == "shell" {
                "bash".to_string()
            } else {
                "tool".to_string()
            }
        });
    let run_id = part
        .tool_call_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or(part.part_id);
    let mut run = ToolRunView::new(run_id, tool_name.clone());
    if let Some(input_args) = payload["input_args"].as_str() {
        run.args_buffer = input_args.to_string();
        run.arguments = serde_json::from_str(input_args).ok();
    }
    let body = payload["result_preview"]
        .as_str()
        .or_else(|| payload["error"].as_str())
        .or_else(|| payload["output_uri"].as_str())
        .or_else(|| payload["input_args"].as_str())
        .unwrap_or("")
        .to_string();
    let success = !matches!(
        part.status.as_deref(),
        Some("failed" | "timed_out" | "cancelled")
    );
    let metadata = serde_json::json!({
        "tool": tool_name,
        "success": success,
        "output_uri": payload["output_uri"].as_str(),
        "error_preview": payload["error"].as_str(),
        "persisted_session_part_id": part.id,
        "projected_to_seq": part.projected_to_seq,
    });
    run.mark_complete_with_metadata(body, Some(metadata));
    run.status = match part.status.as_deref() {
        Some("pending") => ToolRunStatus::Queued,
        Some("running") => ToolRunStatus::Running,
        Some("failed") => ToolRunStatus::Failed,
        Some("timed_out") => ToolRunStatus::TimedOut,
        Some("cancelled") => ToolRunStatus::Cancelled,
        _ => run.status,
    };
    Some(run)
}

fn display_provider_label(provider_family: &str, model: Option<&str>) -> String {
    let family = provider_family.to_ascii_lowercase();
    let model = model.unwrap_or_default().to_ascii_lowercase();
    if family == "deepseek"
        || (family.starts_with("openai_compatible") && model.contains("deepseek"))
    {
        "DeepSeek".to_string()
    } else if family == "minimax" || model.contains("minimax") {
        "MiniMax".to_string()
    } else if family == "kimi-code" || family == "kimi_code" {
        "Kimi Code".to_string()
    } else if family == "kimi" || family == "moonshot" || model.contains("kimi") {
        "Kimi".to_string()
    } else if family == "glm" || family == "zai" || family == "z.ai" || model.contains("glm") {
        "GLM".to_string()
    } else if family == "openai" {
        "OpenAI".to_string()
    } else {
        provider_family.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_and_model_labels_fallback_to_runtime_facade() {
        let mut app = TuiApp::new();
        app.facade_snapshot.provider_request.provider_family =
            Some("openai_compatible".to_string());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());

        assert_eq!(app.current_provider_label(), "DeepSeek");
        assert_eq!(app.current_model_label(), "deepseek-v4-flash");
        assert_eq!(app.current_permission_label(), "auto");
    }
}

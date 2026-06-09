//! API 状态管理
//!
//! 管理 API 服务器状态和提供业务逻辑方法

use super::routes::*;
use crate::api::session_runner::{ApiRunStatus, ApiSessionRunnerRegistry};
use crate::engine::agent_mode::AgentMode;
use crate::engine::run_coordinator::{InputDelivery, PromptAdmissionStatus, SessionRunCoordinator};
use crate::engine::runtime_controller::RuntimeController;
use crate::engine::runtime_facade::ProviderPhase;
use crate::engine::streaming::StreamEvent;
use crate::services::config::AppConfig;
use crate::session_store::SessionStore;
use crate::tools::ToolContext;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};
use tracing::debug;

/// Input shape for the full-agent runtime entrypoint.
#[derive(Debug, Clone)]
pub struct ApiSessionPromptInput {
    pub session_id: String,
    pub message: String,
    pub agent_mode: Option<String>,
    pub stream: bool,
    pub delivery: Option<String>,
    pub idempotency_key: Option<String>,
}

/// Outcome from a full-agent prompt execution.
#[derive(Debug, Clone, Serialize)]
pub struct ApiSessionPromptOutcome {
    pub accepted: bool,
    pub turn_id: Option<String>,
    pub status: String,
    pub events_written: usize,
    pub latest_part_index: Option<i64>,
    pub diagnostic: Option<super::dto::diagnostic::DiagnosticExportDto>,
    pub error: Option<String>,
}

/// Trait for the full-agent runtime entrypoint.
///
/// The real implementation delegates to `RuntimeController`.  Tests inject
/// a fake implementation.  When the handle is `None`, the handler returns
/// a typed 501.
#[async_trait::async_trait]
pub trait ApiAgentRuntime: Send + Sync {
    async fn submit_prompt(
        &self,
        input: ApiSessionPromptInput,
    ) -> anyhow::Result<ApiSessionPromptOutcome>;

    async fn compact(&self, session_id: &str) -> anyhow::Result<Option<ApiCompactOutcome>> {
        let _ = session_id;
        Ok(None) // default: no-op
    }

    async fn cancel(&self, session_id: &str) -> anyhow::Result<bool> {
        let _ = session_id;
        Ok(false)
    }

    async fn context_snapshot(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<crate::desktop_runtime::DesktopContextSnapshot>> {
        let _ = session_id;
        Ok(None)
    }
}

/// Outcome from a manual compaction trigger.
#[derive(Debug, Clone, Serialize)]
pub struct ApiCompactOutcome {
    pub boundary_id: String,
    pub before_tokens: u64,
    pub after_tokens: u64,
    pub messages_before: usize,
    pub messages_after: usize,
}

/// Production full-agent API adapter backed by `RuntimeController`.
#[derive(Clone)]
pub struct RuntimeControllerApiAgentRuntime {
    controller: RuntimeController,
    model: String,
    session_store: Arc<RwLock<SessionStore>>,
    run_coordinator: SessionRunCoordinator,
    runner_registry: Arc<ApiSessionRunnerRegistry>,
    turn_lock: Arc<Mutex<()>>,
}

impl RuntimeControllerApiAgentRuntime {
    pub fn new(
        controller: RuntimeController,
        model: impl Into<String>,
        session_store: Arc<RwLock<SessionStore>>,
        runner_registry: Arc<ApiSessionRunnerRegistry>,
    ) -> Self {
        Self {
            controller,
            model: model.into(),
            session_store,
            run_coordinator: SessionRunCoordinator::new(),
            runner_registry,
            turn_lock: Arc::new(Mutex::new(())),
        }
    }

    async fn ensure_session_record(&self, session_id: &str) -> anyhow::Result<()> {
        let store = self.session_store.read().await;
        if store.get_session(session_id)?.is_none() {
            store.create_session(session_id, "API Session", &self.model)?;
        }
        Ok(())
    }

    async fn admit_input(
        &self,
        input: &ApiSessionPromptInput,
        prompt_id: &str,
        delivery: InputDelivery,
    ) -> anyhow::Result<PromptAdmissionStatus> {
        let metadata = serde_json::json!({
            "agent_mode": input.agent_mode,
        });
        let store = self.session_store.read().await;
        let conn = store.shared_conn();
        let conn = conn.lock().expect("api state sqlite conn lock poisoned");
        Ok(
            crate::engine::run_coordinator::admit_session_input_with_metadata(
                &conn,
                &input.session_id,
                prompt_id,
                &input.message,
                delivery,
                Some(&metadata),
            )?,
        )
    }

    async fn mark_prompt_state(
        &self,
        session_id: &str,
        prompt_id: &str,
        state: &str,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        let store = self.session_store.read().await;
        let conn = store.shared_conn();
        let conn = conn.lock().expect("api state sqlite conn lock poisoned");
        crate::engine::run_coordinator::mark_session_input_state_by_prompt_id(
            &conn, session_id, prompt_id, state, error,
        )?;
        Ok(())
    }

    async fn cancel_session_inputs(&self, session_id: &str) -> anyhow::Result<usize> {
        let store = self.session_store.read().await;
        let conn = store.shared_conn();
        let conn = conn.lock().expect("api state sqlite conn lock poisoned");
        Ok(conn.execute(
            "UPDATE session_inputs
             SET state = 'cancelled', error = 'cancelled_by_user'
             WHERE session_id = ?1 AND state IN ('pending', 'promoted', 'running')",
            rusqlite::params![session_id],
        )?)
    }

    async fn next_pending_input(
        &self,
    ) -> anyhow::Result<Option<crate::engine::run_coordinator::PromotedSessionInput>> {
        let store = self.session_store.read().await;
        let conn = store.shared_conn();
        let conn = conn.lock().expect("api state sqlite conn lock poisoned");
        let session_ids = crate::engine::run_coordinator::pending_session_ids(&conn, 1)?;
        let Some(session_id) = session_ids.first() else {
            return Ok(None);
        };
        Ok(crate::engine::run_coordinator::promote_session_input_record(&conn, session_id)?)
    }

    fn spawn_queue_drain(&self) {
        if !self.run_coordinator.wake() {
            // Another drain is already queued or running.
            return;
        }
        let runtime = self.clone();
        tokio::spawn(async move {
            runtime.run_coordinator.accept_wake();
            if let Err(err) = runtime.drain_pending_inputs().await {
                tracing::warn!("API queued prompt drain failed: {}", err);
            }
        });
    }

    async fn drain_pending_inputs(&self) -> anyhow::Result<()> {
        if !self.run_coordinator.start_run() {
            return Ok(());
        }

        let result = async {
            loop {
                let Some(record) = self.next_pending_input().await? else {
                    break;
                };
                let prompt_id = record
                    .prompt_id
                    .clone()
                    .unwrap_or_else(|| format!("api-queued-{}", uuid::Uuid::new_v4().simple()));
                let input = ApiSessionPromptInput {
                    session_id: record.session_id,
                    message: record.content,
                    agent_mode: record.agent_mode,
                    stream: false,
                    delivery: Some("queue".to_string()),
                    idempotency_key: Some(prompt_id.clone()),
                };
                if let Err(err) = self
                    .run_prompt_inner(input.clone(), prompt_id.clone())
                    .await
                {
                    self.runner_registry.finish_run(
                        &input.session_id,
                        ApiRunStatus::Failed {
                            error: err.to_string(),
                        },
                    );
                    self.mark_prompt_state(
                        &input.session_id,
                        &prompt_id,
                        "failed",
                        Some(&err.to_string()),
                    )
                    .await?;
                    tracing::warn!(
                        "API queued prompt failed: session_id={} prompt_id={} error={}",
                        input.session_id,
                        prompt_id,
                        err
                    );
                }
            }
            Ok(())
        }
        .await;

        self.run_coordinator.finish_run();
        result
    }

    fn latest_part_index(&self, session_id: &str) -> Option<i64> {
        let (store, _) = self.controller.engine().session_binding()?;
        store
            .get_session_parts(session_id)
            .ok()?
            .last()
            .map(|part| part.part_index)
    }

    async fn run_prompt(
        &self,
        input: ApiSessionPromptInput,
        prompt_id: String,
    ) -> anyhow::Result<ApiSessionPromptOutcome> {
        if !self.run_coordinator.start_run() {
            self.runner_registry.enqueue(&input.session_id);
            return Ok(ApiSessionPromptOutcome {
                accepted: true,
                turn_id: Some(prompt_id),
                status: "queued".to_string(),
                events_written: 0,
                latest_part_index: self.latest_part_index(&input.session_id),
                diagnostic: None,
                error: None,
            });
        }

        let result = self.run_prompt_inner(input, prompt_id).await;
        self.run_coordinator.finish_run();
        self.spawn_queue_drain();
        result
    }

    async fn run_prompt_inner(
        &self,
        input: ApiSessionPromptInput,
        prompt_id: String,
    ) -> anyhow::Result<ApiSessionPromptOutcome> {
        use futures::StreamExt;

        let _guard = self.turn_lock.lock().await;
        if !self.runner_registry.start_run(&input.session_id) {
            return Ok(ApiSessionPromptOutcome {
                accepted: true,
                turn_id: Some(prompt_id),
                status: "queued".to_string(),
                events_written: 0,
                latest_part_index: self.latest_part_index(&input.session_id),
                diagnostic: None,
                error: None,
            });
        }
        self.mark_prompt_state(&input.session_id, &prompt_id, "running", None)
            .await?;
        let agent_mode = input
            .agent_mode
            .as_deref()
            .and_then(AgentMode::parse)
            .unwrap_or_default();
        self.controller.set_session(input.session_id.clone());
        if let Some((store, _)) = self.controller.engine().session_binding() {
            if store.get_session(&input.session_id)?.is_none() {
                store.create_session(&input.session_id, "API Session", &self.model)?;
            }
        }

        let mut events_written = 0usize;
        let mut status = "completed".to_string();
        let mut diagnostic = None;
        let mut error = None;
        let mut stream = self
            .controller
            .submit_stream_turn_with_agent_mode(input.message, agent_mode)
            .await;

        while let Some(event) = stream.next().await {
            events_written += 1;
            match event {
                StreamEvent::RuntimeDiagnostic { diagnostic: value } => {
                    if value
                        .get("stage")
                        .and_then(|stage| stage.as_str())
                        .is_some_and(|stage| stage == "permission_request")
                    {
                        self.runner_registry
                            .set_status(&input.session_id, ApiRunStatus::WaitingPermission);
                    }
                    if let Ok(dto) =
                        serde_json::from_value::<super::dto::diagnostic::DiagnosticExportDto>(value)
                    {
                        diagnostic = Some(dto);
                    }
                }
                StreamEvent::Closeout {
                    status: closeout_status,
                    ..
                } => {
                    status = closeout_status;
                }
                StreamEvent::Error(message) => {
                    status = "failed".to_string();
                    error = Some(message);
                    break;
                }
                StreamEvent::Complete => break,
                _ => {}
            }
            if self.runner_registry.is_cancelling(&input.session_id)
                || self
                    .controller
                    .runtime_state()
                    .snapshot()
                    .await
                    .provider_request
                    .phase
                    == ProviderPhase::Cancelled
            {
                status = "cancelled".to_string();
                error = Some("cancelled_by_user".to_string());
                break;
            }
        }

        let final_registry_status = if status == "cancelled" {
            ApiRunStatus::Cancelled
        } else if let Some(error) = &error {
            ApiRunStatus::Failed {
                error: error.clone(),
            }
        } else {
            ApiRunStatus::Completed
        };
        self.runner_registry
            .finish_run(&input.session_id, final_registry_status);
        self.mark_prompt_state(
            &input.session_id,
            &prompt_id,
            if status == "cancelled" {
                "cancelled"
            } else if error.is_some() {
                "failed"
            } else {
                "completed"
            },
            error.as_deref(),
        )
        .await?;

        Ok(ApiSessionPromptOutcome {
            accepted: error.is_none(),
            turn_id: Some(prompt_id),
            status,
            events_written,
            latest_part_index: self.latest_part_index(&input.session_id),
            diagnostic,
            error,
        })
    }
}

#[async_trait::async_trait]
impl ApiAgentRuntime for RuntimeControllerApiAgentRuntime {
    async fn submit_prompt(
        &self,
        input: ApiSessionPromptInput,
    ) -> anyhow::Result<ApiSessionPromptOutcome> {
        let delivery = delivery_from_api(input.delivery.as_deref());
        let prompt_id = input
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("api-turn-{}", uuid::Uuid::new_v4().simple()));

        self.ensure_session_record(&input.session_id).await?;
        match self.admit_input(&input, &prompt_id, delivery).await? {
            PromptAdmissionStatus::Admitted => {}
            PromptAdmissionStatus::AlreadyAdmitted { state } => {
                if matches!(state.as_str(), "pending" | "promoted") {
                    self.runner_registry.enqueue(&input.session_id);
                    self.spawn_queue_drain();
                }
                return Ok(ApiSessionPromptOutcome {
                    accepted: true,
                    turn_id: Some(prompt_id),
                    status: format!("already_{state}"),
                    events_written: 0,
                    latest_part_index: self.latest_part_index(&input.session_id),
                    diagnostic: None,
                    error: None,
                });
            }
            PromptAdmissionStatus::Conflict { .. } => {
                return Ok(ApiSessionPromptOutcome {
                    accepted: false,
                    turn_id: Some(prompt_id),
                    status: "conflict".to_string(),
                    events_written: 0,
                    latest_part_index: self.latest_part_index(&input.session_id),
                    diagnostic: None,
                    error: Some("idempotency_key was reused with different message content".into()),
                });
            }
            PromptAdmissionStatus::Rejected { reason } => {
                return Ok(ApiSessionPromptOutcome {
                    accepted: false,
                    turn_id: Some(prompt_id),
                    status: "rejected".to_string(),
                    events_written: 0,
                    latest_part_index: self.latest_part_index(&input.session_id),
                    diagnostic: None,
                    error: Some(reason),
                });
            }
        }

        match delivery {
            InputDelivery::AdmitOnly => {
                self.mark_prompt_state(&input.session_id, &prompt_id, "admitted", None)
                    .await?;
                self.runner_registry
                    .set_status(&input.session_id, ApiRunStatus::Idle);
                Ok(ApiSessionPromptOutcome {
                    accepted: true,
                    turn_id: Some(prompt_id),
                    status: "admitted".to_string(),
                    events_written: 0,
                    latest_part_index: self.latest_part_index(&input.session_id),
                    diagnostic: None,
                    error: None,
                })
            }
            InputDelivery::Queue => {
                self.runner_registry.enqueue(&input.session_id);
                self.spawn_queue_drain();
                Ok(ApiSessionPromptOutcome {
                    accepted: true,
                    turn_id: Some(prompt_id),
                    status: "queued".to_string(),
                    events_written: 0,
                    latest_part_index: self.latest_part_index(&input.session_id),
                    diagnostic: None,
                    error: None,
                })
            }
            InputDelivery::Run | InputDelivery::Steer => self.run_prompt(input, prompt_id).await,
        }
    }

    async fn compact(&self, session_id: &str) -> anyhow::Result<Option<ApiCompactOutcome>> {
        let _guard = self.turn_lock.lock().await;
        self.controller.set_session(session_id.to_string());
        let Some(record) = self.controller.compact().await else {
            return Ok(None);
        };
        Ok(Some(ApiCompactOutcome {
            boundary_id: record.boundary_id.unwrap_or_default(),
            before_tokens: record.before_tokens,
            after_tokens: record.after_tokens.unwrap_or(record.before_tokens),
            messages_before: record.messages_before,
            messages_after: record.messages_after.unwrap_or(record.messages_before),
        }))
    }

    async fn cancel(&self, session_id: &str) -> anyhow::Result<bool> {
        let requested = self.runner_registry.request_cancel(session_id);
        let cancelled_inputs = self.cancel_session_inputs(session_id).await?;
        self.controller.set_session(session_id.to_string());
        self.controller.cancel().await;
        if requested {
            self.runner_registry.cancel_completed(session_id);
        } else if cancelled_inputs > 0 {
            self.runner_registry
                .set_status(session_id, ApiRunStatus::Cancelled);
        }
        Ok(requested || cancelled_inputs > 0)
    }

    async fn context_snapshot(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<crate::desktop_runtime::DesktopContextSnapshot>> {
        let _guard = self.turn_lock.lock().await;
        self.controller.set_session(session_id.to_string());
        Ok(Some(self.controller.context_snapshot().await))
    }
}

fn delivery_from_api(delivery: Option<&str>) -> InputDelivery {
    match delivery
        .map(str::trim)
        .unwrap_or("run")
        .to_ascii_lowercase()
        .as_str()
    {
        "admit_only" => InputDelivery::AdmitOnly,
        "queue" => InputDelivery::Queue,
        _ => InputDelivery::Run,
    }
}

/// API 服务器状态
pub struct ApiState {
    /// LLM Provider
    pub provider: Arc<dyn crate::services::api::LlmProvider>,
    /// 模型名称
    pub model: String,
    /// 工具注册表
    pub tool_registry: Arc<crate::tools::ToolRegistry>,
    /// 会话存储
    pub session_store: Arc<RwLock<crate::session_store::SessionStore>>,
    /// 应用配置
    pub config: Arc<RwLock<AppConfig>>,
    /// 启动时间
    pub start_time: Instant,
    /// 请求计数
    pub request_count: Arc<RwLock<u64>>,
    /// API 审计追踪（工具调用/失败原因/耗时聚合）
    pub audit_tracker: Arc<RwLock<crate::cost_tracker::CostTracker>>,
    /// LSP 管理器
    pub lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器
    pub worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    /// Full-agent runtime handle.  None means typed 501 for session prompts.
    pub agent_runtime: Option<Arc<dyn ApiAgentRuntime>>,
    /// Per-session runner registry for wait/cancel/status.
    pub runner_registry: Arc<crate::api::session_runner::ApiSessionRunnerRegistry>,
}

impl ApiState {
    /// 创建新的 API 状态
    pub fn new(
        provider: Arc<dyn crate::services::api::LlmProvider>,
        model: String,
        tool_registry: Arc<crate::tools::ToolRegistry>,
        lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
        worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    ) -> anyhow::Result<Self> {
        // 初始化会话存储
        let db_path = dirs::data_dir()
            .map(|d| d.join("priority-agent").join("sessions.db"))
            .unwrap_or_else(|| std::path::PathBuf::from("sessions.db"));

        let session_store = crate::session_store::SessionStore::open(&db_path)?;

        // 加载配置
        let config = AppConfig::load().unwrap_or_default();

        Ok(Self {
            provider,
            model,
            tool_registry,
            session_store: Arc::new(RwLock::new(session_store)),
            config: Arc::new(RwLock::new(config)),
            start_time: Instant::now(),
            request_count: Arc::new(RwLock::new(0)),
            audit_tracker: Arc::new(RwLock::new(crate::cost_tracker::CostTracker::new())),
            lsp_manager,
            worktree_manager,
            agent_runtime: None,
            runner_registry: Arc::new(crate::api::session_runner::ApiSessionRunnerRegistry::new()),
        })
    }

    /// 记录请求
    pub async fn record_request(&self) {
        let mut count = self.request_count.write().await;
        *count += 1;
    }

    // ── Chat Methods ───────────────────────────────────────

    /// 执行聊天请求
    pub async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
        use crate::services::api::{ChatRequest as LlmChatRequest, Message};

        let model = req.model.as_deref().unwrap_or(&self.model);
        let system = req
            .system_prompt
            .as_deref()
            .unwrap_or("You are a helpful AI assistant.");

        debug!("Chat request: model={}, message={}", model, req.message);

        let llm_req = LlmChatRequest::new(model)
            .with_messages(vec![Message::system(system), Message::user(&req.message)])
            .with_temperature(req.temperature.unwrap_or(0.6));

        let response = self.provider.chat(llm_req).await?;
        if let Some(usage) = &response.usage {
            let mut tracker = self.audit_tracker.write().await;
            tracker.record_api_call(
                model,
                usage.prompt_tokens as u64,
                usage.completion_tokens as u64,
                usage.cached_tokens.map(|t| t as u64),
            );
        }

        // 如果有 session_id，保存消息
        if let Some(ref session_id) = req.session_id {
            let store = self.session_store.read().await;
            let _ = store.add_message(session_id, "user", &req.message, None, None);
            let _ = store.add_message(session_id, "assistant", &response.content, None, None);
        }

        Ok(ChatResponse {
            content: response.content,
            session_id: req.session_id.unwrap_or_default(),
            model: model.to_string(),
            usage: response.usage.as_ref().map(|u| UsageInfo {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            execution_kind: "provider_chat".to_string(),
            full_agent: false,
            agent_runtime_entrypoint: None,
            deprecated_route: None,
            replacement_route: None,
        })
    }

    // ── Session Methods ────────────────────────────────────

    /// 列出会话
    pub async fn list_sessions(&self, limit: i64) -> anyhow::Result<Vec<SessionInfo>> {
        let store = self.session_store.read().await;
        let records = store.list_sessions(limit)?;

        let sessions: Vec<SessionInfo> = records
            .into_iter()
            .map(|r| {
                let id = r.id.clone();
                SessionInfo {
                    id,
                    title: r.title,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                    message_count: store.message_count(&r.id).unwrap_or(0),
                }
            })
            .collect();

        Ok(sessions)
    }

    /// 创建会话
    pub async fn create_session(&self, title: Option<String>) -> anyhow::Result<SessionInfo> {
        let id = format!("sess_{}", uuid::Uuid::new_v4().simple());
        self.create_session_with_id(id, title).await
    }

    /// 用指定 ID 创建会话（用于远程桥接路由等需要自定义会话 ID 的场景）
    pub async fn create_session_with_id(
        &self,
        id: impl Into<String>,
        title: Option<String>,
    ) -> anyhow::Result<SessionInfo> {
        let id = id.into();
        let title = title.unwrap_or_else(|| "New Session".to_string());

        let store = self.session_store.read().await;
        store.create_session(&id, &title, &self.model)?;

        // 获取创建的会话
        let session = store
            .get_session(&id)?
            .ok_or_else(|| anyhow::anyhow!("Failed to retrieve created session"))?;

        Ok(SessionInfo {
            id: session.id,
            title: session.title,
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: 0,
        })
    }

    /// 获取会话
    pub async fn get_session(&self, id: &str) -> anyhow::Result<SessionInfo> {
        let store = self.session_store.read().await;
        let session = store
            .get_session(id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        Ok(SessionInfo {
            id: session.id,
            title: session.title,
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: store.message_count(id).unwrap_or(0),
        })
    }

    /// 更新会话
    pub async fn update_session(&self, id: &str, title: &str) -> anyhow::Result<SessionInfo> {
        let store = self.session_store.read().await;
        store.update_session_title(id, title)?;

        let session = store
            .get_session(id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        Ok(SessionInfo {
            id: session.id,
            title: session.title,
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: store.message_count(id).unwrap_or(0),
        })
    }

    /// 删除会话
    pub async fn delete_session(&self, id: &str) -> anyhow::Result<()> {
        let store = self.session_store.read().await;
        store.delete_session(id)?;
        Ok(())
    }

    /// 获取会话消息
    pub async fn get_session_messages(
        &self,
        id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<MessageInfo>> {
        let store = self.session_store.read().await;
        let records = store.get_messages(id)?;

        let messages: Vec<MessageInfo> = records
            .into_iter()
            .take(limit as usize)
            .map(|r| MessageInfo {
                id: r.id,
                role: r.role,
                content: r.content,
                created_at: r.created_at,
            })
            .collect();

        Ok(messages)
    }

    // ── Tool Methods ───────────────────────────────────────

    /// 列出所有工具
    pub async fn list_tools(&self) -> anyhow::Result<Vec<ToolInfo>> {
        let names = self.tool_registry.tool_names();
        let mut tools = Vec::new();

        for name in names {
            if let Some(tool) = self.tool_registry.get(name) {
                tools.push(ToolInfo {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    parameters: tool.parameters(),
                });
            }
        }

        Ok(tools)
    }

    /// 获取单个工具
    pub async fn get_tool(&self, name: &str) -> anyhow::Result<ToolInfo> {
        let tool = self
            .tool_registry
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found"))?;

        Ok(ToolInfo {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            parameters: tool.parameters(),
        })
    }

    /// 调用工具
    pub async fn call_tool(
        &self,
        tool_name: &str,
        params: serde_json::Value,
        session_id: &str,
    ) -> anyhow::Result<ToolCallResponse> {
        let tool = self
            .tool_registry
            .get(tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found", tool_name))?;

        let mut context = ToolContext::new(".", session_id)
            .with_task_manager(crate::task_manager::GLOBAL_TASK_MANAGER.clone())
            .with_cost_tracker(Arc::new(tokio::sync::Mutex::new(
                self.audit_tracker.read().await.clone(),
            )))
            .with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        if let Some(ref lsp) = self.lsp_manager {
            context = context.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            context = context.with_worktree_manager(wt.clone());
        }
        let started_at = std::time::Instant::now();
        let mut result = tool.execute(params, context).await;
        let duration_ms = started_at.elapsed().as_millis() as u64;
        if result.duration_ms.is_none() {
            result.duration_ms = Some(duration_ms);
        }
        {
            let mut tracker = self.audit_tracker.write().await;
            tracker.record_tool_execution(
                tool_name,
                result.success,
                duration_ms,
                result.error.as_deref(),
            );
        }

        Ok(ToolCallResponse {
            success: result.success,
            content: result.content.clone(),
            data: result.data.clone(),
            error: result.error.clone(),
        })
    }

    // ── Config Methods ─────────────────────────────────────

    /// 获取配置
    pub async fn get_config(&self) -> anyhow::Result<ConfigResponse> {
        let config = self.config.read().await;

        Ok(ConfigResponse {
            api: ApiConfigInfo {
                model: config.api.model.clone(),
                base_url: config.api.base_url.clone(),
                temperature: config.api.temperature,
                max_tokens: config.api.max_tokens,
            },
            ui: UiConfigInfo {
                theme: config.ui.theme.clone(),
                show_token_usage: config.ui.show_token_usage,
            },
            features: FeatureFlagsInfo {
                mcp_enabled: config.features.mcp_enabled,
                skills_enabled: config.features.skills_enabled,
                web_search: config.features.web_search,
            },
        })
    }

    /// 更新配置
    pub async fn update_config(&self, req: UpdateConfigRequest) -> anyhow::Result<()> {
        let mut config = self.config.write().await;

        if let Some(api) = req.api {
            config.api.model = api.model;
            // Security: disallow changing base_url via API to prevent redirect attacks
            if !api.base_url.is_empty() && api.base_url != config.api.base_url {
                anyhow::bail!("Changing base_url via API is not allowed for security reasons");
            }
            config.api.temperature = api.temperature;
            config.api.max_tokens = api.max_tokens;
        }

        if let Some(ui) = req.ui {
            config.ui.theme = ui.theme;
            config.ui.show_token_usage = ui.show_token_usage;
        }

        if let Some(features) = req.features {
            config.features.mcp_enabled = features.mcp_enabled;
            config.features.skills_enabled = features.skills_enabled;
            config.features.web_search = features.web_search;
        }

        // 保存配置
        config.save()?;

        Ok(())
    }

    // ── Stats Methods ──────────────────────────────────────

    /// 获取 API 审计概览
    pub async fn get_audit_summary(&self) -> anyhow::Result<serde_json::Value> {
        let tracker = self.audit_tracker.read().await;
        let rounds = tracker.coding_quality.file_change_rounds;
        let first_pass = tracker.coding_quality.first_pass_successes;
        let first_pass_rate = if rounds > 0 {
            (first_pass as f64 / rounds as f64) * 100.0
        } else {
            0.0
        };
        Ok(json!({
            "summary": tracker.tool_diagnostics_line(),
            "slowest": tracker.slowest_tools_line(5),
            "failure_reasons": tracker.top_failure_reasons_line(5),
            "coding_quality": {
                "line": tracker.coding_quality_line(),
                "rounds": rounds,
                "first_pass_successes": first_pass,
                "first_pass_rate_pct": first_pass_rate,
                "verify_failures": tracker.coding_quality.verify_failures,
                "repair_cycles": tracker.coding_quality.repair_cycles,
            },
            "recent_event_count": tracker.recent_tool_event_count(),
        }))
    }

    /// 获取最近审计事件
    pub async fn get_audit_recent(
        &self,
        limit: usize,
    ) -> anyhow::Result<Vec<crate::cost_tracker::ToolExecEvent>> {
        let tracker = self.audit_tracker.read().await;
        Ok(tracker.recent_tool_events(limit.clamp(1, 1000)))
    }

    /// 导出审计快照（并可选写入到文件）
    pub async fn export_audit_snapshot(
        &self,
        session_id: Option<&str>,
        recent_limit: usize,
        path: Option<&std::path::Path>,
    ) -> anyhow::Result<serde_json::Value> {
        let tracker = self.audit_tracker.read().await;
        let json_text = tracker.export_audit_snapshot_json(session_id, recent_limit.clamp(1, 2000));
        drop(tracker);

        if let Some(path) = path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &json_text)?;
        }

        let value: serde_json::Value = serde_json::from_str(&json_text)
            .unwrap_or_else(|_| json!({ "error": "failed to parse exported snapshot" }));
        Ok(value)
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> anyhow::Result<StatsResponse> {
        let store = self.session_store.read().await;
        let stats = store.stats()?;
        let _request_count = *self.request_count.read().await;

        Ok(StatsResponse {
            total_sessions: stats.session_count,
            total_messages: stats.message_count,
            total_input_tokens: stats.total_input_tokens,
            total_output_tokens: stats.total_output_tokens,
            uptime_secs: self.start_time.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// 获取 workflow 每周汇总指标
    pub async fn get_workflow_weekly_metrics(
        &self,
        limit_weeks: usize,
    ) -> anyhow::Result<WorkflowWeeklyMetricsResponse> {
        let rows = crate::engine::workflow::metrics::load_weekly_metric_summary(limit_weeks)
            .map_err(anyhow::Error::msg)?;
        let weeks = rows
            .into_iter()
            .map(|r| WorkflowWeeklyMetricItem {
                week_key: r.week_key,
                runs: r.runs,
                mainline_hit_rate: r.mainline_hit_rate,
                avg_first_plan_coverage: r.avg_first_plan_coverage,
                avg_rework_rate: r.avg_rework_rate,
                avg_objective_score: r.avg_objective_score,
            })
            .collect();
        Ok(WorkflowWeeklyMetricsResponse {
            generated_at: chrono::Utc::now().to_rfc3339(),
            weeks,
        })
    }

    /// 获取 workflow 每周校准偏差指标（自动 vs 人工抽样）
    pub async fn get_workflow_weekly_calibration(
        &self,
        limit_weeks: usize,
    ) -> anyhow::Result<WorkflowWeeklyCalibrationResponse> {
        let rows = crate::engine::workflow::metrics::load_weekly_calibration_summary(limit_weeks)
            .map_err(anyhow::Error::msg)?;
        let weeks = rows
            .into_iter()
            .map(|r| WorkflowWeeklyCalibrationItem {
                week_key: r.week_key,
                samples: r.samples,
                avg_mainline_bias_abs: r.avg_mainline_bias_abs,
                avg_coverage_bias_abs: r.avg_coverage_bias_abs,
                avg_objective_bias_abs: r.avg_objective_bias_abs,
            })
            .collect();
        Ok(WorkflowWeeklyCalibrationResponse {
            generated_at: chrono::Utc::now().to_rfc3339(),
            weeks,
        })
    }
}

/// 消息信息
#[derive(Debug, serde::Serialize)]
pub struct MessageInfo {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// API 错误
#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Forbidden(String),
    Internal(String),
    NotImplemented(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ApiError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            ApiError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            ApiError::Internal(msg) => write!(f, "Internal error: {}", msg),
            ApiError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        use axum::Json;

        let (status, message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg.clone()),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

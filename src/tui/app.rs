//! TUI 应用状态管理
//!
//! 对应 Claude Code 中的 AppState 概念

use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use crate::permissions::{PermissionMode, PermissionRules, RuleSource, SourcedRule};
use crate::state::{AppContext, MessageItem, MessageRole, TaskItem};
use crate::tools::Tool;
use crate::tui::components::input::InputState;
use futures::StreamExt;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::commands::{default_command_registry, CommandRegistry};

pub(crate) fn permission_mode_name(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Default => "default",
        PermissionMode::AutoLowRisk => "auto_low_risk",
        PermissionMode::AutoAll => "auto_all",
        PermissionMode::ReadOnly => "read_only",
    }
}

pub(crate) fn parse_permission_mode(mode: &str) -> Option<PermissionMode> {
    match mode.to_ascii_lowercase().as_str() {
        "default" => Some(PermissionMode::Default),
        "auto_low_risk" | "autolowrisk" | "low_risk" => Some(PermissionMode::AutoLowRisk),
        "auto_all" | "autoall" => Some(PermissionMode::AutoAll),
        "read_only" | "readonly" => Some(PermissionMode::ReadOnly),
        _ => None,
    }
}

fn provider_name_from_base_url(base_url: &str) -> &'static str {
    let u = base_url.to_ascii_lowercase();
    if u.contains("minimaxi.com") {
        "MiniMax"
    } else if u.contains("moonshot") {
        "Kimi"
    } else if u.contains("openai.com") {
        "OpenAI"
    } else {
        "Custom"
    }
}

#[derive(serde::Deserialize, Default)]
struct LegacyPermissionRules {
    #[serde(default)]
    always_allow: Vec<String>,
    #[serde(default)]
    always_deny: Vec<String>,
    #[serde(default)]
    always_ask: Vec<String>,
}

fn load_rules_for_edit(path: &std::path::Path) -> anyhow::Result<PermissionRules> {
    if !path.exists() {
        return Ok(PermissionRules::new());
    }
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(PermissionRules::new());
    }
    if let Ok(rules) = toml::from_str::<PermissionRules>(&content) {
        return Ok(rules);
    }
    let legacy = toml::from_str::<LegacyPermissionRules>(&content)?;
    let mut rules = PermissionRules::new();
    rules.always_allow = legacy
        .always_allow
        .into_iter()
        .map(|p| SourcedRule::new(p, RuleSource::User))
        .collect();
    rules.always_deny = legacy
        .always_deny
        .into_iter()
        .map(|p| SourcedRule::new(p, RuleSource::User))
        .collect();
    rules.always_ask = legacy
        .always_ask
        .into_iter()
        .map(|p| SourcedRule::new(p, RuleSource::User))
        .collect();
    Ok(rules)
}

pub(crate) fn persist_permission_rule(
    scope: RuleSource,
    decision: &str,
    pattern: &str,
    working_dir: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let path = match scope {
        RuleSource::Global => dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".priority-agent")
            .join("permissions.toml"),
        _ => working_dir.join(".priority-agent").join("permissions.toml"),
    };

    let mut rules = load_rules_for_edit(&path)?;
    let source_for_file = match scope {
        RuleSource::Global => RuleSource::Global,
        _ => RuleSource::Project,
    };
    let rule = SourcedRule::new(pattern, source_for_file);
    let target = match decision {
        "allow" => &mut rules.always_allow,
        "deny" => &mut rules.always_deny,
        "ask" => &mut rules.always_ask,
        _ => anyhow::bail!("invalid decision: {}", decision),
    };
    if !target.iter().any(|r| r.pattern == pattern) {
        target.push(rule);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(&rules)?;
    std::fs::write(&path, content)?;
    Ok(path)
}

/// TUI 应用模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Chat,
    Settings,
    PlanApproval,
    PermissionApproval,
    AskUser,
    DiffViewer,
    VimNormal,
    Onboarding,
}

/// TUI 应用状态
pub struct TuiApp {
    /// 当前模式
    pub mode: AppMode,
    /// 输入状态
    pub input: InputState,
    /// 消息列表
    pub messages: Vec<MessageItem>,
    /// 任务列表
    pub tasks: Vec<TaskItem>,
    /// 是否正在查询中
    pub is_querying: bool,
    /// 命令注册表
    command_registry: CommandRegistry,
    /// 滚动位置
    pub scroll_offset: usize,
    /// 应用上下文
    pub context: AppContext,
    /// 最后错误信息
    pub error_message: Option<String>,
    /// 命令历史
    pub history: VecDeque<String>,
    /// 历史索引
    pub history_index: Option<usize>,
    /// 流式查询引擎
    pub streaming_engine: Option<Arc<StreamingQueryEngine>>,
    /// 当前流式响应缓冲
    current_response: Arc<Mutex<String>>,
    /// 正在使用的工具列表
    active_tools: Arc<Mutex<Vec<String>>>,
    /// 流是否已完成（由后台任务设置）
    stream_done: Arc<AtomicBool>,
    /// 后台流式任务句柄（用于取消）
    stream_handle: Option<tokio::task::JoinHandle<()>>,
    /// 会话管理器
    pub session_manager: crate::tui::session_manager::TuiSessionManager,
    /// 设置状态
    pub settings_state: Option<crate::tui::components::settings::SettingsState>,
    /// 待审批的计划
    pub pending_plan: Option<crate::engine::plan_mode::Plan>,
    /// 计划审批响应发送器
    pub plan_response_tx:
        Option<tokio::sync::oneshot::Sender<crate::engine::plan_mode::PlanApproval>>,
    /// 计划修改输入缓冲
    pub plan_modification_input: String,
    /// 待审批的工具权限请求
    pub pending_permission_request: Option<crate::engine::conversation_loop::ToolApprovalRequest>,
    /// 工具权限审批响应发送器
    pub permission_response_tx: Option<tokio::sync::oneshot::Sender<bool>>,
    /// 待回答的用户问题
    pub pending_question: Option<String>,
    /// 用户问题的选项
    pub pending_question_options: Vec<String>,
    /// 用户问题响应发送器
    pub question_response_tx: Option<tokio::sync::oneshot::Sender<String>>,
    /// Diff 查看器内容
    pub diff_content: String,
    /// Diff 查看器标题
    pub diff_title: String,
    /// LSP 管理器
    pub lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器
    pub worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    /// Bundled skills
    pub bundled_skills: std::collections::HashMap<String, crate::skills::Skill>,
    /// 是否启用 Vim 模式
    pub vim_mode: bool,
    /// 键位映射
    pub keybindings: crate::tui::keybindings::Keybindings,
    /// 当前主题
    pub theme: crate::tui::theme::Theme,
    /// 引导状态
    pub onboarding_state: Option<crate::onboarding::OnboardingState>,
}

impl TuiApp {
    pub fn new() -> Self {
        Self::create(None, None, None)
    }

    /// 创建带流式引擎的 TuiApp
    pub fn with_engine(
        engine: Arc<StreamingQueryEngine>,
        lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
        worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    ) -> Self {
        Self::create(Some(engine), lsp_manager, worktree_manager)
    }

    fn create(
        engine: Option<Arc<StreamingQueryEngine>>,
        lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
        worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    ) -> Self {
        info!("Creating new TuiApp");

        let context = AppContext::new();

        // 初始化会话管理器
        let mut session_manager = crate::tui::session_manager::TuiSessionManager::new()
            .unwrap_or_else(|e| {
                warn!("Failed to initialize session manager: {}", e);
                crate::tui::session_manager::TuiSessionManager::in_memory()
                    .expect("Failed to create in-memory session manager")
            });

        // 开始新会话
        let model = engine.as_ref().map(|_| "kimi-k2.5").unwrap_or("unknown");
        let _ = session_manager.start_session("New Session", model);

        // 检测首次启动
        let onboarding_manager = crate::onboarding::OnboardingManager::new();
        let is_first_run = onboarding_manager.is_first_run();

        // 添加欢迎消息
        let welcome_content = if is_first_run {
            "Welcome to Priority Agent! This is your first time here.\nPress Enter to start the onboarding guide, or type /skip to skip.".to_string()
        } else {
            "Welcome to Priority Agent! Type your message and press Enter to chat.\nPress Ctrl+C to exit.".to_string()
        };
        let welcome_message = MessageItem {
            id: "welcome".to_string(),
            role: MessageRole::System,
            content: welcome_content,
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };

        let onboarding_state = if is_first_run {
            Some(crate::onboarding::OnboardingState::new())
        } else {
            None
        };

        Self {
            mode: if is_first_run { AppMode::Onboarding } else { AppMode::Chat },
            input: InputState::new(),
            messages: vec![welcome_message],
            tasks: Vec::new(),
            is_querying: false,
            command_registry: default_command_registry(),
            scroll_offset: 0,
            context,
            error_message: None,
            history: VecDeque::with_capacity(100),
            history_index: None,
            streaming_engine: engine,
            current_response: Arc::new(Mutex::new(String::new())),
            active_tools: Arc::new(Mutex::new(Vec::new())),
            stream_done: Arc::new(AtomicBool::new(true)),
            stream_handle: None,
            session_manager,
            settings_state: None,
            pending_plan: None,
            plan_response_tx: None,
            plan_modification_input: String::new(),
            pending_permission_request: None,
            permission_response_tx: None,
            pending_question: None,
            pending_question_options: Vec::new(),
            question_response_tx: None,
            diff_content: String::new(),
            diff_title: String::new(),
            lsp_manager,
            worktree_manager,
            bundled_skills: {
                let mut map = std::collections::HashMap::new();
                for skill in crate::skills::loader::load_bundled_skills() {
                    map.insert(skill.meta.name.clone(), skill);
                }
                map
            },
            vim_mode: false,
            keybindings: crate::tui::keybindings::Keybindings::load(),
            theme: {
                let config = crate::services::config::AppConfig::load().unwrap_or_default();
                crate::tui::theme::Theme::from_name(&config.ui.theme)
            },
            onboarding_state,
        }
    }

    /// 提交用户消息
    pub async fn submit_message(&mut self) {
        let content = self.input.value().to_string();
        if content.trim().is_empty() {
            return;
        }

        // 清空输入
        self.input.clear();

        // 处理斜杠命令
        if content.starts_with('/') {
            self.handle_slash_command(&content).await;
            return;
        }

        self.send_message(content).await;
    }

    /// 发送消息到 LLM（核心逻辑，可被 skill 调用复用）
    pub(crate) async fn send_message(&mut self, content: String) {
        if content.trim().is_empty() {
            return;
        }

        debug!("Submitting message: {}", content);

        // 取消之前的流式任务（如果有）
        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }

        // 添加到历史
        self.history.push_back(content.clone());
        if self.history.len() > 100 {
            self.history.pop_front();
        }

        // 添加用户消息
        let user_msg = MessageItem {
            id: format!("msg_{}", self.messages.len()),
            role: MessageRole::User,
            content: content.clone(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(user_msg);

        // 保存用户消息到数据库
        if let Err(e) = self
            .session_manager
            .add_message(MessageRole::User, &content)
        {
            warn!("Failed to save user message: {}", e);
        }

        // 更新会话标题（基于第一条用户消息）
        if self.session_manager.current_session_title() == "New Session" {
            let title = self.session_manager.generate_title(&self.messages);
            if let Err(e) = self.session_manager.update_title(&title) {
                warn!("Failed to update session title: {}", e);
            }
        }

        // 标记正在查询
        self.is_querying = true;

        // 滚动到底部
        self.scroll_to_bottom();

        // 使用流式引擎发送查询
        if let Some(ref engine) = self.streaming_engine {
            // 清空当前响应缓冲
            {
                let mut resp = self.current_response.lock().await;
                resp.clear();
            }
            {
                let mut tools = self.active_tools.lock().await;
                tools.clear();
            }
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
            self.messages.push(assistant_msg);

            // 启动流式查询（在后台任务中）
            let engine_clone = engine.clone();
            let response_clone = self.current_response.clone();
            let tools_clone = self.active_tools.clone();
            let done_flag = self.stream_done.clone();
            let user_msg = content.clone();

            let handle = tokio::spawn(async move {
                let mut stream = engine_clone.query_stream(user_msg).await;

                while let Some(event) = stream.next().await {
                    match event {
                        StreamEvent::TextChunk(text) => {
                            let mut resp = response_clone.lock().await;
                            resp.push_str(&text);
                        }
                        StreamEvent::ToolCallStart { name, .. } => {
                            let mut tools = tools_clone.lock().await;
                            tools.push(name);
                        }
                        StreamEvent::ToolExecutionComplete { .. } => {
                            let mut tools = tools_clone.lock().await;
                            if !tools.is_empty() {
                                tools.remove(0);
                            }
                        }
                        StreamEvent::Complete => {
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        StreamEvent::PermissionRequest {
                            tool_name, prompt, ..
                        } => {
                            let mut resp = response_clone.lock().await;
                            resp.push_str(&format!(
                                "\n\n[Permission request: {}]\n{}",
                                tool_name, prompt
                            ));
                        }
                        StreamEvent::Error(e) => {
                            let mut resp = response_clone.lock().await;
                            resp.push_str(&format!("\n[Error: {}]", e));
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        _ => {}
                    }
                }
                // 确保即使流结束也标记完成
                done_flag.store(true, Ordering::SeqCst);
            });
            self.stream_handle = Some(handle);
        } else {
            // 没有引擎，使用占位响应
            self.add_assistant_response(
                "AI engine not available. Set OPENAI_API_KEY or MOONSHOT_API_KEY.".to_string(),
            )
            .await;
        }
    }

    /// 刷新当前响应（从缓冲区读取最新的流式内容）
    pub async fn refresh_response(&mut self) {
        if !self.is_querying {
            return;
        }

        let response = {
            let resp = self.current_response.lock().await;
            resp.clone()
        };

        let tools = {
            let t = self.active_tools.lock().await;
            t.clone()
        };

        // 更新最后一条助手消息
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                let mut display_content = response.clone();

                // 如果有正在执行的工具，显示状态
                if !tools.is_empty() {
                    display_content.push_str(&format!("\n\n[Executing: {}]", tools.join(", ")));
                }

                last_msg.content = display_content;
            }
        }

        // 如果响应已完成（非查询状态），标记完成
        if response.is_empty() && !tools.is_empty() {
            // Still executing tools
        } else if !response.is_empty() && self.is_querying {
            // Check if stream is done by seeing if there are active tools
            if tools.is_empty() {
                // Stream might be done
                // We'll use a simpler approach: check on tick
            }
        }

        self.scroll_to_bottom();
    }

    /// 定时更新 - 处理流式响应刷新和计划审批检查
    pub async fn on_tick(&mut self) {
        if self.is_querying {
            self.refresh_response().await;

            // 使用 AtomicBool 检测流是否完成（由后台任务设置）
            if self.stream_done.load(Ordering::SeqCst) {
                self.is_querying = false;
            }
        }

        // 检查是否有待审批的计划（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_plan.is_none() {
            self.check_pending_plan().await;
        }

        // 检查是否有待审批的工具权限请求（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_permission_request.is_none() {
            self.check_pending_permission_request().await;
        }

        // 检查是否有待回答的用户问题（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_question.is_none() {
            self.check_pending_question().await;
        }
    }

    /// 检查是否有待审批的计划
    async fn check_pending_plan(&mut self) {
        let plan_manager = &crate::engine::plan_mode::GLOBAL_PLAN_MANAGER;
        if !plan_manager.approval_channel().has_pending().await {
            return;
        }

        if let Some((plan, tx)) = plan_manager.approval_channel().take_pending().await {
            info!("TUI received pending plan: {}", plan.title);
            self.pending_plan = Some(plan);
            self.plan_response_tx = Some(tx);
            self.plan_modification_input.clear();
            self.mode = AppMode::PlanApproval;
        }
    }

    /// 响应计划审批
    pub fn respond_to_plan(&mut self, approval: crate::engine::plan_mode::PlanApproval) {
        if let Some(tx) = self.plan_response_tx.take() {
            let _ = tx.send(approval);
        }
        self.pending_plan = None;
        self.plan_modification_input.clear();
        self.mode = AppMode::Chat;
    }

    /// 检查是否有待审批的工具权限请求
    async fn check_pending_permission_request(&mut self) {
        let Some(ref engine) = self.streaming_engine else {
            return;
        };
        let Some(ref channel) = engine.approval_channel() else {
            return;
        };

        if !channel.has_pending().await {
            return;
        }

        if let Some((request, tx)) = channel.take_pending().await {
            info!(
                "TUI received pending permission request: {}",
                request.prompt
            );
            self.pending_permission_request = Some(request);
            self.permission_response_tx = Some(tx);
            self.mode = AppMode::PermissionApproval;
        }
    }

    /// 响应工具权限审批
    pub fn respond_to_permission(&mut self, approved: bool) {
        if let Some(ref req) = self.pending_permission_request {
            let log_msg = format!(
                "Permission {} for tool '{}' with arguments: {}",
                if approved { "approved" } else { "denied" },
                req.tool_call.name,
                serde_json::to_string(&req.tool_call.arguments).unwrap_or_default()
            );
            let _ = self.session_manager.add_message(MessageRole::System, &log_msg);
        }
        if let Some(tx) = self.permission_response_tx.take() {
            let _ = tx.send(approved);
        }
        self.pending_permission_request = None;
        self.mode = AppMode::Chat;
    }

    /// 计算待审批工具的 Diff 预览
    pub fn compute_permission_diff(&self) -> Option<(String, String)> {
        let req = self.pending_permission_request.as_ref()?;
        let name = req.tool_call.name.as_str();
        let args = &req.tool_call.arguments;

        match name {
            "file_write" => {
                let path = args["path"].as_str().unwrap_or("unknown");
                let content = args["content"].as_str().unwrap_or("");
                let line_count = content.lines().count();
                let mut lines = vec![
                    format!("--- /dev/null"),
                    format!("+++ b/{}", path),
                    format!("@@ -0,0 +1,{} @@", line_count),
                ];
                for line in content.lines() {
                    lines.push(format!("+{}", line));
                }
                Some((format!("Preview: {}", path), lines.join("\n")))
            }
            "file_edit" => {
                let path = args["path"].as_str().unwrap_or("unknown");
                // 尝试读取原始文件并生成真实的 unified diff
                if let Ok(original) = std::fs::read_to_string(path) {
                    if let Ok(new_content) =
                        crate::tools::file_tool::FileEditTool::preview_edit(args, &original)
                    {
                        if let Some(diff) = generate_unified_diff(&original, &new_content, path) {
                            return Some((format!("Diff: {}", path), diff));
                        }
                    }
                }
                // 回退：显示旧版本的参数展示
                let old_string = args["old_string"].as_str().unwrap_or("");
                let new_string = args["new_string"].as_str().unwrap_or("");
                let insert_after = args["insert_after"].as_str();
                let insert_before = args["insert_before"].as_str();

                let mut lines = vec![format!("File: {}", path), "".to_string()];

                if let Some(after) = insert_after {
                    lines.push("Insert after:".to_string());
                    lines.push(format!("  {}", after));
                    lines.push("New text:".to_string());
                    for line in new_string.lines() {
                        lines.push(format!("  {}", line));
                    }
                } else if let Some(before) = insert_before {
                    lines.push("Insert before:".to_string());
                    lines.push(format!("  {}", before));
                    lines.push("New text:".to_string());
                    for line in new_string.lines() {
                        lines.push(format!("  {}", line));
                    }
                } else {
                    lines.push("--- old_string ---".to_string());
                    for line in old_string.lines() {
                        lines.push(format!("-{}", line));
                    }
                    lines.push("".to_string());
                    lines.push("+++ new_string +++".to_string());
                    for line in new_string.lines() {
                        lines.push(format!("+{}", line));
                    }
                }
                Some((format!("Preview: {}", path), lines.join("\n")))
            }
            "bash" => {
                let command = args["command"].as_str().unwrap_or("");
                let working_dir = args["working_dir"].as_str().unwrap_or("current directory");
                let mut lines = vec![
                    format!("Command: {}", command),
                    format!("Working directory: {}", working_dir),
                ];
                if let Some(timeout) = args["timeout"].as_u64() {
                    lines.push(format!("Timeout: {}s", timeout));
                }
                Some(("Preview: bash command".to_string(), lines.join("\n")))
            }
            _ => None,
        }
    }
}

/// 生成 unified diff（通过 diff -u 命令）
fn generate_unified_diff(old_content: &str, new_content: &str, path: &str) -> Option<String> {
    let old_file = std::env::temp_dir().join(format!("diff_old_{}", uuid::Uuid::new_v4()));
    let new_file = std::env::temp_dir().join(format!("diff_new_{}", uuid::Uuid::new_v4()));

    std::fs::write(&old_file, old_content).ok()?;
    std::fs::write(&new_file, new_content).ok()?;

    let output = std::process::Command::new("diff")
        .args(["-u", old_file.to_str()?, new_file.to_str()?])
        .output()
        .ok()?;

    let _ = std::fs::remove_file(&old_file);
    let _ = std::fs::remove_file(&new_file);

    let diff = String::from_utf8_lossy(&output.stdout);
    if diff.is_empty() {
        Some(format!("No differences in {}", path))
    } else {
        Some(diff.to_string())
    }
}

impl TuiApp {
    /// 检查是否有待回答的用户问题
    async fn check_pending_question(&mut self) {
        let Some(ref engine) = self.streaming_engine else {
            return;
        };
        let Some(ref channel) = engine.tool_registry().ask_channel() else {
            return;
        };

        if let Some((question, options, tx)) = channel.take_pending().await {
            info!("TUI received pending question: {}", question);
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
        if let Some(ref lsp) = self.lsp_manager {
            context = context.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            context = context.with_worktree_manager(wt.clone());
        }
        context = context.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        context
    }

    /// 处理斜杠命令
    async fn handle_slash_command(&mut self, input: &str) {
        let parts: Vec<&str> = input.trim().splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).unwrap_or(&"");

        use crate::tui::slash_handler as slash;

        let response = match cmd.as_str() {
            "/help" | "/h" => {
                let mut help = self.command_registry.help_text();
                help.push_str("\n\nSession Commands:\n");
                help.push_str("  /sessions    - List recent sessions\n");
                help.push_str("  /session     - Show current session or restore by number/ID\n");
                help.push_str("  /new         - Start a new session\n");
                help.push_str("  /export      - Export current session to JSON\n");
                help.push_str("  /search      - Search through all sessions\n");
                help.push_str("  /stats       - Show session statistics\n");
                help.push_str("\nSettings:\n");
                help.push_str("  /settings    - Open settings interface\n");
                help.push_str("  /permissions - View/update permission mode and policy rules\n");
                help.push_str("  /mcp         - Manage MCP server approvals\n");
                help.push_str("  /voice       - Check voice TTS/STT status\n");
                help.push_str("  /telemetry   - View telemetry status\n");
                help.push_str("  /onboarding  - Restart the onboarding guide\n");
                help.push_str("\nThe agent has 30+ tools (file, bash, web, github, memory, cron, swarm, MCP, skills, project).\nJust ask naturally - the agent will use the right tools.");
                help
            }
            "/clear" => {
                if let Some(ref engine) = self.streaming_engine {
                    engine.clear_history().await;
                }
                self.messages.clear();
                "Conversation history cleared.".to_string()
            }
            "/memory" => {
                let path = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("MEMORY.md");
                match std::fs::read_to_string(&path) {
                    Ok(content) if !content.trim().is_empty() => {
                        let preview: String = content.chars().take(2000).collect();
                        format!("Memory:\n{}", preview)
                    }
                    _ => "No memory saved yet. Use /save <text> to save.".to_string(),
                }
            }
            "/save" => {
                if args.is_empty() {
                    "Usage: /save <text to remember>".to_string()
                } else {
                    let path = dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join(".priority-agent")
                        .join("MEMORY.md");
                    let _ = std::fs::create_dir_all(path.parent().unwrap());
                    let existing = std::fs::read_to_string(&path).unwrap_or_default();
                    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M");
                    let entry = format!("\n## [NOTE] {}\n{}\n", timestamp, args);
                    let new_content = if existing.is_empty() {
                        format!("# Priority Agent Memory\n{}", entry)
                    } else {
                        format!("{}{}", existing, entry)
                    };
                    match std::fs::write(&path, &new_content) {
                        Ok(_) => format!("Saved: {}", args),
                        Err(e) => format!("Failed to save: {}", e),
                    }
                }
            }
            "/model" => {
                if self.streaming_engine.is_some() {
                    let model = self.current_model_label();
                    let provider = self.current_provider_label();
                    let base = self.current_provider_base_url();
                    format!("Model: {} (via {})\nBase URL: {}", model, provider, base)
                } else {
                    "Model: unavailable (no engine connected)".to_string()
                }
            }
            "/status" => slash::handle_status(self),
            "/resume" => slash::handle_resume(self, args).await,
            "/rewind" => slash::handle_rewind(self, args),
            "/cost" => {
                if let Some(ref engine) = self.streaming_engine {
                    let tracker = engine.cost_tracker().lock().await;
                    tracker.generate_report()
                } else {
                    "Cost tracking unavailable (no engine connected).".to_string()
                }
            }
            "/diff" => {
                let tool = crate::tools::GitTool;
                let params = serde_json::json!({ "action": "diff", "range": "HEAD~3..HEAD" });
                let result = tool.execute(params, self.build_tool_context().await).await;
                if result.success {
                    self.diff_title = "Recent changes (last 3 commits)".to_string();
                    self.diff_content = result.content;
                    self.mode = AppMode::DiffViewer;
                } else {
                    self.diff_title = "Error".to_string();
                    self.diff_content = result.error.unwrap_or_else(|| "Unknown error".to_string());
                    self.mode = AppMode::DiffViewer;
                }
                String::new()
            }
            "/quit" | "/exit" | "/q" => {
                if let Some(ref engine) = self.streaming_engine {
                    if let Some(mem) = engine.memory_manager() {
                        let mut mem = mem.lock().await;
                        mem.flush_session(&[]);
                    }
                }
                "Use Ctrl+C to exit".to_string()
            }
            "/sessions" => slash::handle_sessions(self),
            "/session" => slash::handle_session(self, args).await,
            "/new" => slash::handle_new(self).await,
            "/export" => slash::handle_export(self),
            "/search" => slash::handle_search(self, args),
            "/stats" => slash::handle_stats(self),
            "/settings" => {
                let config = crate::services::config::AppConfig::load().unwrap_or_default();
                self.settings_state = Some(crate::tui::components::settings::SettingsState::new(
                    config,
                    self.keybindings.clone(),
                ));
                self.mode = AppMode::Settings;
                "Entering settings mode...".to_string()
            }
            "/tools" => {
                let registry = crate::tools::ToolRegistry::default_registry();
                let mut names = registry.tool_names();
                names.sort();
                format!("Available tools ({}):\n{}", names.len(), names.join(", "))
            }
            "/tasks" => slash::handle_tasks(self).await,
            "/agents" => slash::handle_agents(self).await,
            "/doctor" => slash::handle_doctor(self, args).await,
            "/audit" => slash::handle_audit(self, args).await,
            "/permissions" | "/perm" => slash::handle_permissions(self, args),
            "/commit" => slash::handle_commit(self).await,
            "/review-pr" => slash::handle_review_pr(self, args).await,
            "/review" => slash::handle_review(self).await,
            "/security-review" => slash::handle_security_review(self).await,
            "/explain" => slash::handle_explain(self, args).await,
            "/fix" => slash::handle_fix(self).await,
            "/simplify" => slash::handle_simplify(self, args).await,
            "/verify" => slash::handle_verify(self).await,
            "/debug" => slash::handle_debug(self).await,
            "/stuck" => slash::handle_stuck(self).await,
            "/remember" => slash::handle_remember(self, args).await,
            "/keybindings" => slash::handle_keybindings(self, args),
            "/mcp" => slash::handle_mcp(self, args),
            "/voice" => slash::handle_voice(),
            "/telemetry" => slash::handle_telemetry(),
            "/share" => slash::handle_share(self),
            "/vim" => slash::handle_vim(self),
            "/onboarding" | "/onboard" => slash::handle_onboarding(self),
            "/skip" => slash::handle_skip(self),
            _ => {
                format!(
                    "Unknown command: {}. Type /help for available commands.",
                    cmd
                )
            }
        };

        self.add_system_message(response);
    }

    /// 恢复会话
    pub(crate) async fn restore_session(&mut self, session_id: &str) -> String {
        match self.session_manager.switch_to_session(session_id) {
            Ok(messages) => {
                // 清空当前消息
                self.messages.clear();

                // 加载会话消息到 UI
                for msg in messages {
                    self.messages.push(msg);
                }

                // 同步恢复引擎的对话历史
                if let Some(ref engine) = self.streaming_engine {
                    match self.session_manager.load_api_messages(session_id) {
                        Ok(api_messages) => {
                            engine.set_history(api_messages).await;
                        }
                        Err(e) => {
                            warn!("Failed to restore engine history: {}", e);
                        }
                    }
                }

                format!(
                    "Restored session {} ({} messages). Previous conversation loaded.",
                    &session_id[..8.min(session_id.len())],
                    self.messages.len()
                )
            }
            Err(e) => format!("Failed to restore session: {}", e),
        }
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
            self.theme = crate::tui::theme::Theme::from_name(&state.config.ui.theme);
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

        // 保存助手消息到数据库
        if let Err(e) = self
            .session_manager
            .add_message(MessageRole::Assistant, &content)
        {
            warn!("Failed to save assistant message: {}", e);
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
    }

    /// 向下滚动
    pub fn scroll_down(&mut self) {
        // 实际应该在渲染时计算最大滚动
        self.scroll_offset += 1;
    }

    /// 滚动到底部
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0; // 0 表示底部
    }

    /// 获取可见消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 当前模型名称（用于状态展示）
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
            provider_name_from_base_url(engine.provider_base_url()).to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 当前 Provider Base URL（用于状态展示）
    pub fn current_provider_base_url(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine.provider_base_url().to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 获取消息（考虑滚动）
    pub fn visible_messages(&self) -> &[MessageItem] {
        &self.messages
    }

    /// 设置错误信息
    pub fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
        self.is_querying = false;
    }

    /// 清除错误
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_app_new() {
        let app = TuiApp::new();
        assert_eq!(app.messages.len(), 1); // 欢迎消息
        assert!(!app.is_querying);
    }

    #[tokio::test]
    async fn test_restore_session() {
        let mut app = TuiApp::new();

        // 创建一个测试会话并添加消息
        let session_id = app
            .session_manager
            .start_session("Test Session", "kimi-k2.5")
            .unwrap();
        app.session_manager
            .add_message(MessageRole::User, "Hello")
            .unwrap();
        app.session_manager
            .add_message(MessageRole::Assistant, "Hi there!")
            .unwrap();

        // 验证消息已保存
        let count = app.session_manager.message_count(&session_id).unwrap();
        assert_eq!(count, 2);

        // 清空当前消息（模拟切换到新会话后的状态）
        app.messages.clear();
        app.messages.push(MessageItem {
            id: "temp".to_string(),
            role: MessageRole::System,
            content: "Temp".to_string(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        });

        // 恢复会话
        let result = app.restore_session(&session_id).await;
        assert!(result.contains("Restored session"));
        assert!(result.contains("2 messages"));

        // 验证 UI 消息已恢复
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].role, MessageRole::User);
        assert_eq!(app.messages[0].content, "Hello");
        assert_eq!(app.messages[1].role, MessageRole::Assistant);
        assert_eq!(app.messages[1].content, "Hi there!");

        // 验证当前会话 ID 已更新
        assert_eq!(app.session_manager.current_session_id(), Some(session_id.as_str()));
    }

    #[tokio::test]
    async fn test_restore_session_not_found() {
        let mut app = TuiApp::new();
        let result = app.restore_session("nonexistent_session").await;
        assert!(result.contains("Failed to restore session"));
    }

    #[test]
    fn test_respond_to_permission() {
        let mut app = TuiApp::new();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
            tool_call: crate::services::api::ToolCall {
                id: "tc_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "echo hello"}),
            },
            prompt: "Approve bash?".to_string(),
        });
        app.permission_response_tx = Some(tx);
        app.mode = AppMode::PermissionApproval;

        app.respond_to_permission(true);

        assert_eq!(app.mode, AppMode::Chat);
        assert!(app.pending_permission_request.is_none());
        assert!(app.permission_response_tx.is_none());
        assert!(rx.try_recv().unwrap());
    }

    #[test]
    fn test_compute_permission_diff_file_write() {
        let mut app = TuiApp::new();
        app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
            tool_call: crate::services::api::ToolCall {
                id: "tc_1".to_string(),
                name: "file_write".to_string(),
                arguments: serde_json::json!({
                    "path": "src/main.rs",
                    "content": "fn main() {\n    println!(\"hello\");\n}"
                }),
            },
            prompt: "Approve file write?".to_string(),
        });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: src/main.rs");
        assert!(diff.contains("+++ b/src/main.rs"));
        assert!(diff.contains("+fn main() {"));
        assert!(diff.contains("+    println!(\"hello\");"));
    }

    #[test]
    fn test_compute_permission_diff_file_edit_replace() {
        let mut app = TuiApp::new();
        // 使用不存在的文件路径确保回退到旧行为
        app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
            tool_call: crate::services::api::ToolCall {
                id: "tc_1".to_string(),
                name: "file_edit".to_string(),
                arguments: serde_json::json!({
                    "path": "nonexistent_file.rs",
                    "old_string": "println!(\"hello\");",
                    "new_string": "println!(\"world\");"
                }),
            },
            prompt: "Approve file edit?".to_string(),
        });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: nonexistent_file.rs");
        assert!(diff.contains("--- old_string ---"));
        assert!(diff.contains("-println!(\"hello\");"));
        assert!(diff.contains("+++ new_string +++"));
        assert!(diff.contains("+println!(\"world\");"));
    }

    #[test]
    fn test_compute_permission_diff_file_edit_insert() {
        let mut app = TuiApp::new();
        // 使用不存在的文件路径确保回退到旧行为
        app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
            tool_call: crate::services::api::ToolCall {
                id: "tc_1".to_string(),
                name: "file_edit".to_string(),
                arguments: serde_json::json!({
                    "path": "nonexistent_file.rs",
                    "insert_after": "fn main() {",
                    "new_string": "    // new line"
                }),
            },
            prompt: "Approve file edit?".to_string(),
        });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: nonexistent_file.rs");
        assert!(diff.contains("Insert after:"));
        assert!(diff.contains("fn main() {"));
        assert!(diff.contains("// new line"));
    }

    #[test]
    fn test_compute_permission_diff_bash() {
        let mut app = TuiApp::new();
        app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
            tool_call: crate::services::api::ToolCall {
                id: "tc_1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({
                    "command": "cargo test",
                    "working_dir": "/tmp",
                    "timeout": 60
                }),
            },
            prompt: "Approve bash?".to_string(),
        });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: bash command");
        assert!(diff.contains("cargo test"));
        assert!(diff.contains("/tmp"));
        assert!(diff.contains("60s"));
    }

    #[test]
    fn test_compute_permission_diff_unsupported_tool() {
        let mut app = TuiApp::new();
        app.pending_permission_request = Some(crate::engine::conversation_loop::ToolApprovalRequest {
            tool_call: crate::services::api::ToolCall {
                id: "tc_1".to_string(),
                name: "grep".to_string(),
                arguments: serde_json::json!({"pattern": "foo"}),
            },
            prompt: "Approve grep?".to_string(),
        });

        assert!(app.compute_permission_diff().is_none());
    }
}

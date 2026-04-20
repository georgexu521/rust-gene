//! 应用状态定义
//!
//! 对应 Claude Code 中的 AppState.tsx

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

/// 应用主状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// 当前会话
    pub session: Session,
    /// 所有任务
    pub tasks: HashMap<String, TaskItem>,
    /// 消息历史
    pub messages: Vec<MessageItem>,
    /// 工具调用状态
    pub tool_uses: Vec<ToolUseItem>,
    /// 当前是否正在查询
    pub is_querying: bool,
    /// 最后错误信息
    pub last_error: Option<String>,
    /// UI 状态
    pub ui: UiState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session: Session::new(),
            tasks: HashMap::new(),
            messages: Vec::new(),
            tool_uses: Vec::new(),
            is_querying: false,
            last_error: None,
            ui: UiState::default(),
        }
    }

    /// 添加用户消息
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(MessageItem {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: content.into(),
            timestamp: SystemTime::now(),
            metadata: HashMap::new(),
        });
    }

    /// 添加助手消息
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.messages.push(MessageItem {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: SystemTime::now(),
            metadata: HashMap::new(),
        });
    }

    /// 添加工具消息
    pub fn add_tool_message(
        &mut self,
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
    ) {
        self.messages.push(MessageItem {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Tool,
            content: content.into(),
            timestamp: SystemTime::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("tool_call_id".to_string(), tool_call_id.into());
                map
            },
        });
    }

    /// 添加任务
    pub fn add_task(&mut self, mut task: TaskItem) -> String {
        let id = task.id.clone();
        task.created_at = Some(SystemTime::now());
        self.tasks.insert(id.clone(), task);
        id
    }

    /// 完成任务
    pub fn complete_task(&mut self, task_id: impl AsRef<str>) {
        if let Some(task) = self.tasks.get_mut(task_id.as_ref()) {
            task.status = TaskStatus::Completed;
            task.completed_at = Some(SystemTime::now());
        }
    }

    /// 开始查询
    pub fn start_query(&mut self) {
        self.is_querying = true;
        self.last_error = None;
    }

    /// 结束查询
    pub fn end_query(&mut self) {
        self.is_querying = false;
    }

    /// 设置错误
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.last_error = Some(error.into());
        self.is_querying = false;
    }

    /// 清除消息历史
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }
}

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: SystemTime,
    pub working_dir: String,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "New Session".to_string(),
            created_at: SystemTime::now(),
            working_dir: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string()),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// 任务项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: TaskStatus,
    pub task_type: TaskType,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub created_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub metadata: HashMap<String, String>,
    /// 任务输出日志
    #[serde(default)]
    pub output: Vec<String>,
}

impl TaskItem {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            status: TaskStatus::Pending,
            task_type: TaskType::Local,
            parent_id: None,
            children: Vec::new(),
            created_at: None,
            completed_at: None,
            metadata: HashMap::new(),
            output: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_type(mut self, task_type: TaskType) -> Self {
        self.task_type = task_type;
        self
    }
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Killed,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed
        )
    }
}

/// 任务类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Local,
    Agent,
    Remote,
    Bash,
    Tool,
}

/// 消息项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageItem {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: SystemTime,
    pub metadata: HashMap<String, String>,
}

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 工具调用项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseItem {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: Option<String>,
    pub status: ToolUseStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolUseStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// UI 状态
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiState {
    pub input_value: String,
    pub scroll_position: usize,
    pub selected_message: Option<String>,
    pub show_settings: bool,
    pub theme: String,
}

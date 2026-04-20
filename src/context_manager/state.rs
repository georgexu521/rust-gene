//! 会话状态管理

use crate::weight_engine::types::{Project, TaskId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// 会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionState {
    /// 当前项目
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_project: Option<Project>,
    /// 已完成的任务ID
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub completed_tasks: Vec<TaskId>,
    /// 会话开始时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_start: Option<SystemTime>,
    /// 最后活动时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<SystemTime>,
    /// 用户偏好设置
    pub preferences: UserPreferences,
    /// 临时数据
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub temp_data: HashMap<String, String>,
}

/// 用户偏好设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPreferences {
    /// 自动保存间隔（秒）
    pub auto_save_interval: u64,
    /// 显示详细输出
    pub verbose_output: bool,
    /// 默认权重分配策略
    pub default_weight_strategy: WeightStrategy,
    /// 主题
    pub theme: Theme,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            auto_save_interval: 300, // 5分钟
            verbose_output: false,
            default_weight_strategy: WeightStrategy::Equal,
            theme: Theme::Default,
        }
    }
}

/// 权重分配策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightStrategy {
    /// 平均分配
    Equal,
    /// 基于任务描述长度
    ByDescriptionLength,
    /// 基于依赖数量
    ByDependencyCount,
    /// 手动指定
    Manual,
}

/// 主题
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Default,
    Dark,
    Light,
}

/// 上下文快照
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextSnapshot {
    /// 快照ID
    pub id: String,
    /// 创建时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<SystemTime>,
    /// 项目状态
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<Project>,
    /// 已完成的任务
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub completed_tasks: Vec<TaskId>,
    /// 快照描述
    pub description: String,
}

impl Default for SessionState {
    fn default() -> Self {
        let now = SystemTime::now();
        Self {
            current_project: None,
            completed_tasks: Vec::new(),
            session_start: Some(now),
            last_activity: Some(now),
            preferences: UserPreferences::default(),
            temp_data: HashMap::new(),
        }
    }
}

impl SessionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 更新最后活动时间
    pub fn touch(&mut self) {
        self.last_activity = Some(SystemTime::now());
    }

    /// 设置当前项目
    pub fn set_project(&mut self, project: Project) {
        self.current_project = Some(project);
        self.touch();
    }

    /// 标记任务为完成
    pub fn complete_task(&mut self, task_id: TaskId) {
        if !self.completed_tasks.contains(&task_id) {
            self.completed_tasks.push(task_id);
        }
        self.touch();
    }

    /// 创建上下文快照
    pub fn create_snapshot(&self, description: impl Into<String>) -> ContextSnapshot {
        ContextSnapshot {
            id: format!("snapshot_{}", 
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()),
            created_at: Some(SystemTime::now()),
            project: self.current_project.clone(),
            completed_tasks: self.completed_tasks.clone(),
            description: description.into(),
        }
    }

    /// 从快照恢复
    pub fn restore_from_snapshot(&mut self, snapshot: &ContextSnapshot) {
        self.current_project = snapshot.project.clone();
        self.completed_tasks = snapshot.completed_tasks.clone();
        self.touch();
    }

    /// 获取会话持续时间
    pub fn session_duration(&self) -> std::time::Duration {
        self.session_start
            .map(|start| SystemTime::now().duration_since(start).unwrap_or_default())
            .unwrap_or_default()
    }

    /// 获取空闲时间
    pub fn idle_duration(&self) -> std::time::Duration {
        self.last_activity
            .map(|last| SystemTime::now().duration_since(last).unwrap_or_default())
            .unwrap_or_default()
    }
}

impl Default for ContextSnapshot {
    fn default() -> Self {
        Self {
            id: String::new(),
            created_at: None,
            project: None,
            completed_tasks: Vec::new(),
            description: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_new() {
        let state = SessionState::new();
        assert!(state.current_project.is_none());
        assert!(state.completed_tasks.is_empty());
    }

    #[test]
    fn test_complete_task() {
        let mut state = SessionState::new();
        let task_id = TaskId::new("task1");
        
        state.complete_task(task_id.clone());
        assert_eq!(state.completed_tasks.len(), 1);
        
        // 重复完成不应添加
        state.complete_task(task_id);
        assert_eq!(state.completed_tasks.len(), 1);
    }
}

//! 任务管理器
//!
//! 全局任务注册表，用于跟踪 Agent 创建的任务及其生命周期

use crate::state::{TaskItem, TaskStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 全局任务管理器实例
pub static GLOBAL_TASK_MANAGER: once_cell::sync::Lazy<Arc<TaskManager>> =
    once_cell::sync::Lazy::new(|| Arc::new(TaskManager::new()));

/// 全局任务管理器
#[derive(Debug, Clone)]
pub struct TaskManager {
    tasks: Arc<RwLock<HashMap<String, TaskItem>>>,
}

impl TaskManager {
    /// 创建新的任务管理器
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建任务
    pub async fn create_task(&self, task: TaskItem) -> String {
        let id = task.id.clone();
        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), task);
        info!("Task created: {}", id);
        id
    }

    /// 获取任务
    pub async fn get_task(&self, task_id: &str) -> Option<TaskItem> {
        self.tasks.read().await.get(task_id).cloned()
    }

    /// 列出所有任务
    pub async fn list_tasks(&self, filter_status: Option<TaskStatus>) -> Vec<TaskItem> {
        let tasks = self.tasks.read().await;
        let mut result: Vec<TaskItem> = tasks
            .values()
            .filter(|t| filter_status.is_none_or(|s| t.status == s))
            .cloned()
            .collect();
        result.sort_by(|a, b| {
            b.created_at
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .cmp(&a.created_at.unwrap_or(std::time::SystemTime::UNIX_EPOCH))
        });
        result
    }

    /// 更新任务状态
    pub async fn update_task(&self, task_id: &str, status: TaskStatus) -> anyhow::Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = status;
            if status == TaskStatus::Completed || status == TaskStatus::Failed {
                task.completed_at = Some(std::time::SystemTime::now());
            }
            info!("Task {} updated to {:?}", task_id, status);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Task not found: {}", task_id))
        }
    }

    /// 停止（取消）任务
    pub async fn stop_task(&self, task_id: &str) -> anyhow::Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Killed;
            task.completed_at = Some(std::time::SystemTime::now());
            info!("Task {} stopped", task_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Task not found: {}", task_id))
        }
    }

    /// 追加任务输出
    pub async fn add_output(&self, task_id: &str, output: impl Into<String>) -> anyhow::Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.output.push(output.into());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Task not found: {}", task_id))
        }
    }

    /// 获取任务输出
    pub async fn get_output(&self, task_id: &str) -> Option<Vec<String>> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .map(|t| t.output.clone())
    }

    /// 任务数量
    pub async fn count(&self) -> usize {
        self.tasks.read().await.len()
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{TaskItem, TaskStatus};

    #[tokio::test]
    async fn test_task_manager_crud() {
        let manager = TaskManager::new();

        let task = TaskItem::new("t1", "Test task").with_description("Desc");
        let id = manager.create_task(task).await;
        assert_eq!(id, "t1");

        let retrieved = manager.get_task("t1").await.unwrap();
        assert_eq!(retrieved.name, "Test task");

        manager
            .update_task("t1", TaskStatus::Running)
            .await
            .unwrap();
        let updated = manager.get_task("t1").await.unwrap();
        assert_eq!(updated.status, TaskStatus::Running);

        manager.add_output("t1", "line 1").await.unwrap();
        manager.add_output("t1", "line 2").await.unwrap();
        let output = manager.get_output("t1").await.unwrap();
        assert_eq!(output, vec!["line 1", "line 2"]);

        manager.stop_task("t1").await.unwrap();
        let stopped = manager.get_task("t1").await.unwrap();
        assert_eq!(stopped.status, TaskStatus::Killed);

        let list = manager.list_tasks(None).await;
        assert_eq!(list.len(), 1);

        let pending = manager.list_tasks(Some(TaskStatus::Pending)).await;
        assert!(pending.is_empty());
    }
}

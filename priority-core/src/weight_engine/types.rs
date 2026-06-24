//! 权重系统的核心类型定义

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 任务唯一标识
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 权重值 (0.0 - 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Weight(pub f64);

impl Weight {
    /// 创建新权重，自动限制在 0.0-1.0 范围内
    pub fn new(value: f64) -> Self {
        if value.is_finite() {
            Self(value.clamp(0.0, 1.0))
        } else {
            Self(0.0)
        }
    }

    /// 获取权重值
    pub fn value(&self) -> f64 {
        self.0
    }

    /// 转换为百分比
    pub fn as_percentage(&self) -> f64 {
        self.0 * 100.0
    }
}

impl Default for Weight {
    fn default() -> Self {
        Self(1.0)
    }
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// 待处理
    #[default]
    Pending,
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 阻塞中
    Blocked,
}

/// 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    /// 相对于父任务的权重
    pub local_weight: Weight,
    /// 相对于项目总目标的绝对权重
    pub absolute_weight: Weight,
    pub status: TaskStatus,
    /// 子任务
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<Task>,
    /// 依赖的任务ID
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<TaskId>,
    /// 元数据
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, String>,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: TaskId::new(""),
            name: String::new(),
            description: String::new(),
            local_weight: Weight::default(),
            absolute_weight: Weight::default(),
            status: TaskStatus::default(),
            children: Vec::new(),
            dependencies: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

impl Task {
    /// 创建新任务
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: TaskId::new(id),
            name: name.into(),
            description: String::new(),
            local_weight: Weight::default(),
            absolute_weight: Weight::default(),
            status: TaskStatus::default(),
            children: Vec::new(),
            dependencies: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// 设置本地权重
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.local_weight = Weight::new(weight);
        self
    }

    /// 添加子任务
    pub fn add_child(&mut self, child: Task) {
        self.children.push(child);
    }

    /// 添加依赖
    pub fn add_dependency(&mut self, dep_id: TaskId) {
        self.dependencies.push(dep_id);
    }

    /// 检查是否所有依赖都已完成
    pub fn dependencies_satisfied(&self, completed_tasks: &[TaskId]) -> bool {
        self.dependencies
            .iter()
            .all(|dep| completed_tasks.contains(dep))
    }

    /// 检查是否所有依赖都在已完成集合中
    pub fn dependencies_satisfied_by(&self, completed_tasks: &HashSet<TaskId>) -> bool {
        self.dependencies
            .iter()
            .all(|dep| completed_tasks.contains(dep))
    }

    /// 计算完成进度 (0.0 - 1.0)
    pub fn progress(&self) -> f64 {
        if self.children.is_empty() {
            match self.status {
                TaskStatus::Completed => 1.0,
                TaskStatus::InProgress => 0.5,
                _ => 0.0,
            }
        } else {
            // 加权计算子任务进度
            let total_weight: f64 = self.children.iter().map(|c| c.local_weight.value()).sum();
            if total_weight == 0.0 {
                return 0.0;
            }

            self.children
                .iter()
                .map(|child| child.progress() * child.local_weight.value())
                .sum::<f64>()
                / total_weight
        }
    }
}

/// 项目定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub root_tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<std::time::SystemTime>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            id: TaskId::new(""),
            name: String::new(),
            description: String::new(),
            root_tasks: Vec::new(),
            created_at: None,
        }
    }
}

impl Project {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: TaskId::new(id),
            name: name.into(),
            description: String::new(),
            root_tasks: Vec::new(),
            created_at: Some(std::time::SystemTime::now()),
        }
    }

    /// 添加根任务
    pub fn add_task(&mut self, task: Task) {
        self.root_tasks.push(task);
    }

    /// 获取所有任务（扁平化）
    pub fn all_tasks(&self) -> Vec<&Task> {
        let mut tasks = Vec::new();
        for task in &self.root_tasks {
            self.collect_tasks(task, &mut tasks);
        }
        tasks
    }

    fn collect_tasks<'a>(&self, task: &'a Task, tasks: &mut Vec<&'a Task>) {
        tasks.push(task);
        for child in &task.children {
            self.collect_tasks(child, tasks);
        }
    }

    /// 计算项目整体进度
    pub fn overall_progress(&self) -> f64 {
        if self.root_tasks.is_empty() {
            return 0.0;
        }

        let total_weight: f64 = self.root_tasks.iter().map(|t| t.local_weight.value()).sum();

        if total_weight == 0.0 {
            return 0.0;
        }

        self.root_tasks
            .iter()
            .map(|task| task.progress() * task.local_weight.value())
            .sum::<f64>()
            / total_weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_clamping() {
        let w1 = Weight::new(1.5);
        assert_eq!(w1.value(), 1.0);

        let w2 = Weight::new(-0.5);
        assert_eq!(w2.value(), 0.0);

        let w3 = Weight::new(0.5);
        assert_eq!(w3.value(), 0.5);
    }

    #[test]
    fn test_weight_rejects_non_finite_values() {
        assert_eq!(Weight::new(f64::NAN).value(), 0.0);
        assert_eq!(Weight::new(f64::INFINITY).value(), 0.0);
        assert_eq!(Weight::new(f64::NEG_INFINITY).value(), 0.0);
    }

    #[test]
    fn test_task_progress() {
        let mut task = Task::new("t1", "Test Task").with_weight(1.0);
        assert_eq!(task.progress(), 0.0);

        task.status = TaskStatus::InProgress;
        assert_eq!(task.progress(), 0.5);

        task.status = TaskStatus::Completed;
        assert_eq!(task.progress(), 1.0);
    }

    #[test]
    fn test_task_with_children_progress() {
        let mut parent = Task::new("parent", "Parent").with_weight(1.0);

        let child1 = Task::new("c1", "Child 1")
            .with_weight(0.6)
            .with_description("First child");

        let mut child2 = Task::new("c2", "Child 2")
            .with_weight(0.4)
            .with_description("Second child");
        child2.status = TaskStatus::Completed;

        parent.add_child(child1);
        parent.add_child(child2);

        // 0.0 * 0.6 + 1.0 * 0.4 = 0.4
        assert!((parent.progress() - 0.4).abs() < 0.001);
    }
}

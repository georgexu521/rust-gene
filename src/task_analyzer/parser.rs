//! 任务解析器 - 从各种格式解析任务定义

use crate::weight_engine::types::{Task, TaskId};
use std::collections::HashMap;

/// 解析错误
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    InvalidFormat(String),
    MissingField(String),
    InvalidWeight(String),
    DuplicateId(String),
    CircularDependency(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            ParseError::MissingField(field) => write!(f, "Missing required field: {}", field),
            ParseError::InvalidWeight(msg) => write!(f, "Invalid weight: {}", msg),
            ParseError::DuplicateId(id) => write!(f, "Duplicate task ID: {}", id),
            ParseError::CircularDependency(msg) => write!(f, "Circular dependency: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

/// 原始任务定义（解析前）
#[derive(Debug, Clone)]
pub struct RawTask {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub weight: Option<f64>,
    pub parent_id: Option<String>,
    pub dependencies: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl RawTask {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            weight: None,
            parent_id: None,
            dependencies: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// 任务解析器
pub struct TaskParser {
    /// 已解析的任务缓存
    parsed_tasks: HashMap<String, Task>,
    /// 原始任务定义
    raw_tasks: Vec<RawTask>,
}

impl TaskParser {
    pub fn new() -> Self {
        Self {
            parsed_tasks: HashMap::new(),
            raw_tasks: Vec::new(),
        }
    }

    /// 添加原始任务定义
    pub fn add_raw_task(&mut self, task: RawTask) -> Result<(), ParseError> {
        // 检查重复ID
        if self.raw_tasks.iter().any(|t| t.id == task.id) {
            return Err(ParseError::DuplicateId(task.id));
        }
        self.raw_tasks.push(task);
        Ok(())
    }

    /// 从YAML字符串解析任务
    pub fn parse_yaml(&mut self, yaml: &str) -> Result<Vec<Task>, ParseError> {
        // 简化的YAML解析（实际项目中应使用 serde_yaml）
        // 这里提供一个基础实现框架
        let raw_tasks = self.parse_simple_format(yaml)?;
        for task in raw_tasks {
            self.add_raw_task(task)?;
        }
        self.build_tasks()
    }

    /// 从JSON字符串解析任务
    pub fn parse_json(&mut self, json: &str) -> Result<Vec<Task>, ParseError> {
        // 简化的JSON解析（实际项目中应使用 serde_json）
        let raw_tasks = self.parse_simple_format(json)?;
        for task in raw_tasks {
            self.add_raw_task(task)?;
        }
        self.build_tasks()
    }

    /// 解析简单格式（每行一个任务）
    /// 格式: id|name|weight|parent_id|dependencies
    fn parse_simple_format(&self, input: &str) -> Result<Vec<RawTask>, ParseError> {
        let mut tasks = Vec::new();

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 2 {
                return Err(ParseError::InvalidFormat(
                    format!("Line must have at least id and name: {}", line)
                ));
            }

            let mut task = RawTask::new(parts[0].trim(), parts[1].trim());

            if parts.len() > 2 {
                task.weight = parts[2].trim().parse().ok();
            }

            if parts.len() > 3 && !parts[3].trim().is_empty() {
                task.parent_id = Some(parts[3].trim().to_string());
            }

            if parts.len() > 4 && !parts[4].trim().is_empty() {
                task.dependencies = parts[4]
                    .trim()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }

            tasks.push(task);
        }

        Ok(tasks)
    }

    /// 构建任务树
    fn build_tasks(&mut self) -> Result<Vec<Task>, ParseError> {
        // 首先检查循环依赖
        self.detect_cycles()?;

        // 按parent_id分组
        let mut children_map: HashMap<String, Vec<RawTask>> = HashMap::new();
        let mut root_tasks: Vec<RawTask> = Vec::new();

        for task in &self.raw_tasks {
            if let Some(parent_id) = &task.parent_id {
                children_map
                    .entry(parent_id.clone())
                    .or_default()
                    .push(task.clone());
            } else {
                root_tasks.push(task.clone());
            }
        }

        // 构建任务树
        let mut result = Vec::new();
        for raw_task in root_tasks {
            let task = self.build_task_tree(&raw_task, &children_map)?;
            result.push(task);
        }

        Ok(result)
    }

    /// 递归构建任务树
    fn build_task_tree(
        &self,
        raw: &RawTask,
        children_map: &HashMap<String, Vec<RawTask>>,
    ) -> Result<Task, ParseError> {
        let weight = raw.weight.unwrap_or(1.0);
        let mut task = Task::new(&raw.id, &raw.name)
            .with_weight(weight);

        if let Some(desc) = &raw.description {
            task = task.with_description(desc.clone());
        }

        // 添加依赖
        for dep_id in &raw.dependencies {
            task.add_dependency(TaskId::new(dep_id));
        }

        // 添加元数据
        task.metadata = raw.metadata.clone();

        // 递归添加子任务
        if let Some(children) = children_map.get(&raw.id) {
            for child_raw in children {
                let child_task = self.build_task_tree(child_raw, children_map)?;
                task.add_child(child_task);
            }
        }

        Ok(task)
    }

    /// 检测循环依赖
    fn detect_cycles(&self) -> Result<(), ParseError> {
        let mut visited = HashMap::new();
        let mut recursion_stack = HashMap::new();

        for task in &self.raw_tasks {
            if self.has_cycle(
                &task.id,
                &mut visited,
                &mut recursion_stack,
            )? {
                return Err(ParseError::CircularDependency(
                    format!("Cycle detected starting from task: {}", task.id)
                ));
            }
        }

        Ok(())
    }

    fn has_cycle(
        &self,
        task_id: &str,
        visited: &mut HashMap<String, bool>,
        recursion_stack: &mut HashMap<String, bool>,
    ) -> Result<bool, ParseError> {
        if let Some(&in_stack) = recursion_stack.get(task_id) {
            if in_stack {
                return Ok(true);
            }
        }

        if visited.get(task_id).copied().unwrap_or(false) {
            return Ok(false);
        }

        visited.insert(task_id.to_string(), true);
        recursion_stack.insert(task_id.to_string(), true);

        // 找到这个任务的依赖
        let task = self.raw_tasks.iter().find(|t| t.id == task_id)
            .ok_or_else(|| ParseError::InvalidFormat(
                format!("Task not found: {}", task_id)
            ))?;

        for dep_id in &task.dependencies {
            if self.has_cycle(dep_id, visited, recursion_stack)? {
                return Ok(true);
            }
        }

        recursion_stack.insert(task_id.to_string(), false);
        Ok(false)
    }

    /// 清空解析器状态
    pub fn clear(&mut self) {
        self.parsed_tasks.clear();
        self.raw_tasks.clear();
    }
}

impl Default for TaskParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_format() {
        let parser = TaskParser::new();
        let input = r#"
# 项目任务
task1|First Task|0.5||
task2|Second Task|0.5||task1
        "#;

        let tasks = parser.parse_simple_format(input).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "task1");
        assert_eq!(tasks[0].weight, Some(0.5));
        assert!(tasks[0].dependencies.is_empty());

        assert_eq!(tasks[1].id, "task2");
        assert_eq!(tasks[1].dependencies, vec!["task1"]);
    }

    #[test]
    fn test_duplicate_id_error() {
        let mut parser = TaskParser::new();
        parser.add_raw_task(RawTask::new("task1", "Task 1")).unwrap();
        
        let result = parser.add_raw_task(RawTask::new("task1", "Task 1 Duplicate"));
        assert!(matches!(result, Err(ParseError::DuplicateId(_))));
    }
}

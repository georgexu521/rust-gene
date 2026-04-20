//! Task 工具集 - 创建、查询、管理任务
//!
//! 用于跟踪子任务和执行计划

use crate::state::{TaskItem, TaskStatus, TaskType};
use crate::task_manager::TaskManager;
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// 任务创建工具
pub struct TaskCreateTool {
    manager: Arc<TaskManager>,
}

impl TaskCreateTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    fn description(&self) -> &str {
        "Create a new task to track work. \
         Use this to break down large tasks into smaller, trackable pieces. \
         The task will be monitored and can be executed in parallel."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A clear description of what needs to be done"
                },
                "task_type": {
                    "type": "string",
                    "enum": ["bash", "analysis", "research", "code"],
                    "description": "The type of task (default: analysis)"
                },
                "parent_id": {
                    "type": "string",
                    "description": "Optional parent task ID if this is a subtask"
                }
            },
            "required": ["description"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let description = params["description"].as_str().unwrap_or("");
        if description.is_empty() {
            return ToolResult::error("Description cannot be empty");
        }

        let task_type = params["task_type"].as_str().unwrap_or("analysis");
        let parent_id = params["parent_id"].as_str().map(String::from);

        let task_id = format!("task_{}", &Uuid::new_v4().to_string()[..8]);

        info!(
            "Creating task: {} (type: {}, description: {})",
            task_id, task_type, description
        );

        let task = TaskItem {
            id: task_id.clone(),
            name: description[..description.len().min(50)].to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            task_type: match task_type {
                "bash" => TaskType::Bash,
                "code" => TaskType::Tool,
                "research" => TaskType::Agent,
                "analysis" => TaskType::Local,
                _ => TaskType::Local,
            },
            parent_id,
            children: Vec::new(),
            created_at: Some(std::time::SystemTime::now()),
            completed_at: None,
            metadata: {
                let mut map = std::collections::HashMap::new();
                map.insert("task_type".to_string(), task_type.to_string());
                map
            },
            output: Vec::new(),
        };

        let id = self.manager.create_task(task).await;

        ToolResult::success_with_data(
            format!(
                "Task created successfully: {}\nDescription: {}",
                id, description
            ),
            json!({
                "task_id": id,
                "description": description,
                "task_type": task_type,
                "status": "pending"
            }),
        )
    }
}

/// 任务查询工具
pub struct TaskGetTool {
    manager: Arc<TaskManager>,
}

impl TaskGetTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "task_get"
    }

    fn description(&self) -> &str {
        "Get details of a specific task by ID."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "The task ID" }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let task_id = params["task_id"].as_str().unwrap_or("");
        if task_id.is_empty() {
            return ToolResult::error("task_id cannot be empty");
        }

        match self.manager.get_task(task_id).await {
            Some(task) => ToolResult::success_with_data(
                format!(
                    "Task {}: {} (status: {:?})",
                    task.id, task.name, task.status
                ),
                serde_json::to_value(&task).unwrap_or(serde_json::Value::Null),
            ),
            None => ToolResult::error(format!("Task not found: {}", task_id)),
        }
    }
}

/// 任务列表工具
pub struct TaskListTool {
    manager: Arc<TaskManager>,
}

impl TaskListTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    fn description(&self) -> &str {
        "List all tracked tasks. Optionally filter by status."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["pending", "running", "completed", "failed", "cancelled"],
                    "description": "Optional status filter"
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let status_filter = params["status"].as_str().and_then(|s| match s {
            "pending" => Some(TaskStatus::Pending),
            "running" => Some(TaskStatus::Running),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            "cancelled" => Some(TaskStatus::Killed),
            _ => None,
        });

        let tasks = self.manager.list_tasks(status_filter).await;
        let summaries: Vec<serde_json::Value> = tasks
            .iter()
            .map(|t| {
                json!({
                    "task_id": t.id,
                    "name": t.name,
                    "status": format!("{:?}", t.status),
                    "description": t.description,
                })
            })
            .collect();

        ToolResult::success_with_data(
            format!("Found {} tasks", tasks.len()),
            json!({ "count": tasks.len(), "tasks": summaries }),
        )
    }
}

/// 任务更新工具
pub struct TaskUpdateTool {
    manager: Arc<TaskManager>,
}

impl TaskUpdateTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "task_update"
    }

    fn description(&self) -> &str {
        "Update a task's status."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string" },
                "status": {
                    "type": "string",
                    "enum": ["pending", "running", "completed", "failed", "cancelled"]
                }
            },
            "required": ["task_id", "status"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let task_id = params["task_id"].as_str().unwrap_or("");
        let status_str = params["status"].as_str().unwrap_or("");

        let status = match status_str {
            "pending" => TaskStatus::Pending,
            "running" => TaskStatus::Running,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Killed,
            _ => return ToolResult::error(format!("Invalid status: {}", status_str)),
        };

        match self.manager.update_task(task_id, status).await {
            Ok(()) => ToolResult::success(format!("Task {} updated to {:?}", task_id, status)),
            Err(e) => ToolResult::error(format!("Failed to update task: {}", e)),
        }
    }
}

/// 任务停止工具
pub struct TaskStopTool {
    manager: Arc<TaskManager>,
}

impl TaskStopTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        "task_stop"
    }

    fn description(&self) -> &str {
        "Stop (cancel) a running task by ID."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string" }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let task_id = params["task_id"].as_str().unwrap_or("");
        if task_id.is_empty() {
            return ToolResult::error("task_id cannot be empty");
        }

        match self.manager.stop_task(task_id).await {
            Ok(()) => ToolResult::success(format!("Task {} stopped", task_id)),
            Err(e) => ToolResult::error(format!("Failed to stop task: {}", e)),
        }
    }
}

/// 任务输出工具
pub struct TaskOutputTool {
    manager: Arc<TaskManager>,
}

impl TaskOutputTool {
    pub fn new(manager: Arc<TaskManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "task_output"
    }

    fn description(&self) -> &str {
        "Get the output log of a task, or append a new line to it."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string" },
                "action": {
                    "type": "string",
                    "enum": ["get", "append"],
                    "default": "get"
                },
                "line": { "type": "string", "description": "Line to append (required when action=append)" }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let task_id = params["task_id"].as_str().unwrap_or("");
        let action = params["action"].as_str().unwrap_or("get");

        if task_id.is_empty() {
            return ToolResult::error("task_id cannot be empty");
        }

        match action {
            "append" => {
                let line = params["line"].as_str().unwrap_or("");
                if line.is_empty() {
                    return ToolResult::error("line cannot be empty when appending");
                }
                match self.manager.add_output(task_id, line).await {
                    Ok(()) => ToolResult::success(format!("Output appended to {}", task_id)),
                    Err(e) => ToolResult::error(format!("Failed to append output: {}", e)),
                }
            }
            _ => match self.manager.get_output(task_id).await {
                Some(output) => ToolResult::success_with_data(
                    if output.is_empty() {
                        format!("Task {} has no output yet", task_id)
                    } else {
                        format!(
                            "Task {} output ({} lines):\n{}",
                            task_id,
                            output.len(),
                            output.join("\n")
                        )
                    },
                    json!({ "task_id": task_id, "output": output }),
                ),
                None => ToolResult::error(format!("Task not found: {}", task_id)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_create() {
        let manager = Arc::new(TaskManager::new());
        let tool = TaskCreateTool::new(manager);
        let params = json!({
            "description": "Test task for unit testing"
        });
        let context = ToolContext::new(".", "test-session");

        let result = tool.execute(params, context).await;

        assert!(result.success);
        assert!(result.content.contains("Task created successfully"));
    }

    #[tokio::test]
    async fn test_task_lifecycle() {
        let manager = Arc::new(TaskManager::new());

        // Create
        let create = TaskCreateTool::new(manager.clone());
        let result = create
            .execute(
                json!({"description": "Lifecycle test"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        let task_id = result.data.as_ref().unwrap()["task_id"]
            .as_str()
            .unwrap()
            .to_string();

        // Get
        let get = TaskGetTool::new(manager.clone());
        let result = get
            .execute(json!({"task_id": task_id}), ToolContext::new(".", "test"))
            .await;
        assert!(result.success);

        // Update
        let update = TaskUpdateTool::new(manager.clone());
        let result = update
            .execute(
                json!({"task_id": task_id, "status": "running"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);

        // Output append
        let output = TaskOutputTool::new(manager.clone());
        let result = output
            .execute(
                json!({"task_id": task_id, "action": "append", "line": "log line 1"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);

        // Output get
        let result = output
            .execute(
                json!({"task_id": task_id, "action": "get"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        assert!(result.content.contains("log line 1"));

        // Stop
        let stop = TaskStopTool::new(manager.clone());
        let result = stop
            .execute(json!({"task_id": task_id}), ToolContext::new(".", "test"))
            .await;
        assert!(result.success);

        // List
        let list = TaskListTool::new(manager.clone());
        let result = list.execute(json!({}), ToolContext::new(".", "test")).await;
        assert!(result.success);
        assert_eq!(result.data.as_ref().unwrap()["count"].as_u64().unwrap(), 1);
    }
}

//! 权重计算器 - 计算任务的绝对权重和优先级

use crate::weight_engine::types::{Project, Task, TaskId, TaskStatus, Weight};
use std::collections::{BinaryHeap, HashMap};

/// 可执行的任务项（按优先级排序）
#[derive(Debug, Clone)]
pub struct ExecutableTask {
    pub task_id: TaskId,
    pub task_name: String,
    /// 绝对权重（相对于整个项目）
    pub absolute_weight: Weight,
    /// 优先级分数（考虑权重、依赖、紧急度等）
    pub priority_score: f64,
    /// 阻塞的任务数量
    pub blocking_count: usize,
    /// 依赖链深度
    pub dependency_depth: usize,
}

impl ExecutableTask {
    pub fn new(task_id: TaskId, task_name: String, absolute_weight: Weight) -> Self {
        Self {
            task_id,
            task_name,
            absolute_weight,
            priority_score: 0.0,
            blocking_count: 0,
            dependency_depth: 0,
        }
    }
}

impl PartialEq for ExecutableTask {
    fn eq(&self, other: &Self) -> bool {
        self.absolute_weight == other.absolute_weight
    }
}

impl Eq for ExecutableTask {}

impl PartialOrd for ExecutableTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExecutableTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// 权重计算器
pub struct WeightCalculator {
    /// 已完成的任务ID
    completed_tasks: Vec<TaskId>,
    /// 任务依赖图
    #[allow(dead_code)]
    dependency_graph: HashMap<TaskId, Vec<TaskId>>,
}

impl WeightCalculator {
    pub fn new() -> Self {
        Self {
            completed_tasks: Vec::new(),
            dependency_graph: HashMap::new(),
        }
    }

    /// 标记任务为完成
    pub fn mark_completed(&mut self, task_id: TaskId) {
        if !self.completed_tasks.contains(&task_id) {
            self.completed_tasks.push(task_id);
        }
    }

    /// 计算项目中所有任务的绝对权重
    pub fn calculate_absolute_weights(&self, project: &Project) -> HashMap<TaskId, Weight> {
        let mut weights = HashMap::new();

        for task in &project.root_tasks {
            self.calculate_task_weight(task, 1.0, &mut weights);
        }

        weights
    }

    fn calculate_task_weight(
        &self,
        task: &Task,
        parent_absolute_weight: f64,
        weights: &mut HashMap<TaskId, Weight>,
    ) {
        // 计算当前任务的绝对权重
        // 根任务使用其本地权重作为绝对权重
        // 子任务使用父任务的绝对权重乘以自己的本地权重
        let absolute_weight = if parent_absolute_weight >= 1.0 - f64::EPSILON {
            // 根任务层级
            task.local_weight.value()
        } else {
            // 子任务层级：直接使用父权重乘以本地权重
            parent_absolute_weight * task.local_weight.value()
        };

        weights.insert(task.id.clone(), Weight::new(absolute_weight));

        // 递归计算子任务的权重
        for child in &task.children {
            self.calculate_task_weight(child, absolute_weight, weights);
        }
    }

    /// 获取当前可执行的任务（依赖已满足且未完成的）
    pub fn get_executable_tasks(&self, project: &Project) -> Vec<ExecutableTask> {
        let absolute_weights = self.calculate_absolute_weights(project);
        let mut executable = Vec::new();

        for task in project.all_tasks() {
            if task.status == TaskStatus::Completed {
                continue;
            }

            if task.dependencies_satisfied(&self.completed_tasks) {
                let weight = absolute_weights
                    .get(&task.id)
                    .copied()
                    .unwrap_or(Weight::new(0.0));

                let mut exec_task = ExecutableTask::new(task.id.clone(), task.name.clone(), weight);

                // 计算优先级分数
                exec_task.priority_score =
                    self.calculate_priority_score(&exec_task, task, &absolute_weights);

                // 计算阻塞的任务数
                exec_task.blocking_count = self.count_blocked_tasks(project, &task.id);

                // 计算依赖深度
                exec_task.dependency_depth = self.calculate_dependency_depth(project, &task.id);

                executable.push(exec_task);
            }
        }

        // 按优先级排序
        executable.sort_by(|a, b| {
            b.priority_score
                .partial_cmp(&a.priority_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        executable
    }

    /// 计算优先级分数
    fn calculate_priority_score(
        &self,
        exec_task: &ExecutableTask,
        task: &Task,
        _weights: &HashMap<TaskId, Weight>,
    ) -> f64 {
        let mut score = exec_task.absolute_weight.value();

        // 如果有子任务，降低优先级（先完成叶子节点）
        if !task.children.is_empty() {
            score *= 0.8;
        }

        // 如果任务正在进行中，稍微提高优先级（避免频繁切换）
        if task.status == TaskStatus::InProgress {
            score *= 1.1;
        }

        score
    }

    /// 计算某个任务阻塞了多少其他任务
    fn count_blocked_tasks(&self, project: &Project, task_id: &TaskId) -> usize {
        project
            .all_tasks()
            .iter()
            .filter(|t| t.dependencies.contains(task_id))
            .count()
    }

    /// 计算任务的依赖链深度
    fn calculate_dependency_depth(&self, project: &Project, task_id: &TaskId) -> usize {
        let tasks = project.all_tasks();
        let task = match tasks.iter().find(|t| t.id == *task_id) {
            Some(t) => t,
            None => return 0,
        };

        if task.dependencies.is_empty() {
            return 0;
        }

        let mut max_depth = 0;
        for dep_id in &task.dependencies {
            let depth = self.calculate_dependency_depth(project, dep_id);
            max_depth = max_depth.max(depth + 1);
        }

        max_depth
    }

    /// 获取按优先级排序的任务队列
    pub fn get_priority_queue(&self, project: &Project) -> BinaryHeap<ExecutableTask> {
        self.get_executable_tasks(project).into_iter().collect()
    }

    /// 获取下一个最高优先级的任务
    pub fn next_task(&self, project: &Project) -> Option<ExecutableTask> {
        self.get_executable_tasks(project).into_iter().next()
    }
}

impl Default for WeightCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// 项目进度报告
#[derive(Debug)]
pub struct ProgressReport {
    pub overall_progress: f64,
    pub completed_count: usize,
    pub in_progress_count: usize,
    pub pending_count: usize,
    pub blocked_count: usize,
    pub next_recommended_task: Option<String>,
}

impl ProgressReport {
    pub fn generate(calculator: &WeightCalculator, project: &Project) -> Self {
        let all_tasks = project.all_tasks();
        let mut completed = 0;
        let mut in_progress = 0;
        let mut pending = 0;
        let mut blocked = 0;

        for task in &all_tasks {
            match task.status {
                TaskStatus::Completed => completed += 1,
                TaskStatus::InProgress => in_progress += 1,
                TaskStatus::Pending => {
                    if task.dependencies_satisfied(&calculator.completed_tasks) {
                        pending += 1;
                    } else {
                        blocked += 1;
                    }
                }
                TaskStatus::Blocked => blocked += 1,
            }
        }

        let next_task = calculator.next_task(project).map(|t| {
            format!(
                "{} (权重: {:.1}%)",
                t.task_name,
                t.absolute_weight.as_percentage()
            )
        });

        Self {
            overall_progress: project.overall_progress(),
            completed_count: completed,
            in_progress_count: in_progress,
            pending_count: pending,
            blocked_count: blocked,
            next_recommended_task: next_task,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_project() -> Project {
        let mut project = Project::new("test", "Test Project");

        // 创建任务结构:
        // 项目
        // ├── 任务A (40%)
        // │   ├── 子任务A1 (60%)
        // │   └── 子任务A2 (40%)
        // └── 任务B (60%)

        let mut task_a = Task::new("a", "Task A").with_weight(0.4);
        let task_a1 = Task::new("a1", "Subtask A1").with_weight(0.6);
        let task_a2 = Task::new("a2", "Subtask A2").with_weight(0.4);

        task_a.add_child(task_a1);
        task_a.add_child(task_a2);

        let task_b = Task::new("b", "Task B").with_weight(0.6);

        project.add_task(task_a);
        project.add_task(task_b);

        project
    }

    #[test]
    fn test_calculate_absolute_weights() {
        let project = create_test_project();
        let calculator = WeightCalculator::new();
        let weights = calculator.calculate_absolute_weights(&project);

        // 任务A: 40%
        assert!((weights.get(&TaskId::new("a")).unwrap().value() - 0.4).abs() < 0.001);
        // 任务B: 60%
        assert!((weights.get(&TaskId::new("b")).unwrap().value() - 0.6).abs() < 0.001);
        // 子任务A1: 40% * 60% = 24%
        assert!((weights.get(&TaskId::new("a1")).unwrap().value() - 0.24).abs() < 0.001);
        // 子任务A2: 40% * 40% = 16%
        assert!((weights.get(&TaskId::new("a2")).unwrap().value() - 0.16).abs() < 0.001);
    }

    #[test]
    fn test_executable_tasks() {
        let project = create_test_project();
        let calculator = WeightCalculator::new();
        let executable = calculator.get_executable_tasks(&project);

        // 所有任务都应该可执行（没有依赖）
        assert_eq!(executable.len(), 4);

        // 应该按权重排序：b(60%) > a(40%) > a1(24%) > a2(16%)
        assert_eq!(executable[0].task_id.0, "b");
        assert_eq!(executable[1].task_id.0, "a");
    }

    #[test]
    fn test_priority_queue() {
        let project = create_test_project();
        let calculator = WeightCalculator::new();
        let mut queue = calculator.get_priority_queue(&project);

        let top_task = queue.pop().unwrap();
        assert_eq!(top_task.task_id.0, "b");
    }
}

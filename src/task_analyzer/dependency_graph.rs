//! 依赖图 - 管理任务间的依赖关系

use priority_core::weight_engine::types::{Task, TaskId};
use std::collections::{HashMap, HashSet, VecDeque};

/// 循环依赖错误
#[derive(Debug, Clone, PartialEq)]
pub struct CycleError {
    pub cycle: Vec<String>,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Circular dependency detected: {:?}", self.cycle)
    }
}

impl std::error::Error for CycleError {}

/// 依赖图
#[derive(Debug)]
pub struct DependencyGraph {
    /// 任务ID -> 依赖的任务ID列表
    dependencies: HashMap<TaskId, Vec<TaskId>>,
    /// 任务ID -> 被依赖的任务ID列表（反向依赖）
    dependents: HashMap<TaskId, Vec<TaskId>>,
    /// 所有任务ID
    task_ids: HashSet<TaskId>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
            task_ids: HashSet::new(),
        }
    }

    /// 从任务列表构建依赖图
    pub fn from_tasks(tasks: &[Task]) -> Self {
        let mut graph = Self::new();

        fn add_task_to_graph(graph: &mut DependencyGraph, task: &Task) {
            graph.add_task(&task.id, &task.dependencies);

            for child in &task.children {
                add_task_to_graph(graph, child);
            }
        }

        for task in tasks {
            add_task_to_graph(&mut graph, task);
        }

        graph
    }

    /// 添加任务到依赖图
    pub fn add_task(&mut self, task_id: &TaskId, deps: &[TaskId]) {
        self.task_ids.insert(task_id.clone());

        // 添加依赖关系
        for dep in deps {
            self.dependencies
                .entry(task_id.clone())
                .or_default()
                .push(dep.clone());

            // 更新反向依赖
            self.dependents
                .entry(dep.clone())
                .or_default()
                .push(task_id.clone());

            self.task_ids.insert(dep.clone());
        }
    }

    /// 获取任务的直接依赖
    pub fn get_dependencies(&self, task_id: &TaskId) -> Option<&Vec<TaskId>> {
        self.dependencies.get(task_id)
    }

    /// 获取依赖该任务的所有任务
    pub fn get_dependents(&self, task_id: &TaskId) -> Option<&Vec<TaskId>> {
        self.dependents.get(task_id)
    }

    /// 获取所有任务ID
    pub fn task_ids(&self) -> &HashSet<TaskId> {
        &self.task_ids
    }

    /// 检测循环依赖
    pub fn detect_cycles(&self) -> Result<(), CycleError> {
        let mut visited = HashSet::new();
        let mut recursion_stack = HashSet::new();

        for task_id in &self.task_ids {
            if !visited.contains(task_id) {
                if let Some(cycle) = self.dfs_detect_cycle(
                    task_id,
                    &mut visited,
                    &mut recursion_stack,
                    &mut Vec::new(),
                ) {
                    return Err(CycleError { cycle });
                }
            }
        }

        Ok(())
    }

    fn dfs_detect_cycle(
        &self,
        task_id: &TaskId,
        visited: &mut HashSet<TaskId>,
        recursion_stack: &mut HashSet<TaskId>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        visited.insert(task_id.clone());
        recursion_stack.insert(task_id.clone());
        path.push(task_id.to_string());

        if let Some(deps) = self.dependencies.get(task_id) {
            for dep in deps {
                if !visited.contains(dep) {
                    if let Some(cycle) = self.dfs_detect_cycle(dep, visited, recursion_stack, path)
                    {
                        return Some(cycle);
                    }
                } else if recursion_stack.contains(dep) {
                    // 发现循环
                    let cycle_start = path.iter().position(|p| p == &dep.to_string()).unwrap_or(0);
                    let mut cycle = path[cycle_start..].to_vec();
                    cycle.push(dep.to_string());
                    return Some(cycle);
                }
            }
        }

        path.pop();
        recursion_stack.remove(task_id);
        None
    }

    /// 拓扑排序 - 返回任务的执行顺序
    pub fn topological_sort(&self) -> Result<Vec<TaskId>, CycleError> {
        // 首先检测循环
        self.detect_cycles()?;

        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        // 初始化入度
        for task_id in &self.task_ids {
            in_degree.insert(task_id.clone(), 0);
        }

        // 计算入度
        for (task_id, deps) in &self.dependencies {
            for _ in deps {
                *in_degree.entry(task_id.clone()).or_insert(0) += 1;
            }
        }

        // 找到入度为0的任务
        for (task_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(task_id.clone());
            }
        }

        // Kahn算法
        while let Some(task_id) = queue.pop_front() {
            result.push(task_id.clone());

            if let Some(dependents) = self.dependents.get(&task_id) {
                for dependent in dependents {
                    let degree = in_degree.get_mut(dependent).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }

        Ok(result)
    }

    /// 获取任务的所有传递依赖（包括间接依赖）
    pub fn get_all_dependencies(&self, task_id: &TaskId) -> HashSet<TaskId> {
        let mut all_deps = HashSet::new();
        let mut visited = HashSet::new();
        let mut stack = vec![task_id.clone()];

        while let Some(current) = stack.pop() {
            if visited.insert(current.clone()) {
                if let Some(deps) = self.dependencies.get(&current) {
                    for dep in deps {
                        all_deps.insert(dep.clone());
                        stack.push(dep.clone());
                    }
                }
            }
        }

        all_deps
    }

    /// 获取影响该任务的所有任务（传递依赖的反向）
    pub fn get_all_dependents(&self, task_id: &TaskId) -> HashSet<TaskId> {
        let mut all_dependents = HashSet::new();
        let mut visited = HashSet::new();
        let mut stack = vec![task_id.clone()];

        while let Some(current) = stack.pop() {
            if visited.insert(current.clone()) {
                if let Some(dependents) = self.dependents.get(&current) {
                    for dependent in dependents {
                        all_dependents.insert(dependent.clone());
                        stack.push(dependent.clone());
                    }
                }
            }
        }

        all_dependents
    }

    /// 检查两个任务之间是否存在依赖关系
    pub fn has_dependency(&self, from: &TaskId, to: &TaskId) -> bool {
        self.get_all_dependencies(from).contains(to)
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_no_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_task(&TaskId::new("a"), &[TaskId::new("b"), TaskId::new("c")]);
        graph.add_task(&TaskId::new("b"), &[TaskId::new("c")]);
        graph.add_task(&TaskId::new("c"), &[]);

        assert!(graph.detect_cycles().is_ok());
    }

    #[test]
    fn test_detect_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_task(&TaskId::new("a"), &[TaskId::new("b")]);
        graph.add_task(&TaskId::new("b"), &[TaskId::new("c")]);
        graph.add_task(&TaskId::new("c"), &[TaskId::new("a")]);

        let result = graph.detect_cycles();
        assert!(result.is_err());
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = DependencyGraph::new();
        graph.add_task(&TaskId::new("a"), &[TaskId::new("b"), TaskId::new("c")]);
        graph.add_task(&TaskId::new("b"), &[TaskId::new("c")]);
        graph.add_task(&TaskId::new("c"), &[]);

        let sorted = graph.topological_sort().unwrap();
        // c 必须在 b 和 a 之前，b 必须在 a 之前
        let c_pos = sorted.iter().position(|t| t.0 == "c").unwrap();
        let b_pos = sorted.iter().position(|t| t.0 == "b").unwrap();
        let a_pos = sorted.iter().position(|t| t.0 == "a").unwrap();

        assert!(c_pos < b_pos);
        assert!(b_pos < a_pos);
    }
}

//! 历史记录管理

use priority_core::weight_engine::types::TaskId;
use std::collections::VecDeque;
use std::time::SystemTime;

/// 历史记录条目
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// 条目ID
    pub id: String,
    /// 时间戳
    pub timestamp: SystemTime,
    /// 操作类型
    pub action: Action,
    /// 描述
    pub description: String,
    /// 相关任务ID
    pub task_id: Option<TaskId>,
    /// 操作前的状态（用于撤销）
    pub previous_state: Option<String>,
}

/// 操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    TaskCreated,
    TaskCompleted,
    TaskUpdated,
    TaskDeleted,
    ProjectCreated,
    ProjectUpdated,
    WeightChanged,
    DependencyAdded,
    DependencyRemoved,
    SnapshotCreated,
    SnapshotRestored,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Action::TaskCreated => "任务创建",
            Action::TaskCompleted => "任务完成",
            Action::TaskUpdated => "任务更新",
            Action::TaskDeleted => "任务删除",
            Action::ProjectCreated => "项目创建",
            Action::ProjectUpdated => "项目更新",
            Action::WeightChanged => "权重变更",
            Action::DependencyAdded => "添加依赖",
            Action::DependencyRemoved => "移除依赖",
            Action::SnapshotCreated => "创建快照",
            Action::SnapshotRestored => "恢复快照",
        };
        write!(f, "{}", name)
    }
}

/// 历史记录管理器
pub struct HistoryManager {
    /// 历史记录
    entries: VecDeque<HistoryEntry>,
    /// 最大历史记录数
    max_entries: usize,
    /// 当前位置（用于撤销/重做）
    current_position: usize,
}

impl HistoryManager {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            current_position: 0,
        }
    }

    /// 添加历史记录
    pub fn add_entry(&mut self, entry: HistoryEntry) {
        // 如果不在末尾，删除当前位置之后的记录
        while self.entries.len() > self.current_position {
            self.entries.pop_back();
        }

        // 添加新记录
        self.entries.push_back(entry);
        self.current_position += 1;

        // 限制历史记录数量
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
            self.current_position -= 1;
        }
    }

    /// 创建快速历史记录
    pub fn log(&mut self, action: Action, description: impl Into<String>) {
        let entry = HistoryEntry {
            id: format!(
                "entry_{}",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            ),
            timestamp: SystemTime::now(),
            action,
            description: description.into(),
            task_id: None,
            previous_state: None,
        };
        self.add_entry(entry);
    }

    /// 获取最近的记录
    pub fn recent_entries(&self, count: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(count).collect()
    }

    /// 获取所有记录
    pub fn all_entries(&self) -> Vec<&HistoryEntry> {
        self.entries.iter().collect()
    }

    /// 撤销上一步操作
    pub fn undo(&mut self) -> Option<&HistoryEntry> {
        if self.current_position > 0 {
            self.current_position -= 1;
            self.entries.get(self.current_position)
        } else {
            None
        }
    }

    /// 重做
    pub fn redo(&mut self) -> Option<&HistoryEntry> {
        if self.current_position < self.entries.len() {
            let entry = self.entries.get(self.current_position);
            self.current_position += 1;
            entry
        } else {
            None
        }
    }

    /// 清空历史
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_position = 0;
    }

    /// 生成历史报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# 操作历史\n\n");

        for (i, entry) in self.entries.iter().enumerate() {
            let marker = if i + 1 == self.current_position {
                " -> "
            } else {
                "    "
            };

            report.push_str(&format!(
                "{}{}. [{}] {} - {}\n",
                marker,
                i + 1,
                format_timestamp(entry.timestamp),
                entry.action,
                entry.description
            ));
        }

        report
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new(100)
    }
}

fn format_timestamp(timestamp: SystemTime) -> String {
    let duration = timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_manager() {
        let mut manager = HistoryManager::new(10);

        manager.log(Action::TaskCreated, "创建任务 A");
        manager.log(Action::TaskCreated, "创建任务 B");
        manager.log(Action::TaskCompleted, "完成任务 A");

        assert_eq!(manager.entries.len(), 3);
        assert_eq!(manager.current_position, 3);

        let recent = manager.recent_entries(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].action, Action::TaskCompleted);
    }

    #[test]
    fn test_undo_redo() {
        let mut manager = HistoryManager::new(10);

        manager.log(Action::TaskCreated, "任务 A");
        manager.log(Action::TaskCreated, "任务 B");

        assert_eq!(manager.current_position, 2);

        let undone = manager.undo();
        assert!(undone.is_some());
        assert_eq!(manager.current_position, 1);

        let redone = manager.redo();
        assert!(redone.is_some());
        assert_eq!(manager.current_position, 2);
    }
}

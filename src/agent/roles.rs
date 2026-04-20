//! Agent 角色定义
//!
//! 为不同用途的子 Agent 提供角色枚举和对应的系统提示词模板。

use serde::{Deserialize, Serialize};

/// Agent 角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// 默认通用子 Agent
    #[default]
    Default,
    /// 协作型队友（结对编程风格）
    Teammate,
    /// 领域专家（深度专注特定任务）
    Specialist,
    /// 探索型任务规划者（长周期、 speculative）
    DreamTask,
}

impl AgentRole {
    /// 从字符串解析角色（不区分大小写）
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "default" => Some(AgentRole::Default),
            "teammate" => Some(AgentRole::Teammate),
            "specialist" => Some(AgentRole::Specialist),
            "dream_task" | "dreamtask" => Some(AgentRole::DreamTask),
            _ => None,
        }
    }

    /// 获取角色的系统提示词前缀
    pub fn system_prompt_prefix(&self) -> &'static str {
        match self {
            AgentRole::Default => {
                "You are a helpful sub-agent. Focus on completing the assigned task accurately and concisely."
            }
            AgentRole::Teammate => {
                "You are a collaborative teammate. Engage in active dialogue, ask clarifying questions when needed, \
                 and work alongside the user as a pair programmer. Prioritize shared understanding over speed."
            }
            AgentRole::Specialist => {
                "You are a deep domain specialist. Dive deeply into the details of the assigned task. \
                 Be thorough, cite specifics, and do not generalize. Your goal is expert-level execution."
            }
            AgentRole::DreamTask => {
                "You are a speculative task explorer. Think broadly, consider edge cases, and map out \
                 long-horizon plans. It is okay to propose multiple approaches and explore trade-offs."
            }
        }
    }

    /// 角色显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentRole::Default => "default",
            AgentRole::Teammate => "teammate",
            AgentRole::Specialist => "specialist",
            AgentRole::DreamTask => "dream_task",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_parsing() {
        assert_eq!(AgentRole::from_str("teammate"), Some(AgentRole::Teammate));
        assert_eq!(AgentRole::from_str("Teammate"), Some(AgentRole::Teammate));
        assert_eq!(
            AgentRole::from_str("dream_task"),
            Some(AgentRole::DreamTask)
        );
        assert_eq!(AgentRole::from_str("dreamtask"), Some(AgentRole::DreamTask));
        assert_eq!(AgentRole::from_str("unknown"), None);
    }

    #[test]
    fn test_role_prefixes_non_empty() {
        for role in [
            AgentRole::Default,
            AgentRole::Teammate,
            AgentRole::Specialist,
            AgentRole::DreamTask,
        ] {
            assert!(!role.system_prompt_prefix().is_empty());
        }
    }
}
